//! Runtime state the background process shares with the settings window.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Status {
    /// Shortcuts Windows refused to register, keyed by output ID.
    pub hotkey_errors: BTreeMap<String, String>,
}

/// Missing or unreadable means "nothing to report": the file is rewritten
/// each time the shortcuts are registered.
pub fn load(path: &Path) -> Status {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, status: &Status) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(status).map_err(std::io::Error::other)?;
    crate::settings::write_atomically(path, json.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_tolerates_garbage() {
        let dir = std::env::temp_dir().join(format!("sounddock-status-{}", std::process::id()));
        let path = dir.join("status.json");
        let mut status = Status::default();
        status.hotkey_errors.insert("a".into(), "refusé".into());
        save(&path, &status).unwrap();
        assert_eq!(load(&path), status);

        std::fs::write(&path, "garbage").unwrap();
        assert_eq!(load(&path), Status::default());
    }
}
