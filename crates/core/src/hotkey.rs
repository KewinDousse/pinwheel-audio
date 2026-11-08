//! Keyboard shortcuts bound to audio outputs.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A key combination, stored with the Windows virtual-key code of its main key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hotkey {
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub win: bool,
    /// Virtual-key code, one of [`KEYS`].
    pub key: u16,
}

/// A key offered for shortcuts.
#[derive(Debug, Clone, Copy)]
pub struct Key {
    pub vk: u16,
    pub label: &'static str,
}

const fn key(vk: u16, label: &'static str) -> Key {
    Key { vk, label }
}

const VK_F12: u16 = 0x7B;
const VK_F13: u16 = 0x7C;
const VK_F24: u16 = 0x87;

/// Keys offered in the settings, in display order.
#[rustfmt::skip]
pub const KEYS: &[Key] = &[
    key(0x30, "0"), key(0x31, "1"), key(0x32, "2"), key(0x33, "3"), key(0x34, "4"),
    key(0x35, "5"), key(0x36, "6"), key(0x37, "7"), key(0x38, "8"), key(0x39, "9"),
    key(0x41, "A"), key(0x42, "B"), key(0x43, "C"), key(0x44, "D"), key(0x45, "E"),
    key(0x46, "F"), key(0x47, "G"), key(0x48, "H"), key(0x49, "I"), key(0x4A, "J"),
    key(0x4B, "K"), key(0x4C, "L"), key(0x4D, "M"), key(0x4E, "N"), key(0x4F, "O"),
    key(0x50, "P"), key(0x51, "Q"), key(0x52, "R"), key(0x53, "S"), key(0x54, "T"),
    key(0x55, "U"), key(0x56, "V"), key(0x57, "W"), key(0x58, "X"), key(0x59, "Y"),
    key(0x5A, "Z"),
    key(0x70, "F1"), key(0x71, "F2"), key(0x72, "F3"), key(0x73, "F4"), key(0x74, "F5"),
    key(0x75, "F6"), key(0x76, "F7"), key(0x77, "F8"), key(0x78, "F9"), key(0x79, "F10"),
    key(0x7A, "F11"),
    key(0x7C, "F13"), key(0x7D, "F14"), key(0x7E, "F15"), key(0x7F, "F16"), key(0x80, "F17"),
    key(0x81, "F18"), key(0x82, "F19"), key(0x83, "F20"), key(0x84, "F21"), key(0x85, "F22"),
    key(0x86, "F23"), key(0x87, "F24"),
    key(0x60, "Pavé 0"), key(0x61, "Pavé 1"), key(0x62, "Pavé 2"), key(0x63, "Pavé 3"),
    key(0x64, "Pavé 4"), key(0x65, "Pavé 5"), key(0x66, "Pavé 6"), key(0x67, "Pavé 7"),
    key(0x68, "Pavé 8"), key(0x69, "Pavé 9"), key(0x6A, "Pavé *"), key(0x6B, "Pavé +"),
    key(0x6D, "Pavé -"), key(0x6E, "Pavé ."), key(0x6F, "Pavé /"),
    key(0x25, "Flèche gauche"), key(0x26, "Flèche haut"), key(0x27, "Flèche droite"),
    key(0x28, "Flèche bas"),
    key(0x2D, "Inser"), key(0x2E, "Suppr"), key(0x24, "Début"), key(0x23, "Fin"),
    key(0x21, "Page préc."), key(0x22, "Page suiv."), key(0x13, "Pause"), key(0x20, "Espace"),
];

/// Label of a virtual-key code, if it is one of [`KEYS`].
pub fn key_label(vk: u16) -> Option<&'static str> {
    KEYS.iter().find(|k| k.vk == vk).map(|k| k.label)
}

/// Why a shortcut cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyError {
    /// Not one of [`KEYS`].
    UnknownKey,
    /// F12 is reserved for debuggers by `RegisterHotKey`.
    ReservedKey,
    /// Without Ctrl, Alt or Win, the key could no longer be typed.
    NeedsModifier,
}

impl fmt::Display for HotkeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnknownKey => "Touche non prise en charge.",
            Self::ReservedKey => "F12 est réservée par Windows.",
            Self::NeedsModifier => "Ajoute Ctrl, Alt ou Win (sauf pour F13 à F24).",
        })
    }
}

impl Hotkey {
    /// Checks the combination can be registered and does not hijack typing.
    pub fn validate(&self) -> Result<(), HotkeyError> {
        if self.key == VK_F12 {
            return Err(HotkeyError::ReservedKey);
        }
        if key_label(self.key).is_none() {
            return Err(HotkeyError::UnknownKey);
        }
        let extra_key = (VK_F13..=VK_F24).contains(&self.key);
        if !(self.ctrl || self.alt || self.win || extra_key) {
            return Err(HotkeyError::NeedsModifier);
        }
        Ok(())
    }

    /// Windows reserves many Win combinations for the shell, and some of them
    /// are caught before `RegisterHotKey` sees them, with no error reported.
    pub fn uses_win(&self) -> bool {
        self.win
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let modifiers = [
            (self.ctrl, "Ctrl"),
            (self.alt, "Alt"),
            (self.shift, "Maj"),
            (self.win, "Win"),
        ];
        for (_, name) in modifiers.iter().filter(|(on, _)| *on) {
            write!(f, "{name}+")?;
        }
        f.write_str(key_label(self.key).unwrap_or("?"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hotkey(ctrl: bool, alt: bool, shift: bool, win: bool, key: u16) -> Hotkey {
        Hotkey {
            ctrl,
            alt,
            shift,
            win,
            key,
        }
    }

    #[test]
    fn displays_modifiers_in_fixed_order() {
        assert_eq!(hotkey(true, true, false, false, 0x31).to_string(), "Ctrl+Alt+1");
        assert_eq!(hotkey(true, false, true, true, 0x70).to_string(), "Ctrl+Maj+Win+F1");
        assert_eq!(hotkey(false, false, false, false, 0x80).to_string(), "F17");
    }

    #[test]
    fn accepts_usual_combinations() {
        assert_eq!(hotkey(true, true, false, false, 0x31).validate(), Ok(()));
        assert_eq!(hotkey(false, false, false, false, 0x7C).validate(), Ok(()));
    }

    #[test]
    fn rejects_unusable_combinations() {
        assert_eq!(
            hotkey(true, false, false, false, VK_F12).validate(),
            Err(HotkeyError::ReservedKey)
        );
        assert_eq!(
            hotkey(false, false, true, false, 0x41).validate(),
            Err(HotkeyError::NeedsModifier)
        );
        assert_eq!(
            hotkey(false, false, false, false, 0x41).validate(),
            Err(HotkeyError::NeedsModifier)
        );
        assert_eq!(
            hotkey(true, false, false, false, 0xFF).validate(),
            Err(HotkeyError::UnknownKey)
        );
    }

    #[test]
    fn key_labels_are_unique() {
        let mut seen = std::collections::HashSet::new();
        assert!(KEYS.iter().all(|k| seen.insert(k.vk)));
    }
}
