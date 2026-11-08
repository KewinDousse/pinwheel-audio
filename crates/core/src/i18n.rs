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
    pub on: &'static str,
    pub off: &'static str,

    pub outputs_heading: &'static str,
    pub remove: &'static str,
    pub connected: &'static str,
    pub disconnected: &'static str,
    pub not_found: &'static str,
    pub default_output: &'static str,
    pub communications: &'static str,
    pub audio_unavailable: &'static str,
    pub add_output: &'static str,
    pub add: &'static str,
    pub nothing_to_add: &'static str,

    pub shortcut: &'static str,
    pub set_key: &'static str,
    pub capture_prompt: &'static str,
    pub capture_cancel_hint: &'static str,
    pub cancel: &'static str,
    pub change: &'static str,
    pub clear: &'static str,
    pub no_shortcut: &'static str,
    pub set: &'static str,
    pub capture_failed: &'static str,
    pub duplicate_shortcut: fn(other: &str) -> String,
    pub shortcut_refused: &'static str,
    pub shortcut_no_key: &'static str,
    pub shortcut_reserved_f12: &'static str,
    pub shortcut_uses_win: &'static str,
    pub shortcut_takes_over_key: &'static str,
    pub shift: &'static str,

    /// Follows a drawn Win key: "+ wheel changes the volume".
    pub wheel_heading: &'static str,
    pub wheel_summary: fn(step: i32, style: &str, position: &str) -> String,
    pub volume_step: &'static str,
    pub style: &'static str,
    pub position: &'static str,
    pub style_bar: &'static str,
    pub style_pill: &'static str,
    pub style_big_number: &'static str,
    pub position_top: &'static str,
    pub position_center: &'static str,
    pub position_bottom: &'static str,
    pub try_display: &'static str,

    pub general_heading: &'static str,
    pub general_summary: fn(language: &str, starts_at_sign_in: bool) -> String,
    pub windows_language: &'static str,
    pub startup_checkbox: &'static str,
    pub startup_failed: fn(error: &str) -> String,
    pub startup_disabled_by_windows: &'static str,
    pub logs: &'static str,
    pub open_folder: &'static str,

    pub tray_settings: &'static str,
    pub tray_quit: &'static str,

    pub started: &'static str,
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
    language: "Language",
    language_windows: "Same as Windows",
    on: "On",
    off: "Off",

    outputs_heading: "Switch outputs with one key",
    remove: "Remove from the list",
    connected: "Connected",
    disconnected: "Disconnected",
    not_found: "Not found",
    default_output: "Default output",
    communications: "Also for calls (Teams, Discord…)",
    audio_unavailable: "The Windows audio service is not responding.",
    add_output: "Add an output",
    add: "Add",
    nothing_to_add: "Every connected output is already in the list.",

    shortcut: "Shortcut",
    set_key: "Set a key",
    capture_prompt: "Press the keys you want",
    capture_cancel_hint: "Esc to cancel",
    cancel: "Cancel",
    change: "Change",
    clear: "Clear",
    no_shortcut: "None",
    set: "Set",
    capture_failed: "Could not capture the shortcut: see the log.",
    duplicate_shortcut: |other| format!("Same shortcut as “{other}”: only the first one works."),
    shortcut_refused: "Windows refuses this shortcut: another app probably uses it.",
    shortcut_no_key: "Add a key besides Ctrl, Alt, Shift or Win.",
    shortcut_reserved_f12: "Windows reserves F12.",
    shortcut_uses_win: "Windows reserves many Win shortcuts: this one may not work.",
    shortcut_takes_over_key: "Without Ctrl, Alt or Win, this key will only switch outputs, in every app.",
    shift: "Shift",

    wheel_heading: "+ wheel changes the volume",
    wheel_summary: |step, style, position| format!("{step}% per notch · {style}, {}", position.to_lowercase()),
    volume_step: "Step per notch",
    style: "Display style",
    position: "Position",
    style_bar: "Thin bar",
    style_pill: "Pill",
    style_big_number: "Big number",
    position_top: "Top",
    position_center: "Center",
    position_bottom: "Bottom",
    try_display: "Try the display",

    general_heading: "General",
    general_summary: |language, on| {
        let startup = if on {
            "Starts when you sign in"
        } else {
            "Not started when you sign in"
        };
        format!("{language} · {startup}")
    },
    windows_language: "Windows language",
    startup_checkbox: "Start Pinwheel when I sign in",
    startup_failed: |error| format!("Could not change the startup entry: {error}"),
    startup_disabled_by_windows: "Turned off in the Windows startup apps. Turning it on here turns it back on.",
    logs: "Logs",
    open_folder: "Open folder",

    tray_settings: "Settings",
    tray_quit: "Quit Pinwheel",

    started: "Pinwheel started",
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
    language: "Langue",
    language_windows: "Celle de Windows",
    on: "Activé",
    off: "Désactivé",

    outputs_heading: "Changer de sortie d'une touche",
    remove: "Retirer de la liste",
    connected: "Connectée",
    disconnected: "Déconnectée",
    not_found: "Introuvable",
    default_output: "Sortie par défaut",
    communications: "Aussi pour les appels (Teams, Discord…)",
    audio_unavailable: "Le service audio de Windows ne répond pas.",
    add_output: "Ajouter une sortie",
    add: "Ajouter",
    nothing_to_add: "Toutes les sorties connectées sont déjà dans la liste.",

    shortcut: "Raccourci",
    set_key: "Définir une touche",
    capture_prompt: "Appuie sur la combinaison voulue",
    capture_cancel_hint: "Échap pour annuler",
    cancel: "Annuler",
    change: "Changer",
    clear: "Supprimer",
    no_shortcut: "Aucun",
    set: "Définir",
    capture_failed: "Capture du raccourci impossible : voir le journal.",
    duplicate_shortcut: |other| format!("Même raccourci que « {other} » : seul le premier fonctionne."),
    shortcut_refused: "Windows refuse ce raccourci : une autre application l'utilise sans doute.",
    shortcut_no_key: "Il manque une touche en plus de Ctrl, Alt, Maj ou Win.",
    shortcut_reserved_f12: "F12 est réservée par Windows.",
    shortcut_uses_win: "Windows réserve beaucoup de raccourcis avec Win : celui-ci risque de ne pas marcher.",
    shortcut_takes_over_key: "Sans Ctrl, Alt ni Win, cette touche ne servira plus qu'à changer de sortie, dans toutes les applications.",
    shift: "Maj",

    wheel_heading: "+ molette règle le volume",
    wheel_summary: |step, style, position| format!("{step} % par cran · {style} {}", position.to_lowercase()),
    volume_step: "Pas par cran",
    style: "Style de l'affichage",
    position: "Position",
    style_bar: "Barre fine",
    style_pill: "Pilule",
    style_big_number: "Grand chiffre",
    position_top: "En haut",
    position_center: "Au centre",
    position_bottom: "En bas",
    try_display: "Essayer l'affichage",

    general_heading: "Général",
    general_summary: |language, on| {
        let startup = if on {
            "Lancé à l'ouverture de session"
        } else {
            "Pas lancé à l'ouverture de session"
        };
        format!("{language} · {startup}")
    },
    windows_language: "Langue de Windows",
    startup_checkbox: "Lancer Pinwheel à l'ouverture de session",
    startup_failed: |error| format!("Modification du démarrage impossible : {error}"),
    startup_disabled_by_windows: "Désactivé dans les applications de démarrage de Windows. L'activer ici le réactive.",
    logs: "Journaux",
    open_folder: "Ouvrir le dossier",

    tray_settings: "Paramètres",
    tray_quit: "Quitter Pinwheel",

    started: "Pinwheel est lancé",
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

    #[test]
    fn summarises_the_wheel_settings_as_a_sentence() {
        assert_eq!(
            (ENGLISH.wheel_summary)(10, "Pill", "Bottom"),
            "10% per notch · Pill, bottom"
        );
        assert_eq!(
            (FRENCH.wheel_summary)(10, "Pilule", "En bas"),
            "10 % par cran · Pilule en bas"
        );
    }
}
