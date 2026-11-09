//! Look of the settings window, after the Windows 11 Settings app: Segoe UI,
//! cards that fold open on a plain background, controls in the accent colour.

use crate::accent;
use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, RichText, Stroke, StrokeKind,
    TextStyle, Theme, Visuals,
};
use eframe::epaint::text::VariationCoords;
use pinwheel_core::osd::Rgb;
use std::path::PathBuf;
use std::sync::Arc;

/// Family of titles and output names.
const SEMIBOLD: &str = "semibold";
const CARD_RADIUS: u8 = 6;
const CONTROL_RADIUS: u8 = 4;
/// Height of a [`fold_header`] holding one line, keycaps included.
pub const HEADER_ONE_LINE: f32 = 52.0;
/// Height of a [`fold_header`] holding two lines.
pub const HEADER_TWO_LINES: f32 = 60.0;

/// Colours of one theme, for what the egui visuals do not cover.
pub struct Palette {
    background: Color32,
    card: Color32,
    card_hover: Color32,
    card_stroke: Color32,
    pub text: Color32,
    pub secondary: Color32,
    control: Color32,
    control_hover: Color32,
    control_pressed: Color32,
    control_stroke: Color32,
    pub accent: Color32,
    on_accent: Color32,
    pub error: Color32,
    pub warning: Color32,
    warning_background: Color32,
}

impl Palette {
    fn dark(accent: Rgb) -> Self {
        Self {
            background: Color32::from_rgb(0x20, 0x20, 0x20),
            card: Color32::from_rgb(0x2B, 0x2B, 0x2B),
            card_hover: Color32::from_rgb(0x32, 0x32, 0x32),
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
        }
    }

    fn light(accent: Rgb) -> Self {
        Self {
            background: Color32::from_rgb(0xF3, 0xF3, 0xF3),
            card: Color32::from_rgb(0xFB, 0xFB, 0xFB),
            card_hover: Color32::from_rgb(0xF5, 0xF5, 0xF5),
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
            // A click on a card's title opens the card instead of selecting text.
            style.interaction.selectable_labels = false;
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
            spacing.slider_width = 160.0;
        });
    }

    pub fn palette(&self, ui: &egui::Ui) -> &Palette {
        if ui.visuals().dark_mode {
            &self.dark
        } else {
            &self.light
        }
    }
}

/// Segoe UI Variable on Windows 11, Segoe UI before it; egui's own fonts
/// stay as fallbacks.
fn fonts() -> FontDefinitions {
    let dir = PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into())).join("Fonts");
    let read = |name: &str| std::fs::read(dir.join(name)).ok();
    let mut fonts = FontDefinitions::default();
    let fallbacks = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

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

/// The window's one heading, above the outputs.
pub fn heading(ui: &mut egui::Ui, title: &str) {
    ui.add_space(12.0);
    ui.label(RichText::new(title).text_style(TextStyle::Heading));
    ui.add_space(4.0);
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

/// Text in the accent colour, such as a selected state.
pub fn accented(palette: &Palette, text: &str) -> RichText {
    RichText::new(text).small().color(palette.accent)
}

/// A frameless button in the error colour, for removing something.
pub fn remove_button(ui: &mut egui::Ui, palette: &Palette, text: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(text).color(palette.error)).frame(false))
}

/// A card that folds: [`fold_header`] first, then its rows when open.
/// `reveal` scrolls the whole card into view.
pub fn fold_card<R>(ui: &mut egui::Ui, palette: &Palette, reveal: bool, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let card = egui::Frame::new()
        .fill(palette.card)
        .stroke(Stroke::new(1.0, palette.card_stroke))
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            add(ui)
        });
    if reveal {
        card.response.scroll_to_me(None);
    }
    card.inner
}

/// The always visible line of a [`fold_card`]: `title` on the left, laid out
/// left to right and centred (wrap two lines in `ui.vertical`), `controls`
/// then a chevron on the right, laid out right to left. A click outside the
/// controls returns `true`, to open or close it. `name` is what screen
/// readers announce for it, `height` one of [`HEADER_ONE_LINE`] and
/// [`HEADER_TWO_LINES`].
///
/// The line has a fixed height so that its click area is known before its
/// content is laid out: allocated first, it stays under the controls, and it
/// follows the card at once when another card opens above it.
pub fn fold_header<R>(
    ui: &mut egui::Ui,
    palette: &Palette,
    open: bool,
    name: &str,
    height: f32,
    title: impl FnOnce(&mut egui::Ui),
    controls: impl FnOnce(&mut egui::Ui) -> R,
) -> (bool, R) {
    let size = egui::vec2(ui.available_width(), height);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::DisclosureTriangle, ui.is_enabled(), name));
    if response.hovered() && ui.is_enabled() {
        let bottom = if open { 0 } else { CARD_RADIUS };
        let radius = CornerRadius {
            nw: CARD_RADIUS,
            ne: CARD_RADIUS,
            sw: bottom,
            se: bottom,
        };
        ui.painter().rect_filled(rect, radius, palette.card_hover);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let inner = rect.shrink2(egui::vec2(16.0, 10.0));
    let mut content = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    content.spacing_mut().item_spacing.y = 2.0;
    let controls = egui::containers::Sides::new()
        .height(inner.height())
        .shrink_left()
        .truncate()
        .show(&mut content, title, |ui| {
            chevron(ui, palette, open);
            controls(ui)
        })
        .1;
    (response.clicked(), controls)
}

/// A part of an open [`fold_card`], under a thin line.
pub fn fold_body<R>(ui: &mut egui::Ui, palette: &Palette, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    line(ui, palette);
    egui::Frame::new()
        .inner_margin(Margin::symmetric(16, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// A setting inside an open [`fold_card`]: see [`labelled`].
pub fn fold_row<R>(
    ui: &mut egui::Ui,
    palette: &Palette,
    title: &str,
    help: Option<RichText>,
    control: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    fold_body(ui, palette, |ui| labelled(ui, title, help, control))
}

/// A message across a [`fold_card`], tinted by its `color`.
pub fn fold_message(ui: &mut egui::Ui, palette: &Palette, text: &str, color: Color32) {
    line(ui, palette);
    egui::Frame::new()
        .fill(color.gamma_multiply(0.14))
        .inner_margin(Margin::symmetric(16, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(text).color(color));
        });
}

/// Title and help on the left, `control` on the right.
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
            |ui| match help {
                Some(help) => {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(title);
                        ui.label(help);
                    });
                }
                // Alone, the title stays centred on the control.
                None => {
                    ui.label(title);
                }
            },
            control,
        )
        .1
}

/// Full-width line between the parts of a [`fold_card`].
fn line(ui: &mut egui::Ui, palette: &Palette) {
    let rect = ui.available_rect_before_wrap();
    ui.painter()
        .hline(rect.x_range(), rect.top(), Stroke::new(1.0, palette.card_stroke));
}

/// Points down when closed, up when open.
fn chevron(ui: &mut egui::Ui, palette: &Palette, open: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
    let c = rect.center();
    let (dx, dy) = (4.0, if open { -2.0 } else { 2.0 });
    let points = [
        c + egui::vec2(-dx, -dy),
        c + egui::vec2(0.0, dy),
        c + egui::vec2(dx, -dy),
    ];
    ui.painter().line(points.to_vec(), Stroke::new(1.2, palette.secondary));
}

/// Buttons side by side, one per choice, the chosen one in the accent colour.
/// Returns `true` when the choice changed.
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    palette: &Palette,
    value: &mut T,
    choices: &[(T, &str)],
) -> bool {
    let mut changed = false;
    ui.spacing_mut().item_spacing.x = 0.0;
    let last = choices.len().saturating_sub(1);
    let ordered: Vec<_> = if ui.layout().prefer_right_to_left() {
        choices.iter().enumerate().rev().collect()
    } else {
        choices.iter().enumerate().collect()
    };
    for (index, &(choice, label)) in ordered {
        let selected = *value == choice;
        let r = CONTROL_RADIUS;
        let radius = CornerRadius {
            nw: if index == 0 { r } else { 0 },
            sw: if index == 0 { r } else { 0 },
            ne: if index == last { r } else { 0 },
            se: if index == last { r } else { 0 },
        };
        let (fill, text) = if selected {
            (palette.accent, palette.on_accent)
        } else {
            (palette.control, palette.text)
        };
        let button = egui::Button::new(RichText::new(label).color(text))
            .fill(fill)
            .corner_radius(radius)
            .selected(selected);
        if ui.add(button).clicked() && !selected {
            *value = choice;
            changed = true;
        }
    }
    changed
}

/// What is left of a countdown, as a thin accent line across `ui`.
pub fn countdown(ui: &mut egui::Ui, palette: &Palette, remaining: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 2.0), egui::Sense::hover());
    let mut filled = rect;
    filled.set_width(rect.width() * remaining.clamp(0.0, 1.0));
    ui.painter().rect_filled(rect, 1.0, palette.control_stroke);
    ui.painter().rect_filled(filled, 1.0, palette.accent);
}

/// A box in the accent colour, for what waits on the user.
pub fn waiting_frame<R>(ui: &mut egui::Ui, palette: &Palette, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(palette.control)
        .stroke(Stroke::new(1.5, palette.accent))
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
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

/// A key of a shortcut, drawn as a keycap as high as a button. Its size is
/// fixed, so that it does not stretch to the height of its line.
pub fn keycap(ui: &mut egui::Ui, palette: &Palette, text: &str) {
    let color = if ui.is_enabled() {
        palette.text
    } else {
        palette.secondary
    };
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), TextStyle::Body.resolve(ui.style()), color);
    let size = egui::vec2(galley.size().x + 16.0, ui.spacing().interact_size.y);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, ui.is_enabled(), text));
    if ui.is_rect_visible(rect) {
        let stroke = Stroke::new(1.0, palette.control_stroke);
        let painter = ui.painter();
        painter.rect(
            rect,
            CornerRadius::same(CONTROL_RADIUS),
            palette.control,
            stroke,
            StrokeKind::Inside,
        );
        painter.galley(rect.center() - galley.size() / 2.0, galley, color);
    }
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
            painter.circle_filled(
                egui::pos2(rect.left() + radius, center_y),
                knob - 1.0,
                palette.secondary,
            );
        }
    }
    response
}
