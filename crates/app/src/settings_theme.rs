//! Look of the settings window, after the Windows 11 Settings app: Segoe UI,
//! one card per setting on a plain background, controls in the accent colour.

use crate::accent;
use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, RichText, Shadow, Stroke,
    StrokeKind, TextStyle, Theme, Visuals,
};
use eframe::epaint::text::VariationCoords;
use pinwheel_core::osd::Rgb;
use std::path::PathBuf;
use std::sync::Arc;

/// Family of titles and output names.
const SEMIBOLD: &str = "semibold";
const CARD_RADIUS: u8 = 6;
const CONTROL_RADIUS: u8 = 4;

/// Colours of one theme, for what the egui visuals do not cover.
pub struct Palette {
    background: Color32,
    card: Color32,
    card_stroke: Color32,
    pub text: Color32,
    pub secondary: Color32,
    control: Color32,
    control_hover: Color32,
    control_pressed: Color32,
    control_stroke: Color32,
    accent: Color32,
    on_accent: Color32,
    pub error: Color32,
    pub warning: Color32,
    warning_background: Color32,
    pub success: Color32,
}

impl Palette {
    fn dark(accent: Rgb) -> Self {
        Self {
            background: Color32::from_rgb(0x20, 0x20, 0x20),
            card: Color32::from_rgb(0x2B, 0x2B, 0x2B),
            card_stroke: Color32::from_rgb(0x1D, 0x1D, 0x1D),
            text: Color32::WHITE,
            secondary: Color32::from_rgb(0xC5, 0xC5, 0xC5),
            control: Color32::from_rgb(0x37, 0x37, 0x37),
            control_hover: Color32::from_rgb(0x3E, 0x3E, 0x3E),
            control_pressed: Color32::from_rgb(0x32, 0x32, 0x32),
            control_stroke: Color32::from_rgb(0x46, 0x46, 0x46),
            accent: rgb(accent),
            on_accent: Color32::BLACK,
            error: Color32::from_rgb(0xFF, 0x99, 0xA4),
            warning: Color32::from_rgb(0xFC, 0xE1, 0x00),
            warning_background: Color32::from_rgb(0x43, 0x35, 0x19),
            success: Color32::from_rgb(0x6C, 0xCB, 0x5F),
        }
    }

    fn light(accent: Rgb) -> Self {
        Self {
            background: Color32::from_rgb(0xF3, 0xF3, 0xF3),
            card: Color32::from_rgb(0xFB, 0xFB, 0xFB),
            card_stroke: Color32::from_rgb(0xE5, 0xE5, 0xE5),
            text: Color32::from_rgb(0x1B, 0x1B, 0x1B),
            secondary: Color32::from_rgb(0x5F, 0x5F, 0x5F),
            control: Color32::from_rgb(0xFE, 0xFE, 0xFE),
            control_hover: Color32::from_rgb(0xF6, 0xF6, 0xF6),
            control_pressed: Color32::from_rgb(0xF0, 0xF0, 0xF0),
            control_stroke: Color32::from_rgb(0xD5, 0xD5, 0xD5),
            accent: rgb(accent),
            on_accent: Color32::WHITE,
            error: Color32::from_rgb(0xC4, 0x2B, 0x1C),
            warning: Color32::from_rgb(0x9D, 0x5D, 0x00),
            warning_background: Color32::from_rgb(0xFF, 0xF4, 0xCE),
            success: Color32::from_rgb(0x0F, 0x7B, 0x0F),
        }
    }

    fn visuals(&self, dark: bool) -> Visuals {
        let mut visuals = if dark { Visuals::dark() } else { Visuals::light() };
        let widget = |fill: Color32, stroke: Color32, text: Color32| egui::style::WidgetVisuals {
            bg_fill: fill,
            weak_bg_fill: fill,
            bg_stroke: Stroke::new(1.0, stroke),
            corner_radius: CornerRadius::same(CONTROL_RADIUS),
            fg_stroke: Stroke::new(1.0, text),
            expansion: 0.0,
        };
        visuals.widgets.noninteractive = widget(self.card, self.card_stroke, self.text);
        visuals.widgets.inactive = widget(self.control, self.control_stroke, self.text);
        visuals.widgets.hovered = widget(self.control_hover, self.control_stroke, self.text);
        visuals.widgets.active = widget(self.control_pressed, self.control_stroke, self.secondary);
        visuals.widgets.open = widget(self.control_pressed, self.control_stroke, self.text);
        visuals.selection.bg_fill = self.accent;
        visuals.selection.stroke = Stroke::new(1.0, self.on_accent);
        visuals.hyperlink_color = self.accent;
        visuals.weak_text_color = Some(self.secondary);
        visuals.warn_fg_color = self.warning;
        visuals.error_fg_color = self.error;
        visuals.panel_fill = self.background;
        visuals.window_fill = self.card;
        visuals.window_stroke = Stroke::new(1.0, self.control_stroke);
        visuals.window_corner_radius = CornerRadius::same(8);
        visuals.menu_corner_radius = CornerRadius::same(8);
        visuals.popup_shadow = Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 0,
            color: Color32::from_black_alpha(if dark { 110 } else { 40 }),
        };
        visuals.extreme_bg_color = self.background;
        visuals.slider_trailing_fill = true;
        visuals
    }
}

fn rgb([r, g, b]: Rgb) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Both palettes, with the accent Windows has when the window opens.
pub struct Themes {
    dark: Palette,
    light: Palette,
}

impl Themes {
    pub fn new() -> Self {
        Self {
            dark: Palette::dark(accent::on_dark()),
            light: Palette::light(accent::on_light()),
        }
    }

    pub fn install(&self, ctx: &egui::Context) {
        ctx.set_fonts(fonts());
        ctx.set_visuals_of(Theme::Dark, self.dark.visuals(true));
        ctx.set_visuals_of(Theme::Light, self.light.visuals(false));
        ctx.all_styles_mut(|style| {
            // Popups and menus show at once.
            style.animation_time = 0.0;
            style.text_styles = [
                (TextStyle::Body, FontId::proportional(14.0)),
                (TextStyle::Button, FontId::proportional(14.0)),
                (TextStyle::Small, FontId::proportional(12.0)),
                (TextStyle::Heading, FontId::new(15.0, FontFamily::Name(SEMIBOLD.into()))),
                (TextStyle::Monospace, FontId::monospace(13.0)),
            ]
            .into();
            let spacing = &mut style.spacing;
            spacing.item_spacing = egui::vec2(8.0, 6.0);
            spacing.button_padding = egui::vec2(11.0, 5.0);
            spacing.interact_size.y = 30.0;
            spacing.combo_width = 190.0;
            spacing.slider_width = 160.0;
            spacing.menu_margin = Margin::same(4);
        });
    }

    pub fn palette(&self, ui: &egui::Ui) -> &Palette {
        if ui.visuals().dark_mode { &self.dark } else { &self.light }
    }
}

/// Segoe UI Variable on Windows 11, Segoe UI before it; egui's own fonts
/// stay as fallbacks.
fn fonts() -> FontDefinitions {
    let dir = PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into())).join("Fonts");
    let read = |name: &str| std::fs::read(dir.join(name)).ok();
    let mut fonts = FontDefinitions::default();
    let fallbacks = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();

    let (regular, semibold) = if let Some(variable) = read("SegUIVar.ttf") {
        let mut semibold = FontData::from_owned(variable.clone());
        semibold.tweak.coords = VariationCoords::new([("wght", 600.0)]);
        (Some(FontData::from_owned(variable)), Some(semibold))
    } else {
        (
            read("segoeui.ttf").map(FontData::from_owned),
            read("seguisb.ttf").map(FontData::from_owned),
        )
    };
    let mut family = |name: &str, data: Option<FontData>| {
        let mut chain = fallbacks.clone();
        if let Some(data) = data {
            fonts.font_data.insert(name.into(), Arc::new(data));
            chain.insert(0, name.into());
        }
        chain
    };
    let regular_chain = family("segoe", regular);
    let mut semibold_chain = family("segoe-semibold", semibold);
    // Without a semibold face, titles fall back to the regular one.
    if semibold_chain == fallbacks {
        semibold_chain = regular_chain.clone();
    }
    fonts.families.insert(FontFamily::Proportional, regular_chain);
    fonts.families.insert(FontFamily::Name(SEMIBOLD.into()), semibold_chain);
    fonts
}

/// Space between sections, and the section's title.
pub fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.add_space(18.0);
    ui.label(RichText::new(title).text_style(TextStyle::Heading));
    ui.add_space(2.0);
}

/// Secondary text under a title.
pub fn help(palette: &Palette, text: &str) -> RichText {
    RichText::new(text).small().color(palette.secondary)
}

/// A message about a setting, in `color`.
pub fn note(text: &str, color: Color32) -> RichText {
    RichText::new(text).small().color(color)
}

pub fn strong(text: impl Into<String>) -> RichText {
    RichText::new(text).family(FontFamily::Name(SEMIBOLD.into()))
}

/// A card filling the width, as each group of settings sits on.
pub fn card<R>(ui: &mut egui::Ui, palette: &Palette, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(palette.card)
        .stroke(Stroke::new(1.0, palette.card_stroke))
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .inner_margin(Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// A setting on its card: title and description on the left, its control on the right.
pub fn setting_row<R>(
    ui: &mut egui::Ui,
    palette: &Palette,
    title: &str,
    help: Option<RichText>,
    control: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    card(ui, palette, |ui| labelled(ui, title, help, control))
}

/// Title and help on the left, `control` on the right, within a card.
/// The right side is laid out right to left: its first widget is the rightmost.
pub fn labelled<R>(
    ui: &mut egui::Ui,
    title: impl Into<egui::WidgetText>,
    help: Option<RichText>,
    control: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let height = if help.is_some() { 40.0 } else { 30.0 };
    egui::containers::Sides::new()
        .height(height)
        .shrink_left()
        .wrap()
        .show(
            ui,
            |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(title);
                    if let Some(help) = help {
                        ui.label(help);
                    }
                });
            },
            control,
        )
        .1
}

/// Thin line between the parts of a card.
pub fn divider(ui: &mut egui::Ui, palette: &Palette) {
    ui.add_space(4.0);
    let rect = ui.available_rect_before_wrap();
    let y = rect.top();
    ui.painter()
        .hline(rect.left() - 16.0..=rect.right() + 16.0, y, Stroke::new(1.0, palette.card_stroke));
    ui.add_space(5.0);
}

/// A banner across the top of the window, for what needs attention.
pub fn warning_banner(ui: &mut egui::Ui, palette: &Palette, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(palette.warning_background)
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .inner_margin(Margin::symmetric(16, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

/// A key of a shortcut, drawn as a keycap.
pub fn keycap(ui: &mut egui::Ui, palette: &Palette, text: &str, highlighted: bool) {
    let stroke = if highlighted { palette.accent } else { palette.control_stroke };
    egui::Frame::new()
        .fill(palette.control)
        .stroke(Stroke::new(1.0, stroke))
        .corner_radius(CornerRadius::same(CONTROL_RADIUS))
        .inner_margin(Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(text).color(palette.text));
        });
}

/// The Windows 11 on/off switch.
pub fn toggle(ui: &mut egui::Ui, palette: &Palette, on: &mut bool, label: &str) -> egui::Response {
    let size = egui::vec2(40.0, 20.0);
    let (rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Switch, ui.is_enabled(), *on, label));
    if ui.is_rect_visible(rect) {
        let radius = rect.height() / 2.0;
        let knob = if response.hovered() { 7.0 } else { 6.0 };
        let painter = ui.painter();
        let center_y = rect.center().y;
        if *on {
            painter.rect_filled(rect, radius, palette.accent);
            painter.circle_filled(egui::pos2(rect.right() - radius, center_y), knob, palette.on_accent);
        } else {
            painter.rect(
                rect,
                radius,
                Color32::TRANSPARENT,
                Stroke::new(1.0, palette.secondary),
                StrokeKind::Inside,
            );
            painter.circle_filled(egui::pos2(rect.left() + radius, center_y), knob - 1.0, palette.secondary);
        }
    }
    response
}
