//! Shared design tokens — fonts, spacing, color, and global style.
//!
//! Applied once at startup (see `crates/app/src/app.rs`) so every panel gets
//! consistent typography and padding, instead of each plugin picking its own
//! ad hoc numbers for `ui.add_space(...)`, button padding, etc.
//!
//! Every token here is documented (with the reasoning behind its value) in
//! `docs/design/` — that's the source of truth; this module is where those
//! decisions become code. See `docs/design/design_system.md` for the index.

use egui::{Color32, FontFamily, FontId, TextStyle};

// ─── Spacing (docs/design/spacing.md) ──────────────────────────────────────

/// Extra-small gap — between a label and an inline icon/badge.
pub const SPACE_XS: f32 = 4.0;
/// Small gap — between adjacent controls in a toolbar/row, and the default
/// padding inside a panel's content area.
pub const SPACE_SM: f32 = 8.0;
/// Medium gap — between sections within a panel.
pub const SPACE_MD: f32 = 12.0;
/// Large gap — between major panel regions.
pub const SPACE_LG: f32 = 16.0;
/// Extra-large gap — the gutter between docked panels (Sidebar↔Viewport,
/// Viewport↔Neural Viewer).
pub const SPACE_XL: f32 = 24.0;
/// Dialog/modal outer padding.
pub const SPACE_XXL: f32 = 32.0;
/// Empty-state vertical centering offset.
pub const SPACE_XXXL: f32 = 48.0;

/// Padding applied inside every docked/floating panel's content area (below
/// the edge-to-edge chrome bar, which does not use this).
pub const PANEL_PADDING: f32 = SPACE_SM;

/// Height of the chrome bar (title + Detach/Close buttons) at the top of
/// every non-Viewport panel.
pub const CHROME_HEIGHT: f32 = 22.0;

// ─── Radius / elevation (docs/design/spacing.md) ───────────────────────────

/// Corner radius for tooltips and graph canvases (Neural Viewer, Metrics
/// plot backgrounds).
pub const RADIUS_TIGHT: f32 = 4.0;
/// Corner radius for floating windows, toasts, and context menus.
pub const RADIUS_STD: f32 = 8.0;
/// Corner radius for dialogs/modals.
pub const RADIUS_LOOSE: f32 = 12.0;

// ─── Icon sizes (docs/design/iconography.md) ───────────────────────────────

/// Icon size for glyphs set inline with body text (chrome-bar Close/Detach).
pub const ICON_SM: f32 = 14.0;
/// Icon size for toolbar buttons (step, restart, screenshot, record).
pub const ICON_MD: f32 = 16.0;
/// Icon size for the activity bar / sidebar tab icons.
pub const ICON_LG: f32 = 20.0;
/// Icon size for the splash/main-menu screen only — a deliberately different
/// context from workbench chrome, not part of the standard chrome scale.
pub const ICON_XL: f32 = 40.0;

// ─── Color (docs/design/colors.md) ─────────────────────────────────────────

/// Panel/window chrome background — every docked/floating pane's opaque
/// backdrop. (Previously `layout::PANEL_BG`; relocated here so `theme.rs` is
/// the one place a color is defined.)
pub const CHROME_BG: Color32 = Color32::from_rgb(24, 24, 28);

/// The viewport's baseline tone, independent of the day/night clear-color
/// animation layered on top by the renderer — gives the simulation canvas a
/// fixed, distinguishable tone from the surrounding UI chrome instead of
/// both reading as undifferentiated near-black.
pub const VIEWPORT_FLOOR: Color32 = Color32::from_rgb(10, 14, 20);

/// The one interactive accent color app-wide (active tab underline, focus
/// ring, primary button) — deliberately distinct from every diet color and
/// every semantic color below.
pub const ACCENT: Color32 = Color32::from_rgb(63, 182, 174);
/// Readable ink color for text drawn on top of `ACCENT`/`ACCENT_SOFT`.
pub const ACCENT_INK: Color32 = Color32::from_rgb(207, 243, 238);
/// Muted background tint for accent-colored surfaces.
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(27, 53, 50);

/// Success state — toasts, confirmations.
pub const GOOD: Color32 = Color32::from_rgb(111, 190, 139);
/// Muted background tint for `GOOD`.
pub const GOOD_SOFT: Color32 = Color32::from_rgb(30, 51, 39);
/// Caution state — non-blocking warnings.
pub const WARN: Color32 = Color32::from_rgb(224, 172, 92);
/// Muted background tint for `WARN`.
pub const WARN_SOFT: Color32 = Color32::from_rgb(58, 46, 23);
/// Error/blocking state — failures, destructive-action confirmation.
pub const BAD: Color32 = Color32::from_rgb(225, 126, 116);
/// Muted background tint for `BAD`.
pub const BAD_SOFT: Color32 = Color32::from_rgb(58, 33, 30);

/// The Close button color, everywhere a panel/window can be closed —
/// previously three different implementations each hardcoded their own red,
/// and two of the three didn't even match each other.
pub const CLOSE_RED: Color32 = Color32::from_rgb(220, 80, 80);
/// The Detach/float button color, everywhere a panel can be detached.
pub const DETACH_BLUE: Color32 = Color32::from_rgb(150, 150, 220);

/// Foreground text/icon color for a disabled control.
pub const DISABLED_FG: Color32 = Color32::from_rgb(110, 110, 116);
/// Background fill for a disabled control.
pub const DISABLED_BG: Color32 = Color32::from_rgb(40, 40, 44);

/// Visible keyboard-focus outline color — egui's default focus ring is
/// low-contrast against Phylon's near-black chrome.
pub const FOCUS_RING: Color32 = ACCENT;

/// Converts a linear-space RGB triple (as returned by
/// `ecology::Diet::standard_color()`, which is authored for the WGPU
/// viewport's linear color pipeline) to an sRGB-encoded `Color32` suitable
/// for on-screen egui UI — without this, a naive byte-for-byte copy of the
/// linear floats would render far too dark in the UI.
fn linear_to_srgb(linear: [f32; 3]) -> Color32 {
    let encode = |c: f32| -> u8 { (c.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0).round() as u8 };
    Color32::from_rgb(encode(linear[0]), encode(linear[1]), encode(linear[2]))
}

/// The canonical UI color for a diet category — re-derived from
/// `ecology::Diet::standard_color()` on every call rather than copied, so a
/// chart, legend, or status chip can never drift from the simulation's own
/// visual identity again (this is what `metrics.rs`'s previously-hardcoded,
/// mismatched-with-the-viewport series colors are being fixed to use). See
/// `docs/design/colors.md`.
pub fn chart_color(diet: &ecology::Diet) -> Color32 {
    linear_to_srgb(diet.standard_color())
}

// ─── Typography (docs/design/typography.md) ────────────────────────────────

/// Font family key for headings — IBM Plex Sans SemiBold.
const HEADING_FAMILY: &str = "IBMPlexSans-SemiBold";

/// Font size for dialog/modal titles only (About, Keybinds) — never used in
/// the docked workbench itself.
pub const SIZE_DISPLAY: f32 = 22.0;
/// Font size for section headings (`ui.heading()` / `TextStyle::Heading`).
pub const SIZE_HEADING: f32 = 18.0;
/// Font size for panel/window titles and CollapsingHeader-level
/// sub-sections — bumped from 14 so it reads as distinct from Body at a
/// glance (they used to be visually indistinguishable at arm's length).
pub const SIZE_SUBHEADING: f32 = 15.0;
/// Font size for standard body text, data-grid rows, and interactive
/// options (buttons, toggles) — the app's default text size.
pub const SIZE_BODY: f32 = 13.0;
/// Font size for secondary/meta text — timestamps, counts, footers, hints.
/// Bumped from 11 (below a comfortable floor for an 8-hour session);
/// `SIZE_MICRO` below covers the one place a smaller size is still used.
pub const SIZE_SMALL: f32 = 12.0;
/// Font size reserved for the status bar's secondary/system zone only — the
/// one deliberate exception to the `SIZE_SMALL` floor, not an oversight.
pub const SIZE_MICRO: f32 = 11.0;

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

        // Keyboard-focus visibility (docs/design/accessibility.md): egui
        // renders a keyboard-focused widget with its `active` `WidgetVisuals`
        // (the same state used while a widget is being clicked/dragged — see
        // `egui::style::Widgets::style`), so this is the one place that
        // covers every focusable control app-wide, rather than a per-widget
        // fix. The default active-state stroke was low-contrast against
        // Phylon's near-black chrome; this makes it `FOCUS_RING`-colored and
        // thick enough to actually notice.
        style.visuals.widgets.active.bg_stroke = egui::Stroke::new(2.0, FOCUS_RING);
        style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, FOCUS_RING);
    });
}
