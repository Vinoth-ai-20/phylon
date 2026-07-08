# Phase 5 (SX) Status Summary — What's Done, What's Not, and Why

Companion to `PHASE5_SX_ROADMAP.md`, whose §6 table and §11 Execution Log remain the source of truth (file:line citations, verification commands, full disclosed-limitation text). This file is a condensed, at-a-glance status pass over that document — read the roadmap itself for the full reasoning behind any line below.

**Bottom line: all 9 epics (26 SX milestones) are implemented, verified, and documented.** Nothing in the roadmap's §6 table is outstanding. What remains is a small set of *known, disclosed* gaps below the milestone level — things measured and reported honestly, not silently dropped, and not blocking anything else in the roadmap.

---

## Done — by epic

| Epic | Status | What it actually changed |
|---|---|---|
| **1 — Simulation Readability** | ✅ Done (SX-1a–1e) | Population-wide behavior glyphs, health-brightness outline, disease tint, and death/reproduction floating text — all reusing existing `TimedEffects`/theme tokens, no parallel systems. |
| **2 — Living Organisms** | ✅ Done, with one disclosed gap (SX-2a–2d) | Found and fixed a real body-plan/genetics bug (seed CPPN could only ever decode monotonic Hox codes, making Muscle structurally unreachable) via a modular regulatory-CPPN rewrite (ADR-P5-07). Feeding-moment and growth-fade visuals added. **Gap:** only ~7.8% of founding organisms have an actuatable effector post-fix (up from 0.3%), vs. 90.7% in isolated testing — see "Not done" below. |
| **3 — Ecological Storytelling** | ✅ Done (SX-3a–3d) | Wired a dead `ReproductionEvent` to a real consumer, retired an unused event variant, added live species-distribution tracking, ancestor/descendant lineage chains, and colony-boundary visualization. |
| **4 — Selection Experience** | ✅ Done (SX-4a–4d) | Inspector no longer dead-ends into generic "Not Available": added an entity-existence check, live neural/mutation data, a relationships/trajectory section, and a real species-population figure. |
| **5 — Viewport UX** | ✅ Done (SX-5a–5c) | Opt-in organism labels (density-capped), Spotlight dimming (renamed from a collision with an unrelated existing feature), and fading trajectory trails. |
| **6 — Inspector Redesign** | ✅ Done (SX-6a–6d) | Folded 5 standalone P4-R-tier panels (Physiology/Circulation/Hormone/Immune/Lineage) into Inspector as collapsed-by-default sections, via direct reuse of their existing render functions — zero reimplementation. Standalone panels deliberately kept available too. |
| **7 — Scientific Visualization** | ✅ Done (SX-7a–7c) | Hazard/predation/lineage markers on the Demographics chart (fixed a real `tick: 0` bug along the way), per-series toggles + running-mean overlay, and per-chart PNG export (reusing the existing screenshot GPU readback, cropped). |
| **8 — Visual Hierarchy** | ✅ Done (SX-8a–8c) | Two-tier chrome system (Contextual vs. Secondary panels — accent bar + title color, not a size change), applied through the single already-consolidated `chrome_bar` function; fixed a real Evolution Debugger default-visibility bug; added a Hunting/Diseased status-bar zone. |
| **9 — Onboarding** | ✅ Done (SX-9a–9b) | One dismissible "Welcome to Phylon" dialog (not a tour), shown once per session; a full audit against all 5 of the roadmap's success criteria. |

**Verification, every milestone:** `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` all clean; `cargo test --workspace` shows 0 failures throughout, with real regression tests added where the milestone had testable logic (tick-fix, running-mean math, layout-preset fix, etc.).

---

## Not done / still open — and why

These are the honestly-disclosed gaps still standing after all 9 epics. None of them block the roadmap as written; they're either out of this roadmap's scope by design, or a measured limitation nothing in Phase 5 fully closes.

1. **Population-wide motion prevalence (Epic 2).**
   Only ~7.8% of founding organisms have at least one actuatable effector spring after ADR-P5-07's fix — up from 0.3%, but far below the 90.7% seen in isolated (non-in-app) testing. **Why not closed:** the gap's cause isn't isolated — plausible contributors (growth-completion timing, position-sampling differences from a real organism's pruned segment sequence, diet-weighted population effects) were named but not individually measured. This is the one success criterion in §9 marked only *partially* satisfied (SX-9b's own audit), not silently rounded up to "done."

2. **Neural Viewer zoom/pan and large-network scaling.**
   Never scheduled in this roadmap at all — flagged from the start (Phase 1 discovery / ADR context) as needing its own dedicated plan (layout algorithms, filtering, multi-select, activation playback), not a fit for this roadmap's milestone granularity. **Why not done:** deliberately out of scope, not forgotten.

3. **Metrics as a full analytics workspace** (zoom/pan, time-range selection, multiple Y-axes, saved chart presets).
   SX-7a/b/c added annotations, toggles, running-mean, and PNG export — the roadmap's actual ask — but explicitly did not build a full scientific-analytics workspace on top. **Why not done:** named in `metrics.rs`'s own doc comment as a follow-on initiative, to avoid silently expanding an already-bundled epic's scope.

4. **Selection-highlight decorative pulse (ADR-P5-08).**
   A pre-existing wall-clock sine pulse on the selection outline violates this phase's "no decorative animation" rule, found while auditing unrelated render code. **Why not done:** recorded as architectural debt with a recommended fix, but not assigned to any specific SX milestone — intentionally deferred to whichever future milestone next touches selection-highlight rendering, so it isn't lost.

5. **Fin body-plan segments via the direct Hox-code pathway.**
   ADR-P5-07's fix reaches Muscle (51.2% under mutation) but Fin remains at 0% via that specific code path; a separate branch-pair-derived Fin pathway may already work through `growth_system` but was never measured. **Why not done:** flagged as an open, unverified question rather than assumed either way — not this milestone's fix target.

6. **Onboarding hints are session-scoped, not persisted across restarts (SX-9a).**
   The dialog reappears every time the app restarts, not just the very first time ever. **Why not done:** this codebase has no settings/preferences persistence mechanism at all (confirmed by grep — nothing like `confy` or a settings file exists anywhere). Building one would be a separate initiative, not a Low-effort addition to a hint dialog.

7. **Tabbed-pane chrome titles don't pick up the Contextual/Secondary title color (Epic 8).**
   Only the accent-bar half of the tiering reaches Metrics/Event Log's tab-strip title, since `egui_tiles` draws that text itself, not `chrome_bar`. **Why not done:** both tabbed panels are Secondary tier anyway (no accent bar either way, so no visible regression), and fully theming `egui_tiles`' own tab-strip text would need a deeper styling-hook investigation than this milestone's budget allowed.

8. **A pre-existing unused `egui_plot` dependency in the `analytics` crate.**
   Noticed via grep while auditing Epic 7; nothing in `analytics/src` actually uses it. **Why not done:** unrelated to any file this epic touched — noted, not fixed, to keep the change scoped to what was asked.

9. **No visual/screenshot verification anywhere in Phase 5.**
   Every UI-facing milestone in this roadmap (Epics 1, 5, 6, 7, 8, 9) carries the same disclosed caveat: this session has no screen-capture/automation driver for the native wgpu desktop app, so "does this actually read clearly to a human" is unconfirmed for all of them — verified by code/logic only (build, clippy, tests), never by looking at the running app.

---

## Where to look for more detail

- **`PHASE5_SX_ROADMAP.md` §6** — the full milestone table with effort/risk per item.
- **`PHASE5_SX_ROADMAP.md` §11 (Execution Log)** — one dated, detailed entry per milestone (or bundle), each with its own Re-audit / Implementation / Verification / Disclosed Limitations sections.
- **`PHASE5_SX_ROADMAP.md` §5 (ADRs)** — the architectural decisions referenced above (ADR-P5-06/07/08 for Epic 2's motion investigation and the selection-pulse debt; ADR-P5-04/05 for Epics 6/8).
- **`PHASE5_SX_ROADMAP.md` §9 (Success Criteria)** — the 5 criteria SX-9b's audit checked, with the full reasoning behind the one partial mark.
