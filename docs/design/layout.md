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

**Correction (Phase 7, W4d)**: this section previously presented a table of per-panel minimum sizes (Sidebar 240px, Viewport 320×240, Neural Viewer 240px, Metrics/Event Log 160px) and claimed split ratios get min/max share constraints. Re-checked directly against `layout.rs`: egui_tiles only supports **one single global minimum**, not a per-panel one — `layout.rs`'s own doc comment on `min_size()` states this explicitly, and the actual hardcoded value is `160.0`, applied uniformly to every split. The per-panel numbers above were aspirational targets this one global floor was meant to eventually back, not individually enforced minimums as the table implied. No min/max share-constraint code (no `min_share`/`max_share`/clamp) exists anywhere in `layout.rs` — shares are stored and applied via `set_share` with no bounds.

What's actually true today:

| Constraint | Value | Enforced by |
|---|---|---|
| Global split minimum (all docked panels) | 160px | `layout.rs`'s `min_size()`, applied uniformly — not per-panel |
| Floating windows | 280px wide (`min_width`), 200px tall when not minimized | `layout.rs`'s floating-window constructor |

Per-panel minimums and split min/max share bounds remain a real, un-implemented follow-up — tracked here as a gap, not silently dropped.

## Layout presets

Six named presets (Phase 7, W3b — expanded from the original three), each a fixed `PanelMode` + share configuration, selectable from the View and Windows menus (`LayoutPreset::ALL`, one shared list both menus iterate over):

- **Research** (default) — Sidebar + Viewport + Neural Viewer docked, Metrics/Event Log tabbed at the bottom. The general-purpose default layout.
- **Analytics** — Research, plus Research Dashboard (cross-experiment comparison) and Cell Lineage Viewer (population/lineage analytics) docked; Neural Viewer closed. For comparing runs/populations, not inspecting one organism's internals.
- **Evolution** — Research, plus Evolution Debugger and Cell Lineage Viewer docked (Neural Viewer already is); Research Dashboard closed. For within-run generational/genetic analysis, as distinct from Analytics's cross-experiment focus.
- **Teaching** — Sidebar + Viewport + Metrics/Event Log docked; everything else closed. Distinct from Presentation: Sidebar stays docked (so an instructor can click an organism and show its Inspector card live) and Metrics stays docked, not floating (anchored during a live explanation, not a movable window to manage).
- **Presentation** — Sidebar and Neural Viewer closed, Viewport maximized, Metrics floating (for screen-sharing/recording a clean simulation view with nothing to actively reference).
- **Debug** — everything docked and visible, including panels a researcher might normally close.

A **Restore Defaults** action resets to the Research preset via `apply_layout_preset` — the single function every preset (and Restore Defaults) routes through; no preset has its own parallel tree-construction path. `ui::state::Workspace` — an entirely separate, never-wired 10-variant enum from an earlier design pass (`Ecology`/`Biology`/`Evolution`/`Neural`/`Genetics`/`Rendering`/`Analytics`/`Performance`/`Debug`/`Settings`, set once at `WorkbenchState` construction and never read again) — was deleted rather than repurposed as a second "named workspace" concept: its variant names didn't match the real preset set above, and `LayoutPreset` was already the live, exercised mechanism.

**New panel (Phase 2, M4/M5): "Research Dashboard"** — lists/compares experiment reports from `data/experiments/`. Closed by default in all three presets (same treatment as "Placeholder Panel"); open it from the Windows menu. Shares the root row alongside Sidebar/Neural Viewer/Placeholder Panel — added with zero changes to the docking model itself, the same forward-compatibility slot `docs/design/layout.md`'s Placeholder Panel was built to prove out.

**New panel (Phase 2, M6): "Replay Browser"** — static inspection of a loaded `.phylon-replay` bundle's recorded interventions (seed, tick range, every event). Not a live-playback "Replay Timeline": replay execution (`app::replay::run_replay`) is a separate headless mode that never coexists with the interactive UI, so a scrub/seek panel isn't currently possible without a larger architectural change (see `UI_PHASE2_ROADMAP.md`'s Execution Log for the full discrepancy writeup). Same closed-by-default treatment and root-row slot as Research Dashboard.

## Multi-monitor (future — not implemented this roadmap)

Floating windows already position independently of the main window via `egui::Window`, so dragging a floating panel to a second monitor works today at the OS level. True multi-monitor *workspace* support (remembering which monitor a floating panel lives on across restarts, or a fully independent second viewport) is out of scope for the current 13-milestone roadmap and is noted here as a deliberate future consideration, not a silent gap.

## Interaction standards for docking specifically

See [`components.md`](components.md) and the plan's Interaction Design Standards for the full list; the docking-specific rules are: drop-target regions highlight *before* release (not only after), invalid drop targets are visually distinguished from valid ones, and a floating window's last position/size is remembered across a dock→float→dock cycle within a session.
