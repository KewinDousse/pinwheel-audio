//! Drawing of the on-screen display into a premultiplied RGBA pixmap.
//!
//! Sizes are in logical pixels (96 DPI) and multiplied by the monitor scale.
//! The Windows app copies the pixmap into a layered window.

use crate::settings::OsdStyle;
use crate::volume::VolumeState;
use ab_glyph::{Font, FontVec, PxScale, ScaleFont, point};
use std::path::PathBuf;
use tiny_skia::{Color, FillRule, LineCap, Paint, Path, PathBuilder, Pixmap, Stroke, Transform};

/// What the display shows.
#[derive(Debug, Clone)]
pub enum Content {
    Volume(VolumeState),
    /// The output that just became the default.
    Output(String),
    /// A short notice, such as an output that is unplugged.
    Notice(String),
}

type Rgba = [u8; 4];
const BACKGROUND: Rgba = [28, 28, 28, 235];
const BORDER: Rgba = [255, 255, 255, 24];
const TRACK: Rgba = [255, 255, 255, 60];
const FILL: Rgba = [255, 255, 255, 255];
const FILL_MUTED: Rgba = [255, 255, 255, 110];
const TEXT: Rgba = [255, 255, 255, 255];

const TEXT_MAX_WIDTH: f32 = 480.0;

/// Fonts for the text. Without them, the display still shows its shapes.
pub struct Fonts {
    regular: Option<FontVec>,
    bold: Option<FontVec>,
}

impl Fonts {
    /// The first readable font of each list is used; `bold` falls back to
    /// the regular font.
    pub fn from_candidates(regular: &[PathBuf], bold: &[PathBuf]) -> Self {
        let first = |paths: &[PathBuf]| {
            paths.iter().find_map(|path| {
                std::fs::read(path)
                    .ok()
                    .and_then(|bytes| FontVec::try_from_vec(bytes).ok())
            })
        };
        let regular_font = first(regular);
        let bold_font = first(bold).or_else(|| first(regular));
        Self {
            regular: regular_font,
            bold: bold_font,
        }
    }

    pub fn has_text(&self) -> bool {
        self.regular.is_some()
    }
}

/// Draws `content` at `scale` (monitor DPI / 96).
pub fn render(content: &Content, style: OsdStyle, scale: f32, fonts: &Fonts) -> Pixmap {
    match content {
        Content::Volume(volume) => match style {
            OsdStyle::Bar => bar(*volume, scale),
            OsdStyle::Pill => pill(*volume, scale, fonts),
            OsdStyle::BigNumber => big_number(*volume, scale, fonts),
        },
        Content::Output(name) => label(name, true, style, scale, fonts),
        Content::Notice(text) => label(text, false, style, scale, fonts),
    }
}

/// A canvas of `width` × `height` logical pixels with the rounded background.
struct Canvas {
    pixmap: Pixmap,
    scale: f32,
}

impl Canvas {
    fn new(width: f32, height: f32, radius: f32, scale: f32) -> Self {
        let pixmap =
            Pixmap::new((width * scale).ceil() as u32, (height * scale).ceil() as u32).expect("non-empty display");
        let mut canvas = Self { pixmap, scale };
        canvas.fill(&rounded_rect(0.5, 0.5, width - 1.0, height - 1.0, radius - 0.5), BORDER);
        canvas.fill(
            &rounded_rect(1.0, 1.0, width - 2.0, height - 2.0, radius - 1.0),
            BACKGROUND,
        );
        canvas
    }

    fn transform(&self) -> Transform {
        Transform::from_scale(self.scale, self.scale)
    }

    fn fill(&mut self, path: &Path, color: Rgba) {
        let transform = self.transform();
        self.pixmap
            .fill_path(path, &paint(color), FillRule::Winding, transform, None);
    }

    fn stroke(&mut self, path: &Path, width: f32, color: Rgba) {
        let stroke = Stroke {
            width,
            line_cap: LineCap::Round,
            ..Stroke::default()
        };
        let transform = self.transform();
        self.pixmap.stroke_path(path, &paint(color), &stroke, transform, None);
    }

    /// Horizontal gauge from `x0` to `x1`, centred on `y`.
    fn gauge(&mut self, x0: f32, x1: f32, y: f32, volume: VolumeState) {
        let height = 4.0;
        self.fill(
            &rounded_rect(x0, y - height / 2.0, x1 - x0, height, height / 2.0),
            TRACK,
        );
        let filled = (x1 - x0) * volume.level.clamp(0.0, 1.0);
        if filled > 0.5 {
            let color = if volume.muted { FILL_MUTED } else { FILL };
            self.fill(
                &rounded_rect(x0, y - height / 2.0, filled.max(height), height, height / 2.0),
                color,
            );
        }
    }

    /// Speaker glyph of `size` centred vertically on `cy`; waves follow the
    /// level, a cross replaces them when muted.
    fn speaker(&mut self, x: f32, cy: f32, size: f32, volume: Option<VolumeState>) {
        let unit = size / 24.0;
        let at = |px: f32, py: f32| (x + px * unit, cy + (py - 12.0) * unit);
        let mut body = PathBuilder::new();
        for (i, &(px, py)) in [
            (2.0, 9.0),
            (6.0, 9.0),
            (11.0, 4.5),
            (11.0, 19.5),
            (6.0, 15.0),
            (2.0, 15.0),
        ]
        .iter()
        .enumerate()
        {
            let (sx, sy) = at(px, py);
            if i == 0 {
                body.move_to(sx, sy)
            } else {
                body.line_to(sx, sy)
            }
        }
        body.close();
        self.fill(&body.finish().unwrap(), TEXT);

        let width = 2.0 * unit;
        if volume.is_some_and(|v| v.muted) {
            for (a, b) in [((15.0, 9.0), (21.0, 15.0)), ((21.0, 9.0), (15.0, 15.0))] {
                let mut line = PathBuilder::new();
                let (ax, ay) = at(a.0, a.1);
                let (bx, by) = at(b.0, b.1);
                line.move_to(ax, ay);
                line.line_to(bx, by);
                self.stroke(&line.finish().unwrap(), width, TEXT);
            }
            return;
        }
        let waves = match volume.map(|v| v.level) {
            Some(level) if level <= 0.0 => 0,
            Some(level) if level < 0.5 => 1,
            _ => 2,
        };
        for radius in [4.0, 8.0].into_iter().take(waves) {
            let mut arc = PathBuilder::new();
            let (sx, sy) = at(12.0 + radius * 0.6, 12.0 - radius * 0.8);
            let (cx, cy2) = at(12.0 + radius * 1.35, 12.0);
            let (ex, ey) = at(12.0 + radius * 0.6, 12.0 + radius * 0.8);
            arc.move_to(sx, sy);
            arc.quad_to(cx, cy2, ex, ey);
            self.stroke(&arc.finish().unwrap(), width, TEXT);
        }
    }

    fn text(&mut self, font: Option<&FontVec>, size: f32, text: &str, x: f32, baseline: f32) {
        if let Some(font) = font {
            draw_text(
                &mut self.pixmap,
                font,
                size * self.scale,
                text,
                x * self.scale,
                baseline * self.scale,
                TEXT,
            );
        }
    }
}

fn bar(volume: VolumeState, scale: f32) -> Pixmap {
    let (width, height) = (300.0, 24.0);
    let mut canvas = Canvas::new(width, height, height / 2.0, scale);
    canvas.gauge(14.0, width - 14.0, height / 2.0, volume);
    canvas.pixmap
}

fn pill(volume: VolumeState, scale: f32, fonts: &Fonts) -> Pixmap {
    let (width, height) = (300.0, 52.0);
    let mut canvas = Canvas::new(width, height, height / 2.0, scale);
    let size = 17.0;
    let font = fonts.bold.as_ref();
    // Room for the widest label, so the gauge does not move with the value.
    let reserved = text_width(font, size, "100 %").max(text_width(font, size, "Muet"));
    canvas.speaker(18.0, height / 2.0, 22.0, Some(volume));
    canvas.gauge(54.0, width - 20.0 - reserved - 16.0, height / 2.0, volume);
    let percent = percent_text(volume);
    let text_width = text_width(font, size, &percent);
    canvas.text(
        fonts.bold.as_ref(),
        size,
        &percent,
        width - 20.0 - text_width,
        height / 2.0 + cap_offset(size),
    );
    canvas.pixmap
}

fn big_number(volume: VolumeState, scale: f32, fonts: &Fonts) -> Pixmap {
    let (width, height) = (180.0, 150.0);
    let mut canvas = Canvas::new(width, height, 22.0, scale);
    let baseline = 82.0;
    if volume.muted {
        let size = 34.0;
        let text_width = text_width(fonts.bold.as_ref(), size, "Muet");
        canvas.text(fonts.bold.as_ref(), size, "Muet", (width - text_width) / 2.0, baseline);
    } else {
        let number = format!("{}", (volume.level * 100.0).round() as i32);
        let (big, small) = (56.0, 24.0);
        let number_width = text_width(fonts.bold.as_ref(), big, &number);
        let sign_width = text_width(fonts.regular.as_ref(), small, "%");
        let x = (width - number_width - sign_width - 2.0) / 2.0;
        canvas.text(fonts.bold.as_ref(), big, &number, x, baseline);
        canvas.text(fonts.regular.as_ref(), small, "%", x + number_width + 2.0, baseline);
    }
    canvas.gauge(28.0, width - 28.0, 116.0, volume);
    canvas.pixmap
}

/// Output name or notice, with a speaker icon for outputs.
fn label(text: &str, with_icon: bool, style: OsdStyle, scale: f32, fonts: &Fonts) -> Pixmap {
    let height = if style == OsdStyle::Bar { 40.0 } else { 52.0 };
    let size = 15.0;
    let font = fonts.regular.as_ref();
    let text = truncate(font, size, text, TEXT_MAX_WIDTH);
    let icon = if with_icon { 20.0 + 10.0 } else { 0.0 };
    let padding = 20.0;
    let width = (padding * 2.0 + icon + text_width(font, size, &text)).max(160.0);
    let mut canvas = Canvas::new(width, height, height / 2.0, scale);
    let content_width = icon + text_width(font, size, &text);
    let x = (width - content_width) / 2.0;
    if with_icon {
        canvas.speaker(x, height / 2.0, 20.0, None);
    }
    canvas.text(font, size, &text, x + icon, height / 2.0 + cap_offset(size));
    canvas.pixmap
}

fn percent_text(volume: VolumeState) -> String {
    if volume.muted {
        "Muet".into()
    } else {
        format!("{} %", (volume.level * 100.0).round() as i32)
    }
}

/// Baseline offset that centres capital letters and digits on a line: their
/// height is about 0.7 em in Segoe UI and Arial.
fn cap_offset(size: f32) -> f32 {
    size * 0.35
}

fn text_width(font: Option<&FontVec>, size: f32, text: &str) -> f32 {
    let Some(font) = font else { return 0.0 };
    let scaled = font.as_scaled(PxScale::from(size));
    let mut width = 0.0;
    let mut previous = None;
    for c in text.chars() {
        let id = scaled.glyph_id(c);
        if let Some(prev) = previous {
            width += scaled.kern(prev, id);
        }
        width += scaled.h_advance(id);
        previous = Some(id);
    }
    width
}

fn truncate(font: Option<&FontVec>, size: f32, text: &str, max_width: f32) -> String {
    if text_width(font, size, text) <= max_width {
        return text.to_owned();
    }
    let mut chars: Vec<char> = text.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let candidate: String = chars.iter().collect::<String>() + "…";
        if text_width(font, size, &candidate) <= max_width {
            return candidate;
        }
    }
    "…".into()
}

/// Rasterises `text` with its baseline at `(x, baseline)`, in pixels.
fn draw_text(pixmap: &mut Pixmap, font: &FontVec, size: f32, text: &str, x: f32, baseline: f32, color: Rgba) {
    let scale = PxScale::from(size);
    let scaled = font.as_scaled(scale);
    let (width, height) = (pixmap.width() as i32, pixmap.height() as i32);
    let data = pixmap.data_mut();
    let mut caret = x;
    let mut previous = None;
    for c in text.chars() {
        let id = scaled.glyph_id(c);
        if let Some(prev) = previous {
            caret += scaled.kern(prev, id);
        }
        if let Some(outline) = font.outline_glyph(id.with_scale_and_position(scale, point(caret, baseline))) {
            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, coverage| {
                let px = bounds.min.x as i32 + gx as i32;
                let py = bounds.min.y as i32 + gy as i32;
                if (0..width).contains(&px) && (0..height).contains(&py) {
                    let offset = ((py * width + px) * 4) as usize;
                    blend(&mut data[offset..offset + 4], color, coverage);
                }
            });
        }
        caret += scaled.h_advance(id);
        previous = Some(id);
    }
}

/// Source-over of a straight-alpha colour onto a premultiplied pixel.
fn blend(pixel: &mut [u8], color: Rgba, coverage: f32) {
    let alpha = f32::from(color[3]) / 255.0 * coverage.clamp(0.0, 1.0);
    for channel in 0..3 {
        let src = f32::from(color[channel]) * alpha;
        pixel[channel] = (src + f32::from(pixel[channel]) * (1.0 - alpha)).round().min(255.0) as u8;
    }
    pixel[3] = (alpha * 255.0 + f32::from(pixel[3]) * (1.0 - alpha)).round().min(255.0) as u8;
}

fn paint(color: Rgba) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(color[0], color[1], color[2], color[3]));
    paint.anti_alias = true;
    paint
}

/// Rounded rectangle with circular corners (cubic Bézier approximation).
fn rounded_rect(x: f32, y: f32, w: f32, h: f32, radius: f32) -> Path {
    const KAPPA: f32 = 0.552_284_8;
    let r = radius.min(w / 2.0).min(h / 2.0).max(0.0);
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
    pb.finish().expect("valid rectangle")
}
