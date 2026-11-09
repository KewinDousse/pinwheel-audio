//! Where the settings window was, so that it opens there again.
//!
//! Windows' own placement gives the window's normal rectangle even while it
//! is minimized or maximized, when its current rectangle is off-screen or
//! covers the screen. Setting it back moves a window that would be off
//! every screen onto one.

use crate::paths;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromRect};
use windows::Win32::UI::WindowsAndMessaging::{GetWindowPlacement, SW_HIDE, SetWindowPlacement, WINDOWPLACEMENT};

/// The normal rectangle of `hwnd`, in workspace coordinates.
pub fn current(hwnd: HWND) -> Option<RECT> {
    let mut placement = WINDOWPLACEMENT {
        length: size_of::<WINDOWPLACEMENT>() as u32,
        ..Default::default()
    };
    unsafe { GetWindowPlacement(hwnd, &mut placement) }.ok()?;
    Some(placement.rcNormalPosition)
}

/// Puts the still hidden `hwnd` where the window was last.
pub fn restore(hwnd: HWND) {
    let Some(rect) = load() else { return };
    let placement = WINDOWPLACEMENT {
        length: size_of::<WINDOWPLACEMENT>() as u32,
        showCmd: SW_HIDE.0 as u32,
        rcNormalPosition: rect,
        ..Default::default()
    };
    // Twice: when the rectangle is on a screen of another scale, the first
    // move changes the window's DPI and Windows scales its size with it.
    for _ in 0..2 {
        if let Err(e) = unsafe { SetWindowPlacement(hwnd, &placement) } {
            log::warn!("placing the settings window failed: {e}");
            return;
        }
    }
    // Windows moves a rectangle that is off every screen onto one, as it
    // should; any other difference is worth knowing about.
    let on_a_screen = !unsafe { MonitorFromRect(&rect, MONITOR_DEFAULTTONULL) }.is_invalid();
    if let Some(placed) = current(hwnd)
        && placed != rect
        && on_a_screen
    {
        log::warn!(
            "the settings window is at {:?} instead of {:?}",
            (placed.left, placed.top, placed.right, placed.bottom),
            (rect.left, rect.top, rect.right, rect.bottom)
        );
    }
}

pub fn save(rect: RECT) {
    let text = format!("{} {} {} {}\n", rect.left, rect.top, rect.right, rect.bottom);
    if let Err(e) = std::fs::write(paths::window_file(), text) {
        log::warn!("saving the settings window's place failed: {e}");
    }
}

fn load() -> Option<RECT> {
    let text = std::fs::read_to_string(paths::window_file()).ok()?;
    let [left, top, right, bottom] = parse(&text)?;
    (right > left && bottom > top).then_some(RECT {
        left,
        top,
        right,
        bottom,
    })
}

fn parse(text: &str) -> Option<[i32; 4]> {
    let mut numbers = text.split_whitespace().map(str::parse);
    let rect = [
        numbers.next()?.ok()?,
        numbers.next()?.ok()?,
        numbers.next()?.ok()?,
        numbers.next()?.ok()?,
    ];
    numbers.next().is_none().then_some(rect)
}
