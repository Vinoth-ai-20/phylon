# Phase 7 — Professional Scientific Workbench: Architecture Report & Roadmap

**Status: Phase A (audit) complete. Phase B (this report) and Phase C (this roadmap) complete, reviewed and approved by the user with the priority reordering and governing rules in §3. Phase D (implementation) has not begun — no code has been changed as part of Phase 7. Execution proceeds epic-by-epic in the order: W0 → W3 → W2 → W4 → W6 → W5 → W1 → W7.**

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

## 3. Governing rules (added on user review, apply to every milestone below)

The user reviewed this roadmap and approved it with the following conditions, which now supersede this document's original priority order and apply to all Phase D work:

1. **Priority order is user-workflow-first, not cleanup-first**: W0 → W3 → W2 → W4 → W6 → W5 → W1 → W7. The rationale: the biggest return at this stage comes from making the application feel complete before cleaning technical debt underneath it.
2. **No biology or simulation-mechanics features** during Phase 7, full stop — unchanged from the original scope, restated because it's a hard condition, not a preference.
3. **Every UI change must improve a measurable researcher workflow**, not just aesthetics. If a milestone can't name what a researcher gains, don't do it yet.
4. **Any refactoring must improve cohesion and ownership, not just reduce line count.** Mechanical splitting is out of scope regardless of epic.
5. **No optimization work begins until profiling data identifies a real bottleneck** (Epic W7's own founding rule, restated as a global condition).
6. **The four-question gate**: before implementing any milestone, it must be able to answer:
   - What does the researcher gain?
   - What architectural debt is removed?
   - What measurable metric improves?
   - How does this prepare Phase 8 (3D)?

   A milestone that can't answer these isn't ready to implement yet — either sharpen its scope until it can, or defer it.

## 4. Roadmap — milestones

Epic order per §3's governing rules: **W0 → W3 → W2 → W4 → W6 → W5 → W1 → W7.** Within each epic, sub-items are listed in the order they should be attempted. Every milestone follows the same discipline Phase 6 established: **re-audit the specific file immediately before touching it** (this report is current as of today, but code moves fast), verify with `build`/`clippy`/`fmt`/`test` plus a real interactive run (per this project's own `run` skill guidance — a UI change is not verified by `cargo test` alone), and stop after each milestone for review.

### Epic W0 — Interaction & Workflow Refinement (highest priority — added on user review)

**Scope discipline note**: several items the user's review associated with this epic were already checked by the Phase A audit and found to be in good shape, not friction points — restating them here as "confirmed already fixed" rather than re-scoping them as new work, so this epic doesn't silently duplicate or contradict §2's findings:
- Keyboard shortcuts — already fixed (Phase 6, Epic J); every menu-advertised shortcut works.
- Toolbar actions — audited, all wired to real handlers; nothing dead found.
- Viewport context menu — audited, entity-aware, all real actions.
- Onboarding — already implemented (Phase 6, Epic J), persists via `preferences.rs`, shows once not every launch.

**W0a (audit) — complete.** Traced the actual click/keystroke path for spawn / select / inspect / multi-select / save-load / play-pause, reading `viewport.rs`, `events.rs`'s click/hover/render-tick handling, and `state.rs`'s selection model directly rather than assuming. Findings, each answering the four-question gate:

1. **Left-click selection doesn't open the Inspector — the most natural gesture is the incomplete one.** `WindowEvent::RedrawRequested`'s pending-click handler (`crates/app/src/events.rs:844-856`) resolves a canvas click into `self.ui.selected_entity`/`tracked_entity` but never touches `active_tab`/`sidebar_visible`. The *less* discoverable path — right-click → context menu → "Inspect" (`viewport.rs:78-86`) — is the only one that reliably does `state.active_tab = SidebarTab::Inspector; state.sidebar_visible = true`. A researcher's first-instinct gesture (click the organism) doesn't show its details unless the Inspector tab already happens to be open.
   - *Researcher gains*: seeing what they clicked on, immediately, every time — not just when they happen to right-click or already have the right tab open.
   - *Debt removed*: an inconsistency between two selection entry points that should behave the same.
   - *Metric*: clicks-to-inspect for the common case drops from "1 if lucky, 2+ if not" to a reliable 1.
   - *Phase 8 prep*: a 3D viewport will have the same click-to-select-to-inspect need; fixing the entry point now means Phase 8 inherits one correct behavior, not two divergent ones.
2. **Left-click also silently engages camera-follow.** The same handler sets `tracked_entity = selected` unconditionally (`events.rs:854`) — selecting an organism to glance at it, and telling the camera to lock onto and follow it, are two different intents currently conflated into one click. Right-click → Inspect does *not* set `tracked_entity`, so the two selection paths differ in a second way too.
   - *Researcher gains*: being able to select/inspect something without the camera unexpectedly snapping to follow it.
   - *Debt removed*: same inconsistency as #1, different symptom.
3. **"Compare two organisms" does not exist as a feature**, though the data model has a start: `secondary_selected: HashSet<Entity>` (`state.rs:81`) is populated by marquee multi-select (`select_multiple`, `state.rs:646-650`) and used for viewport highlight/bulk actions (delete/kill), but the Inspector only ever reads `state.selected_entity` (singular) — selecting 2+ organisms gives a bulk-action highlight, never a side-by-side comparison view. This isn't a friction point to reduce, it's a real gap to decide on (build it, or explicitly descope it) rather than silently leave implied by the roadmap's own task list. **Resolved in W0c (below): deferred to a new future epic, W8 (§9) — not a W0-scope fix.**
4. **Save/load, play/pause/step/reset, and chart export are all already low-friction** (single toolbar click or single menu item, confirmed both here and by the Phase A audits) — no action needed.

**Correction to the Phase A audit surfaced during this pass**: one of the 4 parallel audits claimed `recent_selections` is tracked but never displayed. Direct verification found this is wrong — `render_recent_selections` is called from `inspector_ui` (`crates/ui/src/plugins/inspector.rs:69`) and renders a working, clickable "Recent:" row. Corrected in Epic W1/W6 above; no action needed there.

**Remaining W0 milestones, now scoped from W0a's actual findings rather than a guess:**

- **W0b** (complete — see execution log): Fix the left-click selection path to also set `active_tab`/`sidebar_visible` (finding #1) and to leave `tracked_entity` alone, matching what right-click → Inspect already does correctly for the tab/sidebar half.
- **~~W0c: build or descope "compare two organisms"~~ — resolved as an architectural decision, not a build.** Per user review: comparison is a confirmed future researcher workflow, but it is a new scientific-analysis capability, not an interaction refinement — out of scope for W0, which exists to remove friction from workflows that already exist. Not removed from the roadmap and not built here; promoted to its own future epic, **Epic W8 — Comparative Analysis Workspace** (§9), so it's tracked as a real commitment rather than either silently dropped or silently squeezed into a milestone too small for it. No code changed for this decision.
- **W0d**: Fix "Recent Files" (same bug as W1b — bundle, don't duplicate).
- **W0e**: A full read-through of `crates/ui/src/plugins/inspector.rs` (1034 lines, only spot-checked in Phase A) for interaction flow beyond the recent-selections question already resolved above — is the rest of its information surfaced in a sensible order, any redundant controls.
- **W0f**: Audit whether significant simulation events (births, deaths, hazards, mutations of note) get real, useful toast/notification feedback today (ties into W4a's tokenization work but is a workflow question first).

### Epic W3 — Workbench Completeness (Phase 7 Goal 1)

- **W3a**: Persist panel layout across app restarts (extend `app/src/preferences.rs` or a sibling file to serialize `panel_modes`/`layout_shares`/dock-tree shape).
- **W3b**: Add Teaching/Evolution/Analytics layout presets alongside the existing 3, deciding at the same time whether the dead `Workspace` enum becomes the real backing type for a named-workspace switcher (ties to W1d).
- **W3c**: User-defined custom workspaces ("Save Layout As…") — larger scope, own milestone, sequenced after W3a/b since it depends on the same persistence mechanism.
- **Deferred/stretch, not dropped**: panel pinning, general drag-to-tab merging — real gaps, genuinely lower priority than persistence/presets; revisit after W3a-c.

### Epic W2 — Rendering Architecture Separation (Phase 7 Goal 5)

- **W2a**: Extract "what to draw" decision logic (health rings, disease badges, growth fade-in, category rings, colony links, spotlight dimming) out of `render.rs`'s per-node loop into dedicated builder functions/module(s) that produce instance lists — `render.rs` itself should orchestrate and dispatch, not compute biological-visual semantics inline.
- **W2b**: Deduplicate the 3 near-identical food/mineral/corpse rendering blocks into one generic per-entity-kind renderer.
- **W2c**: Consolidate `neural_viewer.rs`'s and `grn_viewer.rs`'s independently-written node/edge draw loops into one shared graph-canvas draw function (building on the pan/zoom/hit-test code they already share).
- **W2d**: Once W2a-c land, reassess whether `render.rs` still needs splitting into submodules — per Phase 7's own instruction, only split if it improves architecture at that point, not mechanically up front.

### Epic W4 — Design System Completion (Phase 7 Goal 2)

- **W4a**: Finish tokenizing `dialogs.rs`/toast rendering (specific literals identified in §2.6).
- **W4b**: Resolve the `spacing.md` shadow/elevation claim — add the tokens for real, or correct the doc; don't leave code and docs disagreeing.
- **W4c**: Measure (not assume) the Herbivore/Decomposer tritanopia risk before changing anything — same "measure before changing" discipline as Phase 6's colorblind fix.
- **W4d**: Cross-check the remaining `docs/design/*.md` files not yet verified (`components.md`, `layout.md`, `biological_visual_language.md`, `accessibility.md`) against current code.

### Epic W6 — Research Productivity Additions (Phase 7 Goal 6)

- **~~W6a: Surface `recent_selections`~~ — retracted, finding was wrong.** Direct verification during W0a (see Execution Log) found `recent_selections` is already rendered by `render_recent_selections`, called from `inspector_ui` (`crates/ui/src/plugins/inspector.rs:69`) — a real, working "Recent:" chip row above the Inspector's content. One of the 4 parallel Phase A audits claimed this was tracked-but-never-shown; that claim is incorrect and is corrected here rather than carried forward. No action needed.
- **W6a (renumbered)**: Global search across organisms/entities — larger, needs its own design pass (what's searchable, how results are shown, keyboard navigation) before implementation; do not start coding this without a short design note first.

### Epic W5 — Code Modernization (Phase 7 Goal 4)

- **W5a**: Split `growth_system` (`crates/organisms/src/systems.rs`) along its 3 real sub-phases (brain wiring / segment growth+decode / apoptosis pruning) — the one large-function finding that's a genuine mixed-responsibility case, not just length.
- **W5b**: Extract a shared device/pipeline-construction helper for `init_gpu`/`init_gpu_headless` in `app.rs`.
- **W5c**: Extract a `zoom_by()` helper in `events.rs` to collapse the 3x repeated zoom-clamp/step logic.
- **W5d**: Consider splitting `ecology/src/lib.rs`'s 6 systems into per-system files within the same crate (module reorganization, not a crate split) — it's the one file the audit found genuinely crammed rather than cohesive.
- **Lower priority, revisit only if time allows**: `window_event`'s oversized `RedrawRequested` arm, `reproduction_system`'s inline special-case, `SimulationSnapshot::from_world`/`restore_world`'s width — all "wide but flat," lower risk than W5a.

### Epic W1 — Dead Code & Documentation Truth

- **W1a**: Decide the fate of `crates/scheduler` — either delete it from the workspace entirely (if the benchmark/test have no independent value) or explicitly demote it to a benchmark-only fixture with its purpose documented. Remove `research`'s unused dependency on it either way.
- **W1b**: Fix or remove "Open Recent" — either wire `recent_files` for real (push on every save/load, make clicking an entry load *that* path instead of opening a picker) or delete the dead submenu. Small, well-scoped, real bug.
- **~~W1c: Surface `recent_selections`~~ — retracted, see the corrected note under Epic W6a.** Already working; not a gap.
- **W1d**: Resolve the dead `Workspace` enum — either becomes the seed of Epic W3's workspace-switcher, or gets deleted. Decide alongside W3b, not in isolation.
- **W1e**: Fix `docs/reference/crate_graph.md` — add the 10 missing crates, correct the `hecs`→`bevy_ecs` claim, correct `scheduler`'s described role.
- **W1f**: Remove the 2 stale `#[allow(dead_code)]` annotations.

### Epic W7 — Performance Measurement (Phase 7 Goal 7, last)

Phase 7 is explicit: *"Profile before optimizing... only optimize measured bottlenecks."* Before proposing any concrete optimization milestone, this epic's first job is re-auditing exactly what measurement infrastructure already exists (the audits noted `crates/benchmarks` and GPU timestamp queries already wired into `render.rs`) and what's missing, then building the missing measurement capability itself. **No optimization milestone should be scheduled until that re-audit happens** — deliberately left unspecified here rather than guessing at bottlenecks with no data, consistent with this project's own established discipline.

---

## 5. What this roadmap deliberately does not do

- No biology, no simulation-mechanics changes, per Phase 7's explicit scope.
- No mechanical file-splitting for its own sake — every split milestone above (W2d, W5a, W5d) is justified by a specific mixed-responsibility or duplication finding, not a line-count threshold alone.
- No optimization work without measurement first (Epic W7).
- No silent scope changes — every milestone here traces to a specific, cited audit finding; nothing is invented.
- No W0 fix implemented before W0a's audit produces its findings — per the four-question gate, "reduce friction" isn't actionable until the friction is located.

---

## 6. Execution Log

### Epic W0, Milestone W0a — Interaction-workflow audit

**What was done**: Traced (not assumed) the actual click/keystroke path for the roadmap's own named common tasks by reading `crates/ui/src/plugins/viewport.rs`, `crates/app/src/events.rs`'s click/hover/redraw handling, and `crates/ui/src/state.rs`'s selection model directly.

**What was found**: 3 real findings, detailed in §4's Epic W0 section above — (1) left-click selection doesn't open the Inspector while right-click → Inspect does, an inconsistency between the two entry points a researcher would expect to behave the same; (2) left-click also silently engages camera-follow, a second inconsistency between the same two paths; (3) "compare two organisms" (named in this roadmap's own task list) does not exist as a feature, though `secondary_selected` multi-select data already exists and is used for bulk actions/highlight only.

**A correction to Phase A, found during this pass**: one of the 4 parallel Phase A audit agents incorrectly claimed `recent_selections` is tracked-but-never-displayed. Direct verification found `render_recent_selections` is genuinely called from `inspector_ui` (`crates/ui/src/plugins/inspector.rs:69`) and works. This is recorded here as a correction, not silently fixed by deleting the original claim — Epic W1's `W1c` and Epic W6's original `W6a` are both struck through with a pointer to this entry, per this project's own "supersede, don't rewrite history" discipline (mirroring how `PHASE6_RESEARCH_PLATFORM_ROADMAP.md` handles corrected findings).

**Verification**: this milestone is itself an audit — no code changed, nothing to build/test. Findings are cited to specific file:line locations, re-verifiable by anyone reading this log.

**Remaining limitations**: this audit covered the specific named tasks in the roadmap's own W0a description (spawn/select/inspect/multi-select/save-load/play-pause/chart-export), not an exhaustive tour of every possible workflow. W0e (a full read-through of `inspector.rs` for information architecture) and W0f (event-feedback audit) remain open, separate follow-on audits.

**Next**: W0b (fix left-click's Inspector/sidebar gap; confirm with the user whether camera-follow-on-select should also change, since it may be intentional) is the natural next milestone, being a small, well-understood, low-risk fix directly off this audit's top finding. Awaiting approval before implementing.

### Epic W0, Milestone W0b — Single selection pathway

**Scope, as directed by the user**: fix left-click/right-click/double-click/Follow inconsistency by introducing one canonical selection pathway every current and future selection source routes through, rather than patching each entry point's symptom independently.

**Architectural change**: added two methods to `WorkbenchState` (`crates/ui/src/state.rs`) as the sole pathways for their respective state:
- `select(entity)` — sets `selected_entity`, opens the Inspector tab, reveals the sidebar. Never touches `tracked_entity`.
- `set_follow(Option<entity>)` — the only method that sets `tracked_entity`. Independent of selection.
- `clear_selection()` (existing method, extended) now also clears `tracked_entity` — there's nothing left to follow once nothing is selected.
- `select_multiple()` (existing method, extended) now also opens the Inspector/sidebar, so marquee multi-select produces the same visible result as single selection.

Every direct field-mutation call site across the codebase was found (via `grep`, not sampled) and routed through these methods: the viewport's pending-click resolution and double-click handler (`app/src/events.rs`, `ui/src/plugins/viewport.rs`), the context menu's "Inspect"/"Track" actions (`MenuAction::SelectEntity`/`TrackEntity`), the Selection menu's `SelectAll`/`SelectHeadOf`/`SelectByDiet`/`InvertSelection`/`Deselect`, both Inspector "Track" checkboxes, the toolbar's Follow button and Spectator toggle, the spectator-mode auto-follow logic (`ui/src/render.rs`), camera-detach-on-drag/pan (`app/src/render.rs`, WASD/arrow keys), `CameraHome`, `KillEntity`/`DeleteSelection`'s tracked-entity cleanup, `ReseedEcosystem` (`app/src/interventions.rs`), and the Evolution Debugger's failure-list selection. A repo-wide grep after the edit confirmed zero remaining direct `tracked_entity = Some/None` assignments outside `set_follow` itself.

**Behavioral changes, matching the requested spec exactly**:
- Left-click: selects, opens Inspector, reveals sidebar, does **not** engage follow (previously it engaged follow unconditionally).
- Double-click: reworked to select (if needed) then dispatch the existing one-shot `MenuAction::FocusSelection` — previously it set `tracked_entity` directly, silently turning "look at this once" into permanent follow.
- Follow: the toolbar button is now a real toggle (`selectable_label`, so it has a clear active/inactive visual state) instead of a one-directional "turn on" button. The Inspector's two "Track" checkboxes and the context menu's "Track / Follow" item all now go through the same `set_follow`.
- Right-click context menu: unchanged in shape (still Inspect/Track/Export/Copy/Kill), but "Inspect" no longer duplicates the tab/sidebar logic inline — it now relies on `SelectEntity`'s handler doing that, removing the second, slightly different implementation W0a's audit found.

**Minor, disclosed edge-case behavior changes** (not requested explicitly, but a direct consequence of consolidating through `clear_selection`, and consistent with "every selection entry point produces identical UI state"):
- Clicking empty viewport space now clears `secondary_selected` too (previously it left a prior marquee multi-select's secondary entities highlighted).
- `ReseedEcosystem` now also clears `secondary_selected` (previously only primary/tracked were reset, leaving stale despawned-entity references behind).
- `InvertSelection` cycling with zero organisms left in the world now clears `tracked_entity` too (previously untouched in that specific empty-world edge case).

**Files changed**: `crates/ui/src/state.rs`, `crates/app/src/events.rs`, `crates/app/src/interventions.rs`, `crates/app/src/render.rs`, `crates/ui/src/render.rs`, `crates/ui/src/plugins/viewport.rs`, `crates/ui/src/plugins/inspector.rs`, `crates/ui/src/plugins/toolbar.rs`, `crates/ui/src/plugins/evolution_debugger.rs`.

**Research workflow improvement**: clicking an organism now reliably shows its details in one action, every time, regardless of which of the (formerly 2, now unified) entry points was used — the single biggest, most-cited friction point from W0a. Follow is now a visibly-stateful, independently-toggleable action instead of an accidental side effect of selecting or double-clicking.

**Verification**: `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` — all clean. `cargo test --workspace` — 0 failures across all 28 crates, including doc-tests (this milestone touched no tested logic paths directly, so the full suite passing unchanged is the relevant signal, not new test coverage). Launched the real windowed binary for a 12-second smoke run — started cleanly, no panics, no runtime errors in the log (only expected cosmetic wgpu warnings and a pre-existing, unrelated `bevy_ecs` `B0003` despawn warning already noted in this session's Phase 6 work).

**Remaining limitation, disclosed rather than silently claimed complete**: the 6 manually-verified behaviors requested (left-click always opens Inspector; left-click never starts follow; double-click focuses exactly once; Follow only activates explicitly; every selection entry point produces identical state; existing shortcuts/context-menu behavior still works) were verified by **tracing the code paths directly** (every call site was located and read, not sampled) plus a crash-free windowed smoke run — not by a human or automated click-through of the running app. No project skill or GUI-automation harness exists in this repo for driving real mouse input into this native winit/wgpu window, and building one was out of scope for this milestone. **Recommend the user manually click through the 6 listed scenarios** before considering this fully closed; if that surfaces a discrepancy, it should be filed as a new, small follow-up rather than assumed away.

### Epic W0, Milestone W0b — Follow-up tasks (per user review, before closing)

The user approved W0b and directed 4 small follow-ups before moving to W0c, on the basis that the selection pathway "should become the mandatory interaction pattern throughout the repository" — i.e. this isn't just a bugfix, it's now a standing architectural rule. All 4 are complete:

**1. ADR-W0-01** — added below, in §7.

**2. Manual interaction verification checklist** — added below, in §8, covering the 10 scenarios requested.

**3. Verification that no code outside the canonical pathway mutates `selected_entity`/`tracked_entity`/`active_tab`/`sidebar_visible`** — re-grepped the whole workspace for all four fields (not sampled):
- `selected_entity`/`tracked_entity`: zero remaining direct mutations outside `state.rs`'s `select`/`set_follow`/`clear_selection`/`select_multiple`, and the two intentionally-independent entity-specific invalidation sites (`KillEntity`/`DeleteSelection` clearing a *specific* just-despawned/deleted entity's own selection, which correctly does not go through the general-purpose `clear_selection` since it must not clear an unrelated entity's selection).
- `active_tab`: 2 remaining direct writes, both confirmed **not** selection-related and correctly left alone — `render.rs:423` (the pre-simulation splash screen's "Settings" button, jumping to the Settings tab before a simulation even exists) and `sidebar.rs:60` (the sidebar's own tab-bar click handler — switching tabs by clicking them is plain navigation, not entity selection, and must not require a fake entity argument to a `select()`-shaped call).
- `sidebar_visible`: 4 remaining direct writes, all confirmed **not** selection-related — the `ToggleSidebar` shortcut/action (`events.rs:290`), the View menu's Sidebar checkbox (`menu.rs:238`), the sidebar's own reveal-on-tab-click (`sidebar.rs:61`), and a settings-panel checkbox (`sidebar.rs:842`). These are generic show/hide controls, unrelated to what's selected.
- **Conclusion**: the canonical pathway is exhaustive for its actual scope (selection-driven state changes); the remaining direct writes are a different, legitimately-independent concern (plain tab/sidebar navigation) that should not be forced through it.

**4. TODO notes for future event-driven evolution** — added as doc comments (not implementations) on `WorkbenchState::select` and `set_follow` in `crates/ui/src/state.rs`, each noting that Phase 8 could emit `SelectionChanged`/`FollowChanged` events once a real event bus exists, and explicitly stating why one isn't invented now (no event bus exists yet in this crate; building one solely for this would be premature architecture).

**Verification after follow-ups**: `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `cargo test --workspace` — all clean (doc-comment-only changes beyond the audit itself; no logic changed).

**W0b is now closed.** Proceeding to W0c.

### Epic W0, Milestone W0c — Fate of "compare two organisms" (architectural decision, no code)

**Decision, per user review**: comparison is a confirmed future researcher workflow, but it is a *new scientific-analysis capability*, not an interaction refinement — W0's charter is removing friction from workflows that already exist, not adding a new one. Building even a minimal version here would be scope creep into a feature that deserves its own design pass (how many organisms, which fields, genome/GRN/neural/HOX/physiology/lineage comparison, experiment-level comparison, export) rather than being squeezed into an interaction-polish milestone. Descoping it entirely from the roadmap was also rejected — it's a real, named need, not a false lead.

**Resolution**: not removed, not built. Promoted to **Epic W8 — Comparative Analysis Workspace** (§9), explicitly marked as backlog/not-yet-scheduled rather than slotted into the current W0→W3→W2→W4→W6→W5→W1→W7 priority order. W0c itself is now closed with this decision as its complete output — no code changed, matching the milestone's own scope ("decide, don't build silently either way").

**Verification**: N/A — no code touched. The decision and its rationale are recorded here and in §9 for anyone auditing why this named task isn't in W0's implementation list.

**W0c is now closed.** Proceeding to W0d.

### Epic W0, Milestone W0d — Recent Files fix via a reusable `RecentItemsService`

**Scope, as directed by the user**: fix the confirmed Recent Files bug (never populated; clicking an entry opened a generic file picker instead of that entry) via a reusable service generic across future categories, not embedded directly in the menu — matching `ADR-W0-01`'s "one canonical pathway" discipline, applied here to recent-items state.

**Architectural changes**:
- New module `crates/ui/src/recent_items.rs`: `RecentCategory` (`Files`, `Replays`, `Experiments`, `Exports`, `WorkspaceLayouts` — only `Files` has a real producer; the rest are named extension points), `RecentItemsList` (private, capped at 10, MRU-ordered, deduplicating), and `RecentItemsService` (the public API: `record`/`remove`/`items`/`is_empty`, keyed by category). Binding policies documented in the module's own doc comment: ordering (MRU-first), duplicate handling (move-to-front, never duplicated), max history size (10, silent LRU eviction), missing-file behavior (zero filesystem I/O in this module; never auto-removes an entry — that's the UI layer's job, and always an explicit user action), persistence (via `Preferences`, same mechanism as `high_contrast`/`ui_scale`), and future extension (add a `RecentCategory` variant, nothing else changes shape).
- `WorkbenchState::recent_files: Vec<String>` (never populated by anything) replaced by `recent_items: RecentItemsService`.
- New `MenuAction::LoadStateFromPath(String)`, handled by a new shared `PhylonApp::load_state_from_path` method that both it and the pre-existing `LoadState` now call — one implementation instead of two, so a future change to load behavior can't silently apply to only one path. Checks `path.exists()` first and shows a toast + returns (never panics) if the file is gone, rather than attempting the load.
- `crates/ui/src/plugins/menu.rs`'s "Open Recent" block is now presentation-only: collects `state.recent_items.items(Files)`, checks existence per entry at render time, renders existing entries as clickable (`LoadStateFromPath`) and missing ones as a disabled, clearly-labeled "(missing)" button with an explicit "×" remove action. No policy logic lives in this file anymore.
- `SaveState`/`LoadState`/`LoadStateFromPath` all call `recent_items.record(Files, path)` — every path a user actually opens or saves gets remembered, symmetrically.
- Persistence: `app::preferences::Preferences` gained a `recent_items: ui::RecentItemsService` field (`#[serde(default)]`, so a preferences file saved before this milestone still loads instead of falling back to full defaults), loaded into `WorkbenchState` at startup and saved back at both existing exit paths (`Quit`, `CloseRequested`) — the same mechanism `high_contrast`/`ui_scale`/`onboarding_seen` already use, extended rather than duplicated.

**Files changed**: `crates/ui/src/recent_items.rs` (new), `crates/ui/src/lib.rs`, `crates/ui/src/state.rs`, `crates/ui/src/types.rs`, `crates/ui/src/plugins/menu.rs`, `crates/ui/Cargo.toml` (new `serde` dependency — this crate had none before), `crates/app/src/events.rs`, `crates/app/src/preferences.rs`, `crates/app/src/app.rs`.

**Verification**: `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` — all clean. `cargo test --workspace` — 0 failures across all 28 crates, including doc-tests. New tests: 6 in `recent_items.rs` (MRU ordering, move-to-front-no-duplicate, cap-with-eviction, explicit removal, category isolation, never-recorded-category-is-empty-not-a-panic) and an extended `preferences.rs` round-trip test asserting `recent_items` survives save/load. Ran the real windowed binary for an 8-second smoke test — clean start, no panics (grepped the log for panic/error, excluding known-cosmetic wgpu/`B0003` noise — none found).

**Remaining limitations, disclosed**: the manual verification list (open multiple files, reopen, delete one from disk, restart, verify ordering/dedup/persistence/graceful-missing-handling) was verified by tracing the code paths directly plus the smoke test above, **not** by an actual human/automated click-through — same disclosed limitation as W0b, for the same reason (no GUI-input-automation harness exists in this repo for this native window). Recommend manually running through the scenario list before treating this as fully closed. `Replays`/`Experiments`/`Exports`/`WorkspaceLayouts` categories exist in the type system but have no real producer yet — intentionally not built now, per "don't expand scope into a project-management system."

**W0d is now closed permanently.** Proceeding to W0e.

### Epic W0, Milestone W0e — Inspector interaction-flow read-through

**What was done**: read `crates/ui/src/plugins/inspector.rs` in full (1034 lines, only spot-checked in Phase A) for information architecture and redundant controls.

**Correction to a claim made during W0b**: W0b's follow-up-task-3 note said `crate::utils::draw_segment_tree` appeared to be dead code (no call site found by grep at the time). Direct reading during W0e found this was wrong — it's called from `render_body_plan` (line ~976 pre-edit), which is wired to the Inspector's "Body Plan" section. Corrected here rather than left standing.

**Findings, classified per the user's direction**:
- **Ordering**: sensible overall (Identity → Physiology → Genetics → Evolution/History → Neural → Morphology → Behavior → Ecology → Relationships/History → Body Plan). Two debatable placements (Ecology's position; Evolution/History collapsed-by-default between two open-by-default sections) — left unchanged (Category C: workflow refinement, not a correctness issue).
- **Category A (remove immediately — objectively incorrect, duplicated, or contradicted elsewhere)**: a permanently-`"Not Available"` `GenomeId` row in Identity, duplicating the real one Genetics already shows a few sections down (the exact fake-vs-real pattern SX-4d's own fix addressed for `SpeciesId`, missed here); permanently-`"Not Available"` `BodyPlan` and `SegmentTree` rows in Morphology — actively misleading, since a real, working segment tree renders a few sections below in "Body Plan"; permanently-`"Not Available"` `SensorArray`/`MuscleSystem` rows in Morphology, no backing data source. All 5 rows removed.
- **Category B (remove until implemented — no backing data model, no placeholder substituted)**: `EntityName` (Identity — no such concept exists anywhere in this codebase), `ActionState`/`MemoryState` (Behavior — no such concepts in `behavior`'s component set). All 3 removed, not replaced with "Coming Soon" or any other placeholder text, per explicit instruction.
- **Category C (left unchanged — workflow refinements, not correctness issues)**: Ecology's section placement, Evolution/History's placement, and the "Go to Head" button showing even when already at the head.

**Information architecture observations**: the Inspector already reads as a sequence of independent sections, not one undifferentiated form — 5 of them (`render_recent_selections`, `physiology_viewer_ui`, `circulation_viewer_ui`, `hormone_viewer_ui`, `immune_viewer_ui`, `lineage_viewer_ui`, `render_body_plan`) are *already* separately-defined functions reused verbatim (per ADR-P5-04's original decision), each following the same `(ctx, ui, state, world, actions)` shape. The remaining 8 sections (Identity, Physiology-summary, Genetics, Neural, Morphology, Behavior, Ecology, Relationships/History) are inline in `inspector_ui` but conceptually the same kind of independent widget — each queries its own component slice and renders independently of the others. Documented directly in the file's new module doc comment (a full section inventory in render order), so this structure is explicit rather than left to be re-discovered by the next reader.

**Future widget decomposition plan** (identified now, not performed — per explicit instruction not to physically split files in W0e): a future repository-modernization pass could extract each of the 8 still-inline sections into its own `fn foo_section_ui(ctx, ui, state, world, actions)` in its own module, exactly mirroring the pattern the 7 already-extracted functions demonstrate — this is not a hypothetical shape, it's the shape half the file already uses. This would shrink `inspector_ui` itself to an orchestration function (open a `CollapsingHeader`, call the section function) matching what `render_body_plan`'s own call site already looks like. Sequencing note for whenever this is picked up: do it as part of Epic W5 (Code Modernization) or a dedicated `ui`-crate-wide pass, not silently inside a future W0-numbered milestone, since W0's charter is interaction/workflow fixes, not file organization.

**Files changed**: `crates/ui/src/plugins/inspector.rs` only.

**Verification**: `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `cargo test --workspace` — all clean, 0 failures across all 28 crates. No behavioral logic changed (only dead-row removal and doc comments), so no new tests were needed or added.

**Rows removed**: 8 total — `GenomeId` (Identity), `EntityName` (Identity), `BodyPlan`, `SegmentTree`, `SensorArray`, `MuscleSystem` (Morphology), `ActionState`, `MemoryState` (Behavior).

**Duplicated logic eliminated**: the `GenomeId` fake/real duplication (Identity vs. Genetics) and the `SegmentTree` fake/real duplication (Morphology's dead row vs. Body Plan's real, working tree).

**Remaining limitations**: this was a read-through-and-trim milestone; it did not verify the Inspector's remaining, real rows are all still individually correct (that would be a much larger content-accuracy audit, out of scope here). The future widget-decomposition plan above is documentation only — no code was restructured.

**W0e is now closed.** Awaiting review before W0f.

---

## 7. Architecture Decision Records

**ADR-W0-01 — Unified selection/follow pathway; direct mutation of selection state is prohibited.**

- **Context**: before Phase 7, W0b, `selected_entity`, `tracked_entity`, `active_tab`, and `sidebar_visible` were each mutated directly from roughly a dozen independent call sites across `crates/app` and `crates/ui` (viewport click/double-click, 5+ Selection-menu actions, the context menu, two Inspector "Track" checkboxes, the toolbar's Follow/Spectator controls, spectator auto-follow, camera-detach-on-pan, kill/delete/reseed cleanup, the Evolution Debugger). No two of these implementations were guaranteed to agree, and in practice two didn't: viewport left-click and the context menu's "Inspect" action produced visibly different results for what a researcher would expect to be the same gesture (W0a's finding #1/#2).
- **Decision**: `crates/ui/src/state.rs`'s `WorkbenchState` exposes exactly two methods as the sole legitimate mutators of this state:
  - `select(entity: Entity)` — sets `selected_entity`, clears `secondary_selected`, opens the Inspector tab, reveals the sidebar. Never touches `tracked_entity`.
  - `set_follow(entity: Option<Entity>)` — the only method permitted to set `tracked_entity`. Independent of selection in both directions (following doesn't require selecting; selecting doesn't start following).
  - `clear_selection()` and `select_multiple()` (pre-existing methods) are extended to route through the same guarantees (clearing follow when clearing selection; opening the Inspector for multi-select, matching single-select).
  - **No code outside these four methods may assign to `selected_entity`, `tracked_entity`, `active_tab`, or `sidebar_visible` as a means of changing what's selected or followed.** This is a standing rule for all future work in this repository, not a one-time cleanup — confirmed by the user's own review: *"the introduction of a canonical selection pathway... should become the mandatory interaction pattern throughout the repository."*
  - **Explicit carve-out**: `active_tab`/`sidebar_visible` may still be mutated directly for reasons that have nothing to do with entity selection — plain tab navigation (clicking a sidebar tab button) and generic show/hide toggles (the Sidebar menu checkbox, `Ctrl+B`). These are a different concern (UI navigation, not "what organism is selected") and forcing them through `select()` would require a nonsensical fake entity argument. See W0b's follow-up-task-3 note above for the exhaustive list of these confirmed-legitimate exceptions as of this writing.
- **Reason**: a researcher's mental model is "I clicked an organism, therefore I am now looking at it" — that has to be true regardless of which of the many equivalent gestures (click, right-click → Inspect, a recent-selection chip, a future search result) they used. A single method is the only way to guarantee that as new selection sources are added (global search, lineage explorer, future 3D picking) they cannot reintroduce the same inconsistency, because there is no second place left to implement "select" differently.
- **Consequences**: any future PR that adds a new way to select an organism, or writes `.selected_entity =`/`.tracked_entity =` directly outside `state.rs`, should be treated as a defect against this ADR, not a style nitpick. `WorkbenchState::select`/`set_follow`'s own doc comments restate this rule at the point of use.
- **Future-trigger**: if/when a real cross-crate event bus is introduced (see the `TODO(Phase 8)` notes on both methods), `select`/`set_follow` become the natural emission points for `SelectionChanged`/`FollowChanged` events — this ADR's "single pathway" property is exactly what makes that later change a one-place edit instead of another repository-wide sweep.

**ADR-W0-02 — Recent Items Service.**

- **Context**: before Phase 7, W0d, "recent files" tracking was a `Vec<String>` field on `WorkbenchState` that nothing in the codebase ever wrote to — the "Open Recent" submenu could therefore never render (its emptiness check always passed), and even a hypothetical populated entry would have opened a generic file picker instead of that entry's own path, since the click handler just re-pushed the same `MenuAction::LoadState` regardless of which entry was clicked. Two independent bugs from one piece of un-owned state.
- **Decision**: introduce `crates/ui/src/recent_items.rs`'s `RecentItemsService` as the single owner of recent-items policy, generic over a `RecentCategory` so this shape is reusable rather than re-invented per feature.

  ```text
  Application  (crates/app — records what the user actually opened/saved,
       ↓        e.g. SaveState/LoadState/LoadStateFromPath handlers)
  RecentItemsService  (crates/ui — ordering, dedup, cap; the only
       ↓                place that knows the policy)
  Preferences  (crates/app — RON persistence; treats the service as an
       ↓        opaque, serializable field, same as high_contrast/ui_scale)
  Menu Presentation  (crates/ui/plugins/menu.rs — reads the service,
                       checks the filesystem, renders; owns no policy)
  ```

  This makes it immediately obvious where policy lives (the service, one box in the diagram) versus where it's just displayed (the menu, the last box, which does filesystem existence-checking for *display* purposes but never decides ordering/dedup/eviction).

- **Responsibilities** (`RecentItemsService`, exhaustive):
  - Recording an item as just-used (`record`) — enforces MRU ordering, move-to-front deduplication, and the 10-item cap.
  - Explicit removal (`remove`) — the only way an entry disappears from history.
  - Reading a category's current list (`items`, `is_empty`) in MRU order.
  - Being serializable as an opaque blob for `Preferences` to persist — it does not know *how* or *when* it's saved.
- **Explicit non-responsibilities** (binding — a permanent rule, not a one-time design note):
  - **No filesystem access of any kind.** The service never calls `Path::exists`, never reads, never writes a tracked file. It stores strings; it does not validate them.
  - **No automatic pruning.** A path staying in history after its file is deleted is correct behavior for this layer — "was recently used" is a historical fact that doesn't become false just because the file later vanished. Removal is always a caller's explicit decision.
  - **No UI rendering, no egui dependency.** `crates/ui/src/recent_items.rs` has no `egui::` reference anywhere in it.
  - **This filesystem-validation/presentation split is a permanent architectural rule, not specific to Recent Files**: any future consumer (Replays, Experiments, Exports, WorkspaceLayouts, or anything else built on this service) must keep existence-checking in its own application/presentation code, never pushed down into the service. If a future change needs the service to "know" whether a path is valid, that is a sign the abstraction is being misused, not a sign the service needs a new method.
- **Extension points**: adding a category is exactly one line — a new `RecentCategory` variant. Wiring a real producer for it (e.g. recording on replay-bundle open) is ordinary application code in whichever crate owns that action, calling `record`/`items` exactly like `events.rs` does for `Files` today. No change to `RecentItemsList`, `RecentItemsService`, or their persistence shape is ever required to add a category.
- **Future categories** (named now, not built): `Replays`, `Experiments`, `Exports`, `WorkspaceLayouts` — see `RecentCategory`'s own doc comment for the reasoning (pairs naturally with Epic W3's layout-persistence work for `WorkspaceLayouts` specifically).
- **Interaction with Preferences**: `Preferences::recent_items: ui::RecentItemsService` is a plain field, saved/loaded via the exact same RON round-trip as `high_contrast`/`ui_scale`/`onboarding_seen` — `#[serde(default)]` so a preferences file predating this field still loads. `Preferences` does not interpret the service's contents in any way; it is purely a persistence carrier, matching the diagram's middle-to-lower arrow.
- **Interaction with the UI**: `menu.rs`'s "Open Recent" block calls `state.recent_items.items(Files)` to get the list, then — and only then, in the presentation layer — checks `Path::new(path).exists()` per entry to decide whether to render it as a clickable button or a disabled "(missing)" row with an explicit remove control. The service is never asked "is this still valid"; it doesn't have an opinion.
- **Consequences**: any future PR that adds filesystem existence-checking, auto-pruning, or egui code inside `recent_items.rs` should be treated as a violation of this ADR. Any future PR that adds a new recent-items-like feature as a bespoke `Vec<String>` field instead of a new `RecentCategory` should be treated the same way.

---

## 8. Manual Interaction Verification Checklist — Selection & Follow (W0b)

Per the user's request, a standing checklist for a human to run through in the actual windowed app (this was not machine-verified beyond code-path tracing and a crash-free smoke run — see W0b's disclosed limitation above). Check off each; if any fails, file it as a small targeted follow-up rather than assume it away.

- [ ] **Left click** an organism in the viewport → it becomes selected, the Inspector tab opens automatically, the sidebar is visible if it was hidden, and the camera does **not** start following it.
- [ ] **Double click** an organism → the camera snaps to center on it exactly once (no continuous following afterward); double-clicking a second, different organism snaps again without leaving the first one "stuck."
- [ ] **Follow** (toolbar button) → clicking it while an organism is selected starts the camera smoothly following it, the button visibly shows an active/pressed state, and clicking it again turns following off (and the active state clears).
- [ ] **Track** (Inspector checkbox) → checking it starts following the inspected organism; unchecking it stops; this stays in sync with the toolbar Follow button acting on the same entity.
- [ ] **Marquee selection** (click-drag over multiple organisms) → all dragged-over organisms become selected/highlighted, the Inspector opens showing the primary one, and none of them are auto-followed.
- [ ] **Deselection** (Esc, or the context menu's "Clear Selection") → selection is fully cleared (including any marquee multi-select highlighting) and following stops.
- [ ] **Delete Selected** → the selected organism is removed from the simulation and selection/follow state referencing it is cleared without affecting an unrelated tracked/selected entity.
- [ ] **Reseed** (Simulation → Reseed Ecosystem, or equivalent) → selection and follow are both fully cleared (no stale references to now-despawned entities lingering in the Inspector or multi-select highlight).
- [ ] **Spectator mode** (toolbar toggle) → turning it on starts auto-following whichever organism the simulation considers "most interesting," switching targets over time; turning it off stops following and does not leave a stale Follow-button active state.
- [ ] **Context menu selection** (right-click → Inspect / Track / Export Genome / Copy ID / Kill) → "Inspect" selects and opens the Inspector (same as left-click); "Track / Follow" selects **and** starts following in one step; the other actions behave as before and are unaffected by this milestone.

---

## 9. Future Epic (Backlog — Not Yet Scheduled)

### Epic W8 — Comparative Analysis Workspace

**Status**: acknowledged as a real, confirmed researcher need (raised by W0a's audit, resolved as out-of-scope-for-W0 in W0c above). Not scheduled into the current priority order (W0 → W3 → W2 → W4 → W6 → W5 → W1 → W7) — this is deliberately a backlog entry, not epic "W9" tacked onto the end of that sequence, since it hasn't been sized or prioritized against the others yet.

**Initial scope** (per user direction — needs its own design pass before estimation, not a commitment to build all of this as one milestone):
- Side-by-side organism comparison
- Genome comparison
- Regulatory network comparison
- Neural network comparison
- Development/HOX comparison
- Physiology comparison
- Lineage comparison
- Experiment comparison
- Export comparison report

**Design requirements** (binding once this epic is scheduled, stated now so they're not lost):
- Build as a complete research workflow, not two duplicated Inspector panels side by side.
- Reuse existing Inspector widgets (`crate::widgets::kv_row`/`kv_row_colored`/`chart_legend_dot`, the graph-canvas draw logic once W2c consolidates it) wherever possible, rather than a parallel implementation.
- Synchronized scrolling, highlighting, and semantic difference visualization (e.g. highlighting which genes/synapses/physiology values actually differ) instead of two static, independently-scrolling panels a researcher has to manually cross-reference.
- Data model designed so a future Phase 8 3D comparison view can reuse it — this epic's own data layer should not assume a 2D-only presentation.

**Dependencies**: benefits from W2c (shared graph-canvas widget) landing first if neural/CPPN comparison is in scope, since it would otherwise duplicate the same node/edge draw logic a third time. Not blocked by it — could start with genome/physiology/lineage comparison (plain data, no graph rendering) independently.

**Next step when scheduled**: a dedicated design pass (which organisms, which fields, what "difference" looks like visually) before any implementation milestone is written — the same discipline this roadmap has applied to every other epic.
