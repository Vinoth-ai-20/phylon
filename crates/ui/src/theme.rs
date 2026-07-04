//! Shared design tokens — fonts, spacing, and global style.
//!
//! Applied once at startup (see `crates/app/src/app.rs`) so every panel gets
//! consistent typography and padding, instead of each plugin picking its own
//! ad hoc numbers for `ui.add_space(...)`, button padding, etc.

use egui::{FontFamily, FontId, TextStyle};

/// Extra-small gap — between a label and an inline icon/badge.
pub const SPACE_XS: f32 = 4.0;
/// Small gap — between adjacent controls in a toolbar/row, and the default
/// padding inside a panel's content area.
pub const SPACE_SM: f32 = 8.0;
/// Medium gap — between sections within a panel.
pub const SPACE_MD: f32 = 12.0;
/// Large gap — between major panel regions.
pub const SPACE_LG: f32 = 16.0;

/// Padding applied inside every docked/floating panel's content area (below
/// the edge-to-edge chrome bar, which does not use this).
pub const PANEL_PADDING: f32 = SPACE_SM;

/// Height of the chrome bar (title + Detach/Close buttons) at the top of
/// every non-Viewport panel.
pub const CHROME_HEIGHT: f32 = 22.0;

/// Font family key for headings — IBM Plex Sans SemiBold.
const HEADING_FAMILY: &str = "IBMPlexSans-SemiBold";

/// Font size for section headings (`ui.heading()` / `TextStyle::Heading`).
pub const SIZE_HEADING: f32 = 18.0;
/// Font size for panel/window titles and CollapsingHeader-level
/// sub-sections — one step down from a heading, one step up from body text.
pub const SIZE_SUBHEADING: f32 = 14.0;
/// Font size for standard body text, data-grid rows, and interactive
/// options (buttons, toggles) — the app's default text size.
pub const SIZE_BODY: f32 = 13.0;
/// Font size for secondary/meta text — timestamps, counts, footers, hints.
pub const SIZE_SMALL: f32 = 11.0;

/// Registers the IBM Plex Sans (UI text) and IBM Plex Mono (tabular/numeric
/// readouts — status bar, Inspector stats) font families.
///
/// Call *before* `egui_remixicon::add_to_fonts`, so Plex Sans is tried first
/// in the Proportional family and the icon glyph font remains a fallback
/// after it (icons are drawn as inline glyphs mixed with Plex Sans text).
pub fn install_fonts(fonts: &mut egui::FontDefinitions) {
    fonts.font_data.insert(
        "IBMPlexSans-Regular".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf")),
    );
    fonts.font_data.insert(
        HEADING_FAMILY.to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/IBMPlexSans-SemiBold.ttf")),
    );
    fonts.font_data.insert(
        "IBMPlexMono-Regular".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf")),
    );

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "IBMPlexSans-Regular".to_owned());

    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "IBMPlexMono-Regular".to_owned());

    fonts.families.insert(
        FontFamily::Name(HEADING_FAMILY.into()),
        vec![HEADING_FAMILY.to_owned()],
    );
}

/// Applies global spacing + text-style tokens. Call once, after `set_fonts`.
pub fn apply_style(ctx: &egui::Context) {
    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(SPACE_SM, SPACE_XS + 2.0);
        style.spacing.window_margin = egui::Margin::same(SPACE_SM);
        style.spacing.button_padding = egui::vec2(SPACE_SM, SPACE_XS);
        style.spacing.indent = SPACE_MD;

        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(SIZE_HEADING, FontFamily::Name(HEADING_FAMILY.into())),
        );
        style.text_styles.insert(
            TextStyle::Body,
            FontId::new(SIZE_BODY, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(SIZE_BODY, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(SIZE_SMALL, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(SIZE_BODY, FontFamily::Monospace),
        );
    });
}
