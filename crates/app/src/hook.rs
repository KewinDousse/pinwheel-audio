//! Win + mouse wheel, through low-level hooks (see
//! `docs/adr/0003-win-wheel-hook.md`).
//!
//! The hooks run on a thread of their own: Windows silently removes a
//! low-level hook whose callback takes more than about a second, and the
//! main thread can be slow while it talks to a Bluetooth device. The mouse
//! callback only turns wheel events into notches for the main window; the
//! keyboard callback only keeps Start closed when Win is released.
//!
//! An application cannot know that its hook was removed, so the hooks are
//! reinstalled on a fixed interval, after resume from sleep and after the
//! session is unlocked.

use crate::ipc::WM_VOLUME_NOTCHES;
use crate::thread::{MessageThread, run_message_loop};
use pinwheel_core::volume::WheelAccumulator;
use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, Ordering};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSLLHOOKSTRUCT, PostMessageW, SetTimer, SetWindowsHookExW,
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL, WINDOWS_HOOK_ID, WM_APP, WM_KEYDOWN, WM_KEYUP, WM_MOUSEWHEEL,
    WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER,
};

const REINSTALL_INTERVAL_MS: u32 = 5 * 60 * 1000;
const WM_REINSTALL: u32 = WM_APP + 100;

/// Unassigned virtual key. Pressing it while Win is down tells Windows the
/// Win key was used in a combination, so releasing Win does not open Start.
pub const VK_MASK: VIRTUAL_KEY = VIRTUAL_KEY(0xE8);

/// `dwExtraInfo` of the keys this module sends, so its hook lets them through.
const INJECTED_TAG: usize = 0x5049_4E57;

static MAIN_WINDOW: AtomicIsize = AtomicIsize::new(0);
/// Notches not yet applied. The main window reads them all at once, so a
/// fast spin never queues up more displays than it can draw.
static PENDING_NOTCHES: AtomicI32 = AtomicI32::new(0);
/// A `WM_VOLUME_NOTCHES` is on its way and has not been read yet.
static NOTIFIED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static MOUSE_HOOK: Cell<Option<HHOOK>> = const { Cell::new(None) };
    static KEYBOARD_HOOK: Cell<Option<HHOOK>> = const { Cell::new(None) };
    static WHEEL: RefCell<WheelAccumulator> = RefCell::new(WheelAccumulator::default());
    /// The wheel was turned since Win went down.
    static WIN_USED: Cell<bool> = const { Cell::new(false) };
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

/// The notches received since the last call, signed. Called by the main
/// window on [`WM_VOLUME_NOTCHES`].
pub fn take_notches() -> i32 {
    // Cleared first: a notch arriving from here on posts a new message.
    NOTIFIED.store(false, Ordering::SeqCst);
    PENDING_NOTCHES.swap(0, Ordering::SeqCst)
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

type HookProc = unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT;

fn install() {
    uninstall();
    let hooks: [(_, WINDOWS_HOOK_ID, HookProc); 2] = [
        (&MOUSE_HOOK, WH_MOUSE_LL, mouse_proc),
        (&KEYBOARD_HOOK, WH_KEYBOARD_LL, keyboard_proc),
    ];
    for (slot, id, proc) in hooks {
        match unsafe { SetWindowsHookExW(id, Some(proc), None, 0) } {
            Ok(hook) => slot.set(Some(hook)),
            Err(e) => log::error!("installing the hook {} failed: {e}", id.0),
        }
    }
}

fn uninstall() {
    for slot in [&MOUSE_HOOK, &KEYBOARD_HOOK] {
        if let Some(hook) = slot.take() {
            // Fails when Windows already removed the hook; nothing to do then.
            let _ = unsafe { UnhookWindowsHookEx(hook) };
        }
    }
}

fn is_down(key: VIRTUAL_KEY) -> bool {
    (unsafe { GetAsyncKeyState(i32::from(key.0)) } as u16 & 0x8000) != 0
}

/// Win alone: Ctrl, Alt or Shift with Win + wheel are left to other apps.
fn win_only() -> bool {
    (is_down(VK_LWIN) || is_down(VK_RWIN)) && !is_down(VK_CONTROL) && !is_down(VK_MENU) && !is_down(VK_SHIFT)
}

fn key_input(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: INJECTED_TAG,
            },
        },
    }
}

/// Sends the mask key then the release of `win`, in place of the release
/// the hook swallows. Sent at release rather than while turning the wheel:
/// a held key repeats, and Windows only looks at what came after the last
/// Win press. Returns `false` when Windows did not take all three keys.
fn release_win_masked(win: VIRTUAL_KEY, scan: u16) -> bool {
    let inputs = [
        key_input(VK_MASK, 0, KEYBD_EVENT_FLAGS(0)),
        key_input(VK_MASK, 0, KEYEVENTF_KEYUP),
        key_input(win, scan, KEYEVENTF_KEYUP | KEYEVENTF_EXTENDEDKEY),
    ];
    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    sent as usize == inputs.len()
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let vk = VIRTUAL_KEY(info.vkCode as u16);
        log::info!("PROBE main hook: vk {:#x} msg {:#x}", vk.0, wparam.0);
        if (vk == VK_LWIN || vk == VK_RWIN) && info.dwExtraInfo != INJECTED_TAG {
            match wparam.0 as u32 {
                // A first press, not a repeat: Win was up before this event.
                WM_KEYDOWN | WM_SYSKEYDOWN if !is_down(vk) => WIN_USED.set(false),
                // Swallowed only once its replacement is sent, so Win cannot stay down.
                WM_KEYUP | WM_SYSKEYUP if WIN_USED.take() && release_win_masked(vk, info.scanCode as u16) => {
                    return LRESULT(1);
                }
                _ => {}
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && wparam.0 as u32 == WM_MOUSEWHEEL && win_only() {
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        let delta = i32::from((info.mouseData >> 16) as u16 as i16);
        let notches = WHEEL.with_borrow_mut(|wheel| wheel.push(delta, info.time));
        WIN_USED.set(true);
        if notches != 0 {
            PENDING_NOTCHES.fetch_add(notches, Ordering::SeqCst);
            if !NOTIFIED.swap(true, Ordering::SeqCst) {
                let main = HWND(MAIN_WINDOW.load(Ordering::SeqCst) as *mut _);
                let _ = unsafe { PostMessageW(Some(main), WM_VOLUME_NOTCHES, WPARAM(0), LPARAM(0)) };
            }
        }
        // Swallowed, so the window under the cursor does not scroll.
        return LRESULT(1);
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}
