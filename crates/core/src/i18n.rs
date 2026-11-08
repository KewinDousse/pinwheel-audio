//! Interface texts in English and French.
//!
//! The language follows the Windows display language unless the user picks
//! one in the settings. Each language is a [`Texts`] table, so a missing
//! translation is a compile error rather than a blank label.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lang {
    English,
    French,
}

/// Windows primary language identifier of French (`LANG_FRENCH`).
const LANG_FRENCH: u16 = 0x0C;

impl Lang {
    pub const ALL: [Self; 2] = [Self::English, Self::French];

    /// The language of a Windows `LANGID`, such as `GetUserDefaultUILanguage`
    /// returns. Every language other than French falls back to English.
    pub fn from_windows(langid: u16) -> Self {
        if langid & 0x3FF == LANG_FRENCH {
            Self::French
        } else {
            Self::English
        }
    }

    /// Name of the language in itself, for the language picker.
    pub fn native_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::French => "Français",
        }
    }

    pub fn texts(self) -> &'static Texts {
        match self {
            Self::English => &ENGLISH,
            Self::French => &FRENCH,
        }
    }
}

pub struct Texts {
    pub settings_title: &'static str,
    pub settings_corrupt: fn(backup: &str) -> String,
    pub settings_unreadable: fn(error: &str) -> String,
    pub settings_save_failed: fn(error: &str) -> String,
    pub not_running: &'static str,
    pub start_app: &'static str,
    pub language: &'static str,
    pub language_windows: &'static str,

    pub outputs_heading: &'static str,
    pub outputs_help: &'static str,
    pub remove: &'static str,
    pub connected: &'static str,
    pub disconnected: &'static str,
    pub not_found: &'static str,
    pub communications: &'static str,
    pub audio_unavailable: &'static str,
    pub add_output: &'static str,

    pub shortcut: &'static str,
    pub capture_prompt: &'static str,
    pub cancel: &'static str,
    pub change: &'static str,
    pub clear: &'static str,
    pub no_shortcut: &'static str,
    pub choose: &'static str,
    pub capture_failed: &'static str,
    pub duplicate_shortcut: fn(other: &str) -> String,
    pub shortcut_refused: &'static str,
    pub shortcut_no_key: &'static str,
    pub shortcut_reserved_f12: &'static str,
    pub shortcut_uses_win: &'static str,
    pub shortcut_takes_over_key: &'static str,
    pub shift: &'static str,

    pub volume_heading: &'static str,
    pub volume_help: &'static str,
    pub per_notch: &'static str,
    pub percent_suffix: &'static str,

    pub display_heading: &'static str,
    pub style: &'static str,
    pub position: &'static str,
    pub style_bar: &'static str,
    pub style_pill: &'static str,
    pub style_big_number: &'static str,
    pub position_top: &'static str,
    pub position_center: &'static str,
    pub position_bottom: &'static str,
    pub preview: &'static str,

    pub startup_heading: &'static str,
    pub startup_checkbox: &'static str,
    pub startup_failed: fn(error: &str) -> String,
    pub startup_disabled_by_windows: &'static str,
    pub open_logs: &'static str,

    pub tray_settings: &'static str,
    pub tray_quit: &'static str,

    pub muted: &'static str,
    pub percent: fn(value: i32) -> String,
    pub output_disconnected: fn(name: &str) -> String,
    pub output_switch_failed: fn(name: &str) -> String,
    pub settings_corrupt_notice: &'static str,
}

pub const ENGLISH: Texts = Texts {
    settings_title: "Pinwheel settings",
    settings_corrupt: |backup| format!("The settings file was unreadable. It was kept as {backup}."),
    settings_unreadable: |error| format!("Could not read the settings: {error}"),
    settings_save_failed: |error| format!("Could not save the settings: {error}"),
    not_running: "Pinwheel is not running: the wheel and the shortcuts do nothing.",
    start_app: "Start Pinwheel",
    language: "Language:",
    language_windows: "Same as Windows",

    outputs_heading: "Audio outputs",
    outputs_help: "An output's shortcut makes it the default output. An unplugged output keeps its shortcut.",
    remove: "Remove",
    connected: "connected",
    disconnected: "disconnected",
    not_found: "not found",
    communications: "Also for calls (Teams, Discord…)",
    audio_unavailable: "The Windows audio service is not responding.",
    add_output: "Add an output…",

    shortcut: "Shortcut:",
    capture_prompt: "Press the keys you want… (Esc to cancel)",
    cancel: "Cancel",
    change: "Change…",
    clear: "Clear",
    no_shortcut: "none",
    choose: "Set…",
    capture_failed: "Could not capture the shortcut: see the log.",
    duplicate_shortcut: |other| format!("Same shortcut as “{other}”: only the first one works."),
    shortcut_refused: "Windows refuses this shortcut: another app probably uses it.",
    shortcut_no_key: "Add a key besides Ctrl, Alt, Shift or Win.",
    shortcut_reserved_f12: "Windows reserves F12.",
    shortcut_uses_win: "Windows reserves many Win shortcuts: this one may not work.",
    shortcut_takes_over_key: "Without Ctrl, Alt or Win, this key will only switch outputs, in every app.",
    shift: "Shift",

    volume_heading: "Volume",
    volume_help: "Win + mouse wheel changes the volume of the default output.",
    per_notch: "per wheel notch",
    percent_suffix: "%",

    display_heading: "Display",
    style: "Style:",
    position: "Position:",
    style_bar: "Thin bar",
    style_pill: "Pill",
    style_big_number: "Big number",
    position_top: "Top",
    position_center: "Center",
    position_bottom: "Bottom",
    preview: "Preview",

    startup_heading: "Startup",
    startup_checkbox: "Start Pinwheel when I sign in",
    startup_failed: |error| format!("Could not change the startup entry: {error}"),
    startup_disabled_by_windows: "Turned off in the Windows startup apps. Ticking the box turns it back on.",
    open_logs: "Open the log folder",

    tray_settings: "Settings",
    tray_quit: "Quit Pinwheel",

    muted: "Muted",
    percent: |value| format!("{value}%"),
    output_disconnected: |name| format!("{name}: disconnected"),
    output_switch_failed: |name| format!("Could not switch to {name}"),
    settings_corrupt_notice: "Unreadable settings: a copy was kept",
};

pub const FRENCH: Texts = Texts {
    settings_title: "Paramètres de Pinwheel",
    settings_corrupt: |backup| format!("Le fichier de paramètres était illisible. Il a été gardé sous {backup}."),
    settings_unreadable: |error| format!("Lecture des paramètres impossible : {error}"),
    settings_save_failed: |error| format!("Enregistrement impossible : {error}"),
    not_running: "Pinwheel ne tourne pas : la molette et les raccourcis sont inactifs.",
    start_app: "Lancer Pinwheel",
    language: "Langue :",
    language_windows: "Celle de Windows",

    outputs_heading: "Sorties audio",
    outputs_help: "Le raccourci d'une sortie en fait la sortie par défaut. Une sortie débranchée garde son raccourci.",
    remove: "Retirer",
    connected: "connectée",
    disconnected: "déconnectée",
    not_found: "introuvable",
    communications: "Aussi pour les appels (Teams, Discord…)",
    audio_unavailable: "Le service audio de Windows ne répond pas.",
    add_output: "Ajouter une sortie…",

    shortcut: "Raccourci :",
    capture_prompt: "Appuie sur la combinaison voulue… (Échap pour annuler)",
    cancel: "Annuler",
    change: "Changer…",
    clear: "Supprimer",
    no_shortcut: "aucun",
    choose: "Choisir…",
    capture_failed: "Capture du raccourci impossible : voir le journal.",
    duplicate_shortcut: |other| format!("Même raccourci que « {other} » : seul le premier fonctionne."),
    shortcut_refused: "Windows refuse ce raccourci : une autre application l'utilise sans doute.",
    shortcut_no_key: "Il manque une touche en plus de Ctrl, Alt, Maj ou Win.",
    shortcut_reserved_f12: "F12 est réservée par Windows.",
    shortcut_uses_win: "Windows réserve beaucoup de raccourcis avec Win : celui-ci risque de ne pas marcher.",
    shortcut_takes_over_key: "Sans Ctrl, Alt ni Win, cette touche ne servira plus qu'à changer de sortie, dans toutes les applications.",
    shift: "Maj",

    volume_heading: "Volume",
    volume_help: "Win + molette règle le volume de la sortie par défaut.",
    per_notch: "par cran de molette",
    percent_suffix: " %",

    display_heading: "Affichage",
    style: "Style :",
    position: "Position :",
    style_bar: "Barre fine",
    style_pill: "Pilule",
    style_big_number: "Grand chiffre",
    position_top: "En haut",
    position_center: "Au centre",
    position_bottom: "En bas",
    preview: "Aperçu",

    startup_heading: "Démarrage",
    startup_checkbox: "Lancer Pinwheel à l'ouverture de session",
    startup_failed: |error| format!("Modification du démarrage impossible : {error}"),
    startup_disabled_by_windows: "Désactivé dans les applications de démarrage de Windows. Cocher la case le réactive.",
    open_logs: "Ouvrir le dossier des journaux",

    tray_settings: "Paramètres",
    tray_quit: "Quitter Pinwheel",

    muted: "Muet",
    percent: |value| format!("{value} %"),
    output_disconnected: |name| format!("{name} : déconnectée"),
    output_switch_failed: |name| format!("Impossible de passer sur {name}"),
    settings_corrupt_notice: "Paramètres illisibles : une copie a été gardée",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_windows_for_every_french_locale() {
        assert_eq!(Lang::from_windows(0x040C), Lang::French);
        assert_eq!(Lang::from_windows(0x100C), Lang::French);
        assert_eq!(Lang::from_windows(0x0409), Lang::English);
        assert_eq!(Lang::from_windows(0x0407), Lang::English);
    }

    #[test]
    fn formats_with_each_language_typography() {
        assert_eq!((ENGLISH.percent)(40), "40%");
        assert_eq!((FRENCH.percent)(40), "40 %");
    }
}
