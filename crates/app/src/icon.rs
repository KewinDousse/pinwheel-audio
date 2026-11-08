//! The images of the embedded `res/pinwheel.ico`, one per size.

use tiny_skia::Pixmap;

/// Each image of the icon file, which holds PNGs (see `tools/make-icon`).
pub fn images() -> Vec<Pixmap> {
    let ico: &[u8] = include_bytes!("../res/pinwheel.ico");
    let count = usize::from(u16::from_le_bytes([ico[4], ico[5]]));
    (0..count)
        .filter_map(|i| {
            let entry = ico.get(6 + 16 * i..22 + 16 * i)?;
            let size = u32::from_le_bytes(entry[8..12].try_into().ok()?) as usize;
            let offset = u32::from_le_bytes(entry[12..16].try_into().ok()?) as usize;
            Pixmap::decode_png(ico.get(offset..offset + size)?).ok()
        })
        .collect()
}
