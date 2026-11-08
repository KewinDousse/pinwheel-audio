//! Registration of the per-output shortcuts with `RegisterHotKey`.
//!
//! A shortcut Windows refuses stays in the settings: the refusal is written
//! to the status file for the settings window, and registration is retried
//! on a timer, since the app holding it may quit.

use pinwheel_core::hotkey::Hotkey;
use pinwheel_core::settings::{OutputBinding, duplicate_hotkey};
use pinwheel_core::status::Status;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey, UnregisterHotKey,
};

/// `WM_HOTKEY` identifiers are the output's index plus this offset.
const ID_OFFSET: i32 = 1;

#[derive(Default)]
pub struct Hotkeys {
    registered: Vec<i32>,
    /// Indexes of outputs whose shortcut Windows refused.
    refused: Vec<usize>,
}

impl Hotkeys {
    /// Replaces every registration with the shortcuts of `outputs`.
    pub fn register_all(&mut self, hwnd: HWND, outputs: &[OutputBinding]) -> Status {
        self.unregister_all(hwnd);
        self.refused.clear();
        let mut status = Status::default();
        for index in 0..outputs.len() {
            self.try_register(hwnd, outputs, index, &mut status);
        }
        status
    }

    /// Tries the refused shortcuts again. Returns the new status when
    /// something changed.
    pub fn retry_refused(&mut self, hwnd: HWND, outputs: &[OutputBinding]) -> Option<Status> {
        if self.refused.is_empty() {
            return None;
        }
        let before = self.refused.len();
        let pending = std::mem::take(&mut self.refused);
        let mut status = Status::default();
        for index in pending {
            self.try_register(hwnd, outputs, index, &mut status);
        }
        (self.refused.len() != before).then_some(status)
    }

    /// Unregisters every shortcut until the next [`Self::register_all`].
    pub fn suspend(&mut self, hwnd: HWND) {
        self.unregister_all(hwnd);
        self.refused.clear();
    }

    pub fn has_refused(&self) -> bool {
        !self.refused.is_empty()
    }

    /// Output index of a `WM_HOTKEY` identifier.
    pub fn output_index(id: usize) -> Option<usize> {
        id.checked_sub(ID_OFFSET as usize)
    }

    fn try_register(&mut self, hwnd: HWND, outputs: &[OutputBinding], index: usize, status: &mut Status) {
        let output = &outputs[index];
        let Some(hotkey) = output.hotkey else { return };
        // The settings window explains invalid and duplicate shortcuts itself.
        if hotkey.validate().is_err() || duplicate_hotkey(outputs, index).is_some() {
            return;
        }
        let id = index as i32 + ID_OFFSET;
        match unsafe { RegisterHotKey(Some(hwnd), id, modifiers(&hotkey), u32::from(hotkey.key)) } {
            Ok(()) => self.registered.push(id),
            Err(e) => {
                log::warn!("shortcut {hotkey} for \"{}\" refused: {e}", output.name);
                self.refused.push(index);
                // The settings window words the refusal in its own language.
                status.hotkey_errors.insert(output.id.clone(), e.to_string());
            }
        }
    }

    fn unregister_all(&mut self, hwnd: HWND) {
        for id in self.registered.drain(..) {
            let _ = unsafe { UnregisterHotKey(Some(hwnd), id) };
        }
    }
}

fn modifiers(hotkey: &Hotkey) -> HOT_KEY_MODIFIERS {
    // Without NOREPEAT, holding the keys would switch the output again and again.
    let mut flags = MOD_NOREPEAT;
    for (on, flag) in [
        (hotkey.ctrl, MOD_CONTROL),
        (hotkey.alt, MOD_ALT),
        (hotkey.shift, MOD_SHIFT),
        (hotkey.win, MOD_WIN),
    ] {
        if on {
            flags |= flag;
        }
    }
    flags
}
