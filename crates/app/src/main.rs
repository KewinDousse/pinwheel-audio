//! Pinwheel: volume with Win + mouse wheel, and one shortcut per audio output.
//!
//! The same executable runs in two modes:
//! - without arguments, the background process (tray icon, hook, shortcuts,
//!   on-screen display);
//! - with `--settings`, the settings window, a separate process so that the
//!   UI toolkit is only loaded while the window is open.
//!
//! See `docs/architecture.md`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod accent;
#[cfg(windows)]
mod app;
#[cfg(windows)]
mod audio;
#[cfg(windows)]
mod hook;
#[cfg(windows)]
mod hotkeys;
#[cfg(windows)]
mod icon;
#[cfg(windows)]
mod ipc;
#[cfg(windows)]
mod keys;
#[cfg(windows)]
mod language;
#[cfg(windows)]
mod logging;
#[cfg(windows)]
mod osd;
#[cfg(windows)]
mod paths;
#[cfg(windows)]
mod settings_theme;
#[cfg(windows)]
mod settings_ui;
#[cfg(windows)]
mod shell;
#[cfg(windows)]
mod start_menu;
#[cfg(windows)]
mod startup;
#[cfg(windows)]
mod thread;
#[cfg(windows)]
mod tray;
#[cfg(windows)]
mod update;
#[cfg(windows)]
mod window_place;

#[cfg(windows)]
fn main() {
    if std::env::args().any(|arg| arg == ipc::SETTINGS_ARG) {
        logging::init("settings", false);
        settings_ui::run();
    } else {
        // Starts the logger once it holds the instance lock.
        app::run();
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Pinwheel only runs on Windows.");
    std::process::exit(1);
}
