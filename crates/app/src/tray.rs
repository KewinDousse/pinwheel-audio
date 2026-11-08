//! Notification area icon and its menu.
//!
//! Explorer forgets every icon when it restarts, or may not be ready yet
//! when Pinwheel starts at sign-in. The icon is added again on the
//! `TaskbarCreated` broadcast, and retried on a timer until the first add
//! succeeds.

use crate::ipc::{WM_TRAY, wide};
use pinwheel_core::i18n::Lang;
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, GetSystemMetrics, HICON, IMAGE_ICON, LR_DEFAULTCOLOR,
    LoadImageW, MF_SEPARATOR, MF_STRING, PostMessageW, RegisterWindowMessageW, SM_CXSMICON, SM_CYSMICON,
    SetForegroundWindow, SetMenuDefaultItem, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WM_NULL,
};
use windows::core::{PCWSTR, w};

const ICON_ID: u32 = 1;
/// Resource ID of the icon in `res/pinwheel.rc`.
const ICON_RESOURCE: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuCommand {
    Settings = 1,
    Quit = 2,
}

pub struct Tray {
    hwnd: HWND,
    icon: HICON,
    shown: bool,
}

impl Tray {
    pub fn new(hwnd: HWND) -> Self {
        let icon = unsafe {
            let module = GetModuleHandleW(None).unwrap_or_default();
            LoadImageW(
                Some(module.into()),
                PCWSTR(ICON_RESOURCE as usize as *const u16),
                IMAGE_ICON,
                GetSystemMetrics(SM_CXSMICON),
                GetSystemMetrics(SM_CYSMICON),
                LR_DEFAULTCOLOR,
            )
        }
        .map(|handle| HICON(handle.0))
        .unwrap_or_default();
        Self {
            hwnd,
            icon,
            shown: false,
        }
    }

    /// Message Explorer broadcasts when the taskbar is (re)created.
    pub fn taskbar_created_message() -> u32 {
        unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) }
    }

    /// Adds the icon. Returns `false` when Explorer is not ready.
    pub fn show(&mut self) -> bool {
        // A second NIM_ADD fails, which would mark a shown icon as missing.
        if self.shown {
            return true;
        }
        let mut data = self.data();
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        data.uCallbackMessage = WM_TRAY;
        data.hIcon = self.icon;
        let tip: Vec<u16> = "Pinwheel".encode_utf16().collect();
        data.szTip[..tip.len()].copy_from_slice(&tip);
        self.shown = unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.as_bool();
        self.shown
    }

    pub fn hide(&mut self) {
        if self.shown {
            let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &self.data()) };
            self.shown = false;
        }
    }

    fn data(&self) -> NOTIFYICONDATAW {
        NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: ICON_ID,
            ..Default::default()
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        self.hide();
    }
}

/// Shows the menu at the cursor and returns the chosen command. Does not
/// touch the app state: the menu runs a modal loop that dispatches other
/// messages meanwhile.
pub fn show_menu(hwnd: HWND, lang: Lang) -> Option<MenuCommand> {
    let texts = lang.texts();
    let (settings, quit) = (wide(texts.tray_settings), wide(texts.tray_quit));
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            MenuCommand::Settings as usize,
            PCWSTR(settings.as_ptr()),
        );
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, MenuCommand::Quit as usize, PCWSTR(quit.as_ptr()));
        let _ = SetMenuDefaultItem(menu, MenuCommand::Settings as u32, 0);

        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        // Without this, the menu does not close when clicking elsewhere.
        let _ = SetForegroundWindow(hwnd);
        let chosen = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY,
            cursor.x,
            cursor.y,
            None,
            hwnd,
            None,
        );
        let _ = PostMessageW(Some(hwnd), WM_NULL, Default::default(), Default::default());
        let _ = DestroyMenu(menu);

        match chosen.0 {
            1 => Some(MenuCommand::Settings),
            2 => Some(MenuCommand::Quit),
            _ => None,
        }
    }
}
