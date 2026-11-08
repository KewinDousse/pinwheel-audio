//! The interface language: the user's choice, or else the Windows display
//! language.

use pinwheel_core::i18n::Lang;
use pinwheel_core::settings::Settings;
use windows::Win32::Globalization::GetUserDefaultUILanguage;

pub fn current(settings: &Settings) -> Lang {
    settings
        .language
        .unwrap_or_else(|| Lang::from_windows(unsafe { GetUserDefaultUILanguage() }))
}
