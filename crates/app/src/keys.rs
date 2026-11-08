//! Shortcut capture and key names, for the settings window.
//!
//! The capture reads the keyboard through Raw Input, on a hidden window of a
//! thread of its own. Low-level keyboard hooks, of this process or of any
//! other, are not called for the keys Windows sends to the settings window,
//! while Raw Input sees every key, Win included. It cannot stop a key, so the
//! background process suspends its shortcuts while a capture lasts.
//!
//! Raw Input also sees keys typed in other applications, so the capture ends
//! by itself as soon as another application takes the focus, and after
//! [`TIMEOUT_MS`].

use crate::hook::VK_MASK;
use crate::ipc::{self, WM_CAPTURE_BEGIN, WM_CAPTURE_END};
use crate::thread::{MessageThread, run_message_loop};
use pinwheel_core::hotkey::{HeldModifiers, Hotkey, has_fixed_name, is_cancel, is_usable_key, key_name};
use pinwheel_core::i18n::Lang;
use std::cell::RefCell;
use std::sync::mpsc;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyNameTextW, MAPVK_VK_TO_VSC_EX, MapVirtualKeyW, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_MENU,
    VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_SHIFT,
};
use windows::Win32::UI::Input::{
    GetRawInputData, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER, RID_INPUT, RIDEV_INPUTSINK, RIDEV_REMOVE,
    RIM_TYPEKEYBOARD, RegisterRawInputDevices,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetForegroundWindow, GetWindowThreadProcessId, HWND_MESSAGE,
    RI_KEY_BREAK, RI_KEY_E0, RegisterClassExW, SetTimer, WINDOW_EX_STYLE, WINDOW_STYLE, WM_INPUT, WM_TIMER,
    WNDCLASSEXW,
};
use windows::core::w;

pub const TIMEOUT_MS: u32 = 15_000;
/// Scan code of the right Shift, which Raw Input reports as plain `VK_SHIFT`.
const SCAN_RIGHT_SHIFT: u16 = 0x36;

/// The keys of a shortcut, with the main key as the current keyboard layout
/// writes it (`["Ctrl", "Alt", "É"]` on a French layout).
pub fn describe(hotkey: &Hotkey, lang: Lang) -> Vec<String> {
    let fixed = || key_name(hotkey.key, lang).into_owned();
    let key = if has_fixed_name(hotkey.key) {
        fixed()
    } else {
        layout_name(hotkey.key).unwrap_or_else(fixed)
    };
    hotkey.modifier_names(lang).map(String::from).chain([key]).collect()
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
    /// The keyboard could not be read (see the log).
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
        log::info!("shortcut capture started");
        ipc::notify_main(WM_CAPTURE_BEGIN);
        Ok(Self {
            receiver,
            _thread: thread,
        })
    }

    pub fn poll(&self) -> Option<Captured> {
        self.receiver.try_recv().ok()
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        ipc::notify_main(WM_CAPTURE_END);
    }
}

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
    let window = match listen() {
        Ok(window) => Some(window),
        Err(e) => {
            log::error!("reading the keyboard for the capture failed: {e}");
            finish(Captured::Failed);
            None
        }
    };
    unsafe { SetTimer(None, 0, TIMEOUT_MS, None) };
    // The thread waits for WM_QUIT (see `thread.rs`), even once finished.
    run_message_loop(|msg| {
        if msg.message == WM_TIMER && msg.hwnd.is_invalid() {
            finish(Captured::Cancelled);
        }
    });
    if let Some(window) = window {
        stop_listening(window);
    }
}

/// A message-only window that receives every key through Raw Input.
fn listen() -> windows::core::Result<HWND> {
    let instance = unsafe { GetModuleHandleW(None) }?.into();
    let class = w!("Pinwheel.Capture");
    let wc = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        lpszClassName: class,
        ..Default::default()
    };
    // Fails harmlessly when an earlier capture registered it.
    unsafe { RegisterClassExW(&wc) };
    let window = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            None,
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance),
            None,
        )
    }?;
    register_keyboard(RIDEV_INPUTSINK, Some(window)).inspect_err(|_| {
        let _ = unsafe { DestroyWindow(window) };
    })?;
    Ok(window)
}

fn stop_listening(window: HWND) {
    let _ = register_keyboard(RIDEV_REMOVE, None);
    let _ = unsafe { DestroyWindow(window) };
}

fn register_keyboard(
    flags: windows::Win32::UI::Input::RAWINPUTDEVICE_FLAGS,
    target: Option<HWND>,
) -> windows::core::Result<()> {
    // Generic desktop page, keyboard usage.
    let device = RAWINPUTDEVICE {
        usUsagePage: 0x01,
        usUsage: 0x06,
        dwFlags: flags,
        hwndTarget: target.unwrap_or_default(),
    };
    unsafe { RegisterRawInputDevices(&[device], size_of::<RAWINPUTDEVICE>() as u32) }
}

/// Sends the first result only.
fn finish(result: Captured) {
    STATE.with_borrow_mut(|state| {
        let Some(state) = state.as_mut().filter(|s| !s.finished) else {
            return;
        };
        state.finished = true;
        log::info!("shortcut capture ended: {result:?}");
        let _ = state.sender.send(result);
        (state.on_result)();
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

fn on_key(vk: u16, down: bool) {
    let finished = STATE.with_borrow(|state| state.as_ref().is_none_or(|s| s.finished));
    // The background process's Start menu mask is not a key the user pressed.
    if finished || vk == VK_MASK.0 {
        return;
    }
    if !settings_window_has_focus() {
        finish(Captured::Cancelled);
        return;
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
}

/// The key of a Raw Input event, with the side of Ctrl, Alt and Shift, which
/// Raw Input reports as their generic key.
fn sided_key(vk: u16, scan: u16, flags: u16) -> u16 {
    let right = flags & RI_KEY_E0 as u16 != 0;
    let pick = |left: u16, right_key: u16, is_right: bool| if is_right { right_key } else { left };
    match vk {
        v if v == VK_SHIFT.0 => pick(VK_LSHIFT.0, VK_RSHIFT.0, scan == SCAN_RIGHT_SHIFT),
        v if v == VK_CONTROL.0 => pick(VK_LCONTROL.0, VK_RCONTROL.0, right),
        v if v == VK_MENU.0 => pick(VK_LMENU.0, VK_RMENU.0, right),
        _ => vk,
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_INPUT {
        let mut input = RAWINPUT::default();
        let mut size = size_of::<RAWINPUT>() as u32;
        let read = unsafe {
            GetRawInputData(
                HRAWINPUT(lparam.0 as *mut _),
                RID_INPUT,
                Some((&raw mut input).cast()),
                &mut size,
                size_of::<RAWINPUTHEADER>() as u32,
            )
        };
        if read != u32::MAX && input.header.dwType == RIM_TYPEKEYBOARD.0 {
            let keyboard = unsafe { input.data.keyboard };
            let down = keyboard.Flags & RI_KEY_BREAK as u16 == 0;
            on_key(sided_key(keyboard.VKey, keyboard.MakeCode, keyboard.Flags), down);
        }
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
