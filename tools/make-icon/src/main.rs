//! Draws the Pinwheel icon and writes it as a multi-size `.ico`.
//!
//! Run from the workspace root: `cargo run -p make-icon`.
//! The output is committed, so the Windows build needs no image tooling.

use tiny_skia::{FillRule, Mask, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

const SIZES: [u32; 6] = [16, 20, 24, 32, 48, 256];
const OUTPUT: &str = "crates/app/res/pinwheel.ico";

const MAGENTA: [u8; 3] = [0xFF, 0x2E, 0x88];
const WHITE: [u8; 3] = [0xFF, 0xFF, 0xFF];
const YELLOW: [u8; 3] = [0xFF, 0xE6, 0x00];

fn main() {
    let pngs: Vec<(u32, Vec<u8>)> = SIZES
        .iter()
        .map(|&size| (size, draw(size).encode_png().expect("PNG encoding")))
        .collect();
    std::fs::write(OUTPUT, ico(&pngs)).expect("writing the icon");
    println!("{OUTPUT} written ({} sizes)", pngs.len());
}

/// One drawing of the crossing, on a 100-unit grid.
struct Crossing {
    /// Stripe positions along each diagonal, in steps from the centre.
    stripes: &'static [f32],
    step: f32,
    stripe_width: f32,
    /// Length of a stripe, across its diagonal.
    band: f32,
    hub_radius: f32,
    hub_ring: f32,
}

/// Fewer, thicker stripes as the icon shrinks, so that they stay apart.
fn crossing(size: u32) -> Crossing {
    match size {
        0..=16 => Crossing {
            stripes: &[-2.0, 2.0],
            step: 23.0,
            stripe_width: 17.0,
            band: 34.0,
            hub_radius: 15.0,
            hub_ring: 8.0,
        },
        17..=24 => Crossing {
            stripes: &[-2.0, -1.0, 1.0, 2.0],
            step: 21.0,
            stripe_width: 13.0,
            band: 30.0,
            hub_radius: 13.0,
            hub_ring: 7.0,
        },
        _ => Crossing {
            stripes: &[-3.0, -2.0, -1.0, 1.0, 2.0, 3.0],
            step: 15.0,
            stripe_width: 9.0,
            band: 26.0,
            hub_radius: 11.0,
            hub_ring: 6.0,
        },
    }
}

/// The Pinwheel icon: a diagonal pedestrian crossing, Shibuya style, in
/// white on a magenta tile, with a yellow hub.
fn draw(size: u32) -> Pixmap {
    let mut pixmap = Pixmap::new(size, size).unwrap();
    let scale = size as f32 / 100.0;
    let transform = Transform::from_scale(scale, scale);
    let shape = crossing(size);
    let tile = rounded_rect(0.0, 0.0, 100.0, 100.0, 22.0);
    let mut clip = Mask::new(size, size).unwrap();
    clip.fill_path(&tile, FillRule::Winding, true, transform);

    pixmap.fill_path(&tile, &paint(MAGENTA), FillRule::Winding, transform, None);
    let stripes = {
        let mut pb = PathBuilder::new();
        for k in shape.stripes {
            let x = 50.0 - shape.stripe_width / 2.0 + k * shape.step;
            let rect = Rect::from_xywh(x, 50.0 - shape.band / 2.0, shape.stripe_width, shape.band).unwrap();
            pb.push_rect(rect);
        }
        pb.finish().unwrap()
    };
    for angle in [45.0, -45.0] {
        let rotation = Transform::from_rotate_at(angle, 50.0, 50.0);
        pixmap.fill_path(
            &stripes,
            &paint(WHITE),
            FillRule::Winding,
            transform.pre_concat(rotation),
            Some(&clip),
        );
    }

    let hub = PathBuilder::from_circle(50.0, 50.0, shape.hub_radius).unwrap();
    pixmap.fill_path(&hub, &paint(YELLOW), FillRule::Winding, transform, Some(&clip));
    let ring = Stroke {
        width: shape.hub_ring,
        ..Stroke::default()
    };
    pixmap.stroke_path(&hub, &paint(MAGENTA), &ring, transform, Some(&clip));
    pixmap
}

fn paint(colour: [u8; 3]) -> Paint<'static> {
    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    paint.set_color_rgba8(colour[0], colour[1], colour[2], 0xFF);
    paint
}

/// Rounded rectangle with circular corners (cubic Bézier approximation).
fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> tiny_skia::Path {
    const KAPPA: f32 = 0.552_284_8;
    let k = r * KAPPA;
    let (right, bottom) = (x + w, y + h);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(right - r, y);
    pb.cubic_to(right - r + k, y, right, y + r - k, right, y + r);
    pb.line_to(right, bottom - r);
    pb.cubic_to(right, bottom - r + k, right - r + k, bottom, right - r, bottom);
    pb.line_to(x + r, bottom);
    pb.cubic_to(x + r - k, bottom, x, bottom - r + k, x, bottom - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
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
