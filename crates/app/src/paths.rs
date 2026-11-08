//! Where SoundDock keeps its files.

use std::path::PathBuf;

const APP_DIR: &str = "SoundDock";

fn known_dir(var: &str) -> PathBuf {
    std::env::var_os(var)
        .map_or_else(std::env::temp_dir, PathBuf::from)
        .join(APP_DIR)
}

/// `%APPDATA%\SoundDock\settings.json`: roams with the Windows profile.
pub fn settings_file() -> PathBuf {
    known_dir("APPDATA").join("settings.json")
}

/// `%LOCALAPPDATA%\SoundDock\status.json`.
pub fn status_file() -> PathBuf {
    known_dir("LOCALAPPDATA").join("status.json")
}

/// `%LOCALAPPDATA%\SoundDock\logs`.
pub fn log_dir() -> PathBuf {
    known_dir("LOCALAPPDATA").join("logs")
}
