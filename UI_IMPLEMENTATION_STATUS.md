# Phylon UI — Implementation Status & Remaining Work

**Document type:** Post-implementation UI/UX audit (analysis only — no code changes made in producing this document)
**Companion to:** [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md) (simulation/backend audit)
**Context:** A prior UI/UX audit scored the interface 5.8/10 against professional-tool conventions and produced a 13-milestone implementation plan (`logical-stirring-mist.md`). **That plan has since been substantially implemented** — this document re-verifies its actual completion state against the real codebase (not the plan's intentions) and defines what remains, using the same evidence-first discipline as the backend audit.

> **Headline finding:** the redesign the newest request describes wanting is, for the most part, **already built**. Design tokens, a shared widget library, chrome-bar consolidation, the shortcut-system bug, sidebar discoverability, status-bar zoning, docking-ratio persistence, layout presets, Neural Viewer zoom/pan, and (per the correction below) the viewport scale reference are all done, verified by file:line evidence below. What remains is narrower: a metrics-tokenization sweep, two small dead-code stubs, and a handful of stray literals — tracked in the Phase 1 execution log at the end of this document.
>
> **Correction (logged during Phase 1, M2):** item 9 below originally read "Not done," based on an Explore subagent's search that missed the actual implementation. Direct file inspection during M2 found `crates/ui/src/render.rs:483` (`render_scale_grid`) fully implements this — committed in `1dfb25e "Complete UI architecture roadmap Milestones 4-13"`, predating this audit. Per the standing rule ("if implementation differs from documentation, document the discrepancy, then follow the repository"), item 9 is corrected in place below rather than silently fixed, so the audit trail stays honest about what was actually re-verified vs. assumed.

---

## 1. UI Audit — Current State by Area

Verified directly against `crates/ui/src/**` (6,603 lines across `lib.rs`, `layout.rs`, `render.rs`, `shortcuts.rs`, `state.rs`, `theme.rs`, `types.rs`, `utils.rs`, `widgets.rs`, and 11 plugin files).

| # | Area | Status | Evidence |
|---|---|---|---|
| 1 | Shortcut system | **Done** | The old dead `render.rs::process_shortcuts` no longer exists anywhere in the crate (confirmed by grep). `render.rs` calls `state.shortcuts.consume_all(ctx, &mut actions)` as the single active path; `shortcuts.rs:93`'s `ShortcutManager::consume_all` is the one live implementation (screenshot, recording, import/export genome, save/load, speed, panel toggles). |
| 2 | Chrome-bar consolidation | **Done** | `layout.rs:285` defines one `chrome_bar` function; `panel_chrome` (236), `floating_chrome` (535), and the tabbed-pane path `top_bar_right_ui` (200) all delegate to it. Uses `theme::CLOSE_RED`/`theme::DETACH_BLUE` throughout — one stray non-semantic minimize-button literal remains (`layout.rs:356`, `Color32::from_rgb(180,180,60)`), outside the close/detach pair originally targeted. |
| 3 | Sidebar discoverability | **Done** | Icon+label activity bar with a pin/collapse toggle (`sidebar.rs:9-13,27-41,73-92`), defaulting to expanded. Rows use the shared `widgets::kv_row`/`kv_row_colored`/`kv_row_mono` — no private `grid_row` remains. |
| 4 | Status bar zoning | **Done** | Three documented zones — Simulation (48-105), Population (109-153), System/hover-reveal (155-187) — all built from `widgets::status_chip`, separated by `zone_separator` (32). |
| 5 | Metrics chart palette | **Partially done** | The 5 Diet-keyed Demographics series correctly call `theme::chart_color(&ecology::Diet::...)` (`metrics.rs:52-56,90-114`) — this was the highest-priority accessibility/consistency fix and it's real. **But** the Performance/Resources/Environment series (FPS, TPS, Mem, Food, Minerals, Corpses, Sunlight, O₂, CO₂, Temp) still use hardcoded `Color32` literals (121-227) — not a regression, just never in scope for the Diet-token fix, and no equivalent token category (e.g. `theme::CHART_PERF_*`) was ever defined for them. Axis titles/units and off-chart `chart_legend_dot` legends are done. Zoom/pan/time-range/export remain explicitly out of scope per the file's own doc comment — this was a deliberate, documented deferral, not an oversight. |
| 6 | Docking / window management | **Done** | `rebuild_tree_from_modes` now takes a live `shares: &HashMap<String,f32>` (`layout.rs:566-569`) instead of hardcoded constants; `extract_shares`/`collect_shares` (672-707) persist dragged ratios into `WorkbenchState::layout_shares` (`state.rs:217`) every frame. Three named presets — Research/Presentation/Debug (`layout.rs:711-758`) — exist with `apply_layout_preset`. |
| 7 | Neural Viewer zoom/pan | **Done** | `handle_pan_zoom` (`neural_viewer.rs:253-267`) reads drag delta for pan and `smooth_scroll_delta.y` for zoom (clamped 0.2–4.0), applied via `apply_view_transform`, wired into both the CTRNN and CPPN canvases, with a live zoom-percentage readout and an on-screen hint. |
| 8 | Dialogs / toasts tokenization | **Done** | `dialogs.rs` uses `theme::SIZE_DISPLAY/ACCENT/DISABLED_FG/SPACE_MD/SM` and `widgets::kv_row` throughout. Toasts (`render.rs:305-313`) use paired `theme::{ACCENT,GOOD,WARN,BAD}_SOFT`/base tokens, replacing four independent hand-picked color pairs (per that code's own doc comment). One stray literal remains outside this scope: the splash-screen "PHYLON" title color (`render.rs:186`). |
| 9 | Viewport scale reference | **Done** *(corrected — see note above)* | `crates/ui/src/render.rs:483` (`render_scale_grid`) draws a low-opacity, camera-relative world-space grid at a fixed `SCALE_GRID_STEP = 100.0` world-unit spacing (`render.rs:473`), with a "X units / grid" text readout anchored to the viewport's corner. Line count is bounded by visible extent ÷ step, not by zoom — negligible overhead. Toggle: `WorkbenchState::show_scale_grid` (`state.rs:159`, default `true`), exposed as View → "Show Scale Grid" (`menu.rs:182-185`). |
| 10 | Dead code | **Still present** | `plugins/navigation.rs:8-15` (`navigation_ui`) remains an empty no-op stub, its own doc comment still says "Currently a placeholder." `utils.rs:83`'s `draw_segment_tree` has no external call site anywhere in the crate — still genuinely dead. |
| 11 | Design documentation (`docs/design/*.md`) | **Done** | All 8 files exist (350 lines total) and are substantive, not stubs: `design_system.md` cross-references the other seven and states five concrete principles; `layout.md` documents exact panel ratios, the docking model, a minimum-size table, and the three named presets; `accessibility.md` contains a real Deuteranopia simulation table with hex values and documents a corrected finding (a Carnivore/Omnivore color collision, not the pair originally flagged) plus its fix. |

**Net result:** 9 of 11 areas fully done, 1 partially done (Metrics palette — by a documented, deliberate scope decision, not a shortfall), plus 2 small residual dead-code stubs.

---

## 2. Design System Specification (as it exists today)

The system requested — separated UI/simulation color roles, an 8-point-adjacent spacing scale, a 6-step type scale, consistent icon sizing — **already exists** in `crates/ui/src/theme.rs` (236 lines) and is documented in `docs/design/`. It is not a proposal; it is the current, in-force system every panel is built against.

### Typography (`docs/design/typography.md`, `theme.rs:133-194`)

| Token | Size | Use |
|---|---|---|
| `SIZE_DISPLAY` | 22 | Dialog/modal titles only (About, Keybinds) — never in the docked workbench |
| `SIZE_HEADING` | 18 | Section headings |
| `SIZE_SUBHEADING` | 15 | Panel/window titles, sub-sections (bumped from 14 for visible distinction from Body) |
| `SIZE_BODY` | 13 | Default app text, data-grid rows, buttons |
| `SIZE_SMALL` | 12 | Secondary/meta text (bumped from 11) |
| `SIZE_MICRO` | 11 | Status bar's system zone only — one deliberate exception, not an oversight |

Faces: IBM Plex Sans (Regular/SemiBold) for UI text, IBM Plex Mono for tabular/numeric readouts (status bar, Inspector stats) — registered via `install_fonts`, applied via `apply_style`.

### Spacing (`docs/design/spacing.md`, `theme.rs:13-38`)

`SPACE_XS(4) · SPACE_SM(8) · SPACE_MD(12) · SPACE_LG(16) · SPACE_XL(24) · SPACE_XXL(32) · SPACE_XXXL(48)` — matches the requested 8-point-adjacent scale exactly (4/8/12/16/24/32/48), with documented uses (inline-icon gap, toolbar-row gap, panel section gap, panel-region gutter, dialog padding, empty-state centering).

### Color (`docs/design/colors.md`, `theme.rs:62-131`)

UI chrome and simulation data are explicitly separated, closing the exact issue named in the request (cyan simultaneously meaning navigation accent, panel title, graph series, and entity color):

- **Chrome:** `CHROME_BG`, `VIEWPORT_FLOOR` (a fixed dark tone distinguishing the simulation canvas from surrounding chrome)
- **Interactive accent:** `ACCENT` / `ACCENT_INK` / `ACCENT_SOFT` — one accent, deliberately distinct from every diet and semantic color
- **Semantic:** `GOOD`/`WARN`/`BAD` (+ `_SOFT` muted variants) for toasts and status
- **Action-specific:** `CLOSE_RED`, `DETACH_BLUE`
- **State:** `DISABLED_FG`/`DISABLED_BG`, `FOCUS_RING`
- **Simulation data:** `chart_color(diet: &ecology::Diet)` re-derives from `ecology::Diet::standard_color()` on every call (with linear→sRGB conversion) rather than copying values, so a chart/legend/status-chip color can never drift from the simulation's own visual identity again — this is the load-bearing mechanism that fixes the "same color, different meaning" problem end-to-end for diet data specifically.

**Resolved (Phase 1, M1):** the non-diet chart series (Performance/Resources/Environment in Metrics) now have their own token category — `CHART_FPS/TPS/MEM`, `CHART_FOOD/MINERALS/CORPSES`, `CHART_SUNLIGHT/O2/CO2/TEMP` (`theme.rs`), documented in `docs/design/colors.md`.

### Icons (`docs/design/iconography.md`)

`ICON_SM(14)` inline glyphs, `ICON_MD(16)` toolbar buttons, `ICON_LG(20)` activity-bar/sidebar tabs, `ICON_XL(40)` splash-only (explicitly exempted from the standard chrome scale). One consistent Remix Icon subset throughout.

### Radius (`theme.rs:40-48`)

`RADIUS_TIGHT(4)` tooltips/graph canvases, `RADIUS_STD(8)` floating windows/toasts/context menus, `RADIUS_LOOSE(12)` dialogs/modals.

---

## 3. Component Library (as built, `crates/ui/src/widgets.rs`)

| Component | Signature | Consolidates | Used by |
|---|---|---|---|
| `chrome_bar` | `layout.rs:285` | 3 previously-separate chrome implementations (docked/tabbed/floating) | `panel_chrome`, `floating_chrome`, `top_bar_right_ui` |
| `kv_row` / `kv_row_colored` / `kv_row_mono` | `widgets.rs:19,32,46` | `sidebar.rs`'s private `grid_row`, `inspector.rs`'s hand-rolled label pairs, `dialogs.rs`'s about-grid rows | `sidebar.rs`, `inspector.rs`, `dialogs.rs` |
| `status_chip` | `widgets.rs:66` | Per-field icon+mono-value hand-rolling previously in `status_bar.rs` | `status_bar.rs` (all three zones) |
| `chart_legend_dot` | `widgets.rs:100` | A Unicode "●" glyph that silently tofu'd in IBM Plex Sans | `neural_viewer.rs` (originated here), `metrics.rs` |
| `empty_state` / `error_state` | `widgets.rs:114,127` | Ad hoc centered-label patterns previously scattered in `inspector.rs`, `neural_viewer.rs` | Panels with a "nothing selected"/"query failed" state |

All are plain functions taking `&mut egui::Ui` plus data — no props/component framework was introduced, matching the original plan's explicit choice to keep this as consolidation discipline, not an architecture change. Each is documented in `docs/design/components.md` with Purpose/Variants/States/Tokens/Accessibility/Owner/Dependencies.

---

## 4. Layout System (`docs/design/layout.md`, `layout.rs`)

- **Panel model:** Docked / Floating / Closed per named tile, generalized so a placeholder panel can be added to the panel-name set and docked/floated/closed like any existing one — verified extensible without redesign (this was explicitly checked as an acceptance criterion in the prior plan).
- **Ratio persistence:** `extract_shares`/`collect_shares` read live drag ratios out of the tile tree every frame into `WorkbenchState::layout_shares`; `rebuild_tree_from_modes` reads them back via `share_of()` instead of hardcoded constants — the "dragged ratio silently discarded" bug from the original audit is fixed.
- **Presets:** three named presets (Research / Presentation / Debug) with `apply_layout_preset`, plus documented minimum panel sizes.
- **Gap:** no multi-monitor support — documented in `docs/design/layout.md` as a future consideration, not attempted.

---

## 5. Remaining Work

| ID | Item | Why it's still open | Files | Difficulty | Priority |
|---|---|---|---|---|---|
| R-1 | ~~Non-diet Metrics series (Performance/Resources/Environment, 10 series) still hardcoded, no token category~~ | **Resolved, Phase 1 M1.** | `crates/ui/src/theme.rs`, `crates/ui/src/plugins/metrics.rs`, `docs/design/colors.md` | — | — |
| R-2 | ~~Viewport scale reference (world-space grid/scale bar) never built~~ | **Not actually open — corrected.** Already implemented (`render.rs:483`'s `render_scale_grid`, committed in `1dfb25e`); the original "Not done" finding was a subagent search miss, corrected during Phase 1 M2 (see §1, item 9's note). | `crates/ui/src/render.rs` | — | — |
| R-3 | ~~`plugins/navigation.rs::navigation_ui` is a permanent no-op stub~~ | **Resolved, Phase 1 M4 — deleted.** Its own doc comment confirmed navigation moved to the sidebar activity bar, so this was superseded code, not unfinished code. Also removed the vestigial, never-read `WorkbenchState::navigation_visible` field. | `plugins/navigation.rs` (deleted), `plugins/mod.rs`, `state.rs` | — | — |
| R-4 | ~~`utils.rs::draw_segment_tree` has no call site~~ | **Resolved, Phase 1 M4 — wired in.** Added a new "Body Plan" `CollapsingHeader` to the Inspector; `render_body_plan` builds the node/spring adjacency map, locates the organism's head node, and calls the existing (fully working) `draw_segment_tree` — completing a feature whose own doc comment already said "in the inspector," just never connected. Removed the now-inapplicable `#[allow(dead_code)]`. | `plugins/inspector.rs`, `utils.rs` | — | — |
| R-5 | ~~Stray non-tokenized color literals~~ | **Resolved, Phase 1 M3.** Sweep found a broader set than the original 2: minimize-button, splash title/Quit button, a duplicate `PANEL_BG`≡`CHROME_BG` constant, an inconsistent destructive-action red across 4 files, a duplicated playback-state color pair across 2 files, Event Log's 5-category palette, 2 duplicate `LIGHT_GREEN` "affirmative state" cues, and Neural Viewer's node/synapse colors (duplicated across its CTRNN/CPPN canvases). New tokens: `DANGER`, `PLAYBACK_LIVE`/`PLAYBACK_PAUSED`, `MINIMIZE_YELLOW`, `LOG_BIRTH`/`LOG_HAZARD`/`LOG_MUTATION`/`LOG_USER` (theme.rs, app-wide); `NODE_INPUT`/`NODE_HIDDEN`/`NODE_OUTPUT`/`SYNAPSE_EXCITATORY_BASE`/`SYNAPSE_INHIBITORY_BASE`/`CTRNN_CANVAS_BG`/`CPPN_CANVAS_BG` (neural_viewer.rs, file-local — not reused elsewhere). One deliberate non-fix: the CTRNN/CPPN canvas backgrounds were *not* unified — their own doc comment confirms the visual distinction is intentional (genotype vs. phenotype), so this was correctly left as two named constants, not one. | `theme.rs`, `layout.rs`, `render.rs`, `viewport.rs`, `toolbar.rs`, `status_bar.rs`, `event_log.rs`, `inspector.rs`, `neural_viewer.rs`, `docs/design/colors.md` | — | — |
| R-6 | Command palette, global search, keyboard-shortcut overlay, quick organism search, breadcrumb navigation, panel search | Named in the newest request's Phase 6 ("Interaction Improvements"); not part of the prior 13-milestone plan at all | New surface area, no existing files | Medium-High per item | Medium — genuinely new capability, not a consistency fix |
| R-7 | Minimap | Named in the newest request's Phase 5; not part of the prior plan | `crates/app/src/render.rs`, `crates/ui/src/plugins/viewport.rs` | Medium | Low-Medium |
| R-8 | Focus mode / fullscreen viewport / panel pinning beyond the sidebar's existing pin | Named in the newest request's Phase 6; sidebar already has a pin toggle (item 3 above) but this is a broader "focus mode" for the whole workbench | `crates/ui/src/layout.rs`, `crates/ui/src/state.rs` | Medium | Low |

---

## 6. Before/After Rationale

| Original finding | Before | After | Rationale |
|---|---|---|---|
| Dead shortcut system | Ctrl+M/L/B, ↑/↓, Import/Export Genome advertised in the menu but silently did nothing | One active `ShortcutManager::consume_all` path; every advertised shortcut fires | A shortcut that doesn't work is worse than no shortcut — it teaches users to distrust the menu |
| Three chrome-bar implementations, mismatched reds | `panel_chrome`/`top_bar_right_ui`/`floating_chrome` each independently styled, two different close-button reds | One `chrome_bar` function, one `CLOSE_RED`/`DETACH_BLUE` pair | A user shouldn't be able to tell which "mode" a panel is in from its close button's exact shade |
| Cyan overloaded across 4 roles | Navigation accent, panel titles, chart series, and simulation entity color all reused one hue | `ACCENT` (UI-only) vs. `chart_color(diet)` (re-derived from simulation truth) — no shared literal | The request's own stated issue — fixed at the mechanism level (a function call, not a copied constant) so it can't silently re-drift |
| Docked ratios didn't persist | Any dock/undock/reset action reverted a dragged split back to hardcoded shares | Ratios extracted from the live tree every frame, fed back into rebuilds | Losing a researcher's carefully-arranged workspace on every reset is a trust-breaking bug, not a cosmetic one |
| Sidebar icon-only, undiscoverable | No label, no tooltip-first affordance | Icon+label by default, collapsible, pinnable | First-time discoverability was the audit's top usability complaint |
| Status bar read as raw debug output | Flat list, inconsistent labeling | Three named zones (Simulation/Population/System), consistent `status_chip` styling | Grouping by meaning, not insertion order, is what turns a debug dump into a status bar |
| Neural Viewer had no zoom/pan | Fixed-scale canvas, unreadable for large genomes | Scroll-to-zoom, drag-to-pan, live zoom readout | A 40-hidden-node genome was previously as cramped as a 4-node one |

---

## 7. Implementation Roadmap for Remaining Work (Phase 8/9 equivalent)

Given 9/11 audit areas are already done (R-1 and R-2 resolved during Phase 1 M1/M2), the remaining roadmap is much smaller than a from-scratch 13-milestone plan. Recommended order:

1. ~~R-1 (Metrics non-diet palette)~~ — done, Phase 1 M1.
2. ~~R-2 (viewport scale reference)~~ — was already done pre-audit; corrected, Phase 1 M2.
3. **R-5 (stray literals)** — trivial sweep for the two remaining hardcoded `Color32` values.
4. **R-3/R-4 (dead code decision)** — low difficulty, but needs a product decision (delete vs. implement) before touching, not just a mechanical fix.
5. **R-7 (minimap)** — the viewport-overlay rendering path (`render_scale_grid`/`render_world_boundary`) is now a proven pattern to extend for this.
6. **R-6 (command palette, global search, shortcut overlay, etc.)** — the largest remaining item and genuinely new scope (not in the prior plan at all); warrants its own dedicated milestone breakdown once R-3/R-4/R-5 land, rather than folding into this document's remaining-work sweep.
7. **R-8 (focus mode / fullscreen viewport)** — pairs naturally with R-6's discoverability work; low priority standalone.

Each item follows the same Definition of Done already established and proven by the prior plan: `cargo check --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` clean, before/after screenshot comparison, a visual-QA pass, and an explicit performance-budget check for anything touching `render.rs`/`layout.rs`.

---

## 8. Executive Summary

**What's fully complete:** design tokens (including, as of Phase 1 M1, the non-diet chart series), the shared widget library, shortcut-system fix, chrome-bar consolidation, sidebar discoverability, status-bar zoning, docking-ratio persistence with named presets, Neural Viewer zoom/pan, dialog/toast tokenization, the viewport scale reference, and full design documentation (`docs/design/*.md`, 8 files). This is the large majority of both the prior 13-milestone plan and the newest request's stated goals.

**What's partially complete:** nothing significant remains partial — the Metrics palette gap (R-1) closed in Phase 1 M1.

**What's not started:** two small dead-code decisions (R-3/R-4) and two stray literals (R-5), plus a handful of newly-named interaction features from the newest request's Phase 6 (command palette, global search, shortcut overlay, breadcrumb nav, panel search) that were never part of the prior plan's scope at all.

**What should happen first:** the remaining low-difficulty sweep (R-5's stray literals, then a decision on R-3/R-4's dead code) before any new large-scope work — then the minimap (R-7), then the larger, genuinely-new command-palette/search/discoverability initiative (R-6/R-8) as its own dedicated plan.

**Await approval before implementation, per the request's own instruction** — this document is analysis only; no files were modified in producing it.

---

## Phase 1 Execution Log

Running log of Phase 1 milestones from the "Complete the Existing UI Architecture" directive, each independently verified/compiled/tested per its own report.

| Milestone | Outcome | Verification |
| --- | --- | --- |
| M1 — Metrics chart tokenization | Implemented: added `CHART_FPS/TPS/MEM`, `CHART_FOOD/MINERALS/CORPSES`, `CHART_SUNLIGHT/O2/CO2/TEMP` to `theme.rs`; replaced 10 literals in `metrics.rs`; documented in `colors.md`. | build/clippy/fmt clean, 180/180 tests pass |
| M2 — Viewport scale reference | No implementation needed — already complete (`render.rs:483`, committed pre-audit in `1dfb25e`). §1 item 9 and related R-2 entries corrected in place rather than silently fixed. | N/A — no code changed |
| M3 — Remaining hardcoded colors | Sweep found 9 groups beyond the original 2 (see R-5). Added 11 app-wide tokens to `theme.rs` and 7 file-local constants to `neural_viewer.rs`; removed the dead duplicate `layout::PANEL_BG` constant in favor of `theme::CHROME_BG`; updated 8 plugin files and `colors.md`. Deliberately did not unify the CTRNN/CPPN canvas backgrounds (confirmed intentional via the code's own doc comment). | build/clippy/fmt clean, 180/180 tests pass |
| M4 — Dead code resolution | `navigation.rs`/`navigation_visible` deleted (confirmed superseded, not unfinished, by its own doc comment). `draw_segment_tree` wired into a new Inspector "Body Plan" section (`render_body_plan` builds the adjacency map and head lookup); `#[allow(dead_code)]` removed. | build/clippy/fmt clean, 180/180 tests pass |
| M5 — Documentation sync | Fixed stale forward-looking language in `design_system.md` ("Status"), `layout.md` (ratio-persistence/presets described as future work, now done), `components.md` (two stale Milestone-7 references), and `colors.md` (a "Milestone 12 not yet closed" reference, and — the one real bug found — `DISABLED_FG`/`FOCUS_RING` described as "currently undefined" despite being fully implemented and wired in). Added a `components.md` entry for the Body Plan tree (M4). Found and fixed a genuine leftover code duplication while cross-checking `components.md`'s consolidation claim: `neural_viewer.rs` still had its own byte-for-byte-identical `legend_dot`, never migrated to the shared `widgets::chart_legend_dot` despite the doc already claiming it had been — deleted the local copy, redirected all 6 call sites. | build/clippy/fmt clean, 180/180 tests pass |

Implementation audit complete.
UI roadmap ready for review.
