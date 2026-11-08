//! The settings window (process started with `--settings`).
//!
//! Every change is saved right away, then the background process is told to
//! reload. The window polls the device list, the status file and the
//! settings file once a second; the latter can change behind its back when
//! the background process rebinds a moved output.

use crate::audio::Audio;
use crate::ipc::{self, InstanceLock, WM_PREVIEW_OSD, WM_RELOAD, wide};
use crate::paths;
use crate::startup::{self, StartupState};
use eframe::egui;
use sounddock_core::devices::{DeviceInfo, Resolution, resolve};
use sounddock_core::hotkey::{Hotkey, KEYS, key_label};
use sounddock_core::settings::{self, OsdPosition, OsdStyle, OutputBinding, Settings, duplicate_hotkey};
use sounddock_core::status::{self, Status};
use sounddock_core::volume::STEP_RANGE;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SW_RESTORE, SW_SHOWNORMAL, SetForegroundWindow, ShowWindow,
};
use windows::core::{PCWSTR, w};

const REFRESH_EVERY: Duration = Duration::from_secs(1);

pub fn run() {
    let Some(_lock) = InstanceLock::acquire(ipc::SETTINGS_MUTEX) else {
        focus_existing_window();
        return;
    };
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(ipc::SETTINGS_TITLE)
        .with_inner_size([620.0, 720.0])
        .with_min_inner_size([480.0, 360.0]);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    if let Err(e) = eframe::run_native(
        ipc::SETTINGS_TITLE,
        options,
        Box::new(|_| Ok(Box::new(SettingsApp::new()))),
    ) {
        log::error!("the settings window failed: {e}");
    }
}

fn focus_existing_window() {
    let title = wide(ipc::SETTINGS_TITLE);
    if let Ok(hwnd) = unsafe { FindWindowW(PCWSTR::null(), PCWSTR(title.as_ptr())) } {
        unsafe {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

/// The 256 px image of the embedded icon, for the title bar and taskbar.
fn window_icon() -> Option<egui::IconData> {
    let ico: &[u8] = include_bytes!("../res/sounddock.ico");
    let count = usize::from(u16::from_le_bytes([ico[4], ico[5]]));
    (0..count).find_map(|i| {
        let entry = ico.get(6 + 16 * i..22 + 16 * i)?;
        if entry[0] != 0 {
            return None;
        }
        let size = u32::from_le_bytes(entry[8..12].try_into().ok()?) as usize;
        let offset = u32::from_le_bytes(entry[12..16].try_into().ok()?) as usize;
        let pixmap = tiny_skia::Pixmap::decode_png(ico.get(offset..offset + size)?).ok()?;
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
    })
}

struct SettingsApp {
    path: PathBuf,
    settings: Settings,
    /// Modification time of the file as last read or written.
    file_time: Option<SystemTime>,
    audio: Option<Audio>,
    devices: Vec<DeviceInfo>,
    status: Status,
    startup: StartupState,
    main_running: bool,
    last_refresh: Option<Instant>,
    error: Option<String>,
}

impl SettingsApp {
    fn new() -> Self {
        let path = paths::settings_file();
        let (settings, issue) = settings::load(&path);
        let error = issue.map(|issue| match issue {
            settings::LoadIssue::Corrupt { backup, .. } => {
                format!(
                    "Le fichier de paramètres était illisible. Il a été gardé sous {}.",
                    backup.display()
                )
            }
            settings::LoadIssue::Io(e) => format!("Lecture des paramètres impossible : {e}"),
        });
        Self {
            file_time: file_time(&path),
            path,
            settings,
            audio: None,
            devices: Vec::new(),
            status: Status::default(),
            startup: startup::state(),
            main_running: false,
            last_refresh: None,
            error,
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

        let time = file_time(&self.path);
        if time != self.file_time {
            self.file_time = time;
            self.settings = settings::load(&self.path).0;
        }
    }

    fn save(&mut self) {
        match settings::save(&self.path, &self.settings) {
            Ok(()) => {
                self.file_time = file_time(&self.path);
                self.error = None;
                ipc::notify_main(WM_RELOAD);
            }
            Err(e) => {
                log::error!("saving the settings failed: {e}");
                self.error = Some(format!("Enregistrement impossible : {e}"));
            }
        }
    }

    fn banners(&mut self, ui: &mut egui::Ui) {
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if !self.main_running {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    "SoundDock ne tourne pas : la molette et les raccourcis sont inactifs.",
                );
                if ui.button("Lancer SoundDock").clicked()
                    && let Ok(exe) = std::env::current_exe()
                {
                    let _ = std::process::Command::new(exe).spawn();
                }
            });
        }
    }

    fn outputs_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("Sorties audio");
        ui.label("Le raccourci d'une sortie en fait la sortie par défaut. Une sortie débranchée garde son raccourci.");
        ui.add_space(4.0);

        let mut remove = None;
        for index in 0..self.settings.outputs.len() {
            let duplicate_of = duplicate_hotkey(&self.settings.outputs, index).map(|o| o.name.clone());
            let refused = self.status.hotkey_errors.get(&self.settings.outputs[index].id).cloned();
            let (state, connected) = device_state(&self.settings.outputs[index], &self.devices);
            let output = &mut self.settings.outputs[index];
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.strong(&output.name);
                    if connected {
                        ui.label(state)
                    } else {
                        ui.weak(state)
                    };
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Retirer").clicked() {
                            remove = Some(index);
                        }
                    });
                });
                hotkey_editor(ui, index, &mut output.hotkey);
                ui.checkbox(&mut output.communications, "Aussi pour les appels (Teams, Discord…)");
                hotkey_messages(ui, output.hotkey.as_ref(), duplicate_of.as_deref(), refused.as_deref());
            });
        }
        if let Some(index) = remove {
            self.settings.outputs.remove(index);
        }

        if self.audio.is_none() {
            ui.colored_label(ui.visuals().warn_fg_color, "Le service audio de Windows ne répond pas.");
        }
        let available: Vec<DeviceInfo> = self
            .devices
            .iter()
            .filter(|d| d.active && !self.settings.outputs.iter().any(|o| o.id == d.id))
            .cloned()
            .collect();
        let mut added = None;
        ui.add_enabled_ui(!available.is_empty(), |ui| {
            egui::ComboBox::from_id_salt("add-output")
                .selected_text("Ajouter une sortie…")
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
        ui.heading("Volume");
        ui.label("Win + molette règle le volume de la sortie par défaut.");
        ui.add(
            egui::Slider::new(&mut self.settings.volume_step, STEP_RANGE)
                .text("par cran de molette")
                .suffix(" %"),
        );
    }

    fn display_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("Affichage");
        ui.horizontal(|ui| {
            ui.label("Style :");
            for style in OsdStyle::ALL {
                ui.radio_value(&mut self.settings.osd.style, style, style.label());
            }
        });
        ui.horizontal(|ui| {
            ui.label("Position :");
            for position in OsdPosition::ALL {
                ui.radio_value(&mut self.settings.osd.position, position, position.label());
            }
        });
        if ui.add_enabled(self.main_running, egui::Button::new("Aperçu")).clicked() {
            ipc::notify_main(WM_PREVIEW_OSD);
        }
    }

    fn startup_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("Démarrage");
        let mut enabled = self.startup == StartupState::On;
        if ui
            .checkbox(&mut enabled, "Lancer SoundDock à l'ouverture de session")
            .changed()
        {
            if let Err(e) = startup::set(enabled) {
                log::error!("changing the startup entry failed: {e}");
                self.error = Some(format!("Modification du démarrage impossible : {e}"));
            }
            self.startup = startup::state();
        }
        if self.startup == StartupState::DisabledByWindows {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "Désactivé dans les applications de démarrage de Windows. Cocher la case le réactive.",
            );
        }
    }
}

impl eframe::App for SettingsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.refresh();
        ui.ctx().request_repaint_after(REFRESH_EVERY);

        let before = self.settings.clone();
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.banners(ui);
                self.outputs_section(ui);
                ui.separator();
                self.volume_section(ui);
                ui.separator();
                self.display_section(ui);
                ui.separator();
                self.startup_section(ui);
                ui.separator();
                if ui.button("Ouvrir le dossier des journaux").clicked() {
                    open_folder(&paths::log_dir());
                }
            });
        });
        if self.settings != before {
            self.save();
        }
    }
}

/// Label of a saved output's state, and whether it is plugged in.
fn device_state(output: &OutputBinding, devices: &[DeviceInfo]) -> (&'static str, bool) {
    match resolve(&output.id, &output.name, devices) {
        Resolution::Ready(_) | Resolution::Moved(_) => ("connectée", true),
        Resolution::Unavailable => ("déconnectée", false),
        Resolution::Unknown => ("introuvable", false),
    }
}

fn hotkey_editor(ui: &mut egui::Ui, index: usize, hotkey: &mut Option<Hotkey>) {
    ui.horizontal(|ui| {
        ui.label("Raccourci :");
        let mut clear = false;
        match hotkey {
            None => {
                if ui.button("Définir un raccourci").clicked() {
                    // Ctrl+Alt+1, Ctrl+Alt+2… in the order of the list.
                    let key = 0x31 + index.min(8) as u16;
                    *hotkey = Some(Hotkey {
                        ctrl: true,
                        alt: true,
                        shift: false,
                        win: false,
                        key,
                    });
                }
            }
            Some(hk) => {
                ui.checkbox(&mut hk.ctrl, "Ctrl");
                ui.checkbox(&mut hk.alt, "Alt");
                ui.checkbox(&mut hk.shift, "Maj");
                ui.checkbox(&mut hk.win, "Win");
                egui::ComboBox::from_id_salt(("key", index))
                    .selected_text(key_label(hk.key).unwrap_or("?"))
                    .width(130.0)
                    .show_ui(ui, |ui| {
                        for key in KEYS {
                            ui.selectable_value(&mut hk.key, key.vk, key.label);
                        }
                    });
                clear = ui.button("Supprimer").on_hover_text("Supprimer ce raccourci").clicked();
            }
        }
        if clear {
            *hotkey = None;
        }
    });
}

fn hotkey_messages(ui: &mut egui::Ui, hotkey: Option<&Hotkey>, duplicate_of: Option<&str>, refused: Option<&str>) {
    let Some(hotkey) = hotkey else { return };
    let error = ui.visuals().error_fg_color;
    if let Err(e) = hotkey.validate() {
        ui.colored_label(error, e.to_string());
    } else if let Some(other) = duplicate_of {
        ui.colored_label(
            error,
            format!("Même raccourci que « {other} » : seul le premier fonctionne."),
        );
    } else if let Some(message) = refused {
        ui.colored_label(error, message);
    }
    if hotkey.uses_win() {
        ui.colored_label(
            ui.visuals().warn_fg_color,
            "Windows réserve beaucoup de raccourcis avec Win : celui-ci risque de ne pas marcher.",
        );
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
