//! The Windows accent colour, for the display's gauge.

use pinwheel_core::osd::{DEFAULT_ACCENT, Rgb};
use windows::UI::ViewManagement::{UIColorType, UISettings};

/// Read at each display, so a change in Windows applies at the next one.
/// `AccentLight2` is the shade Windows 11 uses for controls on dark surfaces.
pub fn current() -> Rgb {
    let color = UISettings::new().and_then(|settings| settings.GetColorValue(UIColorType::AccentLight2));
    match color {
        Ok(color) => [color.R, color.G, color.B],
        Err(e) => {
            log::debug!("reading the accent colour failed: {e}");
            DEFAULT_ACCENT
        }
    }
}
