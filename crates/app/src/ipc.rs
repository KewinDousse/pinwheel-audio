//! Messages between the settings window and the background process.
//!
//! The background process owns a hidden top-level window of class
//! [`MAIN_CLASS`]; other processes find it and post it the messages below.

use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, LPARAM, WPARAM};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW, WM_APP};
use windows::core::{PCWSTR, w};

/// Command-line flag that starts the settings window.
pub const SETTINGS_ARG: &str = "--settings";

pub const MAIN_CLASS: PCWSTR = w!("Pinwheel.Main");
pub const MAIN_MUTEX: PCWSTR = w!("Local\\Pinwheel.Main");
pub const SETTINGS_MUTEX: PCWSTR = w!("Local\\Pinwheel.Settings");
pub const SETTINGS_TITLE: &str = "Paramètres de Pinwheel";

/// Open the settings window (second launch of the executable).
pub const WM_OPEN_SETTINGS: u32 = WM_APP + 1;
/// The settings file changed: reload it and register the shortcuts again.
pub const WM_RELOAD: u32 = WM_APP + 2;
/// Show the on-screen display with the current style, as a preview.
pub const WM_PREVIEW_OSD: u32 = WM_APP + 3;
/// Wheel notches from the hook thread; `wParam` holds the signed count.
pub const WM_VOLUME_NOTCHES: u32 = WM_APP + 4;
/// Tray icon callback.
pub const WM_TRAY: u32 = WM_APP + 5;

/// A named mutex held for the life of the process.
pub struct InstanceLock(HANDLE);

impl InstanceLock {
    /// `None` when another process already holds this lock.
    pub fn acquire(name: PCWSTR) -> Option<Self> {
        let handle = unsafe { CreateMutexW(None, false, name) }.ok()?;
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            let _ = unsafe { CloseHandle(handle) };
            return None;
        }
        Some(Self(handle))
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// Posts `msg` to the background process. `false` when it is not running.
pub fn notify_main(msg: u32) -> bool {
    let Ok(hwnd) = (unsafe { FindWindowW(MAIN_CLASS, PCWSTR::null()) }) else {
        return false;
    };
    unsafe { PostMessageW(Some(hwnd), msg, WPARAM(0), LPARAM(0)) }.is_ok()
}

/// Encodes a string as a null-terminated UTF-16 buffer.
pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
