//! Matching a saved output with the outputs Windows currently reports.

use crate::settings::OutputBinding;

/// An audio output as reported by Windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Endpoint ID string.
    pub id: String,
    /// Friendly name, such as "Casque (Realtek USB Audio)".
    pub name: String,
    /// Plugged in and enabled.
    pub active: bool,
}

/// Where a saved output stands among the current devices.
#[derive(Debug, PartialEq, Eq)]
pub enum Resolution<'a> {
    /// Same ID, active.
    Ready(&'a DeviceInfo),
    /// The saved ID is not active, but exactly one active output has the
    /// saved name. Windows gives an endpoint a new ID after a driver update
    /// or a new Bluetooth pairing, so this is treated as the same output.
    Moved(&'a DeviceInfo),
    /// Known but unplugged or disabled, or several active outputs share its name.
    Unavailable,
    /// Windows no longer knows this output.
    Unknown,
}

/// Finds the device matching a saved output `(id, name)`.
pub fn resolve<'a>(id: &str, name: &str, devices: &'a [DeviceInfo]) -> Resolution<'a> {
    let by_id = devices.iter().find(|d| d.id == id);
    if let Some(device) = by_id.filter(|d| d.active) {
        return Resolution::Ready(device);
    }

    let mut same_name = devices.iter().filter(|d| d.active && d.id != id && d.name == name);
    match (same_name.next(), same_name.next()) {
        (Some(device), None) => Resolution::Moved(device),
        (Some(_), Some(_)) => Resolution::Unavailable,
        (None, _) if by_id.is_some() => Resolution::Unavailable,
        (None, _) => Resolution::Unknown,
    }
}

/// Index in `outputs` of the output after the default one, going round the
/// list and skipping the ones not connected, and those that are the default
/// device too (an unplugged output can resolve to a connected one of the
/// same name). When the default output is not in the list, the first
/// connected one. The default one when no other is connected, `None` when
/// none is.
pub fn next_output(outputs: &[OutputBinding], devices: &[DeviceInfo], default_id: Option<&str>) -> Option<usize> {
    let device = |output: &OutputBinding| match resolve(&output.id, &output.name, devices) {
        Resolution::Ready(device) | Resolution::Moved(device) => Some(device.id.as_str()),
        Resolution::Unavailable | Resolution::Unknown => None,
    };
    let is_default = |output: &OutputBinding| device(output).is_some_and(|id| Some(id) == default_id);
    let current = outputs.iter().position(is_default);
    let start = current.map_or(0, |i| i + 1);
    (0..outputs.len())
        .map(|offset| (start + offset) % outputs.len())
        .find(|&i| device(&outputs[i]).is_some() && !is_default(&outputs[i]))
        .or(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str, name: &str, active: bool) -> DeviceInfo {
        DeviceInfo {
            id: id.into(),
            name: name.into(),
            active,
        }
    }

    #[test]
    fn same_id_active_is_ready() {
        let devices = [device("a", "Casque", true)];
        assert_eq!(resolve("a", "Casque", &devices), Resolution::Ready(&devices[0]));
    }

    #[test]
    fn renamed_device_with_same_id_is_ready() {
        let devices = [device("a", "Casque renommé", true)];
        assert_eq!(resolve("a", "Casque", &devices), Resolution::Ready(&devices[0]));
    }

    #[test]
    fn new_id_with_same_name_is_moved() {
        let devices = [device("a", "Casque", false), device("b", "Casque", true)];
        assert_eq!(resolve("a", "Casque", &devices), Resolution::Moved(&devices[1]));
        assert_eq!(resolve("gone", "Casque", &devices), Resolution::Moved(&devices[1]));
    }

    #[test]
    fn ambiguous_name_is_not_guessed() {
        let devices = [device("b", "Casque", true), device("c", "Casque", true)];
        assert_eq!(resolve("a", "Casque", &devices), Resolution::Unavailable);
    }

    #[test]
    fn unplugged_device_is_unavailable() {
        let devices = [device("a", "Casque", false), device("b", "Haut-parleurs", true)];
        assert_eq!(resolve("a", "Casque", &devices), Resolution::Unavailable);
    }

    #[test]
    fn forgotten_device_is_unknown() {
        let devices = [device("b", "Haut-parleurs", true)];
        assert_eq!(resolve("a", "Casque", &devices), Resolution::Unknown);
    }

    fn saved(id: &str, name: &str) -> OutputBinding {
        OutputBinding {
            id: id.into(),
            name: name.into(),
            hotkey: None,
            communications: false,
        }
    }

    #[test]
    fn next_output_goes_round_the_connected_ones() {
        let outputs = [saved("a", "Casque"), saved("b", "Écran"), saved("c", "Haut-parleurs")];
        let devices = [
            device("a", "Casque", true),
            device("b", "Écran", false),
            device("c", "Haut-parleurs", true),
        ];
        assert_eq!(next_output(&outputs, &devices, Some("a")), Some(2));
        assert_eq!(next_output(&outputs, &devices, Some("c")), Some(0));
    }

    #[test]
    fn next_output_starts_at_the_first_when_the_default_is_not_listed() {
        let outputs = [saved("a", "Casque"), saved("b", "Écran")];
        let devices = [
            device("a", "Casque", true),
            device("b", "Écran", true),
            device("z", "Autre", true),
        ];
        assert_eq!(next_output(&outputs, &devices, Some("z")), Some(0));
        assert_eq!(next_output(&outputs, &devices, None), Some(0));
    }

    #[test]
    fn next_output_follows_a_moved_output() {
        let outputs = [saved("a", "Casque"), saved("old", "Écran")];
        let devices = [device("a", "Casque", true), device("new", "Écran", true)];
        assert_eq!(next_output(&outputs, &devices, Some("new")), Some(0));
        assert_eq!(next_output(&outputs, &devices, Some("a")), Some(1));
    }

    #[test]
    fn next_output_skips_an_unplugged_twin_of_the_default() {
        let outputs = [saved("a", "Casque"), saved("b", "Casque"), saved("c", "Haut-parleurs")];
        let devices = [
            device("a", "Casque", true),
            device("b", "Casque", false),
            device("c", "Haut-parleurs", true),
        ];
        assert_eq!(next_output(&outputs, &devices, Some("a")), Some(2));
    }

    #[test]
    fn next_output_stays_on_the_only_connected_one() {
        let outputs = [saved("a", "Casque"), saved("b", "Écran")];
        let devices = [device("a", "Casque", true)];
        assert_eq!(next_output(&outputs, &devices, Some("a")), Some(0));
        assert_eq!(next_output(&outputs, &[], Some("a")), None);
        assert_eq!(next_output(&[], &devices, Some("a")), None);
    }
}
