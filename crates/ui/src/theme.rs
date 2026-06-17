use egui::{Color32, Stroke, Visuals};

pub const BG_VOID: Color32 = Color32::from_rgb(7, 7, 56); // #070738
pub const BG_DEEP: Color32 = Color32::from_rgb(17, 17, 20); // #111114
pub const BG_PANEL: Color32 = Color32::from_rgb(23, 24, 29); // #17181D
pub const BG_SURFACE: Color32 = Color32::from_rgb(29, 31, 37); // #1D1F25
pub const BG_RAISED: Color32 = Color32::from_rgb(35, 38, 44);
pub const BG_CONTROL: Color32 = Color32::from_rgb(42, 46, 54); // #2A2E36 (Border color, used for controls too)

pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(30, 32, 40);
pub const BORDER_DEFAULT: Color32 = Color32::from_rgb(42, 46, 54); // #2A2E36
pub const BORDER_STRONG: Color32 = Color32::from_rgb(60, 65, 80);

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(216, 220, 229); // #D8DCE5
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(140, 148, 165);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(80, 88, 105);
pub const TEXT_INVERSE: Color32 = Color32::from_rgb(10, 12, 18);

pub const ACCENT_BLUE: Color32 = Color32::from_rgb(77, 132, 255); // #4D84FF
pub const ACCENT_BLUE_DIM: Color32 = Color32::from_rgb(30, 65, 130);
pub const ACCENT_GREEN: Color32 = Color32::from_rgb(60, 200, 80);
pub const ACCENT_AMBER: Color32 = Color32::from_rgb(220, 160, 50);
pub const ACCENT_RED: Color32 = Color32::from_rgb(220, 65, 55);
pub const ACCENT_PURPLE: Color32 = Color32::from_rgb(150, 80, 220);
pub const ACCENT_CYAN: Color32 = Color32::from_rgb(50, 190, 210);

pub const ORG_HERBIVORE: Color32 = Color32::from_rgb(60, 210, 80);
pub const ORG_CARNIVORE: Color32 = Color32::from_rgb(230, 65, 55);
pub const ORG_SCAVENGER: Color32 = Color32::from_rgb(50, 160, 230);
pub const ORG_UNKNOWN: Color32 = Color32::from_rgb(200, 0, 255);

pub fn apply_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    // Configure visuals
    let mut visuals = Visuals::dark();
    visuals.window_fill = BG_SURFACE;
    visuals.panel_fill = BG_PANEL;
    visuals.faint_bg_color = BG_DEEP; // UI shell main bg
    visuals.extreme_bg_color = BG_VOID; // World bg

    visuals.widgets.noninteractive.bg_fill = BG_PANEL;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);

    visuals.widgets.inactive.bg_fill = BG_CONTROL;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);

    visuals.widgets.hovered.bg_fill = BG_RAISED;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);

    visuals.widgets.active.bg_fill = BG_CONTROL;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);

    visuals.selection.bg_fill = ACCENT_BLUE_DIM;
    visuals.selection.stroke = Stroke::new(1.0, ACCENT_BLUE);

    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals.window_stroke = Stroke::new(1.0, BORDER_DEFAULT);

    style.visuals = visuals;

    // Configure text styles
    let mut text_styles = std::collections::BTreeMap::new();

    use egui::FontFamily::Proportional;
    use egui::TextStyle::*;

    text_styles.insert(Small, egui::FontId::new(10.0, Proportional));
    text_styles.insert(Body, egui::FontId::new(11.0, Proportional));
    text_styles.insert(Button, egui::FontId::new(11.0, Proportional));
    text_styles.insert(Heading, egui::FontId::new(13.0, Proportional));
    text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::new(11.0, egui::FontFamily::Monospace),
    );

    style.text_styles = text_styles;

    // Spacing
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.window_margin = egui::Margin::same(6.0);
    style.spacing.button_padding = egui::vec2(6.0, 4.0);

    ctx.set_style(style);
}
