//! Registration of the per-output and "next output" shortcuts with
//! `RegisterHotKey`.
//!
//! A shortcut Windows refuses stays in the settings: the refusal is written
//! to the status file for the settings window, and registration is retried
//! on a timer, since the app holding it may quit.

use pinwheel_core::hotkey::Hotkey;
use pinwheel_core::settings::{Settings, duplicate_hotkey, next_output_duplicate};
use pinwheel_core::status::Status;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey, UnregisterHotKey,
};

/// `WM_HOTKEY` identifiers of the outputs are their index plus this offset.
const ID_OFFSET: i32 = 1;
/// The highest identifier an app may use, far from the outputs' ones.
const NEXT_OUTPUT_ID: i32 = 0xBFFF;

/// What a shortcut does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// Makes the output at this index the default one.
    Output(usize),
    NextOutput,
}

impl Slot {
    /// The slot of a `WM_HOTKEY` identifier.
    pub fn from_id(id: usize) -> Option<Self> {
        if id == NEXT_OUTPUT_ID as usize {
            Some(Self::NextOutput)
        } else {
            id.checked_sub(ID_OFFSET as usize).map(Self::Output)
        }
    }

    fn id(self) -> i32 {
        match self {
            Self::Output(index) => index as i32 + ID_OFFSET,
            Self::NextOutput => NEXT_OUTPUT_ID,
        }
    }
}

#[derive(Default)]
pub struct Hotkeys {
    registered: Vec<i32>,
    /// Shortcuts Windows refused.
    refused: Vec<Slot>,
}

impl Hotkeys {
    /// Replaces every registration with the shortcuts of `settings`. The
    /// status holds only the refusals.
    pub fn register_all(&mut self, hwnd: HWND, settings: &Settings) -> Status {
        self.unregister_all(hwnd);
        self.refused.clear();
        let mut status = Status::default();
        let slots = (0..settings.outputs.len()).map(Slot::Output).chain([Slot::NextOutput]);
        for slot in slots {
            self.try_register(hwnd, settings, slot, &mut status);
        }
        status
    }

    /// Tries the refused shortcuts again. Returns the refusals left when
    /// something changed.
    pub fn retry_refused(&mut self, hwnd: HWND, settings: &Settings) -> Option<Status> {
        if self.refused.is_empty() {
            return None;
        }
        let before = self.refused.len();
        let pending = std::mem::take(&mut self.refused);
        let mut status = Status::default();
        for slot in pending {
            self.try_register(hwnd, settings, slot, &mut status);
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

    fn try_register(&mut self, hwnd: HWND, settings: &Settings, slot: Slot, status: &mut Status) {
        let (hotkey, duplicate, name) = match slot {
            Slot::Output(index) => {
                let output = &settings.outputs[index];
                let duplicate = duplicate_hotkey(&settings.outputs, index).is_some();
                (output.hotkey, duplicate, output.name.as_str())
            }
            Slot::NextOutput => (
                settings.next_output_hotkey,
                next_output_duplicate(settings).is_some(),
                "next output",
            ),
        };
        let Some(hotkey) = hotkey else { return };
        // The settings window explains invalid and duplicate shortcuts itself.
        if hotkey.validate().is_err() || duplicate {
            return;
        }
        let id = slot.id();
        match unsafe { RegisterHotKey(Some(hwnd), id, modifiers(&hotkey), u32::from(hotkey.key)) } {
            Ok(()) => self.registered.push(id),
            Err(e) => {
                log::warn!("shortcut {hotkey} for \"{name}\" refused: {e}");
                self.refused.push(slot);
                // The settings window words the refusal in its own language.
                match slot {
                    Slot::Output(index) => {
                        let output_id = settings.outputs[index].id.clone();
                        status.hotkey_errors.insert(output_id, e.to_string());
                    }
                    Slot::NextOutput => status.next_output_error = Some(e.to_string()),
                }
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
