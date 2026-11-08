//! Draws the Pinwheel icon and writes it as a multi-size `.ico`.
//!
//! Run from the workspace root: `cargo run -p make-icon`.
//! The output is committed, so the Windows build needs no image tooling.

use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

const SIZES: [u32; 6] = [16, 20, 24, 32, 48, 256];
const OUTPUT: &str = "crates/app/res/pinwheel.ico";

fn main() {
    let pngs: Vec<(u32, Vec<u8>)> = SIZES
        .iter()
        .map(|&size| (size, draw(size).encode_png().expect("PNG encoding")))
        .collect();
    std::fs::write(OUTPUT, ico(&pngs)).expect("writing the icon");
    println!("{OUTPUT} written ({} sizes)", pngs.len());
}

/// The Pinwheel badge (see the branding kit): a black pin with four coloured
/// blades, drawn on a 100-unit grid then scaled. From 32 px up it sits on a
/// yellow tile; smaller sizes drop the tile and the inner ring to stay legible.
fn draw(size: u32) -> Pixmap {
    let mut pixmap = Pixmap::new(size, size).unwrap();
    let small = size <= 24;
    let scale = size as f32 / 100.0;
    let mut transform = Transform::from_scale(scale, scale);

    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };

    if !small {
        paint.set_color_rgba8(0xFF, 0xE6, 0x00, 0xFF);
        let tile = rounded_rect(0.0, 0.0, 100.0, 100.0, 22.0);
        pixmap.fill_path(&tile, &paint, FillRule::Winding, transform, None);
        transform = transform.pre_translate(10.0, 10.0).pre_scale(0.8, 0.8);
    }

    paint.set_color_rgba8(0x11, 0x11, 0x11, 0xFF);
    pixmap.fill_path(
        &circle(50.0, 50.0, if small { 49.0 } else { 46.0 }),
        &paint,
        FillRule::Winding,
        transform,
        None,
    );

    if !small {
        paint.set_color_rgba8(0xFF, 0xFF, 0xFF, 0xFF);
        let ring = Stroke {
            width: 3.0,
            ..Stroke::default()
        };
        pixmap.stroke_path(&circle(50.0, 50.0, 42.0), &paint, &ring, transform, None);
    }

    let blade = {
        let mut pb = PathBuilder::new();
        pb.move_to(50.0, 50.0);
        pb.line_to(50.0, 15.0);
        pb.cubic_to(68.0, 15.0, 80.0, 28.0, 80.0, 46.0);
        pb.close();
        pb.finish().unwrap()
    };
    let colours = [
        [0xFF, 0x2E, 0x88],
        [0xFF, 0xE6, 0x00],
        [0x00, 0xC2, 0xFF],
        [0xFF, 0xFF, 0xFF],
    ];
    for (quarter, [r, g, b]) in colours.into_iter().enumerate() {
        paint.set_color_rgba8(r, g, b, 0xFF);
        let rotation = Transform::from_rotate_at(90.0 * quarter as f32, 50.0, 50.0);
        pixmap.fill_path(&blade, &paint, FillRule::Winding, transform.pre_concat(rotation), None);
    }

    let hub = circle(50.0, 50.0, if small { 8.0 } else { 6.0 });
    paint.set_color_rgba8(0x11, 0x11, 0x11, 0xFF);
    pixmap.fill_path(&hub, &paint, FillRule::Winding, transform, None);
    paint.set_color_rgba8(0xFF, 0xFF, 0xFF, 0xFF);
    let hub_outline = Stroke {
        width: if small { 4.0 } else { 2.5 },
        ..Stroke::default()
    };
    pixmap.stroke_path(&hub, &paint, &hub_outline, transform, None);
    pixmap
}

fn circle(cx: f32, cy: f32, r: f32) -> tiny_skia::Path {
    PathBuilder::from_circle(cx, cy, r).unwrap()
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
