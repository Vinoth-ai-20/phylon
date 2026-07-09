# Component Catalog

Every reusable UI primitive in Phylon, documented before it's built or consolidated. `crates/ui/src/widgets.rs` is the new home for the cross-plugin ones; nothing here introduces a component/props framework — every entry is a plain function taking `&mut egui::Ui` plus data, the same idiom already used correctly by the existing `legend_dot`/`grid_row` helpers.

## `chrome_bar`

- **Purpose:** title + action buttons for any panel container (docked, tabbed, or floating).
- **Variants:** docked (2 buttons: Close, Detach), tabbed (2 buttons, drawn into egui_tiles' own tab strip), floating (3 buttons: Close, Dock, Minimize, plus a drag-handle icon).
- **States:** default, hover, dragging (floating only).
- **Tokens:** `CLOSE_RED`, `DETACH_BLUE`, `ICON_MD`, `CHROME_HEIGHT`, `SIZE_SUBHEADING`.
- **Accessibility:** Close/Detach/Dock/Minimize all reachable via keyboard tab order; each has a hover tooltip naming the action and its panel.
- **Owner:** `crates/ui/src/layout.rs` (the tile-tree/chrome owner already).
- **Dependencies:** `theme.rs`.
- **Consolidates:** `panel_chrome`, `top_bar_right_ui`'s inline buttons, `floating_chrome` — three near-duplicate implementations today, including two different "close" reds that don't match each other.

## `kv_row` / `kv_row_colored`

- **Purpose:** one key/value line in any inspector-style data grid.
- **Variants:** plain (gray key, bold value), colored (both key and value tinted, e.g. diet-colored population counts).
- **States:** default, muted (value is "Not Available"/N/A).
- **Tokens:** `SIZE_BODY`, `SPACE_SM` row height.
- **Accessibility:** value text meets contrast minimum against the panel background; muted state is distinguishable by more than color alone (italic).
- **Owner:** new `crates/ui/src/widgets.rs`.
- **Dependencies:** `theme.rs`.
- **Consolidates:** `sidebar.rs`'s private `grid_row`/`grid_row_colored`, `inspector.rs`'s hand-rolled `ui.label(format!(...))` rows, `dialogs.rs`'s `about_grid`, and `neural_viewer.rs`'s tooltip grids — four independent implementations of the identical pattern today.
- **Example:** `kv_row(ui, "Genome ID", &genome.id.0.to_string())`.

## `chart_legend_dot`

- **Purpose:** colored swatch + label for chart/graph legends.
- **Variants:** filled circle (standard).
- **States:** default. (An active-series bold state for Metrics was considered but not built — Metrics has no per-series visibility/focus interaction to key it off of; see its own "Future Scope" note.)
- **Tokens:** `CHART_*`, `SIZE_SMALL`.
- **Accessibility:** never the sole indicator of series identity — always paired with a text label, never color-only.
- **Owner:** `crates/ui/src/widgets.rs` (generalized out of `neural_viewer.rs`, where it originated to work around a font-glyph issue with the Unicode "●" character).
- **Dependencies:** `theme.rs`.
- **Consolidates, Milestone 7 (done):** `neural_viewer.rs`'s original `legend_dot` and `metrics.rs`'s reliance on `egui_plot::Legend::default()`'s on-chart overlay — both now call this one shared function. (A leftover local copy of `legend_dot` survived in `neural_viewer.rs` past the original Milestone 7 close and was found and removed during the Phase 1 documentation-sync pass.)

## `status_chip`

- **Purpose:** icon + label + value cluster in a status bar zone.
- **Variants:** info (simulation state), population (diet/resource counts), system (memory/engine/camera, mostly hover-revealed).
- **States:** default, hover-reveal (system zone only — expands on hover rather than being always-visible).
- **Tokens:** `SIZE_SMALL`.
- **Accessibility:** text meets the 12px floor (`SIZE_SMALL`).
- **Owner:** `crates/ui/src/widgets.rs`.
- **Dependencies:** `theme.rs`.
- **Consolidates:** the entity_count/tick/fps/tps/playback/overlay/selection/cursor-position facts in `status_bar.rs`, which now call this shared function.
- **Correction (Phase 7, W4d)**: this entry previously claimed `SIZE_MICRO`/`SIZE_SMALL` tokens and a per-zone background tint. Re-checked directly against `status_bar.rs`: `SIZE_MICRO` is never used there (it's used in `render.rs`'s viewport scale-grid readout and `toolbar.rs`'s camera-position readout — unrelated to this component), and no `Frame::fill`/background-tint code exists per zone. Also, the Population zone's diet-breakdown (P/H/C/O/D) and Food/Minerals/Corpses counts, and the entire System zone, do not yet call `status_chip` — they still use the ad hoc `tight_row`/`mono` pattern this component was meant to replace. Corrected to reflect only what's actually consolidated today; migrating the remaining call sites is unfinished work, not a documentation gap.

## `labeled_icon_tab`

- **Purpose:** sidebar activity-bar entry.
- **Variants:** icon-only (pinned/collapsed rail), icon+label (expanded rail — the default, fixing the audit's discoverability finding).
- **States:** default, active, hover.
- **Tokens:** `ICON_LG`/`ICON_MD`.
- **Accessibility:** hover tooltip present in both variants, even when the label is already visible — redundant labeling costs nothing and helps anyone skimming quickly.
- **Owner:** `crates/ui/src/plugins/sidebar.rs` (stays local — only one caller).
- **Dependencies:** `theme.rs`.
- **Consolidates:** `activity_bar_ui`'s current icon-only buttons, which rely entirely on hover-tooltip discovery today.
- **Correction (Phase 7, W4d)**: this entry previously claimed `ACCENT` drives the active-tab underline/fill. Re-checked directly against `sidebar.rs`: `theme::ACCENT` is never referenced there, and no `style.visuals.selection` override exists anywhere in `ui/src` — active-tab highlighting relies entirely on egui's default `SelectableLabel` selection color, not this token. Tying the active state to `ACCENT` is unimplemented follow-up work, not a shipped behavior.

## `EmptyState` / `ErrorState`

- **Purpose:** centered placeholder content for a panel with nothing to show.
- **Variants:** empty (no selection), error (failed query).
- **States:** — (each variant is its own visual state).
- **Tokens:** `SPACE_XXXL` (vertical centering offset), `SIZE_BODY`, `WARN`/`BAD` for the error variant.
- **Accessibility:** text explains what to do next ("Select an organism to view its brain"), not just that something is missing ("No organism selected" alone is the current, weaker pattern).
- **Owner:** `crates/ui/src/widgets.rs` (`empty_state`/`error_state`).
- **Dependencies:** `theme.rs`.
- **Consolidates:** ad hoc centered-label patterns scattered in `inspector.rs` and `neural_viewer.rs` today.
- **Correction (Phase 7, W4d)**: this entry previously named a third `LoadingState` variant. Re-checked directly against `widgets.rs`: only `empty_state`/`error_state` exist — no "loading, data not ready" function or call site exists anywhere in the crate. Dropped from the catalog rather than left as an aspirational claim; add it back here if a panel actually needs a loading state in the future.

## `draw_segment_tree` (Inspector "Body Plan")

- **Purpose:** recursive collapsible tree view of the selected organism's segment/spring hierarchy (Head → Torso/Muscle/Tail/Fin), rooted at its head node regardless of which segment is currently selected.
- **Variants:** leaf row (no children — a selectable label) vs. branch row (a `CollapsingHeader`, default-open, showing each child's connecting constraint type and, for actuated muscles, amplitude/phase).
- **States:** default, selected (the row for `state.selected_entity` renders as the active `selectable_label`).
- **Tokens:** `DISABLED_FG` (constraint-type sub-labels).
- **Accessibility:** clicking any row re-selects that segment, mirroring viewport click-to-select — one selection model, not a second parallel one scoped to this tree.
- **Owner:** `crates/ui/src/utils.rs` (recursive drawing function); `crates/ui/src/plugins/inspector.rs::render_body_plan` (adjacency-map construction and head-node lookup, called from a "Body Plan" `CollapsingHeader`).
- **Dependencies:** `physics::{ParticleNode, Spring}`, `theme.rs`.
- **History:** implemented in full before this design system existed, but never wired into the Inspector — a dead-code finding closed during Phase 1, M4 rather than a new feature built for this pass.
