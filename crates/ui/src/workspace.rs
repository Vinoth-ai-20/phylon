//! Workspace lifecycle management — save/rename/duplicate/delete/export/
//! import user-defined panel layouts, plus tracking which workspace
//! (built-in preset or user-saved) is currently active so it can be
//! restored on next launch and reset back to its canonical shape on
//! demand.
//!
//! ## The unified storage model
//!
//! [`WorkspaceLayout`] is the *one* shape both built-in presets
//! (`layout::LayoutPreset`, materialized into data via
//! `layout::built_in_layout`) and user-saved workspaces (held in
//! [`WorkspaceService`]) use. There is no separate "user workspace" struct
//! with different fields — Save/Duplicate/Export all just capture or clone
//! a `WorkspaceLayout`, regardless of whether its origin was a built-in
//! preset or another saved workspace. This is the same "one canonical
//! shape, no parallel second one" discipline this crate already applies to
//! selection state and recent-items.
//!
//! ## Never a second layout-construction pathway
//!
//! Every operation in this module that changes what's on screen —
//! [`WorkspaceLayout::apply`], and by extension `apply_saved`/
//! `reset_active_built_in` below — routes through the exact same
//! `layout::rebuild_tree_from_modes` every other layout change uses
//! (persisted-layout restore on startup, `layout::apply_layout_preset`, the
//! toolbar's Focus Mode toggle). Nothing in this module builds an
//! `egui_tiles::Tree` directly.
//!
//! ## Untrusted input can never produce a broken docking tree
//!
//! [`WorkspaceLayout::sanitized`] is the mandatory step between "data that
//! came from outside this process" (an imported `.ron` file) and "data
//! applied to live state." See its own doc comment for exactly what it
//! guards against.

use crate::layout::LayoutPreset;
use crate::state::PanelMode;
use crate::WorkbenchState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The data shape of one workspace: which named panels are Docked/
/// Floating/Closed, and each split's persisted ratio. See this module's
/// doc comment for why this one type is shared by built-in and user-saved
/// workspaces.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceLayout {
    /// Which named panels are Docked/Floating/Closed.
    pub panel_modes: HashMap<String, PanelMode>,
    /// Each split's persisted ratio, keyed the same way
    /// `layout::extract_shares` keys them.
    pub layout_shares: HashMap<String, f32>,
}

impl WorkspaceLayout {
    /// Captures the layout currently live on `state`.
    pub fn capture(state: &WorkbenchState) -> Self {
        Self {
            panel_modes: state.panel_modes.clone(),
            layout_shares: state.layout_shares.clone(),
        }
    }

    /// Applies this layout to `state` by rebuilding `dock_tree` via
    /// `layout::rebuild_tree_from_modes` — the sole tree builder, per this
    /// module's doc comment. Does not touch `state.workspaces`'s active
    /// marker; callers that want that updated (every caller in this
    /// module does) set it themselves right after calling this.
    pub fn apply(&self, state: &mut WorkbenchState) {
        state.panel_modes = self.panel_modes.clone();
        state.layout_shares = self.layout_shares.clone();
        crate::layout::rebuild_tree_from_modes(
            &mut state.dock_tree,
            &state.panel_modes,
            &state.layout_shares,
        );
    }

    /// Sanitizes an untrusted layout (one read from an imported `.ron`
    /// file) so it can never produce a broken docking tree. Two concrete
    /// guards:
    ///
    /// - **Unknown panel names are dropped.** A workspace exported from a
    ///   future version of this app (a renamed or since-removed panel)
    ///   would otherwise carry a key `rebuild_tree_from_modes` has never
    ///   heard of. `rebuild_tree_from_modes` already ignores keys it
    ///   doesn't look up by name, so this is defensive hygiene rather than
    ///   a fix for an observed crash — kept anyway, since "harmless today"
    ///   is not a guarantee against every future refactor of that
    ///   function.
    /// - **Non-finite or non-positive shares are replaced with `1.0`.** A
    ///   hand-edited or corrupted `.ron` file could contain a `NaN`,
    ///   infinite, zero, or negative share value; `PanelMode` itself can't
    ///   carry an invalid value (a bad enum tag simply fails to parse,
    ///   caught at deserialization), but `f32` share values have no such
    ///   built-in guard, and `egui_tiles::Shares::set_share` receives them
    ///   completely unvalidated (confirmed by reading
    ///   `layout::rebuild_tree_from_modes` directly). Replacing rather
    ///   than rejecting the whole import keeps a partially-bad file
    ///   usable instead of an all-or-nothing failure.
    pub fn sanitized(&self) -> Self {
        let panel_modes = self
            .panel_modes
            .iter()
            .filter(|(name, _)| crate::layout::ALL_PANEL_NAMES.contains(&name.as_str()))
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        let layout_shares = self
            .layout_shares
            .iter()
            .map(|(k, &v)| {
                let safe = if v.is_finite() && v > 0.0 { v } else { 1.0 };
                (k.clone(), safe)
            })
            .collect();
        Self {
            panel_modes,
            layout_shares,
        }
    }
}

/// Which workspace is currently active — a built-in preset, or a
/// user-saved one by name. Purely a label: it never itself holds a
/// `WorkspaceLayout` (that would be a second copy of state that could
/// drift from the real one in `WorkbenchState::panel_modes`/
/// `layout_shares`) — it exists so the UI can show "you're in Evolution
/// right now" and so `reset_active_built_in` knows what to reset *to*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActiveWorkspace {
    /// A built-in `LayoutPreset`.
    BuiltIn(LayoutPreset),
    /// A user-saved workspace, by name.
    Saved(String),
}

/// The on-disk file format for `Export Workspace`/`Import Workspace` — a
/// name paired with its layout, so an imported file can offer its own
/// name as the default rather than requiring the user to retype it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedWorkspace {
    /// The workspace's name at the time it was exported.
    pub name: String,
    /// The exported layout — sanitized on import, never trusted as-is.
    pub layout: WorkspaceLayout,
}

/// Owns every user-saved workspace (name → layout) plus which workspace is
/// currently active. Persisted via `app::preferences::Preferences`, the
/// same mechanism `RecentItemsService`/`panel_modes`/`layout_shares` use.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceService {
    saved: HashMap<String, WorkspaceLayout>,
    active: Option<ActiveWorkspace>,
}

impl WorkspaceService {
    /// Saves (or overwrites) `name` with `layout`, and marks it active.
    pub fn save(&mut self, name: impl Into<String>, layout: WorkspaceLayout) {
        let name = name.into();
        self.saved.insert(name.clone(), layout);
        self.active = Some(ActiveWorkspace::Saved(name));
    }

    /// Renames a saved workspace. Returns `false` (no-op) if `old_name`
    /// isn't a saved workspace — built-in presets can't be renamed, since
    /// they aren't stored entries to begin with.
    pub fn rename(&mut self, old_name: &str, new_name: impl Into<String>) -> bool {
        let Some(layout) = self.saved.remove(old_name) else {
            return false;
        };
        let new_name = new_name.into();
        let was_active = self.active == Some(ActiveWorkspace::Saved(old_name.to_string()));
        self.saved.insert(new_name.clone(), layout);
        if was_active {
            self.active = Some(ActiveWorkspace::Saved(new_name));
        }
        true
    }

    /// Deletes a saved workspace. A no-op if `name` isn't saved (built-ins
    /// can't be deleted). Clears `active` if the deleted workspace was the
    /// active one — there's nothing left to point at.
    pub fn delete(&mut self, name: &str) {
        self.saved.remove(name);
        if self.active == Some(ActiveWorkspace::Saved(name.to_string())) {
            self.active = None;
        }
    }

    /// Looks up a saved workspace's layout by name.
    pub fn get(&self, name: &str) -> Option<&WorkspaceLayout> {
        self.saved.get(name)
    }

    /// Every saved workspace's name, in no particular order — callers that
    /// want a stable display order should sort this themselves.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.saved.keys().map(String::as_str)
    }

    /// The currently-active workspace, if any.
    pub fn active(&self) -> Option<&ActiveWorkspace> {
        self.active.as_ref()
    }

    /// Sets the active-workspace marker directly, without touching
    /// `saved` — used by `layout::apply_layout_preset` when a built-in
    /// preset is applied.
    pub fn set_active(&mut self, active: ActiveWorkspace) {
        self.active = Some(active);
    }

    /// Returns `base` if it isn't already a saved workspace name,
    /// otherwise `"{base} (2)"`, `"{base} (3)"`, etc. — used by workspace
    /// import so an imported name colliding with an existing saved
    /// workspace never silently overwrites it.
    pub fn unique_name(&self, base: &str) -> String {
        if !self.saved.contains_key(base) {
            return base.to_string();
        }
        let mut i = 2;
        loop {
            let candidate = format!("{base} ({i})");
            if !self.saved.contains_key(&candidate) {
                return candidate;
            }
            i += 1;
        }
    }
}

/// Applies a saved workspace by name, marking it active. Returns `false`
/// if `name` isn't a saved workspace (nothing is changed in that case).
pub fn apply_saved(state: &mut WorkbenchState, name: &str) -> bool {
    let Some(layout) = state.workspaces.get(name).cloned() else {
        return false;
    };
    layout.apply(state);
    state
        .workspaces
        .set_active(ActiveWorkspace::Saved(name.to_string()));
    true
}

/// Saves the *current live layout* as a new (or overwritten) saved
/// workspace under `name`.
pub fn save_current_as(state: &mut WorkbenchState, name: impl Into<String>) {
    let layout = WorkspaceLayout::capture(state);
    state.workspaces.save(name, layout);
}

/// Duplicates an existing *saved* workspace under a new name. Returns
/// `false` if `source_name` isn't saved. To duplicate a built-in preset,
/// use [`duplicate_built_in`] instead — a built-in has no `saved` entry to
/// copy from.
pub fn duplicate_saved(
    state: &mut WorkbenchState,
    source_name: &str,
    new_name: impl Into<String>,
) -> bool {
    let Some(layout) = state.workspaces.get(source_name).cloned() else {
        return false;
    };
    state.workspaces.save(new_name, layout);
    true
}

/// Duplicates a built-in preset's canonical layout into a new saved
/// workspace — the starting point for "customize a built-in without
/// modifying the built-in itself."
pub fn duplicate_built_in(
    state: &mut WorkbenchState,
    preset: LayoutPreset,
    new_name: impl Into<String>,
) {
    let layout = crate::layout::built_in_layout(preset);
    state.workspaces.save(new_name, layout);
}

/// If the currently active workspace is a built-in preset, re-applies its
/// canonical layout — discarding any live drift (dragged splits,
/// docked/closed panels) since it was last selected. Returns `false` (a
/// no-op) if the active workspace is a saved one, or if nothing is active.
pub fn reset_active_built_in(state: &mut WorkbenchState) -> bool {
    if let Some(ActiveWorkspace::BuiltIn(preset)) = state.workspaces.active().cloned() {
        crate::layout::apply_layout_preset(state, preset);
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_layout() -> WorkspaceLayout {
        let mut panel_modes = HashMap::new();
        panel_modes.insert("Sidebar".to_string(), PanelMode::Docked);
        panel_modes.insert("Metrics".to_string(), PanelMode::Floating);
        let mut layout_shares = HashMap::new();
        layout_shares.insert("Sidebar".to_string(), 1.0);
        WorkspaceLayout {
            panel_modes,
            layout_shares,
        }
    }

    #[test]
    fn save_then_get_round_trips() {
        let mut service = WorkspaceService::default();
        service.save("My Layout", sample_layout());
        assert_eq!(service.get("My Layout"), Some(&sample_layout()));
        assert_eq!(
            service.active(),
            Some(&ActiveWorkspace::Saved("My Layout".to_string()))
        );
    }

    #[test]
    fn rename_moves_the_entry_and_updates_active_if_it_was_active() {
        let mut service = WorkspaceService::default();
        service.save("Old Name", sample_layout());
        assert!(service.rename("Old Name", "New Name"));
        assert!(service.get("Old Name").is_none());
        assert_eq!(service.get("New Name"), Some(&sample_layout()));
        assert_eq!(
            service.active(),
            Some(&ActiveWorkspace::Saved("New Name".to_string()))
        );
    }

    #[test]
    fn rename_of_a_nonexistent_workspace_is_a_no_op_returning_false() {
        let mut service = WorkspaceService::default();
        assert!(!service.rename("Does Not Exist", "New Name"));
    }

    #[test]
    fn delete_removes_the_entry_and_clears_active_if_it_was_active() {
        let mut service = WorkspaceService::default();
        service.save("Temp", sample_layout());
        service.delete("Temp");
        assert!(service.get("Temp").is_none());
        assert_eq!(service.active(), None);
    }

    #[test]
    fn delete_of_a_non_active_workspace_leaves_active_untouched() {
        let mut service = WorkspaceService::default();
        service.save("A", sample_layout());
        service.save("B", sample_layout());
        // "B" is now active (save() marks its own name active).
        service.delete("A");
        assert_eq!(
            service.active(),
            Some(&ActiveWorkspace::Saved("B".to_string()))
        );
    }

    #[test]
    fn unique_name_returns_base_when_not_taken() {
        let service = WorkspaceService::default();
        assert_eq!(service.unique_name("Fresh"), "Fresh");
    }

    #[test]
    fn unique_name_appends_a_counter_on_collision() {
        let mut service = WorkspaceService::default();
        service.save("Taken", sample_layout());
        assert_eq!(service.unique_name("Taken"), "Taken (2)");
        service.save("Taken (2)", sample_layout());
        assert_eq!(service.unique_name("Taken"), "Taken (3)");
    }

    #[test]
    fn sanitized_drops_unknown_panel_names() {
        let mut layout = sample_layout();
        layout
            .panel_modes
            .insert("Some Future Panel".to_string(), PanelMode::Docked);
        let sanitized = layout.sanitized();
        assert!(!sanitized.panel_modes.contains_key("Some Future Panel"));
        assert!(sanitized.panel_modes.contains_key("Sidebar"));
    }

    #[test]
    fn sanitized_replaces_non_finite_and_non_positive_shares_with_one() {
        let mut layout = sample_layout();
        layout
            .layout_shares
            .insert("Viewport".to_string(), f32::NAN);
        layout
            .layout_shares
            .insert("Metrics".to_string(), f32::INFINITY);
        layout.layout_shares.insert("Event Log".to_string(), -3.0);
        layout
            .layout_shares
            .insert("Neural Viewer".to_string(), 0.0);
        let sanitized = layout.sanitized();
        assert_eq!(sanitized.layout_shares["Viewport"], 1.0);
        assert_eq!(sanitized.layout_shares["Metrics"], 1.0);
        assert_eq!(sanitized.layout_shares["Event Log"], 1.0);
        assert_eq!(sanitized.layout_shares["Neural Viewer"], 1.0);
        // A valid, positive, finite share is left untouched.
        assert_eq!(sanitized.layout_shares["Sidebar"], 1.0);
    }

    #[test]
    fn apply_saved_returns_false_for_an_unknown_workspace_and_changes_nothing() {
        let mut state = WorkbenchState::default();
        let before = state.panel_modes.clone();
        assert!(!apply_saved(&mut state, "Nope"));
        assert_eq!(state.panel_modes, before);
    }

    #[test]
    fn save_current_as_then_apply_saved_round_trips_through_real_state() {
        let mut state = WorkbenchState::default();
        crate::layout::apply_layout_preset(&mut state, LayoutPreset::Debug);
        save_current_as(&mut state, "My Debug Copy");

        crate::layout::apply_layout_preset(&mut state, LayoutPreset::Presentation);
        assert_ne!(
            state.panel_modes.get("Sidebar"),
            Some(&PanelMode::Docked),
            "Presentation should have closed Sidebar"
        );

        assert!(apply_saved(&mut state, "My Debug Copy"));
        assert_eq!(state.panel_modes.get("Sidebar"), Some(&PanelMode::Docked));
        assert_eq!(
            state.workspaces.active(),
            Some(&ActiveWorkspace::Saved("My Debug Copy".to_string()))
        );
    }

    #[test]
    fn duplicate_built_in_creates_a_saved_copy_matching_the_canonical_layout() {
        let mut state = WorkbenchState::default();
        duplicate_built_in(&mut state, LayoutPreset::Evolution, "My Evolution Copy");
        let saved = state
            .workspaces
            .get("My Evolution Copy")
            .expect("should be saved");
        assert_eq!(
            saved,
            &crate::layout::built_in_layout(LayoutPreset::Evolution)
        );
    }

    #[test]
    fn reset_active_built_in_discards_live_drift_back_to_the_canonical_layout() {
        let mut state = WorkbenchState::default();
        crate::layout::apply_layout_preset(&mut state, LayoutPreset::Research);
        // Simulate live drift: manually close the Viewport-adjacent Sidebar.
        state
            .panel_modes
            .insert("Sidebar".to_string(), PanelMode::Closed);

        assert!(reset_active_built_in(&mut state));
        assert_eq!(
            state.panel_modes.get("Sidebar"),
            Some(&PanelMode::Docked),
            "reset should restore Research's canonical Sidebar: Docked"
        );
    }

    #[test]
    fn reset_active_built_in_is_a_no_op_when_a_saved_workspace_is_active() {
        let mut state = WorkbenchState::default();
        save_current_as(&mut state, "Custom");
        assert!(!reset_active_built_in(&mut state));
    }
}
