# Phase 7 — Professional Scientific Workbench: Architecture Report & Roadmap

**Status: Phase A (audit) complete. Phase B (this report) and Phase C (this roadmap) complete. Phase D (implementation) has not begun — no code has been changed as part of Phase 7. This document is the required approval gate per `PHASE 7.md`'s own instruction: "Do NOT begin coding. Produce the roadmap first."**

Per `PHASE 7.md`: no biological work, no simulation expansion. Everything below is UX, workbench, maintainability, performance, architecture, and research-productivity scoped. Phase 6's biological roadmap (`PHASE6_RESEARCH_PLATFORM_ROADMAP.md`) is paused, not abandoned — it resumes only on explicit future direction.

---

## 1. How this audit was conducted

Four parallel read-only audits, no code written:

1. **UI/UX functional completeness** — every panel, dialog, menu, toolbar, shortcut, and the docking/workspace system, re-verified against current code (a lot changed this session under Phase 6's "Epic J — UI/UX Debt Cleanup," so a prior, separate, never-implemented UI planning pass from earlier in this session was known to be partly stale going in).
2. **Rendering architecture** — separation of concerns across viewport/overlay/selection/highlight/labels/particles/biological-VFX/inspector/charts/debug rendering; `crates/gpu` compute/render boundary; duplication.
3. **Repository-wide code health** — large files, dead code, duplicate code, large functions, every TODO/FIXME, crate-dependency-graph accuracy, across all ~30 crates.
4. **Research productivity & design tokens** — command palette, search, bookmarks, recent-items, undo/redo/history, and `theme.rs` token coverage.

Full findings are preserved in this session's transcript; this document synthesizes them into a prioritized, re-auditable roadmap. Every milestone below states what's actually wrong (not assumed) and cites where.

---

## 2. Architecture Report — key findings

### 2.1 The good news: several things Phase 7 asks for are already done

This matters for scoping — Phase 7 should not re-litigate work Phase 6's Epic J already completed this session:

- **Docking/undocking/floating/close/restore** all work correctly (`crates/ui/src/layout.rs`). Panel ratio persistence — previously broken — is now **fixed**: `extract_shares`/`share_of` round-trip live split ratios through dock/undock cycles.
- **The "two competing shortcut systems" bug is fixed** — `ShortcutManager::consume_all` is now the single active path; every menu-advertised shortcut (Ctrl+M/L/B, speed up/down, Ctrl+Shift+P) actually fires.
- **Chrome-bar triplication is fixed** — one `chrome_bar()` function now serves docked, tabbed, and floating panels.
- **`kv_row`/`kv_row_colored` duplication is fixed** — consolidated into `crates/ui/src/widgets.rs`, used by sidebar/inspector/dialogs/neural_viewer/replay_browser.
- **Metrics chart colors now derive from `ecology::Diet::standard_color()`** via `theme::chart_color()` — no more hand-picked divergent palette.
- **Neural Viewer has real zoom/pan** — the "unreadable past 10 hidden nodes" finding is stale.
- **Design tokens exist** — color/spacing/radius/icon/typography tokens are all in `theme.rs`; the "zero color tokens" finding from an earlier, unimplemented planning pass is stale.
- **`crates/gpu` is confirmed compute-only** — zero `RenderPass`/`RenderPipeline` usage. The render/compute boundary Phase 7 cares about is already clean at the crate level.
- **The codebase has almost no TODO backlog** — exactly 1 genuine live TODO in ~30 crates (`crates/app/src/systems.rs:371`), and zero `todo!()`/`unimplemented!()` panics anywhere.

### 2.2 Real gaps: workbench completeness (Phase 7 Goal 1)

| Capability | Status |
|---|---|
| Save/restore layout across app restarts | **Missing.** `panel_modes`/`layout_shares`/`dock_tree` are in-memory only; `app/src/preferences.rs` deliberately excludes them. Every restart reverts to the Research preset's shape. |
| Named layout presets | **Partial.** Only Research/Presentation/Debug exist (`layout.rs`'s `LayoutPreset` enum, closed/hardcoded). Phase 7 also wants Teaching/Evolution/Analytics. |
| User-defined custom workspaces | **Missing entirely.** No "Save Current Layout As…" of any kind. |
| Panel pinning (always-on-top) | **Missing** (the sidebar's icon-pin toggle is a different, unrelated feature). |
| General drag-to-tab merging | **Missing** — Metrics/Event-Log tabbing is hardcoded in `rebuild_tree_from_modes`, not a free user action. |
| Dead scaffolding relevant to this goal | A 10-variant `Workspace` enum (`Ecology, Biology, Evolution, Neural, Genetics, Rendering, Analytics, Performance, Debug, Settings`) exists in `crates/ui/src/state.rs`, is set once at construction, and is **never read again anywhere**. This is prior art for named workspaces — wire it up or delete it, don't leave it as dead weight. |

### 2.3 Real gaps: rendering architecture (Phase 7 Goal 5)

- **`crates/app/src/render.rs` (1617 lines) mixes categories Phase 7 wants separated.** Viewport instance-building, debug rendering, "biological VFX" (health rings, disease badges, growth fade-in, colony links), and selection/highlight bookkeeping are all computed in the same loops over the same `Vec<DebugInstance>`/SDF-bone lists, rather than through distinct per-category builders. `crates/rendering/` already has clean, generic, reusable *how-to-draw* types (`DebugRenderer`, `FieldRenderer`, `SplatComputePipeline`, `SdfSkinRenderer`) — the *what-to-draw* decision logic (which is where the mixing happens) all lives inline in `app`, not delegated anywhere.
- **No "Particles" category exists at all** (not necessarily a gap — just noting Phase 7's category list has no current owner; only relevant if a future biological-VFX need arises, and biology work is paused).
- **No world-space "Labels" category exists at all** — all text is egui-space (tooltips/tables), nothing draws entity names/ids into the wgpu-rendered viewport itself.
- **Duplication**: 3 near-identical food/mineral/corpse rendering blocks in `render.rs`; two independently-written graph-canvas viewers (`neural_viewer.rs` and `grn_viewer.rs`) share only the pan/zoom/hit-test extraction (`graph_canvas`), not their actual node/edge draw loops; a third hand-rolled painter-based viz exists in `hox_visualizer.rs`.

### 2.4 Real gaps: repository modernization (Phase 7 Goal 4)

- **`crates/scheduler` (441 lines) is a dead crate in production** — kept alive only by a benchmark and an integration test, not by the app itself (confirmed: `app`'s `Cargo.toml` has no `scheduler` dependency, consistent with this session's own Phase 6 Epic A removal). `crates/research` also carries an unused `scheduler` dependency.
- **Largest functions**: `growth_system` (`crates/organisms/src/systems.rs`, ~540 lines) is the single largest, and genuinely mixes 3 distinct sub-phases (brain wiring, segment growth/decode, apoptosis pruning) in one function body — this is the one large-function finding that's a real "one branch doing too much" case rather than "wide but flat."
- **Real duplication**: `init_gpu`/`init_gpu_headless` in `app.rs` share ~60 lines of near-identical adapter/device/pipeline-construction boilerplate. `events.rs`'s zoom handling repeats the same clamp/step logic 3 times. `ecology/src/lib.rs` (929 lines) is the one file that's genuinely "several distinct systems crammed together" rather than one cohesive concern.
- **Documentation staleness**: `docs/reference/crate_graph.md` documents only 20 of ~30 crates, claims `world` wraps `hecs` (it wraps `bevy_ecs` — no crate in the workspace depends on `hecs` at all), and claims `scheduler` still orchestrates `app`'s systems (it doesn't, as of this session).
- **Two stale `#[allow(dead_code)]` annotations** (`app.rs`'s `max_ticks_per_frame` and `storage` fields) — both fields are actually used elsewhere; the annotations are leftover, not real dead code.

### 2.5 Real gaps: research productivity (Phase 7 Goal 6)

- **Global search does not exist** — no cross-entity/organism/experiment search surface anywhere, only panel-local filters (Event Log, Lineage tab, Evolution Debugger).
- **"Recent Organisms" is tracked but never surfaced** — `recent_selections` (`state.rs`) is written every frame, read by nothing.
- **"Open Recent" (files) is fully dead** — `recent_files` is declared and read by the menu, but nothing in the entire codebase ever pushes to it, so the submenu never renders. Even if fixed, clicking an entry currently discards the specific path and just opens a generic file picker.
- **Command Palette, Bookmarks, and Undo/Redo removal are all in good shape** — Palette covers 23-24 real, working, context-free actions; Bookmarks are a real working camera-position feature, session-scoped by explicit design; Undo/Redo's removal (Phase 6, Epic J) is clean with zero remnants anywhere.

### 2.6 Design system gaps (Phase 7 Goal 2)

- Dialogs (`dialogs.rs`) and toasts (`render.rs`) are **partially** tokenized, not un-tokenized as originally assumed — spacing/radius/most colors already route through `theme.rs`, but several hardcoded literals remain: dialog sizes (`500.0, 400.0`), one off-palette orange in the onboarding dialog (the other 3 rows correctly use `GOOD`/`WARN`/`BAD`), toast stacking offsets and stroke width, and one hardcoded `Color32::WHITE`.
- `docs/design/spacing.md` claims a shadow/elevation token pair exists per radius tier — it doesn't; this is the one confirmed stale claim across the design docs (the other 8 docs largely match current code).
- One **unverified** (not confirmed, not dismissed) accessibility risk: Herbivore and Decomposer's colors share a near-identical blue channel (0.776 vs 0.789) — plausible tritanopia collision risk, not measured. Flagged for the same "measure before changing" discipline this project already applies to biology.

---

## 3. Roadmap — milestones

Ordered by priority within each epic; epics themselves are roughly priority-ordered but W1/W2 can run in either order. Every milestone follows the same discipline Phase 6 established: **re-audit the specific file immediately before touching it** (this report is current as of today, but code moves fast), verify with `build`/`clippy`/`fmt`/`test` plus a real interactive run (per this project's own `run` skill guidance — a UI change is not verified by `cargo test` alone), and stop after each milestone for review.

### Epic W1 — Dead Code & Documentation Truth (lowest risk, do first)

- **W1a**: Decide the fate of `crates/scheduler` — either delete it from the workspace entirely (if the benchmark/test have no independent value) or explicitly demote it to a benchmark-only fixture with its purpose documented. Remove `research`'s unused dependency on it either way.
- **W1b**: Fix or remove "Open Recent" — either wire `recent_files` for real (push on every save/load, make clicking an entry load *that* path instead of opening a picker) or delete the dead submenu. Small, well-scoped, real bug.
- **W1c**: Surface `recent_selections` in the UI (it's already tracked — likely the cheapest real win in this whole roadmap) or remove the tracking if not wanted.
- **W1d**: Resolve the dead `Workspace` enum — either becomes the seed of Epic W3's workspace-switcher, or gets deleted. Decide alongside W3b, not in isolation.
- **W1e**: Fix `docs/reference/crate_graph.md` — add the 10 missing crates, correct the `hecs`→`bevy_ecs` claim, correct `scheduler`'s described role.
- **W1f**: Remove the 2 stale `#[allow(dead_code)]` annotations.

### Epic W2 — Rendering Architecture Separation (Phase 7 Goal 5)

- **W2a**: Extract "what to draw" decision logic (health rings, disease badges, growth fade-in, category rings, colony links, spotlight dimming) out of `render.rs`'s per-node loop into dedicated builder functions/module(s) that produce instance lists — `render.rs` itself should orchestrate and dispatch, not compute biological-visual semantics inline.
- **W2b**: Deduplicate the 3 near-identical food/mineral/corpse rendering blocks into one generic per-entity-kind renderer.
- **W2c**: Consolidate `neural_viewer.rs`'s and `grn_viewer.rs`'s independently-written node/edge draw loops into one shared graph-canvas draw function (building on the pan/zoom/hit-test code they already share).
- **W2d**: Once W2a-c land, reassess whether `render.rs` still needs splitting into submodules — per Phase 7's own instruction, only split if it improves architecture at that point, not mechanically up front.

### Epic W3 — Workbench Completeness (Phase 7 Goal 1)

- **W3a**: Persist panel layout across app restarts (extend `app/src/preferences.rs` or a sibling file to serialize `panel_modes`/`layout_shares`/dock-tree shape).
- **W3b**: Add Teaching/Evolution/Analytics layout presets alongside the existing 3, deciding at the same time whether the dead `Workspace` enum becomes the real backing type for a named-workspace switcher (ties to W1d).
- **W3c**: User-defined custom workspaces ("Save Layout As…") — larger scope, own milestone, sequenced after W3a/b since it depends on the same persistence mechanism.
- **Deferred/stretch, not dropped**: panel pinning, general drag-to-tab merging — real gaps, genuinely lower priority than persistence/presets; revisit after W3a-c.

### Epic W4 — Design System Completion (Phase 7 Goal 2)

- **W4a**: Finish tokenizing `dialogs.rs`/toast rendering (specific literals identified in §2.6).
- **W4b**: Resolve the `spacing.md` shadow/elevation claim — add the tokens for real, or correct the doc; don't leave code and docs disagreeing.
- **W4c**: Measure (not assume) the Herbivore/Decomposer tritanopia risk before changing anything — same "measure before changing" discipline as Phase 6's colorblind fix.
- **W4d**: Cross-check the remaining `docs/design/*.md` files not yet verified (`components.md`, `layout.md`, `biological_visual_language.md`, `accessibility.md`) against current code.

### Epic W5 — Code Modernization (Phase 7 Goal 4)

- **W5a**: Split `growth_system` (`crates/organisms/src/systems.rs`) along its 3 real sub-phases (brain wiring / segment growth+decode / apoptosis pruning) — the one large-function finding that's a genuine mixed-responsibility case, not just length.
- **W5b**: Extract a shared device/pipeline-construction helper for `init_gpu`/`init_gpu_headless` in `app.rs`.
- **W5c**: Extract a `zoom_by()` helper in `events.rs` to collapse the 3x repeated zoom-clamp/step logic.
- **W5d**: Consider splitting `ecology/src/lib.rs`'s 6 systems into per-system files within the same crate (module reorganization, not a crate split) — it's the one file the audit found genuinely crammed rather than cohesive.
- **Lower priority, revisit only if time allows**: `window_event`'s oversized `RedrawRequested` arm, `reproduction_system`'s inline special-case, `SimulationSnapshot::from_world`/`restore_world`'s width — all "wide but flat," lower risk than W5a.

### Epic W6 — Research Productivity Additions (Phase 7 Goal 6)

- **W6a**: Surface `recent_selections` (same as W1c — listed here too since it's as much a productivity feature as a dead-code cleanup; implement once, closes both).
- **W6b**: Fix "Recent Files" to actually reopen the clicked path once populated (bundled with W1b).
- **W6c**: Global search across organisms/entities — larger, needs its own design pass (what's searchable, how results are shown, keyboard navigation) before implementation; do not start coding this without a short design note first.

### Epic W7 — Performance Measurement (Phase 7 Goal 7)

Phase 7 is explicit: *"Profile before optimizing... only optimize measured bottlenecks."* Before proposing any concrete optimization milestone, this epic's first job is re-auditing exactly what measurement infrastructure already exists (the audits noted `crates/benchmarks` and GPU timestamp queries already wired into `render.rs`) and what's missing, then building the missing measurement capability itself. **No optimization milestone should be scheduled until that re-audit happens** — deliberately left unspecified here rather than guessing at bottlenecks with no data, consistent with this project's own established discipline.

---

## 4. What this roadmap deliberately does not do

- No biology, no simulation-mechanics changes, per Phase 7's explicit scope.
- No mechanical file-splitting for its own sake — every split milestone above (W2d, W5a, W5d) is justified by a specific mixed-responsibility or duplication finding, not a line-count threshold alone.
- No optimization work without measurement first (Epic W7).
- No silent scope changes — every milestone here traces to a specific, cited audit finding; nothing is invented.

---

## 5. Execution Log

*(Empty — Phase D has not started. Entries will be appended here, one per milestone, following the same discipline as `PHASE6_RESEARCH_PLATFORM_ROADMAP.md`'s own execution log: re-audit, implementation, verification, remaining limitations, then stop for approval.)*
