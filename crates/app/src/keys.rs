//! Shortcut capture and key names, for the settings window.
//!
//! The capture installs a low-level keyboard hook on a thread of its own and
//! swallows every key until a combination is complete. This is what lets any
//! key be chosen, Win combinations included: the shell never sees them, so
//! Win+E does not open the Explorer while it is being captured, and the
//! background process's shortcuts do not fire.
//!
//! The hook is global, so the capture ends by itself, letting the key
//! through, as soon as another application takes the focus, and after
//! [`TIMEOUT_MS`].

use crate::hook::VK_MASK;
use crate::thread::{MessageThread, run_message_loop};
use pinwheel_core::hotkey::{HeldModifiers, Hotkey, has_fixed_name, is_cancel, is_usable_key, key_name};
use pinwheel_core::i18n::Lang;
use std::cell::RefCell;
use std::sync::mpsc;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyNameTextW, MAPVK_VK_TO_VSC_EX, MapVirtualKeyW};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, GetWindowThreadProcessId, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT,
    PostThreadMessageW, SetTimer, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_APP, WM_KEYDOWN,
    WM_SYSKEYDOWN, WM_TIMER,
};

const TIMEOUT_MS: u32 = 15_000;

/// Name of a shortcut with the main key as the current keyboard layout
/// writes it ("Ctrl+Alt+É" on a French layout).
pub fn describe(hotkey: &Hotkey, lang: Lang) -> String {
    let fixed = || key_name(hotkey.key, lang).into_owned();
    let key = if has_fixed_name(hotkey.key) {
        fixed()
    } else {
        layout_name(hotkey.key).unwrap_or_else(fixed)
    };
    let mut text = String::new();
    let _ = hotkey.format_with(&mut text, &key, lang);
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
    /// Escape alone, another application took the focus, or the time ran out.
    Cancelled,
    /// The keyboard hook could not be installed (see the log).
    Failed,
}

/// A capture in progress. Dropping it stops the capture.
pub struct Capture {
    receiver: mpsc::Receiver<Captured>,
    _thread: MessageThread,
}

impl Capture {
    /// Starts capturing. `on_result` runs on the capture thread once the
    /// result is available, to wake the UI up.
    pub fn start(on_result: impl Fn() + Send + 'static) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let thread = MessageThread::spawn("shortcut-capture", move || run(sender, Box::new(on_result)))?;
        Ok(Self {
            receiver,
            _thread: thread,
        })
    }

    pub fn poll(&self) -> Option<Captured> {
        self.receiver.try_recv().ok()
    }
}

/// Posted by the hook to its own thread once the result is sent.
const WM_FINISHED: u32 = WM_APP + 1;

struct State {
    sender: mpsc::Sender<Captured>,
    on_result: Box<dyn Fn()>,
    held: HeldModifiers,
    finished: bool,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn run(sender: mpsc::Sender<Captured>, on_result: Box<dyn Fn()>) {
    STATE.set(Some(State {
        sender,
        on_result,
        held: HeldModifiers::default(),
        finished: false,
    }));
    let mut hook: Option<HHOOK> = match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0) } {
        Ok(hook) => {
            log::info!("PROBE capture hook installed");
            Some(hook)
        }
        Err(e) => {
            log::error!("installing the capture hook failed: {e}");
            finish(Captured::Failed);
            None
        }
    };
    unsafe { SetTimer(None, 0, TIMEOUT_MS, None) };
    // The thread waits for WM_QUIT (see `thread.rs`), even once finished.
    run_message_loop(|msg| {
        if msg.message == WM_TIMER {
            finish(Captured::Cancelled);
        }
        if (msg.message == WM_FINISHED || msg.message == WM_TIMER)
            && let Some(hook) = hook.take()
        {
            let _ = unsafe { UnhookWindowsHookEx(hook) };
        }
    });
    if let Some(hook) = hook {
        let _ = unsafe { UnhookWindowsHookEx(hook) };
    }
}

/// Sends the first result only.
fn finish(result: Captured) {
    STATE.with_borrow_mut(|state| {
        let Some(state) = state.as_mut().filter(|s| !s.finished) else {
            return;
        };
        state.finished = true;
        log::info!("PROBE capture finished: {result:?}");
        let _ = state.sender.send(result);
        (state.on_result)();
        let _ = unsafe { PostThreadMessageW(GetCurrentThreadId(), WM_FINISHED, WPARAM(0), LPARAM(0)) };
    });
}

fn settings_window_has_focus() -> bool {
    let window = unsafe { GetForegroundWindow() };
    if window.is_invalid() {
        // No foreground window for a moment, while the focus moves.
        return true;
    }
    let mut process = 0;
    unsafe { GetWindowThreadProcessId(window, Some(&mut process)) };
    process == unsafe { GetCurrentProcessId() }
}

/// What the hook does with a key event.
enum Verdict {
    PassThrough,
    Swallow,
}

fn on_key(vk: u16, down: bool) -> Verdict {
    let finished = STATE.with_borrow(|state| state.as_ref().is_none_or(|s| s.finished));
    log::info!("PROBE key {vk:#x} down={down} finished={finished} focus={}", settings_window_has_focus());
    // The wheel hook's Start menu mask is not a key the user pressed.
    if finished || vk == VK_MASK.0 {
        return Verdict::PassThrough;
    }
    if !settings_window_has_focus() {
        finish(Captured::Cancelled);
        return Verdict::PassThrough;
    }
    let held = STATE.with_borrow_mut(|state| {
        let state = state.as_mut().expect("state checked above");
        if state.held.update(vk, down) {
            None
        } else {
            Some(state.held)
        }
    });
    if let Some(held) = held
        && down
        && is_usable_key(vk)
    {
        let hotkey = held.with_key(vk);
        finish(if is_cancel(&hotkey) {
            Captured::Cancelled
        } else {
            Captured::Hotkey(hotkey)
        });
    }
    // Key-ups, modifiers and unusable keys are swallowed too: neither the
    // shell nor the focused window sees anything during the capture.
    Verdict::Swallow
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        // Injected keys are accepted: macro software (G Hub, Synapse,
        // AutoHotkey) sends its keys that way.
        let down = matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
        if let Verdict::Swallow = on_key(info.vkCode as u16, down) {
            return LRESULT(1);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}
