//! Renders every display style into one PNG, to check the drawing without
//! Windows.
//!
//! `cargo run -p pinwheel-core --example osd-preview -- <out.png> <regular.ttf> [bold.ttf]`

use pinwheel_core::osd::{Content, Fonts, render};
use pinwheel_core::settings::OsdStyle;
use pinwheel_core::volume::VolumeState;
use std::path::PathBuf;
use tiny_skia::{Color, Pixmap, PixmapPaint, Transform};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [out, regular, rest @ ..] = args.as_slice() else {
        eprintln!("usage: osd-preview <out.png> <regular.ttf> [bold.ttf]");
        std::process::exit(2);
    };
    let bold: Vec<PathBuf> = rest.iter().map(PathBuf::from).collect();
    let fonts = Fonts::from_candidates(&[PathBuf::from(regular)], &bold);

    let contents = [
        Content::Volume(VolumeState {
            level: 0.4,
            muted: false,
        }),
        Content::Volume(VolumeState {
            level: 1.0,
            muted: false,
        }),
        Content::Volume(VolumeState {
            level: 0.3,
            muted: true,
        }),
        Content::Output("Casque (Realtek USB Audio)".into()),
        Content::Notice("Haut-parleurs : déconnectée".into()),
    ];
    let scale = 1.5;
    let images: Vec<Vec<Pixmap>> = OsdStyle::ALL
        .iter()
        .map(|&style| contents.iter().map(|c| render(c, style, scale, &fonts)).collect())
        .collect();

    let gap = 24;
    let column_width: Vec<u32> = (0..contents.len())
        .map(|i| images.iter().map(|row| row[i].width()).max().unwrap())
        .collect();
    let row_height: Vec<u32> = images
        .iter()
        .map(|row| row.iter().map(Pixmap::height).max().unwrap())
        .collect();
    let width = column_width.iter().sum::<u32>() + gap * (contents.len() as u32 + 1);
    let height = row_height.iter().sum::<u32>() + gap * (images.len() as u32 + 1);
    let mut sheet = Pixmap::new(width, height).unwrap();
    sheet.fill(Color::from_rgba8(90, 120, 160, 255));

    let mut y = gap;
    for (row, images) in images.iter().enumerate() {
        let mut x = gap;
        for (col, image) in images.iter().enumerate() {
            sheet.draw_pixmap(
                x as i32,
                y as i32,
                image.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
            x += column_width[col] + gap;
        }
        y += row_height[row] + gap;
    }
    sheet.save_png(out).expect("writing the PNG");
    println!("{out}");
}
