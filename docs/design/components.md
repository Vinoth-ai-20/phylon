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
- **States:** default, active-series (bold, for the currently-tracked entity's series in Metrics once Milestone 7 lands).
- **Tokens:** `CHART_*`, `SIZE_SMALL`.
- **Accessibility:** never the sole indicator of series identity — always paired with a text label, never color-only.
- **Owner:** `crates/ui/src/widgets.rs` (generalized out of `neural_viewer.rs`, where it originated to work around a font-glyph issue with the Unicode "●" character).
- **Dependencies:** `theme.rs`.
- **Consolidates:** `neural_viewer.rs`'s `legend_dot` (currently the only implementation; Milestone 7 gives Metrics a matching one instead of relying on `egui_plot::Legend::default()`'s on-chart overlay).

## `status_chip`

- **Purpose:** icon + label + value cluster in a status bar zone.
- **Variants:** info (simulation state), population (diet/resource counts), system (memory/engine/camera, mostly hover-revealed).
- **States:** default, hover-reveal (system zone only — expands on hover rather than being always-visible).
- **Tokens:** `SIZE_MICRO`/`SIZE_SMALL`, per-zone background tint.
- **Accessibility:** text meets the 12px floor (`SIZE_SMALL`); the one exception, `SIZE_MICRO` (11px), is reserved for the system zone specifically, a deliberate choice not an oversight.
- **Owner:** `crates/ui/src/widgets.rs`.
- **Dependencies:** `theme.rs`.
- **Consolidates:** ad hoc `ui.label(format!(...))` calls in `status_bar.rs`, currently eleven distinct facts in one undifferentiated row.

## `labeled_icon_tab`

- **Purpose:** sidebar activity-bar entry.
- **Variants:** icon-only (pinned/collapsed rail), icon+label (expanded rail — the default, fixing the audit's discoverability finding).
- **States:** default, active, hover.
- **Tokens:** `ICON_LG`, `ACCENT` (active-state underline/fill).
- **Accessibility:** hover tooltip present in both variants, even when the label is already visible — redundant labeling costs nothing and helps anyone skimming quickly.
- **Owner:** `crates/ui/src/plugins/sidebar.rs` (stays local — only one caller).
- **Dependencies:** `theme.rs`.
- **Consolidates:** `activity_bar_ui`'s current icon-only buttons, which rely entirely on hover-tooltip discovery today.

## `EmptyState` / `LoadingState` / `ErrorState`

- **Purpose:** centered placeholder content for a panel with nothing to show.
- **Variants:** empty (no selection), loading (data not ready), error (failed query).
- **States:** — (each variant is its own visual state).
- **Tokens:** `SPACE_XXXL` (vertical centering offset), `SIZE_BODY`, `WARN`/`BAD` for the error variant.
- **Accessibility:** text explains what to do next ("Select an organism to view its brain"), not just that something is missing ("No organism selected" alone is the current, weaker pattern).
- **Owner:** `crates/ui/src/widgets.rs`.
- **Dependencies:** `theme.rs`.
- **Consolidates:** ad hoc centered-label patterns scattered in `inspector.rs` and `neural_viewer.rs` today.
