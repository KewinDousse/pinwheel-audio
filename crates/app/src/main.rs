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
mod app;
#[cfg(windows)]
mod audio;
#[cfg(windows)]
mod hook;
#[cfg(windows)]
mod hotkeys;
#[cfg(windows)]
mod ipc;
#[cfg(windows)]
mod keys;
#[cfg(windows)]
mod logging;
#[cfg(windows)]
mod osd;
#[cfg(windows)]
mod paths;
#[cfg(windows)]
mod settings_ui;
#[cfg(windows)]
mod startup;
#[cfg(windows)]
mod thread;
#[cfg(windows)]
mod tray;

#[cfg(windows)]
fn main() {
    let settings_mode = std::env::args().any(|arg| arg == ipc::SETTINGS_ARG);
    logging::init(if settings_mode { "settings" } else { "main" });
    if settings_mode {
        settings_ui::run();
    } else {
        app::run();
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Pinwheel ne fonctionne que sous Windows.");
    std::process::exit(1);
}
