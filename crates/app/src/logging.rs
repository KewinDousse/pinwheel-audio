//! Plain-text log in `%LOCALAPPDATA%\Pinwheel\logs\pinwheel.log`.
//!
//! Both processes append to the same file, each line tagged with its process.
//! The background process rotates it at startup.

use crate::paths;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::Mutex;
use windows::Win32::System::SystemInformation::GetLocalTime;

const MAX_SIZE: u64 = 1024 * 1024;

struct FileLogger {
    process: &'static str,
    file: Mutex<File>,
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let t = unsafe { GetLocalTime() };
        let line = format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03} {:<5} [{}] {}\r\n",
            t.wYear,
            t.wMonth,
            t.wDay,
            t.wHour,
            t.wMinute,
            t.wSecond,
            t.wMilliseconds,
            record.level(),
            self.process,
            record.args()
        );
        if let Ok(mut file) = self.file.lock() {
            // One write per line: appends from both processes do not interleave.
            let _ = file.write_all(line.as_bytes());
        }
    }

    fn flush(&self) {}
}

/// Installs the logger and a panic hook that logs the panic. Logging is
/// best effort: without a writable log folder, the app runs without it.
pub fn init(process: &'static str) {
    let dir = paths::log_dir();
    let path = dir.join("pinwheel.log");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    if process == "main" && std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_SIZE) {
        let _ = std::fs::rename(&path, dir.join("pinwheel.1.log"));
    }
    let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    let logger = Box::leak(Box::new(FileLogger {
        process,
        file: Mutex::new(file),
    }));
    if log::set_logger(logger).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("panic: {info}");
        default_hook(info);
    }));
    log::info!("Pinwheel {} started", env!("CARGO_PKG_VERSION"));
}
