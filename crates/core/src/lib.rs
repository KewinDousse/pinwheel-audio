//! Platform-independent logic of Pinwheel.
//!
//! Everything here builds and is tested on any OS; the Windows app wires it
//! to the system APIs.

pub mod devices;
pub mod hotkey;
pub mod osd;
pub mod settings;
pub mod status;
pub mod volume;
