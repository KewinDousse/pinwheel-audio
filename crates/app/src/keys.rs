//! Shortcut capture and key names, for the settings window.
//!
//! The capture installs a low-level keyboard hook on a thread of its own and
//! swallows every key until a combination is complete. This is what lets any
//! key be chosen, Win combinations included: the shell never sees them, so
//! Win+E does not open the Explorer while it is being captured, and the
//! background process's shortcuts do not fire.

use sounddock_core::hotkey::{Hotkey, MODIFIER_KEYS, has_fixed_name, is_cancel, key_name};
use std::cell::Cell;
use std::sync::mpsc;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyNameTextW, MAPVK_VK_TO_VSC_EX, MapVirtualKeyW};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, PM_NOREMOVE, PeekMessageW,
    PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_APP, WM_KEYDOWN, WM_QUIT,
    WM_SYSKEYDOWN,
};

/// Name of a shortcut with the main key as the current keyboard layout
/// writes it ("Ctrl+Alt+É" on a French layout).
pub fn describe(hotkey: &Hotkey) -> String {
    let key = if has_fixed_name(hotkey.key) {
        key_name(hotkey.key).into_owned()
    } else {
        layout_name(hotkey.key).unwrap_or_else(|| key_name(hotkey.key).into_owned())
    };
    let mut text = String::new();
    let _ = hotkey.format_with(&mut text, &key);
    text
}

fn layout_name(vk: u16) -> Option<String> {
    let scan = unsafe { MapVirtualKeyW(u32::from(vk), MAPVK_VK_TO_VSC_EX) };
    if scan == 0 {
        return None;
    }
    let mut buffer = [0u16; 64];
    let length = unsafe { GetKeyNameTextW(((scan & 0xFF) << 16) as i32, &mut buffer) };
    (length > 0).then(|| String::from_utf16_lossy(&buffer[..length as usize]))
}

/// Result of a capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Captured {
    Hotkey(Hotkey),
    /// Escape alone.
    Cancelled,
}

/// A capture in progress. Dropping it stops the capture.
pub struct Capture {
    receiver: mpsc::Receiver<Captured>,
    thread_id: u32,
}

impl Capture {
    /// Starts capturing; `on_result` runs on the capture thread when a
    /// combination is complete (to wake the UI up).
    pub fn start(on_result: impl Fn() + Send + 'static) -> Option<Self> {
        let (result_sender, receiver) = mpsc::channel();
        let (id_sender, id_receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("shortcut-capture".into())
            .spawn(move || run(&id_sender, &result_sender, &on_result))
            .ok()?;
        let thread_id = id_receiver.recv().ok()??;
        Some(Self { receiver, thread_id })
    }

    pub fn poll(&self) -> Option<Captured> {
        self.receiver.try_recv().ok()
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        let _ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
    }
}

/// Posted by the hook to its own thread when a combination is complete.
const WM_CAPTURED: u32 = WM_APP + 1;

thread_local! {
    static HELD: Cell<Modifiers> = const { Cell::new(Modifiers::NONE) };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Modifiers {
    ctrl: bool,
    alt: bool,
    shift: bool,
    win: bool,
}

impl Modifiers {
    const NONE: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
        win: false,
    };

    fn with(mut self, vk: u16, down: bool) -> Self {
        match vk {
            0x11 | 0xA2 | 0xA3 => self.ctrl = down,
            0x12 | 0xA4 | 0xA5 => self.alt = down,
            0x10 | 0xA0 | 0xA1 => self.shift = down,
            0x5B | 0x5C => self.win = down,
            _ => {}
        }
        self
    }
}

fn run(id: &mpsc::Sender<Option<u32>>, result: &mpsc::Sender<Captured>, on_result: &dyn Fn()) {
    let mut msg = MSG::default();
    // Creates the thread's message queue before anyone posts to it.
    let _ = unsafe { PeekMessageW(&mut msg, None, WM_APP, WM_APP, PM_NOREMOVE) };
    let hook = match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0) } {
        Ok(hook) => hook,
        Err(e) => {
            log::error!("installing the capture hook failed: {e}");
            let _ = id.send(None);
            return;
        }
    };
    let _ = id.send(Some(unsafe { GetCurrentThreadId() }));

    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {
        if msg.message == WM_CAPTURED {
            let hotkey = unpack(msg.wParam.0);
            let _ = result.send(if is_cancel(&hotkey) {
                Captured::Cancelled
            } else {
                Captured::Hotkey(hotkey)
            });
            on_result();
            break;
        }
    }
    let _ = unsafe { UnhookWindowsHookEx(hook) };
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code != HC_ACTION as i32 {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    // Keys sent by programs (such as the Start menu mask of the wheel hook)
    // are not the user's choice.
    if info.flags.0 & LLKHF_INJECTED.0 != 0 {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    let vk = info.vkCode as u16;
    let down = matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
    let held = HELD.get();
    if MODIFIER_KEYS.contains(&vk) {
        HELD.set(held.with(vk, down));
    } else if down {
        let hotkey = Hotkey {
            ctrl: held.ctrl,
            alt: held.alt,
            shift: held.shift,
            win: held.win,
            key: vk,
        };
        let _ = unsafe { PostThreadMessageW(GetCurrentThreadId(), WM_CAPTURED, WPARAM(pack(&hotkey)), LPARAM(0)) };
    }
    // Swallowed: neither the shell nor the focused window sees the keys.
    LRESULT(1)
}

fn pack(hotkey: &Hotkey) -> usize {
    let flags = usize::from(hotkey.ctrl)
        | usize::from(hotkey.alt) << 1
        | usize::from(hotkey.shift) << 2
        | usize::from(hotkey.win) << 3;
    usize::from(hotkey.key) | flags << 16
}

fn unpack(packed: usize) -> Hotkey {
    let flags = packed >> 16;
    Hotkey {
        ctrl: flags & 1 != 0,
        alt: flags & 2 != 0,
        shift: flags & 4 != 0,
        win: flags & 8 != 0,
        key: (packed & 0xFFFF) as u16,
    }
}
