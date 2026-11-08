//! Draws the SoundDock icon and writes it as a multi-size `.ico`.
//!
//! Run from the workspace root: `cargo run -p make-icon`.
//! The output is committed, so the Windows build needs no image tooling.

use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

const SIZES: [u32; 6] = [16, 20, 24, 32, 48, 256];
const OUTPUT: &str = "crates/app/res/sounddock.ico";

fn main() {
    let pngs: Vec<(u32, Vec<u8>)> = SIZES
        .iter()
        .map(|&size| (size, draw(size).encode_png().expect("PNG encoding")))
        .collect();
    std::fs::write(OUTPUT, ico(&pngs)).expect("writing the icon");
    println!("{OUTPUT} written ({} sizes)", pngs.len());
}

/// Rounded blue tile with a white speaker and two sound waves, drawn on a
/// 64-unit grid then scaled.
fn draw(size: u32) -> Pixmap {
    let mut pixmap = Pixmap::new(size, size).unwrap();
    let scale = size as f32 / 64.0;
    let transform = Transform::from_scale(scale, scale);

    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };

    paint.set_color_rgba8(0x25, 0x63, 0xEB, 0xFF);
    let tile = rounded_rect(2.0, 2.0, 60.0, 60.0, 14.0);
    pixmap.fill_path(&tile, &paint, FillRule::Winding, transform, None);

    paint.set_color_rgba8(0xFF, 0xFF, 0xFF, 0xFF);
    let mut speaker = PathBuilder::new();
    speaker.move_to(12.0, 25.0);
    speaker.line_to(21.0, 25.0);
    speaker.line_to(32.0, 15.0);
    speaker.line_to(32.0, 49.0);
    speaker.line_to(21.0, 39.0);
    speaker.line_to(12.0, 39.0);
    speaker.close();
    pixmap.fill_path(&speaker.finish().unwrap(), &paint, FillRule::Winding, transform, None);

    // Small sizes keep a single, thicker wave to stay legible.
    let waves: &[(f32, f32)] = if size <= 24 {
        &[(9.0, 5.5)]
    } else {
        &[(8.0, 4.5), (16.0, 4.5)]
    };
    for &(radius, width) in waves {
        let mut arc = PathBuilder::new();
        let (cx, cy) = (34.0, 32.0);
        arc.move_to(cx + radius * 0.6, cy - radius * 0.8);
        arc.quad_to(cx + radius * 1.35, cy, cx + radius * 0.6, cy + radius * 0.8);
        let stroke = Stroke {
            width,
            line_cap: tiny_skia::LineCap::Round,
            ..Stroke::default()
        };
        pixmap.stroke_path(&arc.finish().unwrap(), &paint, &stroke, transform, None);
    }
    pixmap
}

fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> tiny_skia::Path {
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish().unwrap()
}

/// ICO container holding PNG images, which Windows reads since Vista.
fn ico(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (size, png) in images {
        let dim = if *size >= 256 { 0 } else { *size as u8 };
        out.extend_from_slice(&[dim, dim, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, png) in images {
        out.extend_from_slice(png);
    }
    out
}
