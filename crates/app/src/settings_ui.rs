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
        .with_inner_size([640.0, 760.0])
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
    /// Language of the window title, updated when the setting changes.
    title_lang: Lang,
    themes: Themes,
}

/// Shortcut being captured for the output at `index`.
struct ActiveCapture {
    index: usize,
    capture: Capture,
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
            Ok(capture) => self.capture = Some(ActiveCapture { index, capture }),
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
        log::info!("PROBE ui got {result:?}");
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
    }

    fn outputs_section(&mut self, ui: &mut egui::Ui) {
        let (lang, t) = (self.lang(), self.texts());
        let palette = self.themes.palette(ui);
        theme::section_title(ui, t.outputs_heading);
        ui.label(theme::help(palette, t.outputs_help));
        ui.add_space(4.0);

        let mut remove = None;
        let mut capture_request = None;
        let capturing = self.capture.as_ref().map(|c| c.index);
        for index in 0..self.settings.outputs.len() {
            let duplicate_of = duplicate_hotkey(&self.settings.outputs, index).map(|o| o.name.clone());
            let refused = self.status.hotkey_errors.contains_key(&self.settings.outputs[index].id);
            let (state, connected) = device_state(&self.settings.outputs[index], &self.devices, t);
            let output = &mut self.settings.outputs[index];
            theme::card(ui, palette, |ui| {
                let state_color = if connected { palette.success } else { palette.secondary };
                theme::labelled(
                    ui,
                    theme::strong(&output.name),
                    Some(theme::note(state, state_color)),
                    |ui| {
                        if ui.button(t.remove).clicked() {
                            remove = Some(index);
                        }
                    },
                );
                theme::divider(ui, palette);
                let action = theme::labelled(ui, t.shortcut, None, |ui| {
                    hotkey_controls(ui, palette, output.hotkey.as_ref(), capturing == Some(index), lang)
                });
                match action {
                    HotkeyAction::Capture => capture_request = Some(Some(index)),
                    HotkeyAction::StopCapture => capture_request = Some(None),
                    HotkeyAction::Clear => output.hotkey = None,
                    HotkeyAction::None => {}
                }
                hotkey_messages(ui, palette, output.hotkey.as_ref(), duplicate_of.as_deref(), refused, lang);
                theme::divider(ui, palette);
                theme::labelled(ui, t.communications, None, |ui| {
                    theme::toggle(ui, palette, &mut output.communications, t.communications);
                });
            });
        }
        match capture_request {
            Some(Some(index)) => self.start_capture(index, ui.ctx()),
            Some(None) => self.capture = None,
            None => {}
        }
        if let Some(index) = remove {
            // Indexes shift: a capture in progress would land on another output.
            self.capture = None;
            self.settings.outputs.remove(index);
        }

        if self.audio.is_none() {
            ui.label(theme::note(t.audio_unavailable, self.themes.palette(ui).warning));
        }
        let available: Vec<DeviceInfo> = self
            .devices
            .iter()
            .filter(|d| d.active && !self.settings.outputs.iter().any(|o| o.id == d.id))
            .cloned()
            .collect();
        let mut added = None;
        ui.add_space(2.0);
        ui.add_enabled_ui(!available.is_empty(), |ui| {
            egui::ComboBox::from_id_salt("add-output")
                .selected_text(t.add_output)
                .width(320.0)
                .show_ui(ui, |ui| {
                    for device in &available {
                        if ui.selectable_label(false, &device.name).clicked() {
                            added = Some(device.clone());
                        }
                    }
                });
        });
        if let Some(device) = added {
            self.settings.outputs.push(OutputBinding {
                id: device.id,
                name: device.name,
                hotkey: None,
                communications: false,
            });
        }
    }

    fn volume_section(&mut self, ui: &mut egui::Ui) {
        let t = self.texts();
        let palette = self.themes.palette(ui);
        theme::section_title(ui, t.volume_heading);
        let step = &mut self.settings.volume_step;
        theme::setting_row(ui, palette, t.volume_step, Some(theme::help(palette, t.volume_help)), |ui| {
            ui.label((t.percent)(i32::from(*step)));
            ui.add(egui::Slider::new(step, STEP_RANGE).show_value(false));
        });
    }

    fn display_section(&mut self, ui: &mut egui::Ui) {
        let (lang, t) = (self.lang(), self.texts());
        let palette = self.themes.palette(ui);
        theme::section_title(ui, t.display_heading);
        let osd = &mut self.settings.osd;
        theme::setting_row(ui, palette, t.style, None, |ui| {
            egui::ComboBox::from_id_salt("style")
                .selected_text(osd.style.label(lang))
                .show_ui(ui, |ui| {
                    for style in OsdStyle::ALL {
                        ui.selectable_value(&mut osd.style, style, style.label(lang));
                    }
                });
        });
        theme::setting_row(ui, palette, t.position, None, |ui| {
            egui::ComboBox::from_id_salt("position")
                .selected_text(osd.position.label(lang))
                .show_ui(ui, |ui| {
                    for position in OsdPosition::ALL {
                        ui.selectable_value(&mut osd.position, position, position.label(lang));
                    }
                });
        });
        let main_running = self.main_running;
        theme::setting_row(ui, palette, t.preview, Some(theme::help(palette, t.preview_help)), |ui| {
            if ui.add_enabled(main_running, egui::Button::new(t.show)).clicked() {
                ipc::notify_main(WM_PREVIEW_OSD);
            }
        });
    }

    fn general_section(&mut self, ui: &mut egui::Ui) {
        let t = self.texts();
        let palette = self.themes.palette(ui);
        theme::section_title(ui, t.general_heading);
        let language = &mut self.settings.language;
        theme::setting_row(ui, palette, t.language, None, |ui| {
            let selected = language.map_or(t.language_windows, Lang::native_name);
            egui::ComboBox::from_id_salt("language")
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    ui.selectable_value(language, None, t.language_windows);
                    for lang in Lang::ALL {
                        ui.selectable_value(language, Some(lang), lang.native_name());
                    }
                });
        });

        let disabled_by_windows = self.startup == StartupState::DisabledByWindows;
        let help = disabled_by_windows.then(|| theme::note(t.startup_disabled_by_windows, palette.warning));
        let mut enabled = self.startup == StartupState::On;
        let changed = theme::setting_row(ui, palette, t.startup_checkbox, help, |ui| {
            theme::toggle(ui, palette, &mut enabled, t.startup_checkbox).changed()
        });
        if changed {
            if let Err(e) = startup::set(enabled) {
                log::error!("changing the startup entry failed: {e}");
                self.error = Some((t.startup_failed)(&e.to_string()));
            }
            self.startup = startup::state();
        }

        theme::setting_row(ui, palette, t.logs, None, |ui| {
            if ui.button(t.open_folder).clicked() {
                open_folder(&paths::log_dir());
            }
        });
    }
}

impl eframe::App for SettingsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.refresh();
        ui.ctx().request_repaint_after(REFRESH_EVERY);
        ui.ctx().input(|i| {
            for event in &i.events {
                if let egui::Event::Key { key, pressed, modifiers, .. } = event {
                    log::info!("PROBE egui key {key:?} pressed={pressed} {modifiers:?}");
                }
            }
        });

        self.poll_capture();
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
                    self.volume_section(ui);
                    self.display_section(ui);
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

/// Label of a saved output's state, and whether it is plugged in.
fn device_state(output: &OutputBinding, devices: &[DeviceInfo], t: &'static Texts) -> (&'static str, bool) {
    match resolve(&output.id, &output.name, devices) {
        Resolution::Ready(_) | Resolution::Moved(_) => (t.connected, true),
        Resolution::Unavailable => (t.disconnected, false),
        Resolution::Unknown => (t.not_found, false),
    }
}

enum HotkeyAction {
    None,
    Capture,
    StopCapture,
    Clear,
}

/// The shortcut's keys and buttons, added right to left.
fn hotkey_controls(
    ui: &mut egui::Ui,
    palette: &Palette,
    hotkey: Option<&Hotkey>,
    capturing: bool,
    lang: Lang,
) -> HotkeyAction {
    let t = lang.texts();
    let mut action = HotkeyAction::None;
    if capturing {
        if ui.button(t.cancel).clicked() {
            action = HotkeyAction::StopCapture;
        }
        theme::keycap(ui, palette, t.capture_prompt, true);
        return action;
    }
    match hotkey {
        Some(hotkey) => {
            if ui.button(t.clear).clicked() {
                action = HotkeyAction::Clear;
            }
            if ui.button(t.change).clicked() {
                action = HotkeyAction::Capture;
            }
            ui.spacing_mut().item_spacing.x = 4.0;
            for key in keys::describe(hotkey, lang).iter().rev() {
                theme::keycap(ui, palette, key, false);
            }
        }
        None => {
            if ui.button(t.choose).clicked() {
                action = HotkeyAction::Capture;
            }
            ui.label(RichText::new(t.no_shortcut).color(palette.secondary));
        }
    }
    action
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
        ui.label(theme::note(e.message(lang), palette.error));
    } else if let Some(other) = duplicate_of {
        ui.label(theme::note(&(t.duplicate_shortcut)(other), palette.error));
    } else if refused {
        ui.label(theme::note(t.shortcut_refused, palette.error));
    }
    for warning in hotkey.warnings() {
        ui.label(theme::note(warning.message(lang), palette.warning));
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
