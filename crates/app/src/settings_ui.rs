//! The settings window (process started with `--settings`).
//!
//! Every change is saved right away, then the background process is told to
//! reload. The window polls the device list, the status file and the
//! settings file once a second; the latter can change behind its back when
//! the background process rebinds a moved output.

use crate::audio::Audio;
use crate::ipc::{self, InstanceLock, WM_PREVIEW_OSD, WM_RELOAD, wide};
use crate::keys::{self, Capture, Captured};
use crate::settings_theme::{self as theme, Palette, Themes};
use crate::startup::{self, StartupState};
use crate::{icon, language, paths};
use eframe::egui::{self, Margin, RichText};
use pinwheel_core::devices::{DeviceInfo, Resolution, resolve};
use pinwheel_core::hotkey::Hotkey;
use pinwheel_core::i18n::{Lang, Texts};
use pinwheel_core::settings::{self, OsdPosition, OsdStyle, OutputBinding, Settings, duplicate_hotkey};
use pinwheel_core::status::{self, Status};
use pinwheel_core::volume::STEP_RANGE;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SW_RESTORE, SW_SHOWNORMAL, SetForegroundWindow, ShowWindow,
};
use windows::core::{PCWSTR, w};

const REFRESH_EVERY: Duration = Duration::from_secs(1);
const PAGE_MARGIN: Margin = Margin {
    left: 24,
    right: 24,
    top: 4,
    bottom: 24,
};

pub fn run() {
    let Some(_lock) = InstanceLock::acquire(ipc::SETTINGS_MUTEX) else {
        focus_existing_window();
        return;
    };
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

    let app = SettingsApp::new();
    let title = app.texts().settings_title;
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(title)
        .with_inner_size([640.0, 600.0])
        .with_min_inner_size([520.0, 420.0]);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    let setup = Box::new(|cc: &eframe::CreationContext| {
        app.themes.install(&cc.egui_ctx);
        Ok(Box::new(app) as Box<dyn eframe::App>)
    });
    if let Err(e) = eframe::run_native(title, options, setup) {
        log::error!("the settings window failed: {e}");
    }
}

/// The open window may still have the title of another language.
fn focus_existing_window() {
    for lang in Lang::ALL {
        let title = wide(lang.texts().settings_title);
        if let Ok(hwnd) = unsafe { FindWindowW(PCWSTR::null(), PCWSTR(title.as_ptr())) } {
            unsafe {
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(hwnd);
            }
            return;
        }
    }
}

/// The largest image of the icon, for the title bar and taskbar.
fn window_icon() -> Option<egui::IconData> {
    let pixmap = icon::images().into_iter().max_by_key(tiny_skia::Pixmap::width)?;
    let rgba = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    Some(egui::IconData {
        rgba,
        width: pixmap.width(),
        height: pixmap.height(),
    })
}

struct SettingsApp {
    path: PathBuf,
    settings: Settings,
    /// The settings as last read from or written to the file.
    saved: Settings,
    /// Modification time of the file as last read or written.
    file_time: Option<SystemTime>,
    audio: Option<Audio>,
    devices: Vec<DeviceInfo>,
    status: Status,
    startup: StartupState,
    main_running: bool,
    last_refresh: Option<Instant>,
    error: Option<String>,
    capture: Option<ActiveCapture>,
    /// ID of the Windows default output.
    default_id: Option<String>,
    open: Option<Fold>,
    /// Language of the window title, updated when the setting changes.
    title_lang: Lang,
    themes: Themes,
}

/// Shortcut being captured for the output at `index`.
struct ActiveCapture {
    index: usize,
    capture: Capture,
    started: Instant,
}

impl ActiveCapture {
    /// The share of the capture's time left, from 1 to 0.
    fn remaining(&self) -> f32 {
        let timeout = Duration::from_millis(u64::from(keys::TIMEOUT_MS));
        1.0 - self.started.elapsed().as_secs_f32() / timeout.as_secs_f32()
    }
}

impl SettingsApp {
    fn new() -> Self {
        let path = paths::settings_file();
        let (settings, issue) = settings::load(&path);
        let lang = language::current(&settings);
        let texts = lang.texts();
        let error = issue.map(|issue| match issue {
            settings::LoadIssue::Corrupt { backup, .. } => (texts.settings_corrupt)(&backup.display().to_string()),
            settings::LoadIssue::Io(e) => (texts.settings_unreadable)(&e.to_string()),
        });
        Self {
            file_time: file_time(&path),
            path,
            saved: settings.clone(),
            settings,
            audio: None,
            devices: Vec::new(),
            status: Status::default(),
            startup: startup::state(),
            main_running: false,
            last_refresh: None,
            error,
            capture: None,
            default_id: None,
            open: None,
            title_lang: lang,
            themes: Themes::new(),
        }
    }

    fn lang(&self) -> Lang {
        language::current(&self.settings)
    }

    fn texts(&self) -> &'static Texts {
        self.lang().texts()
    }

    fn start_capture(&mut self, index: usize, ctx: &egui::Context) {
        // Dropping a previous capture stops it first.
        self.capture = None;
        let ctx = ctx.clone();
        match Capture::start(move || ctx.request_repaint()) {
            Ok(capture) => {
                self.capture = Some(ActiveCapture {
                    index,
                    capture,
                    started: Instant::now(),
                })
            }
            Err(e) => {
                log::error!("starting the shortcut capture failed: {e}");
                self.error = Some(self.texts().capture_failed.into());
            }
        }
    }

    /// The capture ends by itself (key pressed, Escape, focus lost, timeout)
    /// and wakes the UI up with its result.
    fn poll_capture(&mut self) {
        let Some(active) = &self.capture else { return };
        let index = active.index;
        let Some(result) = active.capture.poll() else { return };
        self.capture = None;
        match result {
            Captured::Hotkey(hotkey) => {
                if let Some(output) = self.settings.outputs.get_mut(index) {
                    output.hotkey = Some(hotkey);
                }
            }
            Captured::Cancelled => {}
            Captured::Failed => self.error = Some(self.texts().capture_failed.into()),
        }
    }

    fn refresh(&mut self) {
        if self.last_refresh.is_some_and(|at| at.elapsed() < REFRESH_EVERY) {
            return;
        }
        self.last_refresh = Some(Instant::now());

        if self.audio.is_none() {
            self.audio = Audio::new().inspect_err(|e| log::warn!("audio unavailable: {e}")).ok();
        }
        if let Some(audio) = &self.audio {
            self.default_id = audio.default_output_id().ok();
            match audio.outputs() {
                Ok(devices) => self.devices = devices,
                Err(e) => {
                    log::warn!("listing outputs failed: {e}");
                    self.audio = None;
                }
            }
        }
        self.status = status::load(&paths::status_file());
        self.startup = startup::state();
        self.main_running = unsafe { FindWindowW(ipc::MAIN_CLASS, PCWSTR::null()) }.is_ok();

        if file_time(&self.path) != self.file_time {
            match settings::load(&self.path) {
                // Kept as they are, so a failed read cannot turn into saved
                // defaults; the file is read again on the next refresh.
                (_, Some(settings::LoadIssue::Io(e))) => log::warn!("reading the settings failed: {e}"),
                (loaded, issue) => {
                    if let Some(settings::LoadIssue::Corrupt { backup, .. }) = issue {
                        self.error = Some((self.texts().settings_corrupt)(&backup.display().to_string()));
                    }
                    self.file_time = file_time(&self.path);
                    self.saved = loaded.clone();
                    self.settings = loaded;
                }
            }
        }
    }

    fn save(&mut self) {
        // Marked as saved even on failure: the error shows, and the next
        // change tries again instead of every frame.
        self.saved = self.settings.clone();
        match settings::save(&self.path, &self.settings) {
            Ok(()) => {
                self.file_time = file_time(&self.path);
                self.error = None;
                ipc::notify_main(WM_RELOAD);
            }
            Err(e) => {
                log::error!("saving the settings failed: {e}");
                self.error = Some((self.texts().settings_save_failed)(&e.to_string()));
            }
        }
    }

    fn toggle_fold(&mut self, fold: Fold) {
        self.open = if self.open.as_ref() == Some(&fold) {
            None
        } else {
            Some(fold)
        };
    }

    fn banners(&mut self, ui: &mut egui::Ui) {
        let t = self.texts();
        let palette = self.themes.palette(ui);
        if let Some(error) = &self.error {
            theme::warning_banner(ui, palette, |ui| {
                ui.label(RichText::new(error).color(palette.error));
            });
        }
        if !self.main_running {
            theme::warning_banner(ui, palette, |ui| {
                theme::labelled(ui, t.not_running, None, |ui| {
                    if ui.button(t.start_app).clicked()
                        && let Ok(exe) = std::env::current_exe()
                    {
                        let _ = std::process::Command::new(exe).spawn();
                    }
                });
            });
        }
        if self.audio.is_none() {
            theme::warning_banner(ui, palette, |ui| {
                ui.label(RichText::new(t.audio_unavailable).color(palette.warning));
            });
        }
    }

    fn outputs_section(&mut self, ui: &mut egui::Ui) {
        let (lang, t) = (self.lang(), self.texts());
        let palette = self.themes.palette(ui);
        theme::heading(ui, t.outputs_heading);

        let capturing = self.capture.as_ref().map(|c| (c.index, c.remaining()));
        let mut event = None;
        for index in 0..self.settings.outputs.len() {
            let duplicate_of = duplicate_hotkey(&self.settings.outputs, index).map(|o| o.name.clone());
            let refused = self.status.hotkey_errors.contains_key(&self.settings.outputs[index].id);
            let state = device_state(&self.settings.outputs[index], &self.devices, self.default_id.as_deref());
            let output = &mut self.settings.outputs[index];
            let open = self.open.as_ref() == Some(&Fold::Output(output.id.clone()));
            let captured = capturing.filter(|(i, _)| *i == index).map(|(_, remaining)| remaining);
            ui.add_enabled_ui(capturing.is_none() || captured.is_some(), |ui| {
                theme::fold_card(ui, palette, |ui| {
                    let (clicked, action) = theme::fold_header(
                        ui,
                        palette,
                        open || captured.is_some(),
                        |ui| {
                            ui.vertical(|ui| {
                                let name = RichText::new(&output.name);
                                ui.label(if state.connected() {
                                    name
                                } else {
                                    name.color(palette.secondary)
                                });
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 0.0;
                                    ui.label(theme::help(palette, state.label(t)));
                                    if state.is_default() {
                                        ui.label(theme::help(palette, " · "));
                                        ui.label(theme::accented(palette, t.default_output));
                                    }
                                });
                            });
                        },
                        |ui| {
                            if open || captured.is_some() {
                                return theme::remove_button(ui, palette, t.remove)
                                    .clicked()
                                    .then_some(OutputAction::Remove);
                            }
                            match &output.hotkey {
                                Some(hotkey) => {
                                    keycaps(ui, palette, hotkey, lang);
                                    None
                                }
                                None => ui.button(t.set_key).clicked().then_some(OutputAction::Capture),
                            }
                        },
                    );
                    let mut action = action.or((clicked && captured.is_none()).then_some(OutputAction::Toggle));
                    if let Some(remaining) = captured {
                        if capture_box(ui, palette, t, remaining) {
                            action = Some(OutputAction::StopCapture);
                        }
                    } else if open {
                        let controls = theme::fold_row(ui, palette, t.shortcut, None, |ui| {
                            hotkey_controls(ui, palette, output.hotkey.as_ref(), lang)
                        });
                        action = action.or(controls);
                    }
                    hotkey_messages(
                        ui,
                        palette,
                        output.hotkey.as_ref(),
                        duplicate_of.as_deref(),
                        refused,
                        lang,
                    );
                    if open && captured.is_none() {
                        theme::fold_row(ui, palette, t.communications, None, |ui| {
                            switch(ui, palette, &mut output.communications, t.communications, t);
                        });
                    }
                    if let Some(action) = action {
                        event = Some((index, action));
                    }
                });
            });
        }

        let Some((index, action)) = event else { return };
        let fold = Fold::Output(self.settings.outputs[index].id.clone());
        match action {
            OutputAction::Toggle => self.toggle_fold(fold),
            OutputAction::Capture => {
                self.open = Some(fold);
                self.start_capture(index, ui.ctx());
            }
            OutputAction::StopCapture => self.capture = None,
            OutputAction::Clear => self.settings.outputs[index].hotkey = None,
            OutputAction::Remove => {
                // Indexes shift: a capture in progress would land on another output.
                self.capture = None;
                self.open = None;
                self.settings.outputs.remove(index);
            }
        }
    }

    fn add_section(&mut self, ui: &mut egui::Ui) {
        let t = self.texts();
        let palette = self.themes.palette(ui);
        let open = self.open == Some(Fold::Add);
        let available: Vec<&DeviceInfo> = self
            .devices
            .iter()
            .filter(|d| d.active && !self.settings.outputs.iter().any(|o| o.id == d.id))
            .collect();
        let mut toggled = false;
        let mut added = None;
        ui.add_enabled_ui(self.capture.is_none(), |ui| {
            theme::fold_card(ui, palette, |ui| {
                (toggled, ()) = theme::fold_header(
                    ui,
                    palette,
                    open,
                    |ui| {
                        ui.label(RichText::new(format!("+  {}", t.add_output)).color(palette.accent));
                    },
                    |_| {},
                );
                if !open {
                    return;
                }
                if available.is_empty() {
                    theme::fold_body(ui, palette, |ui| ui.label(theme::help(palette, t.nothing_to_add)));
                }
                for device in &available {
                    theme::fold_row(ui, palette, &device.name, None, |ui| {
                        if ui.button(t.add).clicked() {
                            added = Some((*device).clone());
                        }
                    });
                }
            });
        });
        if let Some(device) = added {
            self.open = Some(Fold::Output(device.id.clone()));
            self.settings.outputs.push(OutputBinding {
                id: device.id,
                name: device.name,
                hotkey: None,
                communications: false,
            });
        } else if toggled {
            self.toggle_fold(Fold::Add);
        }
    }

    fn wheel_section(&mut self, ui: &mut egui::Ui) {
        let (lang, t) = (self.lang(), self.texts());
        let palette = self.themes.palette(ui);
        let open = self.open == Some(Fold::Wheel);
        let main_running = self.main_running;
        let step = &mut self.settings.volume_step;
        let osd = &mut self.settings.osd;
        let mut toggled = false;
        ui.add_enabled_ui(self.capture.is_none(), |ui| {
            theme::fold_card(ui, palette, |ui| {
                let summary = (t.wheel_summary)(i32::from(*step), osd.style.label(lang), osd.position.label(lang));
                (toggled, ()) = theme::fold_header(
                    ui,
                    palette,
                    open,
                    |ui| {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                theme::keycap(ui, palette, "Win");
                                ui.label(t.wheel_heading);
                            });
                            ui.label(theme::help(palette, &summary));
                        });
                    },
                    |ui| {
                        if open && ui.add_enabled(main_running, egui::Button::new(t.try_display)).clicked() {
                            ipc::notify_main(WM_PREVIEW_OSD);
                        }
                    },
                );
                if !open {
                    return;
                }
                theme::fold_row(ui, palette, t.volume_step, None, |ui| {
                    ui.label((t.percent)(i32::from(*step)));
                    ui.add(egui::Slider::new(step, STEP_RANGE).show_value(false));
                });
                theme::fold_row(ui, palette, t.style, None, |ui| {
                    theme::segmented(ui, palette, &mut osd.style, &OsdStyle::ALL.map(|s| (s, s.label(lang))));
                });
                theme::fold_row(ui, palette, t.position, None, |ui| {
                    theme::segmented(
                        ui,
                        palette,
                        &mut osd.position,
                        &OsdPosition::ALL.map(|p| (p, p.label(lang))),
                    );
                });
            });
        });
        if toggled {
            self.toggle_fold(Fold::Wheel);
        }
    }

    fn general_section(&mut self, ui: &mut egui::Ui) {
        let t = self.texts();
        let palette = self.themes.palette(ui);
        let open = self.open == Some(Fold::General);
        let disabled_by_windows = self.startup == StartupState::DisabledByWindows;
        let mut startup = self.startup == StartupState::On;
        let language = &mut self.settings.language;
        let mut toggled = false;
        let mut startup_changed = false;
        ui.add_enabled_ui(self.capture.is_none(), |ui| {
            theme::fold_card(ui, palette, |ui| {
                let language_name = language.map_or(t.windows_language, Lang::native_name);
                let summary = (t.general_summary)(language_name, startup);
                (toggled, ()) = theme::fold_header(
                    ui,
                    palette,
                    open,
                    |ui| {
                        ui.vertical(|ui| {
                            ui.label(t.general_heading);
                            ui.label(theme::help(palette, &summary));
                        });
                    },
                    |_| {},
                );
                if !open {
                    return;
                }
                theme::fold_row(ui, palette, t.language, None, |ui| {
                    let choices = [Lang::English, Lang::French].map(|l| (Some(l), l.native_name()));
                    let choices = [(None, t.language_windows), choices[0], choices[1]];
                    theme::segmented(ui, palette, language, &choices);
                });
                let help = disabled_by_windows.then(|| theme::note(t.startup_disabled_by_windows, palette.warning));
                startup_changed = theme::fold_row(ui, palette, t.startup_checkbox, help, |ui| {
                    switch(ui, palette, &mut startup, t.startup_checkbox, t)
                });
                theme::fold_row(ui, palette, t.logs, None, |ui| {
                    if ui.button(t.open_folder).clicked() {
                        open_folder(&paths::log_dir());
                    }
                });
            });
        });
        if startup_changed {
            if let Err(e) = startup::set(startup) {
                log::error!("changing the startup entry failed: {e}");
                self.error = Some((t.startup_failed)(&e.to_string()));
            }
            self.startup = startup::state();
        }
        if toggled {
            self.toggle_fold(Fold::General);
        }
    }
}

impl eframe::App for SettingsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.refresh();
        ui.ctx().request_repaint_after(REFRESH_EVERY);

        self.poll_capture();
        if self.capture.is_some() {
            // The countdown moves.
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        let lang = self.lang();
        if lang != self.title_lang {
            self.title_lang = lang;
            let title = lang.texts().settings_title.into();
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
        let page = egui::Frame::new().fill(ui.visuals().panel_fill);
        egui::CentralPanel::default().frame(page).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Frame::new().inner_margin(PAGE_MARGIN).show(ui, |ui| {
                    // The gap between cards.
                    ui.spacing_mut().item_spacing.y = 4.0;
                    self.banners(ui);
                    self.outputs_section(ui);
                    self.add_section(ui);
                    ui.add_space(16.0);
                    self.wheel_section(ui);
                    self.general_section(ui);
                });
            });
        });
        // A slider being dragged is saved once, when it is released: each
        // save makes the background process register every shortcut again.
        let dragging = ui.ctx().input(|i| i.pointer.any_down());
        if self.settings != self.saved && !dragging {
            self.save();
        }
    }
}

/// Where a saved output stands, for its card.
enum DeviceState {
    /// `default`: it is the Windows default output right now.
    Connected {
        default: bool,
    },
    Disconnected,
    NotFound,
}

impl DeviceState {
    fn label(&self, t: &'static Texts) -> &'static str {
        match self {
            Self::Connected { .. } => t.connected,
            Self::Disconnected => t.disconnected,
            Self::NotFound => t.not_found,
        }
    }

    fn connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    fn is_default(&self) -> bool {
        matches!(self, Self::Connected { default: true })
    }
}

fn device_state(output: &OutputBinding, devices: &[DeviceInfo], default_id: Option<&str>) -> DeviceState {
    match resolve(&output.id, &output.name, devices) {
        Resolution::Ready(device) | Resolution::Moved(device) => DeviceState::Connected {
            default: default_id == Some(device.id.as_str()),
        },
        Resolution::Unavailable => DeviceState::Disconnected,
        Resolution::Unknown => DeviceState::NotFound,
    }
}

/// The fold open in the window: one at a time.
#[derive(Clone, PartialEq, Eq)]
enum Fold {
    /// The output with this ID.
    Output(String),
    Add,
    Wheel,
    General,
}

enum OutputAction {
    Toggle,
    Capture,
    StopCapture,
    Clear,
    Remove,
}

/// The keys of a shortcut, added right to left.
fn keycaps(ui: &mut egui::Ui, palette: &Palette, hotkey: &Hotkey, lang: Lang) {
    ui.spacing_mut().item_spacing.x = 4.0;
    for key in keys::describe(hotkey, lang).iter().rev() {
        theme::keycap(ui, palette, key);
    }
}

/// The shortcut's keys and buttons, added right to left.
fn hotkey_controls(ui: &mut egui::Ui, palette: &Palette, hotkey: Option<&Hotkey>, lang: Lang) -> Option<OutputAction> {
    let t = lang.texts();
    let mut action = None;
    match hotkey {
        Some(hotkey) => {
            if ui.button(t.clear).clicked() {
                action = Some(OutputAction::Clear);
            }
            if ui.button(t.change).clicked() {
                action = Some(OutputAction::Capture);
            }
            keycaps(ui, palette, hotkey, lang);
        }
        None => {
            if ui.button(t.set).clicked() {
                action = Some(OutputAction::Capture);
            }
            ui.label(RichText::new(t.no_shortcut).color(palette.secondary));
        }
    }
    action
}

/// The box that replaces the shortcut while a capture waits for keys.
/// Returns `true` when Cancel is clicked.
fn capture_box(ui: &mut egui::Ui, palette: &Palette, t: &'static Texts, remaining: f32) -> bool {
    theme::fold_body(ui, palette, |ui| {
        theme::waiting_frame(ui, palette, |ui| {
            let cancel = theme::labelled(
                ui,
                theme::strong(t.capture_prompt),
                Some(theme::help(palette, t.capture_cancel_hint)),
                |ui| ui.button(t.cancel).clicked(),
            );
            ui.add_space(6.0);
            theme::countdown(ui, palette, remaining);
            cancel
        })
    })
}

/// A switch with its state written next to it, added right to left.
fn switch(ui: &mut egui::Ui, palette: &Palette, on: &mut bool, label: &str, t: &'static Texts) -> bool {
    let changed = theme::toggle(ui, palette, on, label).changed();
    ui.label(if *on { t.on } else { t.off });
    changed
}

fn hotkey_messages(
    ui: &mut egui::Ui,
    palette: &Palette,
    hotkey: Option<&Hotkey>,
    duplicate_of: Option<&str>,
    refused: bool,
    lang: Lang,
) {
    let Some(hotkey) = hotkey else { return };
    let t = lang.texts();
    if let Err(e) = hotkey.validate() {
        theme::fold_message(ui, palette, e.message(lang), palette.error);
    } else if let Some(other) = duplicate_of {
        theme::fold_message(ui, palette, &(t.duplicate_shortcut)(other), palette.error);
    } else if refused {
        theme::fold_message(ui, palette, t.shortcut_refused, palette.error);
    }
    for warning in hotkey.warnings() {
        theme::fold_message(ui, palette, warning.message(lang), palette.warning);
    }
}

fn file_time(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn open_folder(path: &std::path::Path) {
    let _ = std::fs::create_dir_all(path);
    let target = wide(&path.display().to_string());
    unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(target.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
}
