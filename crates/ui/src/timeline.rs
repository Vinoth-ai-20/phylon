//! Shared Development Timeline scrubber (Phase 3, M13) — used by both HOX
//! Visualizer and GRN Viewer so scrubbing through an organism's actual
//! growth order in one panel carries over to the other (`WorkbenchState`'s
//! single `timeline_step` field is shared between them).
//!
//! Per `PHASE3_ROADMAP.md`'s ADR-P3-04, this scrubber does **not** read any
//! persisted Body Graph — there isn't one; `organisms::simulate_growth_timeline`
//! deterministically reconstructs the full growth order from the genome
//! alone, on demand, exactly like every other Phase 3 research panel's data.

/// The ordered sequence of body positions a genome's organism would
/// actually grow (skipping pruned positions) — spine positions only;
/// branch nodes share their spine parent's position, so including them
/// here would just duplicate entries.
pub(crate) fn grown_positions(regulatory_cppn: &genetics::Cppn) -> Vec<usize> {
    organisms::simulate_growth_timeline(regulatory_cppn)
        .nodes
        .into_iter()
        .filter(|n| !n.is_branch)
        .map(|n| n.position)
        .collect()
}

/// Renders a Prev/Next + slider scrubber over `positions`, clamping `step`
/// into range and returning the body position it currently refers to.
/// Returns `None` if `positions` is empty (a degenerate regulatory network
/// whose head immediately decodes as something that stops growth with zero
/// recorded spine positions — shouldn't happen in practice since the head
/// always grows, but handled rather than assumed).
pub(crate) fn timeline_scrubber_ui(
    ui: &mut egui::Ui,
    positions: &[usize],
    step: &mut usize,
) -> Option<usize> {
    if positions.is_empty() {
        ui.label(
            egui::RichText::new("No grown positions to step through.")
                .italics()
                .color(crate::theme::DISABLED_FG),
        );
        return None;
    }
    *step = (*step).min(positions.len() - 1);

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Development Timeline").strong());
        if ui.small_button("◀").clicked() && *step > 0 {
            *step -= 1;
        }
        ui.add(egui::Slider::new(step, 0..=(positions.len() - 1)).text("growth step"));
        if ui.small_button("▶").clicked() && *step < positions.len() - 1 {
            *step += 1;
        }
        ui.label(
            egui::RichText::new(format!("body position {}", positions[*step]))
                .small()
                .color(crate::theme::DISABLED_FG),
        );
    });

    Some(positions[*step])
}
