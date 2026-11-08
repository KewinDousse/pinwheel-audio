//! Win + mouse wheel, through a low-level mouse hook (see
//! `docs/adr/0003-win-wheel-hook.md`).
//!
//! The hook runs on a thread of its own: Windows silently removes a
//! low-level hook whose callback takes more than about a second, and the
//! main thread can be slow while it talks to a Bluetooth device. The callback
//! only turns wheel events into notches and posts them to the main window.
//!
//! An application cannot know that its hook was removed, so the hook is
//! reinstalled on a fixed interval, after resume from sleep and after the
//! session is unlocked.

use crate::ipc::WM_VOLUME_NOTCHES;
use crate::thread::{MessageThread, run_message_loop};
use sounddock_core::volume::WheelAccumulator;
use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicIsize, Ordering};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HC_ACTION, HHOOK, MSLLHOOKSTRUCT, PostMessageW, SetTimer, SetWindowsHookExW, UnhookWindowsHookEx,
    WH_MOUSE_LL, WM_APP, WM_MOUSEWHEEL, WM_TIMER,
};

const REINSTALL_INTERVAL_MS: u32 = 5 * 60 * 1000;
const WM_REINSTALL: u32 = WM_APP + 100;

/// Unassigned virtual key. Pressing it while Win is down tells Windows the
/// Win key was used in a combination, so releasing Win does not open Start.
pub const VK_MASK: VIRTUAL_KEY = VIRTUAL_KEY(0xE8);

static MAIN_WINDOW: AtomicIsize = AtomicIsize::new(0);

thread_local! {
    static HOOK: Cell<Option<HHOOK>> = const { Cell::new(None) };
    static WHEEL: RefCell<WheelAccumulator> = RefCell::new(WheelAccumulator::default());
}

/// Handle to the hook thread. Dropping it stops the thread.
pub struct HookThread {
    thread: MessageThread,
}

impl HookThread {
    /// Starts the hook thread, which posts [`WM_VOLUME_NOTCHES`] to `main_window`.
    pub fn start(main_window: HWND) -> std::io::Result<Self> {
        MAIN_WINDOW.store(main_window.0 as isize, Ordering::SeqCst);
        let thread = MessageThread::spawn("wheel-hook", run)?;
        Ok(Self { thread })
    }

    pub fn reinstall(&self) {
        self.thread.post(WM_REINSTALL);
    }
}

fn run() {
    install();
    unsafe { SetTimer(None, 0, REINSTALL_INTERVAL_MS, None) };
    run_message_loop(|msg| {
        if msg.message == WM_TIMER || msg.message == WM_REINSTALL {
            install();
        }
    });
    uninstall();
}

fn install() {
    uninstall();
    match unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0) } {
        Ok(hook) => HOOK.set(Some(hook)),
        Err(e) => log::error!("installing the mouse hook failed: {e}"),
    }
}

fn uninstall() {
    if let Some(hook) = HOOK.take() {
        // Fails when Windows already removed the hook; nothing to do then.
        let _ = unsafe { UnhookWindowsHookEx(hook) };
    }
}

fn is_down(key: VIRTUAL_KEY) -> bool {
    (unsafe { GetAsyncKeyState(i32::from(key.0)) } as u16 & 0x8000) != 0
}

/// Win alone: Ctrl, Alt or Shift with Win + wheel are left to other apps.
fn win_only() -> bool {
    (is_down(VK_LWIN) || is_down(VK_RWIN)) && !is_down(VK_CONTROL) && !is_down(VK_MENU) && !is_down(VK_SHIFT)
}

fn mask_start_menu() {
    let key = |flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_MASK,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let inputs = [key(KEYBD_EVENT_FLAGS(0)), key(KEYEVENTF_KEYUP)];
    unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && wparam.0 as u32 == WM_MOUSEWHEEL && win_only() {
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        let delta = i32::from((info.mouseData >> 16) as u16 as i16);
        let notches = WHEEL.with_borrow_mut(|wheel| wheel.push(delta, info.time));
        mask_start_menu();
        if notches != 0 {
            let main = HWND(MAIN_WINDOW.load(Ordering::SeqCst) as *mut _);
            let _ = unsafe {
                PostMessageW(
                    Some(main),
                    WM_VOLUME_NOTCHES,
                    WPARAM(notches as isize as usize),
                    LPARAM(0),
                )
            };
        }
        // Swallowed, so the window under the cursor does not scroll.
        return LRESULT(1);
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}
