//! Win + mouse wheel and Win + middle click, through low-level hooks (see
//! `docs/adr/0003-win-wheel-hook.md`).
//!
//! The hooks run on a thread of their own: Windows silently removes a
//! low-level hook whose callback takes more than about a second, and the
//! main thread can be slow while it talks to a Bluetooth device. The mouse
//! callback only turns wheel events into notches, and middle clicks into a
//! mute, for the main window; the keyboard callback only keeps Start closed
//! when Win is released.
//!
//! An application cannot know that its hook was removed, so the hooks are
//! reinstalled on a fixed interval, after resume from sleep and after the
//! session is unlocked.

use crate::ipc::{WM_TOGGLE_MUTE, WM_VOLUME_NOTCHES};
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
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL, WINDOWS_HOOK_ID, WM_APP, WM_KEYDOWN, WM_KEYUP, WM_MBUTTONDOWN,
    WM_MBUTTONUP, WM_MOUSEWHEEL, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER,
};

const REINSTALL_INTERVAL_MS: u32 = 5 * 60 * 1000;
const WM_REINSTALL: u32 = WM_APP + 100;

/// Unassigned virtual key. Pressing it while Win is down tells Windows the
/// Win key was used in a combination, so releasing Win does not open Start.
pub const VK_MASK: VIRTUAL_KEY = VIRTUAL_KEY(0xE8);

/// `dwExtraInfo` of the keys this module sends, so its hook lets them through.
const INJECTED_TAG: usize = 0x5049_4E57;

static MAIN_WINDOW: AtomicIsize = AtomicIsize::new(0);
/// Notches not yet applied, of Win + wheel and of Win + Shift + wheel. The
/// main window reads them all at once, so a fast spin never queues up more
/// displays than it can draw.
static PENDING_NOTCHES: AtomicI32 = AtomicI32::new(0);
static PENDING_FINE_NOTCHES: AtomicI32 = AtomicI32::new(0);
/// A `WM_VOLUME_NOTCHES` is on its way and has not been read yet.
static NOTIFIED: AtomicBool = AtomicBool::new(false);
/// Set by the main thread from the settings. Off, the combination is left
/// to other apps.
static FINE_STEP: AtomicBool = AtomicBool::new(false);
static MIDDLE_CLICK_MUTE: AtomicBool = AtomicBool::new(false);

thread_local! {
    static MOUSE_HOOK: Cell<Option<HHOOK>> = const { Cell::new(None) };
    static KEYBOARD_HOOK: Cell<Option<HHOOK>> = const { Cell::new(None) };
    static WHEEL: RefCell<WheelAccumulator> = RefCell::new(WheelAccumulator::default());
    /// The wheel was turned, or the middle button pressed, since Win went down.
    static WIN_USED: Cell<bool> = const { Cell::new(false) };
    /// The middle button press was swallowed, so its release must be too.
    static MIDDLE_SWALLOWED: Cell<bool> = const { Cell::new(false) };
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

/// Which combinations the hooks take.
pub fn configure(fine_step: bool, middle_click_mute: bool) {
    FINE_STEP.store(fine_step, Ordering::SeqCst);
    MIDDLE_CLICK_MUTE.store(middle_click_mute, Ordering::SeqCst);
}

/// Notches received since the last call, signed.
#[derive(Debug, Clone, Copy)]
pub struct Notches {
    /// Win + wheel.
    pub normal: i32,
    /// Win + Shift + wheel.
    pub fine: i32,
}

/// Called by the main window on [`WM_VOLUME_NOTCHES`].
pub fn take_notches() -> Notches {
    // Cleared first: a notch arriving from here on posts a new message.
    NOTIFIED.store(false, Ordering::SeqCst);
    Notches {
        normal: PENDING_NOTCHES.swap(0, Ordering::SeqCst),
        fine: PENDING_FINE_NOTCHES.swap(0, Ordering::SeqCst),
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

#[derive(PartialEq, Eq)]
enum Held {
    Win,
    WinShift,
}

/// Ctrl or Alt with Win are left to other apps.
fn held() -> Option<Held> {
    if !(is_down(VK_LWIN) || is_down(VK_RWIN)) || is_down(VK_CONTROL) || is_down(VK_MENU) {
        return None;
    }
    Some(if is_down(VK_SHIFT) { Held::WinShift } else { Held::Win })
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

fn post_main(msg: u32) {
    let main = HWND(MAIN_WINDOW.load(Ordering::SeqCst) as *mut _);
    let _ = unsafe { PostMessageW(Some(main), msg, WPARAM(0), LPARAM(0)) };
}

/// Returns `true` when the event is swallowed, so that the window under
/// the cursor neither scrolls nor gets the click.
fn on_mouse(msg: u32, info: &MSLLHOOKSTRUCT) -> bool {
    match msg {
        WM_MOUSEWHEEL => {
            let pending = match held() {
                Some(Held::Win) => &PENDING_NOTCHES,
                Some(Held::WinShift) if FINE_STEP.load(Ordering::SeqCst) => &PENDING_FINE_NOTCHES,
                _ => return false,
            };
            let delta = i32::from((info.mouseData >> 16) as u16 as i16);
            let notches = WHEEL.with_borrow_mut(|wheel| wheel.push(delta, info.time));
            WIN_USED.set(true);
            if notches != 0 {
                pending.fetch_add(notches, Ordering::SeqCst);
                if !NOTIFIED.swap(true, Ordering::SeqCst) {
                    post_main(WM_VOLUME_NOTCHES);
                }
            }
            true
        }
        WM_MBUTTONDOWN if MIDDLE_CLICK_MUTE.load(Ordering::SeqCst) && held() == Some(Held::Win) => {
            WIN_USED.set(true);
            MIDDLE_SWALLOWED.set(true);
            post_main(WM_TOGGLE_MUTE);
            true
        }
        // A release the hook missed must not swallow the next one.
        WM_MBUTTONDOWN => {
            MIDDLE_SWALLOWED.set(false);
            false
        }
        // Even when Win is already up.
        WM_MBUTTONUP => MIDDLE_SWALLOWED.take(),
        _ => false,
    }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        if on_mouse(wparam.0 as u32, info) {
            return LRESULT(1);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}
