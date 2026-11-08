//! Matching a saved output with the outputs Windows currently reports.

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
}
