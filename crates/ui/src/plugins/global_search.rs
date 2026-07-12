//! Global Search overlay — fuzzy-searchable list of every currently-alive
//! organism, by diet or the same `{Idx, Gen}` identifier `inspector.rs`'s
//! own header shows (bevy_ecs's entity index/generation, not an
//! evolutionary generation count — matching that existing label convention
//! exactly rather than inventing a second meaning for "Gen"). Selecting a
//! result routes through `WorkbenchState::select` — the sole canonical
//! selection pathway — same as every other selection entry point.
//!
//! ## Design note
//!
//! - **What's searchable**: one entry per organism (head segment,
//!   `physics::ParticleNode::segment_type == 0`), matched by a
//!   case-insensitive substring against `"{diet:?} {{Idx: N, Gen: G}}"` —
//!   the exact same string `inspector.rs`'s header already renders, so a
//!   result the user sees here reads identically once selected.
//! - **How results are shown**: a scrollable list capped at
//!   `MAX_RESULTS` (population-wide, uncapped search could otherwise
//!   render thousands of rows), mirroring `command_palette.rs`'s exact
//!   list/click-to-invoke pattern — deliberately not a new UI idiom.
//! - **Keyboard navigation**: `Ctrl+F` toggles the overlay (mirroring
//!   `Ctrl+Shift+P`'s Command Palette toggle); the search box is
//!   auto-focused on open. Matches Command Palette's own current behavior
//!   exactly: no arrow-key result navigation and no Escape-to-close exist
//!   for Command Palette either, so Global Search does not invent either
//!   one only for itself — clicking a result or toggling the shortcut
//!   again are the two ways out, consistent with that precedent.

/// Population-wide search could otherwise render thousands of rows; capped
/// the same way a real search UI would paginate, without adding pagination.
const MAX_RESULTS: usize = 50;

/// Renders the Global Search overlay when `state.show_global_search` is set
/// (toggled by Ctrl+F — see `shortcuts.rs`).
pub fn global_search_ui(
    ctx: &egui::Context,
    world: &mut world::World,
    state: &mut crate::WorkbenchState,
) {
    if !state.show_global_search {
        return;
    }

    let mut open = true;
    let mut clicked_entity = None;
    egui::Window::new("Global Search")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 80.0))
        .fixed_size(egui::vec2(380.0, 360.0))
        .show(ctx, |ui| {
            let search = ui.text_edit_singleline(&mut state.global_search_query);
            search.request_focus();

            let needle = state.global_search_query.to_lowercase();

            let mut query = world.ecs.query::<(
                bevy_ecs::entity::Entity,
                &physics::ParticleNode,
                Option<&ecology::Diet>,
            )>();
            let mut results: Vec<(bevy_ecs::entity::Entity, String)> = query
                .iter(&world.ecs)
                .filter(|(_, node, _)| node.segment_type == 0)
                .filter_map(|(entity, _, diet)| {
                    let label = match diet {
                        Some(diet) => format!(
                            "{:?} {{Idx: {}, Gen: {}}}",
                            diet,
                            entity.index(),
                            entity.generation()
                        ),
                        None => format!("Selected: {:?}", entity),
                    };
                    (needle.is_empty() || label.to_lowercase().contains(&needle))
                        .then_some((entity, label))
                })
                .collect();
            results.truncate(MAX_RESULTS);

            ui.separator();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if results.is_empty() {
                        crate::widgets::empty_state(ui, "No matching organisms.");
                    }
                    for (entity, label) in &results {
                        if ui.selectable_label(false, label.as_str()).clicked() {
                            clicked_entity = Some(*entity);
                        }
                    }
                });
        });

    if let Some(entity) = clicked_entity {
        state.select(entity);
        state.show_global_search = false;
    }
    if !open {
        state.show_global_search = false;
    }
}
