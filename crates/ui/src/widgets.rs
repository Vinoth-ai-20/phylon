//! Shared, reusable UI primitives — plain functions taking `&mut egui::Ui`
//! plus data, the same idiom the codebase already used correctly for
//! `legend_dot`/`grid_row` before this module existed. No component/props
//! framework is introduced here; this is consolidation of an existing
//! pattern that had drifted into three or four near-duplicate
//! implementations across `sidebar.rs`, `inspector.rs`, `dialogs.rs`, and
//! `neural_viewer.rs`. See `docs/design/components.md` for the full catalog
//! (Purpose/Variants/States/Tokens/Accessibility/Owner/Dependencies) each of
//! these implements.

use crate::theme;

/// One key/value line in any inspector-style data grid — call inside an
/// `egui::Grid` (each call is one row: two cells + `end_row()`).
///
/// Consolidates what used to be three independent implementations:
/// `sidebar.rs`'s private `grid_row`, `inspector.rs`'s hand-rolled
/// `ui.label(format!(...))` pairs, and `dialogs.rs`'s `about_grid` rows.
pub fn kv_row(ui: &mut egui::Ui, key: &str, val: &str) {
    ui.label(
        egui::RichText::new(key)
            .color(crate::theme::DISABLED_FG)
            .size(theme::SIZE_BODY),
    );
    ui.label(egui::RichText::new(val).strong().size(theme::SIZE_BODY));
    ui.end_row();
}

/// Same as [`kv_row`], but both cells are tinted `color` instead of the
/// default gray-key/white-value — used for diet-colored population counts
/// and similar semantically-colored data.
pub fn kv_row_colored(ui: &mut egui::Ui, key: &str, val: &str, color: egui::Color32) {
    ui.label(egui::RichText::new(key).color(color).size(theme::SIZE_BODY));
    ui.label(
        egui::RichText::new(val)
            .color(color)
            .strong()
            .size(theme::SIZE_BODY),
    );
    ui.end_row();
}

/// Same as [`kv_row`], but the value renders in the Monospace family so
/// digits stay tabular — use for any row whose value updates live (a tick
/// count, a live sensor reading), per `docs/design/typography.md`.
pub fn kv_row_mono(ui: &mut egui::Ui, key: &str, val: &str) {
    ui.label(
        egui::RichText::new(key)
            .color(crate::theme::DISABLED_FG)
            .size(theme::SIZE_BODY),
    );
    ui.label(
        egui::RichText::new(val)
            .monospace()
            .strong()
            .size(theme::SIZE_BODY),
    );
    ui.end_row();
}

/// One compact "icon + mono value" cluster for the status bar — e.g. `⏱ 1024`.
/// Consolidates the `tight_row`+`mono` pairing `status_bar.rs` previously
/// hand-rolled per field into the single reusable primitive named in
/// `docs/design/components.md`. `color` tints both icon and value; pass
/// `None` for the default text color.
pub fn status_chip(
    ui: &mut egui::Ui,
    icon: &str,
    value: impl Into<String>,
    color: Option<egui::Color32>,
) {
    let text_color = color.unwrap_or_else(|| ui.visuals().text_color());
    let saved = ui.spacing().item_spacing.x;
    ui.spacing_mut().item_spacing.x = 2.0;
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(icon)
                .color(text_color)
                .size(theme::SIZE_SMALL),
        );
        ui.label(
            egui::RichText::new(value.into())
                .monospace()
                .strong()
                .color(text_color)
                .size(theme::SIZE_SMALL),
        );
    });
    ui.spacing_mut().item_spacing.x = saved;
}

/// A colored circular swatch followed by a label — used for chart/graph
/// legends instead of a Unicode "●" glyph, which silently falls back to a
/// tofu/box glyph in fonts that don't carry that codepoint (IBM Plex Sans
/// doesn't), regardless of which fallback font is configured after it.
///
/// Originated in `neural_viewer.rs` (to work around exactly that glyph
/// issue) and generalized here so `metrics.rs` can use the same legend
/// styling instead of relying on `egui_plot::Legend`'s on-chart overlay.
pub fn chart_legend_dot(ui: &mut egui::Ui, color: egui::Color32, label: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 4.0, color);
        ui.add_space(2.0);
        ui.label(egui::RichText::new(label).small());
    });
}

/// Centered placeholder content for a panel with nothing to show —
/// consolidates ad hoc centered-label patterns previously scattered in
/// `inspector.rs` and `neural_viewer.rs`. `hint` should say what to do next
/// ("Select an organism to view its brain"), not just that something's
/// missing ("No organism selected" alone).
pub fn empty_state(ui: &mut egui::Ui, hint: &str) {
    ui.add_space(theme::SPACE_XXXL);
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new(hint)
                .color(crate::theme::DISABLED_FG)
                .italics(),
        );
    });
}

/// Same as [`empty_state`], but styled for an error condition (e.g. a failed
/// query) rather than a normal "nothing selected yet" state.
pub fn error_state(ui: &mut egui::Ui, message: &str) {
    ui.add_space(theme::SPACE_XXXL);
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new(message).color(theme::BAD).italics());
    });
}
