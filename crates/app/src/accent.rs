//! The Windows accent colour, for the display's gauge and the settings window.

use pinwheel_core::osd::{DEFAULT_ACCENT, Rgb};
use windows::UI::ViewManagement::{UIColorType, UISettings};

/// The shade Windows 11 uses for controls on dark surfaces.
pub fn on_dark() -> Rgb {
    read(UIColorType::AccentLight2)
}

/// The shade Windows 11 uses for controls on light surfaces.
pub fn on_light() -> Rgb {
    read(UIColorType::AccentDark1)
}

fn read(kind: UIColorType) -> Rgb {
    let color = UISettings::new().and_then(|settings| settings.GetColorValue(kind));
    match color {
        Ok(color) => [color.R, color.G, color.B],
        Err(e) => {
            log::debug!("reading the accent colour failed: {e}");
            DEFAULT_ACCENT
        }
    }
}
