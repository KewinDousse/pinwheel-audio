//! User settings and their JSON file.
//!
//! The file is only written on a user action (a change in the settings window,
//! or a shortcut press that rebinds a moved output). Reading never rewrites it,
//! so a device missing at startup cannot shrink the saved list.

use crate::hotkey::Hotkey;
use crate::i18n::Lang;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Schema version written in the file.
pub const VERSION: u32 = 1;

pub const DEFAULT_VOLUME_STEP: u8 = 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    /// `None` follows the Windows display language.
    pub language: Option<Lang>,
    /// Volume change per wheel notch, in percent.
    pub volume_step: u8,
    /// Win + Shift + wheel changes the volume by [`FINE_STEP`].
    pub fine_step: bool,
    /// Win + middle click mutes or unmutes the default output.
    pub middle_click_mute: bool,
    pub osd: OsdSettings,
    /// Outputs the user picked, in display order. Unplugged ones stay here
    /// with their shortcut.
    pub outputs: Vec<OutputBinding>,
    /// Makes the next connected output of the list the default one.
    pub next_output_hotkey: Option<Hotkey>,
    /// Asks GitHub for the latest release, at startup then once a day.
    pub check_updates: bool,
}

/// Volume change per notch of Win + Shift + wheel, in percent.
pub const FINE_STEP: u8 = 1;

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: VERSION,
            language: None,
            volume_step: DEFAULT_VOLUME_STEP,
            fine_step: true,
            middle_click_mute: true,
            osd: OsdSettings::default(),
            outputs: Vec::new(),
            next_output_hotkey: None,
            check_updates: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputBinding {
    /// Endpoint ID at the time of the last binding.
    pub id: String,
    /// Friendly name, shown while unplugged and used to find the output
    /// again when Windows gives it a new ID.
    pub name: String,
    #[serde(default)]
    pub hotkey: Option<Hotkey>,
    /// Also make it the default for calls (Teams, Discord…). Off by default:
    /// on Bluetooth headsets it can switch to the low-quality hands-free mode.
    #[serde(default)]
    pub communications: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OsdSettings {
    /// Off, nothing shows, not even notices.
    pub enabled: bool,
    pub style: OsdStyle,
    pub position: OsdPosition,
    pub screen: OsdScreen,
    /// Time shown before the fade, in milliseconds, within [`OSD_DURATION_MS`].
    pub duration_ms: u32,
    /// Within [`OSD_OPACITY`], in percent.
    pub opacity: u8,
    /// Nothing shows while a full-screen app, a game or a presentation runs.
    pub hide_in_fullscreen: bool,
}

pub const OSD_DURATION_MS: std::ops::RangeInclusive<u32> = 500..=5_000;
pub const OSD_OPACITY: std::ops::RangeInclusive<u8> = 20..=100;

impl Default for OsdSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            style: OsdStyle::default(),
            position: OsdPosition::default(),
            screen: OsdScreen::default(),
            duration_ms: 1_000,
            opacity: 100,
            hide_in_fullscreen: false,
        }
    }
}

impl OsdSettings {
    /// The duration and opacity within their ranges, for a hand-edited file.
    pub fn clamped(self) -> Self {
        Self {
            duration_ms: self.duration_ms.clamp(*OSD_DURATION_MS.start(), *OSD_DURATION_MS.end()),
            opacity: self.opacity.clamp(*OSD_OPACITY.start(), *OSD_OPACITY.end()),
            ..self
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OsdStyle {
    /// A thin bar only.
    Bar,
    /// Speaker icon, bar and percentage in a rounded capsule.
    #[default]
    Pill,
    /// The percentage in large type.
    BigNumber,
}

impl OsdStyle {
    pub const ALL: [Self; 3] = [Self::Bar, Self::Pill, Self::BigNumber];

    pub fn label(self, lang: Lang) -> &'static str {
        let texts = lang.texts();
        match self {
            Self::Bar => texts.style_bar,
            Self::Pill => texts.style_pill,
            Self::BigNumber => texts.style_big_number,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OsdPosition {
    Top,
    Center,
    #[default]
    Bottom,
}

impl OsdPosition {
    pub const ALL: [Self; 3] = [Self::Top, Self::Center, Self::Bottom];

    pub fn label(self, lang: Lang) -> &'static str {
        let texts = lang.texts();
        match self {
            Self::Top => texts.position_top,
            Self::Center => texts.position_center,
            Self::Bottom => texts.position_bottom,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OsdScreen {
    /// The screen under the mouse pointer.
    #[default]
    Pointer,
    /// The main screen, set in the Windows display settings.
    Primary,
}

impl OsdScreen {
    pub const ALL: [Self; 2] = [Self::Pointer, Self::Primary];

    pub fn label(self, lang: Lang) -> &'static str {
        let texts = lang.texts();
        match self {
            Self::Pointer => texts.screen_pointer,
            Self::Primary => texts.screen_primary,
        }
    }
}

/// Two outputs sharing a shortcut: Windows would only register the first.
pub fn duplicate_hotkey(outputs: &[OutputBinding], index: usize) -> Option<&OutputBinding> {
    let hotkey = outputs.get(index)?.hotkey?;
    outputs[..index].iter().find(|o| o.hotkey == Some(hotkey))
}

/// The output whose shortcut is also the "next output" one. The outputs
/// are registered first, so that one wins.
pub fn next_output_duplicate(settings: &Settings) -> Option<&OutputBinding> {
    let hotkey = settings.next_output_hotkey?;
    settings.outputs.iter().find(|o| o.hotkey == Some(hotkey))
}

/// Problem found while reading the file.
#[derive(Debug)]
pub enum LoadIssue {
    /// Unreadable content, moved aside so the user can recover it.
    Corrupt { backup: PathBuf, error: String },
    /// The file exists but could not be read (permissions, lock…).
    Io(std::io::Error),
}

/// Reads the settings. A missing file gives the defaults. An unreadable one is
/// renamed to a timestamped `.bak` before the defaults are used, so the next
/// save does not destroy it.
pub fn load(path: &Path) -> (Settings, Option<LoadIssue>) {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Settings::default(), None),
        Err(e) => return (Settings::default(), Some(LoadIssue::Io(e))),
    };
    match serde_json::from_str::<Settings>(&text) {
        Ok(mut settings) => {
            settings.version = VERSION;
            settings.osd = settings.osd.clamped();
            (settings, None)
        }
        Err(error) => {
            let backup = backup_path(path);
            let issue = match std::fs::rename(path, &backup) {
                Ok(()) => LoadIssue::Corrupt {
                    backup,
                    error: error.to_string(),
                },
                Err(e) => LoadIssue::Io(e),
            };
            (Settings::default(), Some(issue))
        }
    }
}

fn backup_path(path: &Path) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{stamp}.bak"));
    path.with_file_name(name)
}

/// Writes the settings atomically: a crash mid-write leaves the previous
/// file intact.
pub fn save(path: &Path, settings: &Settings) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(settings).map_err(std::io::Error::other)?;
    write_atomically(path, json.as_bytes())
}

/// Writes next to `path`, then renames over it (which replaces the
/// destination on Windows too). The temporary name holds the process ID:
/// both processes write the settings, and must not share a temporary file.
pub(crate) fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp_name = path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(format!(".{}.tmp", std::process::id()));
    let tmp = path.with_file_name(tmp_name);
    {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pinwheel-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample() -> Settings {
        Settings {
            outputs: vec![OutputBinding {
                id: "{0.0.0.00000000}.{abc}".into(),
                name: "Casque".into(),
                hotkey: Some(Hotkey {
                    ctrl: true,
                    alt: true,
                    shift: false,
                    win: false,
                    key: 0x31,
                }),
                communications: true,
            }],
            language: Some(Lang::French),
            ..Settings::default()
        }
    }

    #[test]
    fn round_trips() {
        let path = temp_dir("roundtrip").join("settings.json");
        save(&path, &sample()).unwrap();
        let (loaded, issue) = load(&path);
        assert!(issue.is_none());
        assert_eq!(loaded, sample());
    }

    #[test]
    fn missing_file_gives_defaults() {
        let path = temp_dir("missing").join("settings.json");
        let (loaded, issue) = load(&path);
        assert!(issue.is_none());
        assert_eq!(loaded, Settings::default());
        assert!(!path.exists());
    }

    #[test]
    fn partial_file_fills_defaults() {
        let path = temp_dir("partial").join("settings.json");
        std::fs::write(&path, r#"{"outputs":[{"id":"x","name":"Casque"}]}"#).unwrap();
        let (loaded, issue) = load(&path);
        assert!(issue.is_none());
        assert_eq!(loaded.volume_step, DEFAULT_VOLUME_STEP);
        assert_eq!(loaded.outputs[0].hotkey, None);
        assert!(!loaded.outputs[0].communications);
        assert!(loaded.fine_step && loaded.middle_click_mute && loaded.check_updates);
    }

    #[test]
    fn an_older_display_section_keeps_the_display_on() {
        let path = temp_dir("older-osd").join("settings.json");
        std::fs::write(&path, r#"{"osd":{"style":"bar","position":"top"}}"#).unwrap();
        let osd = load(&path).0.osd;
        assert_eq!((osd.style, osd.position), (OsdStyle::Bar, OsdPosition::Top));
        assert!(osd.enabled);
        assert_eq!(osd.duration_ms, 1_000);
        assert_eq!(osd.opacity, 100);
    }

    #[test]
    fn clamps_a_hand_edited_display() {
        let osd = OsdSettings {
            duration_ms: 60_000,
            opacity: 0,
            ..OsdSettings::default()
        }
        .clamped();
        assert_eq!((osd.duration_ms, osd.opacity), (5_000, 20));
    }

    #[test]
    fn corrupt_file_is_kept_aside() {
        let dir = temp_dir("corrupt");
        let path = dir.join("settings.json");
        std::fs::write(&path, "{ not json").unwrap();
        let (loaded, issue) = load(&path);
        assert_eq!(loaded, Settings::default());
        let Some(LoadIssue::Corrupt { backup, .. }) = issue else {
            panic!("expected Corrupt, got {issue:?}")
        };
        assert_eq!(std::fs::read_to_string(backup).unwrap(), "{ not json");
        assert!(!path.exists());
    }

    #[test]
    fn save_replaces_existing_file() {
        let path = temp_dir("replace").join("settings.json");
        save(&path, &Settings::default()).unwrap();
        save(&path, &sample()).unwrap();
        assert_eq!(load(&path).0, sample());
        let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn detects_duplicate_hotkeys() {
        let mut settings = sample();
        let mut second = settings.outputs[0].clone();
        second.id = "other".into();
        settings.outputs.push(second);
        assert!(duplicate_hotkey(&settings.outputs, 0).is_none());
        assert_eq!(
            duplicate_hotkey(&settings.outputs, 1).map(|o| o.id.as_str()),
            Some("{0.0.0.00000000}.{abc}")
        );
    }

    #[test]
    fn detects_a_next_output_hotkey_taken_by_an_output() {
        let mut settings = sample();
        assert!(next_output_duplicate(&settings).is_none());
        settings.next_output_hotkey = settings.outputs[0].hotkey;
        assert_eq!(
            next_output_duplicate(&settings).map(|o| o.name.as_str()),
            Some("Casque")
        );
    }
}
