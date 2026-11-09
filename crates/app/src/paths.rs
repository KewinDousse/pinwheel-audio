//! Where Pinwheel keeps its files.

use std::path::PathBuf;

const APP_DIR: &str = "Pinwheel";

fn known_dir(var: &str) -> PathBuf {
    std::env::var_os(var)
        .map_or_else(std::env::temp_dir, PathBuf::from)
        .join(APP_DIR)
}

/// `%APPDATA%\Pinwheel\settings.json`: roams with the Windows profile.
pub fn settings_file() -> PathBuf {
    known_dir("APPDATA").join("settings.json")
}

/// `%LOCALAPPDATA%\Pinwheel\status.json`.
pub fn status_file() -> PathBuf {
    known_dir("LOCALAPPDATA").join("status.json")
}

/// `%LOCALAPPDATA%\Pinwheel\window.txt`: where the settings window was
/// last, which only makes sense on this machine's screens.
pub fn window_file() -> PathBuf {
    known_dir("LOCALAPPDATA").join("window.txt")
}

/// `%LOCALAPPDATA%\Pinwheel\logs`.
pub fn log_dir() -> PathBuf {
    known_dir("LOCALAPPDATA").join("logs")
}
