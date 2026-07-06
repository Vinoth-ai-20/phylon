# Layout, Docking &amp; Window Management

## Panel ratios

The root split (`crates/ui/src/layout.rs::rebuild_tree_from_modes`) is Sidebar (share 1.0) | Viewport + bottom tabs (share 3.0) | Neural Viewer (share 1.0) — roughly **20% / 60% / 20%**, close to the Blender/Unity convention this design system targets. The nested bottom split is Viewport (share 3.0) vs. Metrics/Event Log tabs (share 1.0).

**Fixed, Milestone 9:** these shares used to be hardcoded and rebuilt from scratch on every dock/undock/reset action, silently discarding any ratio the user dragged at runtime. `extract_shares`/`collect_shares` now persist last-known shares in `WorkbenchState::layout_shares` every frame, and `rebuild_tree_from_modes` reads them back via `share_of()` instead of the old hardcoded constants — never changing *which* panels exist, only *how much space* they get, keeping `PanelMode` (Docked/Floating/Closed) as the sole structural source of truth.

## Docking model

Every panel is one of three `PanelMode`s:

- **Docked** — lives in the `egui_tiles::Tree`, positioned by `rebuild_tree_from_modes`.
- **Floating** — rendered as a real `egui::Window` via `render_floating_panels`, using the same content-dispatch match as docked panels.
- **Closed** — drawn nowhere; reopened via the Windows menu.

This model already generalizes to new panel types without redesign: adding a name to `ALL_PANEL_NAMES` and a content-dispatch arm is the entire integration surface. Milestone 9 explicitly verifies this by docking/floating/closing a placeholder panel with no real content, proving the slot future modules — Experiment Manager, Replay Timeline, Genome Editor, Behavior Inspector, Research Notebook, Batch Runner, Profiler, Python Console, Plugin Manager — will land in later.

## Minimum sizes &amp; split constraints

Every panel must have a documented minimum size below which it clips content gracefully (scrolls) rather than overlapping a neighbor:

| Panel | Minimum width/height |
|---|---|
| Sidebar | 240px wide |
| Viewport | 320px wide, 240px tall |
| Neural Viewer | 240px wide |
| Metrics / Event Log (bottom tabs) | 160px tall |
| Floating windows | 280px wide (existing `min_width`), 200px tall when not minimized |

Split ratios themselves get min/max share constraints so a panel can be shrunk but never fully squeezed out by an accidental drag.

## Layout presets

Three named presets, each a fixed `PanelMode` + share configuration, selectable from the Windows menu:

- **Research** (default) — Sidebar + Viewport + Neural Viewer docked, Metrics/Event Log tabbed at the bottom. The current default layout.
- **Presentation** — Sidebar and Neural Viewer closed, Viewport maximized, Metrics floating (for screen-sharing a clean simulation view).
- **Debug** — everything docked and visible, including panels a researcher might normally close.

A **Restore Defaults** action resets to the Research preset via `apply_layout_preset`, which (Milestone 9) now supports all three named presets rather than only the one original default.

**New panel (Phase 2, M4/M5): "Research Dashboard"** — lists/compares experiment reports from `data/experiments/`. Closed by default in all three presets (same treatment as "Placeholder Panel"); open it from the Windows menu. Shares the root row alongside Sidebar/Neural Viewer/Placeholder Panel — added with zero changes to the docking model itself, the same forward-compatibility slot `docs/design/layout.md`'s Placeholder Panel was built to prove out.

**New panel (Phase 2, M6): "Replay Browser"** — static inspection of a loaded `.phylon-replay` bundle's recorded interventions (seed, tick range, every event). Not a live-playback "Replay Timeline": replay execution (`app::replay::run_replay`) is a separate headless mode that never coexists with the interactive UI, so a scrub/seek panel isn't currently possible without a larger architectural change (see `UI_PHASE2_ROADMAP.md`'s Execution Log for the full discrepancy writeup). Same closed-by-default treatment and root-row slot as Research Dashboard.

## Multi-monitor (future — not implemented this roadmap)

Floating windows already position independently of the main window via `egui::Window`, so dragging a floating panel to a second monitor works today at the OS level. True multi-monitor *workspace* support (remembering which monitor a floating panel lives on across restarts, or a fully independent second viewport) is out of scope for the current 13-milestone roadmap and is noted here as a deliberate future consideration, not a silent gap.

## Interaction standards for docking specifically

See [`components.md`](components.md) and the plan's Interaction Design Standards for the full list; the docking-specific rules are: drop-target regions highlight *before* release (not only after), invalid drop targets are visually distinguished from valid ones, and a floating window's last position/size is remembered across a dock→float→dock cycle within a session.
