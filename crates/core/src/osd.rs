//! Drawing of the on-screen display into a premultiplied RGBA pixmap.
//!
//! Sizes are in logical pixels (96 DPI) and multiplied by the monitor scale.
//! The Windows app copies the pixmap into a layered window. The look follows
//! the Windows 11 flyouts: dark surface, 8 px corners, soft shadow, and a
//! gauge in the system accent colour.

use crate::i18n::{Lang, Texts};
use crate::settings::OsdStyle;
use crate::volume::VolumeState;
use ab_glyph::{Font, FontVec, PxScale, ScaleFont, point};
use std::path::PathBuf;
use tiny_skia::{
    Color, FillRule, FilterQuality, LineCap, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform,
};

/// What the display shows.
#[derive(Debug, Clone)]
pub enum Content {
    Volume(VolumeState),
    /// The output that just became the default.
    Output(String),
    /// A short notice, such as an output that is unplugged.
    Notice(String),
    /// Pinwheel just started, with the app icon in its sizes.
    Started {
        text: String,
        icons: Vec<Pixmap>,
    },
}

type Rgba = [u8; 4];
/// An opaque colour, such as the Windows accent.
pub type Rgb = [u8; 3];

/// Gauge colour when Windows gives no accent colour.
pub const DEFAULT_ACCENT: Rgb = [0x4C, 0xC2, 0xFF];

/// Room around the panel for its shadow, on every side, in logical pixels.
/// The app subtracts it when placing the window.
pub const SHADOW_MARGIN: f32 = 12.0;
const SHADOW_OFFSET_Y: f32 = 2.0;
const SHADOW_BLUR: f32 = 10.0;
const SHADOW_LAYER_ALPHA: u8 = 6;

const RADIUS: f32 = 8.0;
const BACKGROUND: Rgba = [44, 44, 44, 250];
const BORDER: Rgba = [255, 255, 255, 20];
const TRACK: Rgba = [255, 255, 255, 72];
const MUTED_ALPHA: u8 = 100;
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

/// What the display needs besides its content.
#[derive(Debug, Clone, Copy)]
pub struct Look {
    pub style: OsdStyle,
    /// Monitor DPI / 96.
    pub scale: f32,
    pub accent: Rgb,
    pub lang: Lang,
}

/// Draws `content`, with [`SHADOW_MARGIN`] of shadow around the panel.
pub fn render(content: &Content, look: Look, fonts: &Fonts) -> Pixmap {
    let texts = look.lang.texts();
    match content {
        Content::Volume(volume) => match look.style {
            OsdStyle::Bar => bar(*volume, look),
            OsdStyle::Pill => pill(*volume, look, fonts, texts),
            OsdStyle::BigNumber => big_number(*volume, look, fonts, texts),
        },
        Content::Output(name) => label(name, Icon::Speaker, look, fonts),
        Content::Notice(text) => label(text, Icon::None, look, fonts),
        Content::Started { text, icons } => label(text, Icon::Image(icons), look, fonts),
    }
}

/// A panel of `width` × `height` logical pixels on its shadow. Drawing
/// coordinates start at the panel's top-left corner.
struct Canvas {
    pixmap: Pixmap,
    scale: f32,
    accent: Rgb,
}

impl Canvas {
    fn new(width: f32, height: f32, look: Look) -> Self {
        let outer = |size: f32| ((size + 2.0 * SHADOW_MARGIN) * look.scale).ceil() as u32;
        let pixmap = Pixmap::new(outer(width), outer(height)).expect("non-empty display");
        let mut canvas = Self {
            pixmap,
            scale: look.scale,
            accent: look.accent,
        };
        canvas.shadow(width, height);
        canvas.fill(&rounded_rect(0.5, 0.5, width - 1.0, height - 1.0, RADIUS - 0.5), BORDER);
        canvas.fill(
            &rounded_rect(1.0, 1.0, width - 2.0, height - 2.0, RADIUS - 1.0),
            BACKGROUND,
        );
        canvas
    }

    /// Stacked translucent outlines, each one pixel smaller: tiny-skia has
    /// no blur, and the panel covers all but the fading edge.
    fn shadow(&mut self, width: f32, height: f32) {
        let color = [0, 0, 0, SHADOW_LAYER_ALPHA];
        for step in 0..SHADOW_BLUR as u32 {
            let grow = SHADOW_BLUR - step as f32;
            self.fill(
                &rounded_rect(
                    -grow,
                    SHADOW_OFFSET_Y - grow,
                    width + 2.0 * grow,
                    height + 2.0 * grow,
                    RADIUS + grow,
                ),
                color,
            );
        }
    }

    fn transform(&self) -> Transform {
        Transform::from_translate(SHADOW_MARGIN, SHADOW_MARGIN).post_scale(self.scale, self.scale)
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
            let [r, g, b] = self.accent;
            let color = [r, g, b, if volume.muted { MUTED_ALPHA } else { 255 }];
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

    /// Draws the image of `icons` that best fits a `size` square at `(x, top)`.
    fn image(&mut self, icons: &[Pixmap], x: f32, top: f32, size: f32) {
        let pixels = size * self.scale;
        // The smallest image at least as large, so it is only ever scaled down a little.
        let best = icons
            .iter()
            .filter(|icon| icon.width() as f32 >= pixels)
            .min_by_key(|icon| icon.width())
            .or_else(|| icons.iter().max_by_key(|icon| icon.width()));
        let Some(icon) = best else { return };
        let factor = pixels / icon.width() as f32;
        let transform = Transform::from_scale(factor, factor)
            .post_translate((x + SHADOW_MARGIN) * self.scale, (top + SHADOW_MARGIN) * self.scale);
        let paint = PixmapPaint {
            quality: FilterQuality::Bicubic,
            ..PixmapPaint::default()
        };
        self.pixmap.draw_pixmap(0, 0, icon.as_ref(), &paint, transform, None);
    }

    fn text(&mut self, font: Option<&FontVec>, size: f32, text: &str, x: f32, baseline: f32) {
        if let Some(font) = font {
            draw_text(
                &mut self.pixmap,
                font,
                size * self.scale,
                text,
                (x + SHADOW_MARGIN) * self.scale,
                (baseline + SHADOW_MARGIN) * self.scale,
                TEXT,
            );
        }
    }
}

fn bar(volume: VolumeState, look: Look) -> Pixmap {
    let (width, height) = (300.0, 24.0);
    let mut canvas = Canvas::new(width, height, look);
    canvas.gauge(14.0, width - 14.0, height / 2.0, volume);
    canvas.pixmap
}

fn pill(volume: VolumeState, look: Look, fonts: &Fonts, texts: &Texts) -> Pixmap {
    let (width, height) = (300.0, 52.0);
    let mut canvas = Canvas::new(width, height, look);
    let size = 17.0;
    let font = fonts.bold.as_ref();
    // Room for the widest label, so the gauge does not move with the value.
    let reserved = text_width(font, size, &(texts.percent)(100)).max(text_width(font, size, texts.muted));
    canvas.speaker(18.0, height / 2.0, 22.0, Some(volume));
    canvas.gauge(54.0, width - 20.0 - reserved - 16.0, height / 2.0, volume);
    let percent = percent_text(volume, texts);
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

fn big_number(volume: VolumeState, look: Look, fonts: &Fonts, texts: &Texts) -> Pixmap {
    let (width, height) = (180.0, 150.0);
    let mut canvas = Canvas::new(width, height, look);
    let baseline = 82.0;
    if volume.muted {
        let size = 34.0;
        let text_width = text_width(fonts.bold.as_ref(), size, texts.muted);
        canvas.text(
            fonts.bold.as_ref(),
            size,
            texts.muted,
            (width - text_width) / 2.0,
            baseline,
        );
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

/// What goes before the text of a label.
enum Icon<'a> {
    None,
    Speaker,
    Image(&'a [Pixmap]),
}

/// Output name or notice, with its icon.
fn label(text: &str, icon: Icon, look: Look, fonts: &Fonts) -> Pixmap {
    let height = if look.style == OsdStyle::Bar { 40.0 } else { 52.0 };
    let size = 15.0;
    let font = fonts.regular.as_ref();
    let text = truncate(font, size, text, TEXT_MAX_WIDTH);
    let icon_size = 20.0;
    let icon_room = match icon {
        Icon::None => 0.0,
        Icon::Speaker | Icon::Image(_) => icon_size + 10.0,
    };
    let padding = 20.0;
    let width = (padding * 2.0 + icon_room + text_width(font, size, &text)).max(160.0);
    let mut canvas = Canvas::new(width, height, look);
    let content_width = icon_room + text_width(font, size, &text);
    let x = (width - content_width) / 2.0;
    match icon {
        Icon::None => {}
        Icon::Speaker => canvas.speaker(x, height / 2.0, icon_size, None),
        Icon::Image(icons) => canvas.image(icons, x, (height - icon_size) / 2.0, icon_size),
    }
    canvas.text(font, size, &text, x + icon_room, height / 2.0 + cap_offset(size));
    canvas.pixmap
}

fn percent_text(volume: VolumeState, texts: &Texts) -> String {
    if volume.muted {
        texts.muted.into()
    } else {
        (texts.percent)((volume.level * 100.0).round() as i32)
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

#[cfg(test)]
mod tests {
    use super::*;

    const ACCENT: Rgb = [200, 40, 120];

    fn look(scale: f32) -> Look {
        Look {
            style: OsdStyle::Bar,
            scale,
            accent: ACCENT,
            lang: Lang::English,
        }
    }

    fn no_fonts() -> Fonts {
        Fonts::from_candidates(&[], &[])
    }

    fn volume(level: f32, muted: bool) -> Content {
        Content::Volume(VolumeState { level, muted })
    }

    /// Straight-alpha colour of the pixel at logical `(x, y)` of the panel.
    fn pixel(pixmap: &Pixmap, scale: f32, x: f32, y: f32) -> Rgba {
        let px = ((x + SHADOW_MARGIN) * scale) as u32;
        let py = ((y + SHADOW_MARGIN) * scale) as u32;
        let c = pixmap.pixel(px, py).unwrap().demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }

    #[test]
    fn the_shadow_margin_surrounds_the_panel() {
        for scale in [1.0, 1.5] {
            let pixmap = render(&volume(0.4, false), look(scale), &no_fonts());
            assert_eq!(pixmap.width(), ((300.0 + 2.0 * SHADOW_MARGIN) * scale).ceil() as u32);
            assert_eq!(pixmap.height(), ((24.0 + 2.0 * SHADOW_MARGIN) * scale).ceil() as u32);
            assert_eq!(
                pixmap.pixel(0, 0).unwrap().alpha(),
                0,
                "the corner is beyond the shadow"
            );
        }
    }

    #[test]
    fn the_gauge_takes_the_accent_colour() {
        let pixmap = render(&volume(0.5, false), look(1.0), &no_fonts());
        assert_eq!(pixel(&pixmap, 1.0, 40.0, 12.0), [ACCENT[0], ACCENT[1], ACCENT[2], 255]);
        let [r, g, b, _] = pixel(&pixmap, 1.0, 250.0, 12.0);
        assert!(
            r == g && g == b && r > BACKGROUND[0],
            "grey track beyond the level: {r} {g} {b}"
        );
    }

    #[test]
    fn a_muted_gauge_fades_the_accent() {
        let muted = render(&volume(0.5, true), look(1.0), &no_fonts());
        let [r, g, b, _] = pixel(&muted, 1.0, 40.0, 12.0);
        let [ar, ag, ab] = ACCENT;
        assert!(
            r < ar && g > ag && b < ab,
            "accent blended with the background: {r} {g} {b}"
        );
    }

    #[test]
    fn the_started_notice_draws_the_closest_larger_icon() {
        let solid = |size, color: Color| {
            let mut icon = Pixmap::new(size, size).unwrap();
            icon.fill(color);
            icon
        };
        let content = Content::Started {
            text: String::new(),
            icons: vec![
                solid(16, Color::from_rgba8(255, 0, 0, 255)),
                solid(32, Color::from_rgba8(0, 255, 0, 255)),
            ],
        };
        // 20 logical pixels at 150 % need 30: the 32 px image, not the 16 px one.
        let pixmap = render(&content, look(1.5), &no_fonts());
        // Without text, the 20 px icon is centred in the 160 × 40 panel.
        assert_eq!(pixel(&pixmap, 1.5, 80.0, 20.0), [0, 255, 0, 255]);
    }
}
