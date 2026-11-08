//! Keyboard shortcuts bound to audio outputs.
//!
//! A shortcut is any key, with or without modifiers, as `RegisterHotKey`
//! accepts it: Ctrl, Alt, Shift and Win plus one other key.

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
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
    /// Virtual-key code of the main key.
    pub key: u16,
}

const VK_ESCAPE: u16 = 0x1B;
const VK_F12: u16 = 0x7B;
const VK_F13: u16 = 0x7C;
const VK_F24: u16 = 0x87;

/// Virtual keys of Shift, Ctrl, Alt and Win (generic, left and right): they
/// are modifiers, never the main key.
pub const MODIFIER_KEYS: [u16; 11] = [0x10, 0x11, 0x12, 0x5B, 0x5C, 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5];

/// Escape alone cancels a capture in the settings window.
pub fn is_cancel(hotkey: &Hotkey) -> bool {
    hotkey.key == VK_ESCAPE && !(hotkey.ctrl || hotkey.alt || hotkey.shift || hotkey.win)
}

/// Name of keys whose label does not depend on the keyboard layout.
/// Characters (letters, digits, punctuation) are named by the keyboard
/// layout on Windows; this gives a fallback for them.
pub fn key_name(vk: u16) -> Cow<'static, str> {
    let name = match vk {
        0x08 => "Retour arrière",
        0x09 => "Tab",
        0x0C => "Effacer",
        0x0D => "Entrée",
        0x13 => "Pause",
        0x14 => "Verr. Maj",
        0x1B => "Échap",
        0x20 => "Espace",
        0x21 => "Page préc.",
        0x22 => "Page suiv.",
        0x23 => "Fin",
        0x24 => "Début",
        0x25 => "Flèche gauche",
        0x26 => "Flèche haut",
        0x27 => "Flèche droite",
        0x28 => "Flèche bas",
        0x2C => "Impr. écran",
        0x2D => "Inser",
        0x2E => "Suppr",
        0x5D => "Menu",
        0x5F => "Veille",
        0x6A => "Pavé *",
        0x6B => "Pavé +",
        0x6C => "Pavé séparateur",
        0x6D => "Pavé -",
        0x6E => "Pavé .",
        0x6F => "Pavé /",
        0x90 => "Verr. Num",
        0x91 => "Arrêt défil",
        0xA6 => "Navigateur : précédent",
        0xA7 => "Navigateur : suivant",
        0xA8 => "Navigateur : actualiser",
        0xA9 => "Navigateur : arrêter",
        0xAA => "Navigateur : rechercher",
        0xAB => "Navigateur : favoris",
        0xAC => "Navigateur : accueil",
        0xAD => "Muet",
        0xAE => "Volume -",
        0xAF => "Volume +",
        0xB0 => "Piste suivante",
        0xB1 => "Piste précédente",
        0xB2 => "Arrêt média",
        0xB3 => "Lecture/Pause",
        0xB4 => "Courrier",
        0xB5 => "Lecteur multimédia",
        0xB6 => "Application 1",
        0xB7 => "Application 2",
        0x30..=0x39 | 0x41..=0x5A => return Cow::Owned(char::from(vk as u8).to_string()),
        0x60..=0x69 => return Cow::Owned(format!("Pavé {}", vk - 0x60)),
        0x70..=0x87 => return Cow::Owned(format!("F{}", vk - 0x6F)),
        _ => return Cow::Owned(format!("Touche 0x{vk:02X}")),
    };
    Cow::Borrowed(name)
}

/// Whether Windows gives the key a fixed name ([`key_name`]) rather than the
/// character of the current keyboard layout.
pub fn has_fixed_name(vk: u16) -> bool {
    !matches!(vk, 0x30..=0x39 | 0x41..=0x5A | 0xBA..=0xC0 | 0xDB..=0xDF | 0xE2)
}

/// Why a shortcut cannot be registered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyError {
    /// No main key, or a modifier in its place.
    NoKey,
    /// `RegisterHotKey` reserves F12 for debuggers.
    ReservedKey,
}

impl fmt::Display for HotkeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NoKey => "Il manque une touche en plus de Ctrl, Alt, Maj ou Win.",
            Self::ReservedKey => "F12 est réservée par Windows.",
        })
    }
}

/// A shortcut that works but has a drawback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyWarning {
    /// Windows reserves many Win combinations for the shell, and catches some
    /// of them before `RegisterHotKey` sees them, with no error reported.
    UsesWin,
    /// Without Ctrl, Alt or Win, the key loses its usual function in every
    /// application.
    TakesOverKey,
}

impl fmt::Display for HotkeyWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UsesWin => "Windows réserve beaucoup de raccourcis avec Win : celui-ci risque de ne pas marcher.",
            Self::TakesOverKey => {
                "Sans Ctrl, Alt ni Win, cette touche ne servira plus qu'à changer de sortie, dans toutes les applications."
            }
        })
    }
}

impl Hotkey {
    /// Checks that `RegisterHotKey` can take the combination.
    pub fn validate(&self) -> Result<(), HotkeyError> {
        if self.key == 0 || MODIFIER_KEYS.contains(&self.key) {
            return Err(HotkeyError::NoKey);
        }
        if self.key == VK_F12 {
            return Err(HotkeyError::ReservedKey);
        }
        Ok(())
    }

    pub fn warnings(&self) -> Vec<HotkeyWarning> {
        let mut warnings = Vec::new();
        if self.win {
            warnings.push(HotkeyWarning::UsesWin);
        }
        // F13 to F24 have no function of their own: macro keys often send them.
        let spare_key = (VK_F13..=VK_F24).contains(&self.key);
        if !(self.ctrl || self.alt || self.win || spare_key) {
            warnings.push(HotkeyWarning::TakesOverKey);
        }
        warnings
    }

    /// Writes the modifiers then `key`, the name of the main key.
    pub fn format_with(&self, f: &mut impl fmt::Write, key: &str) -> fmt::Result {
        let modifiers = [
            (self.ctrl, "Ctrl"),
            (self.alt, "Alt"),
            (self.shift, "Maj"),
            (self.win, "Win"),
        ];
        for (_, name) in modifiers.iter().filter(|(on, _)| *on) {
            write!(f, "{name}+")?;
        }
        f.write_str(key)
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.format_with(f, &key_name(self.key))
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
        assert_eq!(hotkey(false, true, false, false, 0xB3).to_string(), "Alt+Lecture/Pause");
        assert_eq!(hotkey(true, false, false, false, 0xBA).to_string(), "Ctrl+Touche 0xBA");
    }

    #[test]
    fn accepts_any_key() {
        for key in [0x31, 0x7C, 0xB3, 0xBA, 0x13, 0x41] {
            assert_eq!(
                hotkey(false, false, false, false, key).validate(),
                Ok(()),
                "key {key:#X}"
            );
        }
    }

    #[test]
    fn rejects_what_windows_cannot_register() {
        assert_eq!(
            hotkey(true, false, false, false, VK_F12).validate(),
            Err(HotkeyError::ReservedKey)
        );
        assert_eq!(hotkey(true, false, false, false, 0).validate(), Err(HotkeyError::NoKey));
        assert_eq!(
            hotkey(false, false, false, true, 0x5B).validate(),
            Err(HotkeyError::NoKey)
        );
    }

    #[test]
    fn warns_about_risky_combinations() {
        assert!(hotkey(true, true, false, false, 0x31).warnings().is_empty());
        assert!(hotkey(false, false, false, false, 0x7C).warnings().is_empty());
        assert_eq!(
            hotkey(false, false, true, false, 0x41).warnings(),
            [HotkeyWarning::TakesOverKey]
        );
        assert_eq!(
            hotkey(false, false, false, true, 0x41).warnings(),
            [HotkeyWarning::UsesWin]
        );
    }

    #[test]
    fn escape_alone_cancels() {
        assert!(is_cancel(&hotkey(false, false, false, false, VK_ESCAPE)));
        assert!(!is_cancel(&hotkey(true, false, false, false, VK_ESCAPE)));
    }

    #[test]
    fn names_cover_the_key_ranges() {
        assert_eq!(key_name(0x5A), "Z");
        assert_eq!(key_name(0x69), "Pavé 9");
        assert_eq!(key_name(0x87), "F24");
        assert!(!has_fixed_name(0x41));
        assert!(!has_fixed_name(0xBA));
        assert!(has_fixed_name(0x70));
    }
}
