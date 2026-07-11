# Phase 6 — Research Platform Roadmap

> **Archived historical record.** This document describes the project as of when it was written and is retained only for provenance and source-code cross-references — it is not maintained going forward. For current documentation, see [docs/](../../index.md); for durable decisions and knowledge extracted from this document, see [Architecture Decisions](../decisions.md) and [Project History](../history.md).

**Status: APPROVED, frozen as the authoritative Phase 6 plan, implementation in progress (milestone-by-milestone). This document is not to be silently expanded or re-scoped during implementation — any decision change gets a superseding note, not a rewrite.**

This is the master roadmap for the work remaining before Phylon can reasonably be considered a modern computational artificial-life research platform, produced by a full repository audit following the completion of Phase 5 (Simulation Experience — `PHASE5_SX_ROADMAP.md`). Every claim below was verified against current source (file:line citations given where load-bearing), not assumed from a stale document. Where implementation had already superseded planning, that is called out explicitly rather than re-proposed.

**Approval conditions (recorded verbatim in effect, not paraphrased away):**

1. Work milestone-by-milestone, not epic-by-epic; re-audit the relevant source immediately before every milestone; never assume this document is still correct if the code has changed.
2. Preserve ADR discipline — a changed decision gets a superseding ADR/note, never a silent rewrite of history.
3. Continue the "measure before changing" philosophy — any biological or architectural modification must be backed by measurement or evidence, not assumption.
4. Do not redesign a completed system unless a verified architectural defect is discovered during the mandatory re-audit.
5. Every milestone must be deterministic, reproducible, and fully verified (`build`/`clippy`/`fmt`/`test`, plus milestone-specific validation) before being considered done.
6. Stop after every milestone (or an explicitly approved bundle) and report: what was implemented, what was discovered, verification results, remaining limitations, and whether the next milestone proceeds unchanged or needs a roadmap correction.
7. **Biological completion is prioritized ahead of expanding research-platform tooling** — Epic G (Research Platform Maturity) is deliberately sequenced after the biological/neuroscience/physiology epics (B-F) in §11, not before them.
8. A dedicated **Scientific Validation & Calibration** epic (§5, Epic M) is added, gating whether Phase 6 can be considered complete.
9. **No 3D implementation work begins in Phase 6.** 3D remains documentation/architecture-only, exactly as `PHASE4_EPIC9_3D_READINESS.md` already scoped it — this document does not change that.

---

## 0. A note on "Phase 6" — reconciling three numbering schemes

The audit found **three different phase-numbering schemes** in this repository, and they do not agree. This must be resolved before "Phase 6" can mean anything unambiguous:

1. **The in-repo roadmap-document lineage** (`PHASE3_ROADMAP.md` → `PHASE4_ROADMAP.md` + its 3 sub-roadmaps → `PHASE5_SX_ROADMAP.md`) — this is the scheme the user's request is written against ("Phase 5 (Simulation Experience)" just completed), and the one this document continues as **Phase 6**.
2. **`PHYLON_PROMPT_v2.md`'s own original 12-phase vision plan** (Phase 0 Foundation → ... → Phase 6 "Neural Systems" → ... → Phase 12 "Distributed and Enterprise Expansion"). Under *this* scheme, "Phase 6" means something narrower and different (NEAT/Hebbian/neuromodulators) — and the codebase's actual CTRNN+CPPN brain work is already well past where this scheme's "Phase 5" (Evolution/genetics) would place it. This scheme was evidently abandoned early and never used as the actual execution sequence.
3. **A stray reference to an internal "Phase 16"** in `docs/how_to/troubleshooting.md` ("In Phase 16, organisms are spawned with completely randomized neural wiring..."). No other document corroborates a 16-phase sequence; this reads as a leftover artifact from an earlier internal iteration, not a live numbering scheme.

**Resolution adopted by this document:** "Phase 6" below refers exclusively to scheme (1) — the direct sequel to `PHASE5_SX_ROADMAP.md`. Scheme (2)'s vision content is treated as the project's long-horizon aspirational spec (§3, §9), not a phase count to reconcile against. Scheme (3) is logged as a documentation-debt item (§4.7) worth a cleanup pass, not investigated further here. This mismatch is itself evidence that `IMPLEMENTATION_STATUS.md` and the `docs/explanation/*` set have drifted from what actually shipped — addressed in §4.7.

---

## 1. Executive Summary

Phylon's core simulation is substantial and largely sound: a 28-crate deterministic ECS architecture, GPU-accelerated physics and diffusion, a CPPN-driven genetics/development pipeline (Phase 3), a full physiology/regional-anatomy/life-cycle layer (Phase 4), and a UI that went from a 5.8/10 audit score to a fully tokenized, redesigned workbench with scientific-visualization and onboarding support (Phase 5). All of Phases 3-5's own milestone tables are closed, verified via `cargo build`/`clippy`/`test`/`fmt` at every step.

What remains falls into two very different tiers:

- **Tier A — finish what's already started.** Two Phase 4 epics (Regional Brains, Reaction-Diffusion Morphogens) already have fully-audited, approval-ready sub-roadmaps sitting untouched (`PHASE4_EPIC1_NEURAL_ROADMAP.md`, `PHASE4_EPIC4_MORPHOGEN_ROADMAP.md`). A `research` crate and an `app::batch` orchestrator already run real multi-seed headless experiments and write Markdown/RON reports — but the loop has no statistical analysis, no comparison tooling, and no way to sweep anything but the RNG seed. A `network` crate already runs a real single-agent MARL WebSocket server. A `plugins` crate already runs a sandboxed `rhai` scripting engine for scenario authoring. None of this is greenfield; all of it needs depth, not invention.
- **Tier B — repair confirmed defects.** A handful of real, verified bugs sit in already-shipped code: three call sites (`ecology::lib.rs:149-156,605-607`, `organisms::systems.rs:560-564`) use unseeded `fastrand::` instead of the project's own seeded `SimRng`, silently breaking the bit-exact-determinism guarantee `CONTRIBUTING.md` calls non-negotiable. `autosave_interval_ticks` is a config field with a default value and zero readers — autosave is 100% manual despite the config implying otherwise. A `SimulationScheduler` is constructed every run and never advanced. Two menu buttons ("Screenshot"/"Recording" in the Tools menu) show a "Not yet implemented" tooltip and silently do nothing, even though both features are fully implemented and working elsewhere (the toolbar button and `Ctrl+Shift+S`/`Ctrl+Shift+R` shortcuts). Seven `MenuAction`s (`Undo`, `Redo`, `DuplicateSelection`, `SpawnPaste`, `JoinSelection`, `GrabSelection`, `FocusSelection`) are advertised in the Keybinds dialog and/or menus but only `tracing::warn!()` and do nothing.

Beyond these two tiers sits a much larger, explicitly out-of-scope-for-Phase-6 category: the original vision document (`PHYLON_PROMPT_v2.md`) describes terrain/weather/seasons/climate, colonial/microbial/quorum-sensing biology, 14 sensory modalities beyond the 9 implemented, 22 sandbox "god-mode" tools, ML backend integration (`burn`/`candle`/`pyo3`), and a distributed/multi-user platform — essentially none of which exists today. This is named honestly in §9 as the long-horizon frontier, not folded into Phase 6's epics, per the instruction not to invent biology or propose speculative engineering.

**This document proposes 12 Phase 6 epics**, ordered by a mix of severity (determinism repair first) and dependency (things already fully audited and approval-ready next), covering Architecture, Biological Development, Neuroscience, Physiology, Life Cycle, Research Tooling, Scientific Visualization, Verification Strategy, UI/UX Debt, Performance, and Documentation. No code changes are proposed to be made until this document is approved.

---

## 2. Repository Status (verified)

- **28 workspace crates** (`Cargo.toml` members, confirmed): `common`, `config`, `events`, `scheduler`, `world`, `spatial`, `physics`, `diffusion`, `organisms`, `genetics`, `evolution`, `reproduction`, `behavior`, `metabolism`, `sensing`, `brain`, `learning`, `environment`, `ecology`, `gpu`, `rendering`, `ui`, `analytics`, `storage`, `research`, `network`, `plugins`, `tests`, `benchmarks`, `app`.
- **Phase 3** (`PHASE3_ROADMAP.md`): GRN/Hox/CPPN-driven development — all 13 core milestones done; 2 stretch goals (M14 neural-development coupling, M15 metamorphosis) explicitly deferred, later superseded by Phase 4 Epics 1 and 5 respectively.
- **Phase 4** (`PHASE4_ROADMAP.md` + 3 sub-roadmaps): persistent Body Graph, per-segment physiology (transport/endocrine/immune/waste), life-cycle re-entrant growth, 5 research-instrumentation panels, interaction VFX, 3D-readiness audit — all done **except** Epic 1 (Regional Brains) and Epic 4 (Reaction-Diffusion Morphogens), which were deliberately scoped out into their own sub-roadmaps and never implemented (no code, proposals only, awaiting approval — confirmed still true).
- **Phase 5** (`PHASE5_SX_ROADMAP.md` + `PHASE5_SX_STATUS.md`): all 9 epics / 26 milestones done — behavior/health/disease visual language, motion-capability fix (ADR-P5-07), ecological storytelling, Inspector redesign, viewport UX, scientific-visualization Metrics upgrades, panel visual hierarchy, onboarding. One success criterion (population-wide motion prevalence) explicitly marked only partially satisfied.
- **UI tracks** (`UI_IMPLEMENTATION_STATUS.md`, `UI_PHASE2_ROADMAP.md`): both fully closed and explicitly frozen; `UI_PHASE2_ROADMAP.md` itself names the follow-on work (Interactive Replay, tick-tied Bookmarks, Density Maps, `palette`-crate migration) as belonging in a `UI_PHASE3_ROADMAP.md` that does not yet exist — folded into this document's epics instead of a separate UI-only roadmap, since most of it depends on the same replay/research-platform work Phase 6 already needs to do.
- **Verification standard maintained throughout**: every closed milestone in Phases 3-5 was checked against `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `cargo test --workspace`. This document assumes the same bar for every Phase 6 milestone.

---

## 3. Categorized Remaining Work (by research theme, not by historical phase)

### 3.1 Biological Realism & Development
- Regional brains (anatomical neural structure) — audited, not implemented (§5 Epic C).
- Reaction-diffusion morphogens + temporal gene expression — audited, not implemented (§5 Epic D).
- Ganglion/Vascular segments still share Torso's generic stiffness — no differentiated physics (Phase 3 M14 stretch, never done).
- Metamorphosis / re-differentiation (DEF-004) — genuinely hard, needs a life-stage trigger system that "doesn't exist in any form" per Phase 3's own audit; still true.
- Fin segments unreachable via the direct Hox-code pathway (0% per ADR-P5-07's measurement); a separate branch-pair-derived Fin pathway may already work but was never measured.

### 3.2 Behavior & Neuroscience
- Population-wide locomotion prevalence stuck at ~7.8% of founders having any actuatable effector (vs. 90.7% in isolated per-position testing) — ADR-P5-07's own unexplained gap.
- `learning` crate: real `PolicyProvider`/`ExternalAgent` interface, zero bundled backend (`burn`/`candle`/`pyo3` all absent by design), single-external-agent only — no true multi-agent RL.
- `network` crate: a genuinely working single-agent MARL WebSocket server (`NetworkServer`, confirmed wired via `app::learning_bridge`) — multi-user collaboration sessions not built.

### 3.3 Ecology
- True diffused disease-concentration field (DEF-009) remains deferred, explicitly pending the morphogen-field work (§5 Epic D) landing first.
- Extended `Diet` taxonomy (Fungivore/Scavenger/Parasite/Detritivore, DEF-008) — vision-only, not started, orthogonal to any current epic.
- Lineage ancestor-chain depth: `LineageTracker::extract_completed_records()` runs unconditionally every tick, so ancestor chains are almost always 0-1 hops deep in practice (SX-3c's own disclosed architectural asymmetry) — real limit on any "multi-generation developmental evolution" analysis.

### 3.4 Physiology
- Every per-segment rate/threshold constant introduced in Phase 4 (`TRANSPORT_RATE`, `ENDOCRINE_RATE`, immune `SPREAD_RATE`/clearance baseline, `DEATH_EFFECT_DURATION_TICKS`, `MATURITY_AGE_FRACTION`) is confirmed, in its own doc comment, as an untuned placeholder.
- None of Phase 4's new per-segment state (`HormoneLevel`, `SegmentInfection`, `SegmentImmunity`) is included in `storage::SimulationSnapshot` — a saved-and-reloaded organism silently loses all of it. The persistent Body Graph itself (`DevelopmentalGraph`) has the same gap.
- Circulation Viewer shows static per-segment levels, not animated flow — disclosed simplification in both P4-V2 and SX era.

### 3.5 Research Productivity
- `research`/`app::batch` already runs real, working multi-seed headless batches and writes per-seed + aggregate Markdown/RON reports — but the only swept parameter is RNG seed; runs are sequential (no parallelism); `ExperimentReport` carries exactly 3 fields (`ticks_run`, `final_population`, `final_species_count` — confirmed via direct read of `crates/research/src/lib.rs`); there is no statistical analysis (mean/variance/confidence interval) across seeds and no comparison dashboard (DEF-012, confirmed still open — data exists, nothing visualizes it).
- No parameter-sweep mechanism exists at all — every experiment variation requires editing `crates/app/src/app.rs` directly and recompiling (confirmed via `docs/how_to/add_custom_genomes.md`/`modify_environment.md`), despite a working `rhai` scripting crate (`plugins`) and a `.ron`-based `config` crate already existing as the natural home for this.
- `.phylon-research` bundled archive format (DEF-013) — time-boxed out of the original roadmap, still not built; CSV/RON exist as separate artifacts, not one bundle.
- Replay is real but headless-only and action-based (`storage::replay::ReplayLog`/`ReplayBundle`) — no scrub/seek timeline, no bookmarks, no annotations exist anywhere in the codebase (confirmed via grep — no stub even). Interactive/live-scrub replay was explicitly deferred (`UI_PHASE2_ROADMAP.md` ADR-001) as a distinct architectural change (would require restructuring `main.rs`'s branching).
- Genome/brain/morphology cross-organism comparison already exists in the Evolution Debugger panel (mutation diff, parent-vs-offspring, arbitrary-pair) — this is more built than the vision doc's "comparison tooling" ask suggests; the gap is specifically at the *experiment* level (comparing whole runs/seeds), not the organism level.

### 3.6 Scientific Visualization
- Neural Viewer: zoom/pan already shipped (UI Phase 1); scaling to 100+ node genomes, layout algorithms, filtering, multi-select, and activation playback remain explicitly flagged (since Phase 5 Epic 8's own doc comment) as needing a dedicated sub-plan, never started.
- Metrics dashboard: annotations/toggles/running-mean/PNG-export shipped (Phase 5 Epic 7); zoom/pan, time-range selection, multiple Y-axes, and saved chart presets remain an explicitly named, unscheduled follow-on.
- HOX Visualizer intentionally still recomputes the body graph rather than reading the now-persistent `DevelopmentalGraph`, and per-segment "produced organs"/deep Lineage-Explorer cross-linking was deliberately deferred.
- `docs/design/biological_visual_language.md`'s own "not yet covered" list: Disease/Infection state absent from Inspector's Ecology section; Development section absent from Inspector; `LOG_MUTATION` event-log category defined but nothing ever publishes to it; Speciation has no viewport badge or NarrationLog announcement; population-level behavior/disease-count charts don't exist.
- Publication-quality output: Phase 5's PNG export (SX-7c) crops a whole-window screenshot readback — real and working, but raster-only, one chart at a time, no vector/figure-quality export path.

### 3.7 Performance
- `README.md`'s own Performance Targets table (100,000 active organisms, 512 max chunks) is stated as an engineering target, not a benchmarked, confirmed achievement — no evidence in any audited document that this has been measured at that scale.
- Benchmark coverage is thin: only `metabolism_parallel` and `scheduler_throughput` benchmarks exist; none of Phase 4's transport/endocrine/immune systems or Phase 5's UI-adjacent systems have benchmark coverage (DEBT-013, confirmed still true).

### 3.8 Architecture
- **Determinism break, confirmed live in source**: `fastrand::f32()`/`fastrand::usize()` calls at `crates/ecology/src/lib.rs:149-156,605-607` and `crates/organisms/src/systems.rs:560-564` bypass the seeded `common::SimRng` every other system uses — a real, currently-shipping violation of the project's own stated non-negotiable determinism guarantee.
- `SimulationScheduler` is constructed every run (`crates/app/src/app.rs:205`) and carries an `#[allow(dead_code)]` field — it is never advanced; the real tick loop is hand-written elsewhere (confirmed via `main.rs`'s own doc comment admitting this).
- No schema-migration framework exists — every `GENOME_SCHEMA_VERSION` bump (already happened multiple times across Phases 3-5) is a hard, undocumented-to-users break with no upgrade path.
- No compiler/lint-enforced check of the "only `app` may depend on everything" dependency-graph rule — currently convention-only.
- `analytics` crate declares an `egui_plot` dependency nothing in its source uses (confirmed via grep, found during Phase 5 Epic 7).

### 3.9 UI/UX Debt
- Tools menu's "Screenshot"/"Recording" buttons (`crates/ui/src/plugins/menu.rs:456-469`) show a `"Not yet implemented"` tooltip and call only `ui.close_menu()` — genuinely dead, stray duplicates of a feature that works correctly elsewhere (toolbar button, `Ctrl+Shift+S`/`Ctrl+Shift+R`).
- Seven `MenuAction` variants are wired into menus/dialogs but their handlers only `tracing::warn!(...)` and do nothing: `Undo`, `Redo` (both listed with real keybindings in the Keybinds dialog), `DuplicateSelection`, `SpawnPaste`, `JoinSelection`, `GrabSelection`, `FocusSelection`.
- No application-preferences persistence mechanism exists anywhere (confirmed via grep for `confy`/settings-file patterns — none found) — Phase 5's onboarding-hint dialog is session-scoped only as a direct, disclosed consequence.
- `egui_tiles`' own tab-strip widget draws tabbed-pane titles directly, so Phase 5's Contextual/Secondary chrome-tier title coloring can't reach Metrics/Event Log's tab titles (only the accent-bar half applies) — flagged, not investigated further, in Phase 5's own execution log.
- Colorblind-collision finding from `docs/design/accessibility.md`, never fixed: Carnivore and Omnivore diet colors converge to a near-identical yellow-olive under Deuteranopia simulation. Flagged explicitly as needing its own reviewable sub-change (changes the simulation's visual identity outside the `ui` crate), not touched since.
- `ADR-P5-08`: a wall-clock decorative sine pulse still drives the selection-highlight's alpha, violating this project's own "no decorative animation" rule — recorded as debt, not assigned to any milestone.

### 3.10 Documentation
- `IMPLEMENTATION_STATUS.md` is stale for anything Phase 3/4 has touched since (its own text admits several Deferred Work items were "activated" without the table being updated).
- `docs/explanation/architecture.md` and `docs/reference/crate_graph.md` only name 11-15 of the 28 real crates — `analytics`, `environment`, `evolution`, `learning`, `network`, `plugins`, `research`, `spatial`, `storage`, `world` are absent from both, making it look (incorrectly) like these crates don't exist or aren't wired in.
- `docs/how_to/*` guides describe editing `crates/app/src/app.rs` directly to add genomes/environments — this is real and current, but it directly contradicts the governing spec's own "no magic numbers, all tunables in `.ron` config files" standard, and is the concrete symptom behind §3.5's scenario-authoring gap.
- The stray "Phase 16" reference in `docs/how_to/troubleshooting.md` (§0).

---

## 4. Architectural, Biological, UI, Research, Visualization, Performance, and Documentation Debt — Consolidated Severity Table

| # | Item | Category | Severity | Confirmed live in source? |
|---|---|---|---|---|
| 1 | `fastrand::` bypassing seeded `SimRng` (3 call sites, 2 crates) | Architecture | **High** — breaks a stated non-negotiable guarantee | Yes, exact lines cited |
| 2 | `autosave_interval_ticks` config field has zero readers | Architecture | Medium — silent config lie | Yes |
| 3 | `SimulationScheduler` constructed, never advanced | Architecture | Low-Medium — dead weight, misleading to a new contributor | Yes |
| 4 | No schema-migration framework | Architecture | Medium — recurring pain on every genome-schema bump | Yes (pattern repeats every phase) |
| 5 | 7 dead `MenuAction` handlers (`Undo`/`Redo`/etc.) | UI/UX | Medium — advertised, silently non-functional | Yes, exact lines cited |
| 6 | Tools-menu Screenshot/Recording stray dead buttons | UI/UX | Low — cosmetic but misleading (feature works elsewhere) | Yes, exact lines cited |
| 7 | No app-preferences persistence | UI/UX | Low-Medium — blocks true first-run onboarding, settings memory | Yes (grep confirmed absent) |
| 8 | Population-wide motion prevalence (~7.8%) | Biological | Medium-High — undermines "living organisms" framing broadly | Yes, ADR-P5-07's own numbers |
| 9 | Regional brains not implemented | Neuroscience | Planned, ready to start | Sub-roadmap exists, unimplemented |
| 10 | Reaction-diffusion morphogens not implemented | Development | Planned, ready to start | Sub-roadmap exists, unimplemented |
| 11 | Experiment comparison / statistics (DEF-012) | Research | Medium — blocks any real multi-seed research use | Yes |
| 12 | No parameter-sweep mechanism (source-edit only) | Research | Medium-High — blocks reproducible experiment authoring | Yes |
| 13 | No replay timeline/bookmarks/annotations | Research | Medium | Yes (confirmed absent, not stubbed) |
| 14 | Neural Viewer scale limits | Visualization | Medium — flagged repeatedly, never started | Yes |
| 15 | Unused `egui_plot` dep in `analytics` | Architecture | Trivial | Yes |
| 16 | Documentation crate-list staleness | Documentation | Low-Medium — misleads new contributors | Yes |
| 17 | Thin benchmark coverage / unverified 100k target | Performance | Medium | Yes |

---

## 5. Prioritized Epics

Each epic below follows the required ADR discipline: Context, Decision, Reason, Dependencies, Risk, Future Trigger, Verification.

### Epic A — Determinism & Architectural Integrity Repair
**Priority: 1 (highest — correctness, not a feature)**

- **Context:** Three confirmed call sites use unseeded `fastrand::` instead of `common::SimRng`, breaking bit-exact reproducibility — a guarantee `CONTRIBUTING.md` calls non-negotiable and multiple ADRs across all three prior phases depend on for their own verification methodology (same-seed-same-output tests). Separately, `autosave_interval_ticks` is a no-op config field and `SimulationScheduler` is dead weight.
- **Decision:** Replace all 3 `fastrand::` call sites with the existing `SimRng` resource threading pattern already used by every other stochastic system (no new mechanism — this is a bug fix, not a feature). Either wire `autosave_interval_ticks` to a real periodic-save system or remove the field and its default and document manual-save-only as the actual behavior. Either remove `SimulationScheduler` entirely (if `scheduler` crate has no other planned consumer) or replace the hand-written tick loop with it (larger, riskier option — recommend removal unless a Phase 6 epic elsewhere needs the scheduler's specific semantics).
- **Reason:** This is the single most severe finding in the audit — a correctness guarantee the project explicitly claims is non-negotiable is currently false in 3 places. Everything downstream (replay, batch experiments, research reproducibility) inherits this risk silently.
- **Dependencies:** None — can start immediately, independent of every other epic.
- **Risk:** Low-Medium. Replacing RNG calls is mechanical but must preserve existing behavior's statistical properties (same distribution, just seeded) — requires a same-seed-same-output regression test per changed system, matching this project's own established verification pattern.
- **Future Trigger:** N/A — this should simply be fixed, not deferred further.
- **Verification:** New determinism tests: run each affected system twice with the same seed, assert identical output; existing `cargo test --workspace` must still pass; a full `cargo build`/`clippy`/`fmt` pass. For the scheduler/autosave decision, verification is a documentation update plus (if wired) a real autosave-triggers-at-interval test.

### Epic B — Locomotion & Body-Plan Completion
**Priority: 2**

- **Context:** ADR-P5-07 fixed a structural bug making Muscle segments unreachable, raising in-app effector-actuation from 0.3% to 7.8% of founders — but this remains far below the 90.7% seen in isolated (non-in-app) testing, and the gap's cause was explicitly left unexplained (growth-completion timing, position-sampling differences, diet-weighted composition effects were named as plausible but unmeasured). Separately, Fin remains 0% reachable via the direct Hox-code pathway, and Ganglion/Vascular segments still share Torso's generic stiffness with no differentiated physics (a Phase 3 M14 stretch goal never picked up).
- **Decision:** Instrument the specific gap ADR-P5-07 left open — measure, per the same `PYLON_MOTION_DIAGNOSTIC` headless diagnostic already built, which of the three named candidate causes (growth-completion timing / position-sampling divergence / diet-weighted composition) actually accounts for the 7.8%-vs-90.7% gap, before proposing any further fix. Separately (lower priority within this epic), measure whether the branch-pair-derived Fin pathway (`growth_system`) already provides actuatable fin structures through a mechanism distinct from the direct Hox code, since this was flagged as "unverified, not assumed either way."
- **Reason:** This is the one Phase 5 success criterion marked only partially satisfied — closing it (or at least fully explaining it) is the most direct continuation of Phase 5's own unfinished business, and blocks any credible claim that "organisms are alive and moving" population-wide.
- **Dependencies:** Reuses the existing `PHYLON_MOTION_DIAGNOSTIC` instrumentation (no new tooling needed) and the modular regulatory-CPPN architecture from ADR-P5-07 (no architectural change expected, an investigation first).
- **Risk:** Medium — per this project's own repeated experience on this exact problem (two prior "fixes" during Phase 5's Epic 2 investigation each uncovered a *new* root cause instead of closing the gap), a third round of investigation may again reveal a different, deeper issue rather than a quick fix.
- **Future Trigger:** If investigation finds the gap is fundamentally a population/generational-selection question (bodies *evolve* toward locomotion, they aren't handed it) rather than a bug, this should be re-scoped as a long-running observational study, not a fix.
- **Verification:** Same measured-not-guessed discipline ADR-P5-07 itself used: a real headless diagnostic run, with numbers, before any fix is proposed; determinism-preserving regression test if any code changes.

### Epic C — Regional Brains (P4-N1/N2)
**Priority: 3 — fully audited, ready to implement pending approval**

- **Context:** `PHASE4_EPIC1_NEURAL_ROADMAP.md` is a complete, code-free audit already proposing N1a (region plumbing), N1b (region-bound wiring), N1c (Ganglion becomes a real neural anchor), all Low/Medium-High/Medium risk, ~7 days estimated. N2 (axon guidance/migration/pruning) was deliberately left unscoped pending N1's existence in code.
- **Decision:** Implement N1a → N1b → N1c in that order, exactly as ADR-N1-01 already specifies (region as CPU-side wiring metadata only, zero GPU/`brain.wgsl` changes) — this document does not re-litigate that ADR, only schedules it.
- **Reason:** This is the largest single piece of already-complete planning sitting unimplemented in the repository. No further audit is needed before starting; the sub-roadmap already did the work.
- **Dependencies:** None blocking; independent of Epics A/B.
- **Risk:** As stated in the sub-roadmap: N1b is Medium-High (the real behavior change); N1a/N1c Low/Medium. `hidden_count = 4` may prove too coarse to meaningfully split across regions — flagged as a possible follow-on, not a blocker.
- **Future Trigger:** N2 (axon guidance/migration/pruning) gets its own re-audit only once N1a-c exist in code, per the sub-roadmap's own explicit instruction.
- **Verification:** Exactly as the sub-roadmap specifies — same-seed determinism tests per milestone, `cargo build`/`clippy`/`fmt`/`test` clean, no GPU buffer layout changes (verified by confirming `brain.wgsl` is untouched).

### Epic D — Reaction-Diffusion Morphogens + Temporal Gene Expression (P4-D1/D2)
**Priority: 4 — fully audited, ready to implement pending approval**

- **Context:** `PHASE4_EPIC4_MORPHOGEN_ROADMAP.md` is a complete, code-free audit proposing D1a (intra-organism graph-based morphogen signaling, reusing `transport_system`'s exact architecture), D1b (inter-organism/environmental coupling via a 5th GPU diffusion-texture layer), D1c (`simulate_growth_timeline` reconciliation + divergence test), ~8 days total. D2 (temporal gene expression) explicitly deferred pending D1's shape.
- **Decision:** Implement D1a → D1b → D1c in that order, per ADR-D1-01 (which already supersedes the earlier, GPU-field-only ADR-P4-02 for the intra-organism case, backed by P4-F3/F4's own shipped precedent that graph-relaxation is the architecturally-matched approach for a ~15-node body).
- **Reason:** Second-largest piece of already-complete planning sitting unimplemented. This also directly unblocks DEF-009 (true diffused disease spread), which Phase 4's immune-system work explicitly left pending exactly this landing.
- **Dependencies:** None blocking Epic C or this epic against each other — they touch different subsystems (brain vs. development) and can proceed in either order or in parallel. D1b specifically touches the existing GPU diffusion-texture array (already carries 4 layers) — must not break the 3 pre-existing field layers.
- **Risk:** D1b is explicitly the highest-risk piece (real GPU shader-layer-count change); D1a/D1c are Low-Medium. `develop_at_position`'s signature changing (to accept field state) ripples across call sites — flagged in the sub-roadmap already.
- **Future Trigger:** D2 (temporal gene expression / activation windows / checkpoints) re-audited only once D1's actual shape is known in code.
- **Verification:** Regression test confirming `simulate_growth_timeline` (baseline/no-field call) still matches pre-D1 fixture output exactly, per the sub-roadmap's own stated requirement; isolation tests proving the 3 existing diffusion layers are unaffected by the new 4th; full workspace build/clippy/fmt/test.

### Epic E — Physiology Calibration & Save/Load Coverage
**Priority: 5**

- **Context:** Every per-segment physiological rate constant added in Phase 4 is a confirmed, self-documented placeholder never biologically tuned. Separately, none of Phase 4's new per-segment state (`HormoneLevel`, `SegmentInfection`, `SegmentImmunity`, the persistent `DevelopmentalGraph` itself) is included in `storage::SimulationSnapshot` — save/load silently discards it.
- **Decision:** Two independent workstreams, either can proceed alone: (1) a calibration pass giving each placeholder rate a real, documented justification (even if the justification is "chosen to match observed X behavior," it must stop being silently arbitrary); (2) extend `SimulationSnapshot`'s schema to include the missing physiology/graph state, with the same "bump `GENOME_SCHEMA_VERSION`-equivalent, document the break, no migration path" policy this project has used consistently.
- **Reason:** A saved-and-reloaded organism today silently loses real simulation state — a correctness gap adjacent to Epic A's determinism concern, though lower severity (doesn't affect a single run, only save/load round-trips). Calibration affects the scientific credibility of anything measured from these systems.
- **Dependencies:** Independent of Epics A-D; benefits from D1a landing first if calibration wants to account for morphogen-driven variation, but doesn't require it.
- **Risk:** Low for both workstreams — additive schema change (2), and constant-value tuning with no structural change (1). Main risk is schema-version churn if done piecemeal rather than batched with other pending schema needs.
- **Future Trigger:** Batch this schema bump together with any other pending `SimulationSnapshot` change (e.g., from Epic D) rather than bumping the version twice in quick succession.
- **Verification:** Save-then-load round-trip test asserting the previously-lost fields survive; existing snapshot tests must still pass unchanged for pre-existing fields.

### Epic F — Life-Cycle & Multi-Generation Depth
**Priority: 6**

- **Context:** Life-stage transitions (P4-L1) rebuild the brain from scratch, discarding all Hebbian-learned weights — a known, disclosed cost of resolving "Brain reconciliation" as a full rebuild rather than an in-place extension. Separately, `LineageTracker`'s ancestor-chain depth is architecturally shallow in practice (dead records extracted almost immediately, every tick, unconditionally) even though descendant chains work as intended.
- **Decision:** Investigate (not yet commit to a fix for) an in-place brain-topology extension API that could preserve at least some synapse weights across a life-stage transition, scoped narrowly to what P4-L1's rebuild already needs (not a general brain-mutation API). Separately, evaluate whether ancestor-chain retention should be time-boxed (keep N ticks of history before cold-storage extraction) rather than immediate, to make multi-generation lineage analysis genuinely useful.
- **Reason:** Both are real, previously-disclosed limitations that specifically undermine "multi-generation developmental evolution" — an explicitly named audit target. Neither was a silent gap; both were named and left for a future milestone.
- **Dependencies:** Loosely related to Epic C (regional brains) — if Epic C changes `Brain`'s internal wiring representation, any brain-preservation work here should be sequenced after Epic C to avoid solving the same problem twice.
- **Risk:** Medium — brain-topology preservation touches the same reconciliation logic P4-L1 already found "genuinely hard" once; ancestor-chain retention changes memory-retention behavior and needs a bounded-cost design (same discipline as `recent_selections`/`trajectory_history`'s existing ring-buffer pattern).
- **Future Trigger:** If Epic C's regional-brain work substantially changes `Brain`'s structure, re-scope this epic's brain-preservation half against the new shape rather than the current one.
- **Verification:** A determinism-preserving test proving a life-stage transition with the new preservation logic still produces a valid, evaluable brain; a lineage test proving ancestor-chain depth genuinely reaches N>1 hops under the new retention policy.

### Epic M — Scientific Validation & Calibration
**Priority: 7 (added at approval time, per explicit direction) — gates whether Phase 6 can be considered complete**

- **Context:** Added at roadmap-approval time, not part of the original audit's 12 epics. Its purpose is distinct from Epic K (performance/benchmark coverage): where Epic K asks "is it fast enough," this epic asks "is the biology real, stable, and reproducible enough to trust." Every placeholder rate this audit found (Epic E), the still-open locomotion-prevalence gap (Epic B), and the newly-fixed determinism/lifecycle defects (Epic A, see §12 Execution Log) all feed into this epic as inputs to validate, not as separate untracked concerns.
- **Decision:** Before Phase 6 is considered complete, run a dedicated validation pass covering three questions, each with a real measurement, not an assertion: (1) **Reproducibility** — does the same seed, run twice (including across the batch/parameter-sweep tooling Epic G builds, once it exists), produce byte-identical or statistically indistinguishable outcomes, now that Epic A's determinism fixes are in place; (2) **Stability** — does a long headless run (reusing the existing headless-mode infrastructure, e.g. tens of thousands of ticks) avoid population collapse, unbounded resource growth, or the kind of silent-forever-Impending-hazard bug Epic A's milestone A1 just found, across a range of seeds, not just one; (3) **Biological realism/calibration** — do Epic E's now-tuned physiology rates and Epic B's locomotion-prevalence numbers, taken together, produce population dynamics (birth/death rates, lifespan distribution, trophic-tier ratios) that are at least internally consistent (e.g., predators don't systematically starve before finding prey, producers don't uniformly out-reproduce every consumer), not compared against real-world biological data (which this project's own "do not invent biology" discipline has never claimed to target).
- **Reason:** Every prior phase in this repository has verified individual milestones in isolation (build/clippy/test per change) but never run a single dedicated pass asking "does the *whole simulation*, after all these changes, still behave like a coherent artificial-life system over a long run." This is the direct, explicit gap the approval called out.
- **Dependencies:** Meaningfully depends on Epic A (determinism must be real first), Epic B (locomotion numbers feed the realism check), and Epic E (calibrated rates feed the realism check) having landed. Should run once as an interim checkpoint after those three, and once more as the final Phase 6 gate after every other epic lands — not just once at the very end, so a regression introduced by a later epic (e.g., Epic C's regional brains, Epic D's morphogens) doesn't go unnoticed until the last possible moment.
- **Risk:** Medium — "biological realism" has no single pass/fail number; the risk is this epic either becomes a rubber stamp (checking nothing meaningfully) or an open-ended research project with no defined end. Mitigated by scoping it to the 3 concrete questions above, each with a stated, falsifiable measurement, not a vague "does it feel alive" judgment call.
- **Future Trigger:** If the interim checkpoint (after A/B/E) finds a serious realism problem, treat that as new evidence requiring its own follow-up epic (per the same "measure, report, stop" discipline as Epic B), not a same-day fix bolted onto this epic.
- **Verification:** A written validation report (living in this document's Execution Log, §12) covering all 3 questions with real numbers from real headless runs, at both the interim checkpoint and the final Phase 6 gate; any finding of instability or non-reproducibility blocks declaring Phase 6 complete until addressed or explicitly accepted as a known, disclosed limitation (matching every prior phase's own honesty standard).

### Epic G — Research Platform Maturity
**Priority: 7 (high research value, but genuinely new engineering, not a repair)**

- **Context:** `research`/`app::batch::run_batch` is real and works — sequential, seed-only, per-seed Markdown+RON reports with exactly 3 summary fields. `network`'s MARL WebSocket protocol works for a single external agent. `plugins`' `rhai` scripting engine works for scenario/god-mode scripting via a safe, deferred-command pattern. None of this is wired to a config/scenario-driven parameter-sweep workflow; DEF-012 (comparison dashboard) and DEF-013 (`.phylon-research` bundle format) remain open; replay has no timeline/bookmarks/annotations at all.
- **Decision:** Scope this as its own multi-milestone epic (too large for a single ADR-sized decision), covering, in priority order: (1) a genuine parameter-sweep mechanism — extending `BatchRunConfig` beyond seed-only to sweep arbitrary `config::PhylonConfig` fields, likely via the existing `.ron`/rhai infrastructure rather than a new DSL; (2) statistical aggregation across a batch's seeds (mean/variance/confidence interval on `ExperimentReport`'s existing + expanded fields); (3) an Experiment Comparison Dashboard UI panel reading multiple `ExperimentReport`s (closes DEF-012, follows the `Research Dashboard`/`Replay Browser` panel-slot pattern already proven extensible in `docs/design/layout.md`); (4) a Replay Timeline UI with bookmarks/annotations — but only after confirming whether this requires the `main.rs` restructuring `UI_PHASE2_ROADMAP.md`'s ADR-001 flagged, since that's a materially larger change than the other three items; (5) the `.phylon-research` bundle format (DEF-013), lowest priority since CSV/RON/Markdown already cover the same data separately.
- **Reason:** This is the most direct path to Phylon actually being usable by a researcher rather than requiring source edits and recompilation for every experiment variation — the audit's single clearest "not yet a research platform" finding.
- **Dependencies:** Item (1) benefits from, but doesn't strictly require, `plugins`' existing `rhai` engine. Item (4) depends on resolving ADR-001's architectural question before scoping further. Items (2)/(3) are independent of everything else in this document and could start immediately.
- **Risk:** Medium overall. (1) risks becoming a speculative "config DSL" if not scoped tightly — recommend starting with only the fields already proven to matter (RNG seed, mutation rate, hazard probability) rather than making every config field sweepable on day one. (4) is High risk specifically if it requires the `main.rs` restructuring — should get its own dedicated sub-audit before committing, mirroring how Epics C/D got their own sub-roadmaps.
- **Future Trigger:** Re-audit item (4) specifically once (or if) an architectural path around ADR-001's constraint is found; until then, keep replay headless-only as `UI_PHASE2_ROADMAP.md` already decided.
- **Verification:** A real multi-parameter batch run producing statistically distinguishable reports across at least 2 swept parameters; a comparison dashboard rendering ≥2 real `ExperimentReport`s side by side; existing single-seed batch behavior unchanged (regression-tested).

### Epic H — Scientific Visualization Maturity
**Priority: 8**

- **Context:** Neural Viewer scaling and Metrics-as-full-analytics-workspace have both been repeatedly named, since as early as the original UI audit, as needing their own dedicated plans rather than incremental extension — never started. `biological_visual_language.md`'s own "not yet covered" list names concrete Inspector/Event-Log/viewport gaps (Disease/Development sections, unused `LOG_MUTATION` category, missing Speciation badges).
- **Decision:** Two independent sub-initiatives, each deserving its own dedicated audit-then-roadmap document (per this project's own established pattern for Epic-scale UI work) rather than being pre-specified here: (1) Neural Viewer — layout algorithms for large networks, filtering, multi-select, activation playback; (2) Metrics — zoom/pan, time-range selection, multiple Y-axes, saved presets. Separately, close the smaller, already-named `biological_visual_language.md` gaps (Inspector Disease/Development sections, wiring a real `LOG_MUTATION` publisher, Speciation viewport badge) as their own small, Low-risk milestones — these don't need a dedicated sub-roadmap, they're direct continuations of Phase 5's own Epic 1/3 pattern.
- **Reason:** These are the two panels researchers will spend the most time in during any real analysis session; both are explicitly acknowledged as under-scoped relative to their importance.
- **Dependencies:** None blocking; the small `biological_visual_language.md` gaps can start immediately and don't require the two larger sub-audits to happen first.
- **Risk:** Low for the small gaps; Medium-High for the two dedicated sub-initiatives once scoped (matching the risk profile Phase 5's own Metrics/Neural Viewer work already flagged).
- **Future Trigger:** Commission the two dedicated sub-roadmaps only once Epics A-G have made this document's higher-priority items real — these are valuable but not urgent relative to the determinism/research-platform work above.
- **Verification:** For the small gaps — a real Disease/Development Inspector section reading live component state (not hardcoded placeholders, matching the exact discipline SX-4a/4b already established); a real `NarrationLog` "Mutation"/"Speciation" entry produced by an actual system, not a stub.

### Epic I — Visual Verification Strategy
**Priority: 9 (structural, cross-cutting)**

- **Context:** Every single UI-facing milestone across Phases 3, 4, and 5 — dozens of them — carries the identical disclosed caveat: "no screen-capture/automation driver exists for this native wgpu desktop app," so whether a change actually reads correctly to a human has never once been confirmed by anything other than code review. This is the single most repeated limitation in the entire audited history.
- **Decision:** Investigate (this is a genuine open engineering question, not a known-good pattern to just apply) whether Phylon's existing `crates/app/src/capture.rs` GPU-texture-readback infrastructure (already used for screenshots and Phase 5's chart PNG export) can be extended into a scripted, offscreen, headless-GPU screenshot-and-compare harness — driven the same way the existing headless mode (`PHYLON_MOTION_DIAGNOSTIC`, `research.headless`) already drives simulation-only runs, but also rendering a frame and diffing it against a reference image. This would not be a general Playwright-style UI automation framework (no click/drag simulation of egui widgets is proposed) — narrower: "render this exact scene state once, headless, and compare pixels."
- **Reason:** This is the one gap that, if closed even partially, would retroactively increase confidence in a large fraction of all previously-shipped UI work, and prevents every future UI milestone from repeating the same disclosed limitation indefinitely.
- **Dependencies:** None blocking, but lowest-confidence item in this document — genuinely unproven whether egui's immediate-mode rendering can be driven deterministically enough for stable pixel-diffing (font rasterization, anti-aliasing, and any timing-sensitive layout could all introduce non-determinism unrelated to real regressions).
- **Risk:** High relative to its Medium priority ranking — this could turn out to be substantially harder than it sounds (egui doesn't render identically across GPU vendors/driver versions in every case), in which case the honest outcome is "not currently feasible, re-confirm the limitation and move on," which is itself a valid, useful finding.
- **Future Trigger:** If an initial spike (render one known-static screen, e.g. the About dialog, twice, and diff) shows non-deterministic pixel output even on the same GPU/driver, stop and report that finding rather than continuing to chase determinism that may not be achievable.
- **Verification:** A spike/proof-of-concept only, not a claim of a finished framework: render the same static UI state twice in a headless context and confirm byte-identical (or near-identical, with a defined tolerance) output before proposing this as a real CI-integrated tool.

### Epic J — UI/UX Debt Cleanup
**Priority: 10 (low individual risk, real user-facing correctness)**

- **Context:** §3.9/§4 above lists 7 dead `MenuAction` handlers, 2 stray dead buttons, no preferences persistence, an unfixed colorblind collision, and the ADR-P5-08 decorative pulse — a batch of small, independently-fixable UI defects.
- **Decision:** For each dead `MenuAction` (`Undo`/`Redo`/`DuplicateSelection`/`SpawnPaste`/`JoinSelection`/`GrabSelection`/`FocusSelection`): either implement it for real, or remove it from every menu/dialog/shortcut that advertises it (never leave a control that silently does nothing while claiming otherwise — the same standard Phase 5 Epic 2 already applied when it found and fixed the dead shortcut system). Remove the two stray Tools-menu buttons outright (the working screenshot/recording path already exists elsewhere; these are confirmed dead duplicates, not partial implementations). Add a minimal app-preferences persistence file (a `.ron`-based settings file mirroring `config::PhylonConfig`'s own pattern) so Phase 5's onboarding-hints dialog and any future preference can survive a restart. Fix the Carnivore/Omnivore colorblind collision per `accessibility.md`'s own recommended approach (shift Omnivore's hue/lightness, re-run the Deuteranopia simulation, treat as its own reviewable sub-change since it touches `ecology::Diet::standard_color()`). Resolve ADR-P5-08 (replace the decorative pulse with a static or event-driven alternative, per its own two already-proposed options).
- **Reason:** Every item here is either a currently-misleading UI element (worse than simply absent — it actively claims a feature works) or a small, previously-deferred fix with a clear resolution path already documented.
- **Dependencies:** None — fully independent of every other epic, safe to interleave with anything else.
- **Risk:** Low across the board, except the colorblind-color-shift (explicitly flagged in its own source doc as needing sign-off since it changes the simulation's visual identity, not just chrome).
- **Future Trigger:** N/A — these are ready to schedule whenever convenient; no blocking condition.
- **Verification:** For each dead action — either a passing test exercising the new real behavior, or a confirmed absence from every menu/dialog/shortcut/keybind-list it used to appear in (grep-verified, matching this project's own established "confirm the fix, don't just claim it" discipline). For the color shift — a re-run of the Deuteranopia simulation showing genuine separation, appended to `accessibility.md`.

### Epic K — Performance & Benchmark Coverage
**Priority: 11**

- **Context:** `README.md` states a 100,000-organism performance target as an engineering goal, not a confirmed measurement — no benchmark in the repository exercises anything close to that scale, and only 2 of the many systems added since (`metabolism_parallel`, `scheduler_throughput`) have any `criterion` coverage at all.
- **Decision:** Before any performance *optimization* work is proposed, first establish real measurement: add `criterion` benchmarks for Phase 4's transport/endocrine/immune systems and Phase 5's per-frame UI-adjacent systems (behavior glyphs, health/disease rendering), then run a real large-population headless stress test (reusing the existing headless-mode infrastructure) to get an honest current-scale number to compare the 100k target against.
- **Reason:** Optimizing without a baseline risks solving the wrong problem — this project's own repeated lesson from Phase 5's motion-prevalence investigation (measure before fixing) applies here too.
- **Dependencies:** None blocking; can start immediately, ideally before or alongside Epic G's batch-tooling work since a parameter-sweep mechanism would make repeated performance measurement much easier.
- **Risk:** Low — this is instrumentation and measurement, not a behavior change.
- **Future Trigger:** Once a real baseline number exists, a *separate*, future optimization epic should be scoped against the specific bottleneck the measurement finds — not proposed speculatively here.
- **Verification:** A committed benchmark suite covering the currently-uncovered systems; one documented, reproducible large-population headless run with a real organism-count/tick-rate number, checked into this document's own execution log once available.

### Epic L — Documentation & Numbering Hygiene
**Priority: 12 (do last, low individual complexity, but real payoff for future contributors)**

- **Context:** §0/§3.10 above — three conflicting phase-numbering schemes, a stale `IMPLEMENTATION_STATUS.md`, an `architecture.md`/`crate_graph.md` pair that only documents about half the real crates, and how-to guides that contradict the governing spec's own "no source-edit, config-driven tunables" standard.
- **Decision:** Update `docs/explanation/architecture.md` and `docs/reference/crate_graph.md` to list and describe all 28 real crates (not just the original 11-15), explicitly noting which ones (`learning`, `network`, `plugins`, `research`) are real-but-narrow rather than absent. Add a short note to `IMPLEMENTATION_STATUS.md`'s header pointing to `PHASE3_ROADMAP.md`/`PHASE4_ROADMAP.md`/`PHASE5_SX_ROADMAP.md`/this document as the current source of truth for anything past Phase 2. Remove or explain the stray "Phase 16" reference. Once Epic G's scenario-authoring work (if approved) lands, update `docs/how_to/add_custom_genomes.md`/`modify_environment.md` to describe the new config/rhai-driven path instead of direct source editing.
- **Reason:** A new contributor or researcher reading the docs today would materially misjudge the project's actual scope and maturity — several real, working crates read as if they don't exist.
- **Dependencies:** The how-to-guide update specifically depends on Epic G landing first; everything else in this epic is independent and can happen immediately.
- **Risk:** Minimal — documentation-only changes.
- **Future Trigger:** N/A.
- **Verification:** A fresh read-through of the updated docs against `Cargo.toml`'s actual member list, confirming 28-for-28 crate coverage; no remaining references to a numbering scheme this document doesn't reconcile.

---

## 6. Milestone Dependency Graph (epics, textual)

```
Epic A (determinism repair)        — independent, do first
Epic B (locomotion investigation)  — independent, reuses A's discipline
Epic C (regional brains)           — independent; sequence before F if F touches brain internals
Epic D (morphogens)                — independent of C; D1a/c before D1b (GPU-touching)
Epic E (physiology calibration)    — independent; benefits from D landing first (optional)
Epic F (life-cycle/lineage depth)  — sequence after C if C changes Brain's shape
Epic G (research platform)         — independent; item (4) gated on its own architecture sub-question
Epic H (sci-viz maturity)          — small gaps independent; 2 large sub-plans deferred until A-G land
Epic I (visual verification)       — independent, exploratory/high-uncertainty
Epic J (UI/UX debt)                — fully independent, interleave anywhere
Epic K (performance/benchmarks)    — independent; pairs well with Epic G's sweep tooling
Epic L (documentation)             — do last; how-to portion gated on Epic G
```

No epic in this document blocks another except where explicitly noted (C→F ordering, D1a/c→D1b ordering, G item 4's internal gate, L's partial gate on G).

---

## 7. Risk Analysis (summary, cross-epic)

| Risk | Where | Mitigation |
|---|---|---|
| RNG-fix changes existing statistical behavior unintentionally | Epic A | Same-seed-same-output regression test per changed system before/after |
| Locomotion gap has a 3rd, deeper root cause (pattern already seen twice) | Epic B | Budget for "measure, report, stop" as a valid outcome — don't force a fix under a false certainty |
| Regional-brain N1b behavior change is genuinely Medium-High risk | Epic C | Sub-roadmap's own N1a→b→c staging already mitigates this; follow it as written |
| D1b GPU diffusion-layer change breaks 3 existing layers | Epic D | Isolation tests per the sub-roadmap's own plan |
| Parameter-sweep mechanism scope-creeps into a speculative config DSL | Epic G | Start with only 2-3 proven-useful sweepable fields, expand only on demonstrated need |
| Replay Timeline requires the larger `main.rs` restructuring ADR-001 flagged | Epic G | Get its own dedicated sub-audit before committing effort, same as Epics C/D did |
| Visual verification spike proves genuinely infeasible | Epic I | Treat a negative result as a valid, useful finding — don't force a framework that doesn't work |
| Colorblind color-shift changes simulation visual identity | Epic J | Explicit sign-off step already required by `accessibility.md`/`colors.md`'s own text |
| Performance benchmarking reveals the 100k target was never realistic | Epic K | Report the real number honestly; scope any optimization epic separately, later |

---

## 8. Verification Plan (applies to every Phase 6 milestone)

Matching the standard every milestone in Phases 3-5 was already held to:

1. Re-audit the specific area against current source immediately before implementing (not against this document's own citations, which will drift the moment any code changes).
2. `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `cargo test --workspace` — all clean, zero regressions, before any milestone is considered done.
3. A real regression test proving the specific fix/feature works — not just "it compiles" — following this project's own established pattern (same-seed determinism tests, save/load round-trip tests, etc., as appropriate per epic).
4. Honest, explicit disclosure of any remaining limitation, exactly as every prior phase's execution log already does — no milestone claims more than it verified.
5. Stop after each milestone (or an explicitly user-approved bundle of milestones, per the pattern established in Phase 5) and wait for approval before continuing, unless directed otherwise.

---

## 9. Long-Horizon / Not Yet Started (explicitly beyond Phase 6)

Per the instruction not to invent biology or propose speculative engineering, the following categories from `PHYLON_PROMPT_v2.md`'s original vision are named here for completeness and honesty, but are **not** proposed as Phase 6 epics — each would need its own dedicated audit-and-roadmap document, the same way Regional Brains and Morphogens got theirs, before any of them should be scoped into actionable milestones:

- **Environment & terrain**: heightmap terrain, water/aquatic biomes, weather (wind/precipitation), true seasons/climate zones, soil composition, fire/erosion/flood/drought, canopy occlusion, salinity gradients — none exist; only a single global day/night sinusoid exists today.
- **Colonial, microbial, and cooperative systems**: biofilm/quorum-sensing, HGT/plasmid transfer, encystment, germ-soma apoptosis beyond what Phase 3 already built, flocking/pack-hunting/nursing/altruism/coalition dynamics — entirely unbuilt.
- **Extended sensory modalities**: of ~14 modalities in the vision (hearing, nociception, proprioception, vestibular, baroreception, thermoreception, electroreception, magnetoreception, foveated/compound vision, etc.), only 9 fixed CTRNN inputs exist today (Olfaction, Signals, Hazards, Energy, Age, 3× Vision, Internal Pacemaker). No audio subsystem (`kira`/`cpal`) exists anywhere.
- **Extended `Diet` taxonomy** (DEF-008): Fungivore/Scavenger/Parasite/Detritivore — vision names 8 diet types, 5 exist.
- **Sandbox / "god-mode" tools**: of 22 named tools (time dilation beyond pause/speed, terrain sculpting, field painting, agent possession, weather override, quarantine walls, live brain-pathway editing, fossilization, disease lab, etc.), only pause/speed control and manual per-organism mutation buttons exist.
- **ML backend integration**: `burn`/`candle`/`pyo3` are all named in the vision and in `learning`'s own doc comment as intentionally not bundled; true multi-agent RL, curriculum learning, behavior clustering, and anomaly detection are all unbuilt.
- **3D migration**: fully audited in `PHASE4_EPIC9_3D_READINESS.md` (§2 above summarizes it) — a real, scoped 7-step migration path exists (3D-M1 through 3D-M7) but is explicitly not authorized for implementation by that document, and this document does not change that.
- **Distributed/multi-user platform**: `quinn`/QUIC-based multi-user sessions, distributed chunk-owning processes — named in the vision's Phase 12, nothing built beyond the single-agent WebSocket protocol already in `network`.

---

## 10. Success Criteria for Phase 6

Checked once, at the end of Phase 6 (mirroring `PHASE5_SX_ROADMAP.md`'s own §9 pattern):

- The 3 confirmed `fastrand::` determinism-breaking call sites no longer exist; a same-seed run of the affected systems is provably identical across two runs.
- Regional brains (Epic C) and reaction-diffusion morphogens (Epic D) are implemented, or a documented, honest reason exists for why either was not started.
- A researcher can vary at least one simulation parameter beyond RNG seed across a batch run, and see a statistical (not just single-value) comparison of the results, without editing and recompiling `app.rs`.
- No menu, dialog, or keybind advertises a control that silently does nothing.
- The population-wide locomotion-prevalence gap is either closed, or its root cause is fully explained with real measurements (not left as an open, unexplained percentage).
- `docs/explanation/architecture.md` and `docs/reference/crate_graph.md` correctly list all 28 real crates.

---

## 11. Estimated Implementation Order

1. Epic A — Determinism repair (independent, do first, correctness issue)
2. Epic J — UI/UX debt cleanup (independent, low risk, can interleave with anything)
3. Epic C — Regional Brains (ready to implement)
4. Epic D — Morphogens (ready to implement, independent of C)
5. Epic B — Locomotion investigation (independent, medium uncertainty)
6. Epic E — Physiology calibration & save/load coverage
7. Epic F — Life-cycle & lineage depth (sequence after C if C changes Brain's shape)
8. **Epic M — Scientific Validation & Calibration, interim checkpoint** (after A/B/C/D/E/F land — the biological block is now complete; validate before moving into research-tooling work)
9. Epic K — Performance & benchmark baseline (pairs well with Epic G)
10. Epic G — Research platform maturity (deliberately after all biological epics, per approval condition 7)
11. Epic H — Scientific visualization maturity (small gaps first, large sub-plans deferred)
12. Epic I — Visual verification strategy (exploratory, do when bandwidth allows)
13. Epic L — Documentation & numbering hygiene (do last; partially gated on Epic G)
14. **Epic M — Scientific Validation & Calibration, final Phase 6 gate** (re-run after everything else lands, per its own two-checkpoint design)

---

**This document is the approved, frozen Phase 6 plan.** Implementation proceeds milestone-by-milestone per the approval conditions in the header. See §12 (Execution Log) for what has actually been implemented, discovered, and verified so far.

## 12. Execution Log

### Epic A, Milestone A1 — `ecology` crate: `fastrand` → `SimRng`, plus a re-audit-discovered hazard-lifecycle defect

**Re-audit before implementing:** confirmed, by direct source read immediately before touching anything, that `crates/ecology/src/lib.rs` still had exactly the 3 `fastrand::` call sites this roadmap's audit named — 2 in `food_spawner_system` (lines ~149-156) and 3 in `catastrophe_system` (formerly lines ~605-607, plus the spawn-probability check). Confirmed `common::SimRng` (a seeded `ChaCha8Rng` wrapper, `Deref`/`DerefMut` to the inner RNG) is already inserted as a resource at app startup (`crates/app/src/app.rs:297`) and is the established pattern every other stochastic system in this codebase already uses (confirmed via `crates/reproduction/src/lib.rs`'s `ResMut<SimRng>` + `rand::Rng::gen()` usage).

**A second, more severe defect was found during this same re-audit, in the same function this milestone already needed to touch:** `catastrophe_system` took `mut local_tick: Local<u64>` and derived its notion of "the current tick" from it (`*local_tick += 1; let tick = common::Tick(*local_tick);`). Confirmed via `crates/app/src/simulation.rs:276` that this system is driven by `self.world.ecs.run_system_once(ecology::catastrophe_system)` every tick — the exact `Local<T>`-resets-every-call anti-pattern SX-1a's diagnostic (Phase 5) already documented and fixed elsewhere in this codebase. This meant `tick` was **always `Tick(1)`**, every single tick, forever. Since hazard lifecycle transitions are computed as `elapsed = tick.0.saturating_sub(start_tick.0)`, and both sides of that subtraction were always `Tick(1)`, `elapsed` was always `0` — **every hazard spawned into `Impending` state and never transitioned to `Active`, and never expired**, regardless of `impending_duration`/`active_duration`. This is a verified architectural defect discovered during the mandatory re-audit (approval condition 4's exception), not a speculative addition, and was fixed in this same milestone rather than deferred, since it required touching the exact same function signature the `fastrand` fix already needed to change.

**Implementation:**

- `food_spawner_system`: added `mut rng: ResMut<common::SimRng>` parameter; replaced 3 `fastrand::f32()` calls with `rng.gen::<f32>()` (`rand::Rng` trait, already a dependency).
- `catastrophe_system`: added `mut rng: ResMut<common::SimRng>` and `atmosphere: Res<metabolism::GlobalAtmosphere>` parameters; removed the `Local<u64>` parameter entirely; replaced `tick`'s derivation with `common::Tick(atmosphere.ticks)`, reusing the exact tick-source pattern already established at Phase 5's SX-7a fix (`process_narrative_events_system`) rather than inventing a new resource. Confirmed via `crates/app/src/simulation.rs` that `metabolism::day_night_cycle_system` (which increments `atmosphere.ticks`) already runs earlier in the same tick's system order, so no new resource or ordering constraint was introduced. Replaced 2 `fastrand::f32()` calls with `rng.gen::<f32>()`. Added `#[allow(clippy::too_many_arguments)]` (8 params, over clippy's default-7 threshold — consistent with the existing pattern used elsewhere in this codebase for wide ECS system signatures).

**Verification:** `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` — all clean. `cargo test --workspace` — 0 failures across all 28 crates. 3 new regression tests added to `crates/ecology/src/lib.rs`:

- `hazard_transitions_to_active_once_impending_duration_has_really_elapsed` — directly proves the lifecycle-bug fix: a hazard with a `start_tick` far enough in the past (measured via a real `GlobalAtmosphere::ticks` value) now genuinely transitions from `Impending` to `Active`, which was previously impossible under any circumstance.
- `catastrophe_system_is_deterministic_for_a_given_seed` — proves the `fastrand`→`SimRng` migration preserved (not broke) determinism: two independent `World`s seeded identically produce identical hazard-spawn positions.
- `food_spawner_system_is_deterministic_for_a_given_seed` — same determinism proof for the other fixed system.

**Remaining limitations, disclosed:** the third `fastrand` call site (`crates/organisms/src/systems.rs:560-564`, budding target/offset selection) is untouched — reserved for the next milestone (A2), per the "one milestone at a time" instruction. `SimulationScheduler`'s dead-code status and `autosave_interval_ticks`'s no-op status (both named in Epic A's original decision text) are also untouched, reserved for later Epic A milestones. No population-level/long-run behavioral change from the hazard-lifecycle fix has been measured yet (e.g., hazards now actually expiring will change catastrophe-driven population dynamics) — this is exactly the kind of question Epic M's stability checkpoint should measure once the biological-epic block lands, not something this milestone claims to have assessed.

**Roadmap correction status:** no change to Epic A's Context/Decision/Reason/Dependencies/Risk/Future-Trigger/Verification text was made — the hazard-lifecycle defect is additive evidence folded into this same milestone's execution log, not a rewrite of the epic's original framing (which only knew about the `fastrand` issue at audit time). Recorded here per approval condition 2 (superseding note, not silent rewrite).

**Recommendation: proceed to the next milestone (A2 — `organisms::systems.rs` fastrand fix) unchanged, no roadmap correction needed.** Epic A's remaining scope (A2, plus the `SimulationScheduler`/`autosave_interval_ticks` decisions) stands as originally written.

### Epic A, Milestone A2 — `organisms` crate: `fastrand` → `SimRng`, plus removal of both now-dead `fastrand` dependencies

**Re-audit before implementing:** confirmed, by direct source read, that `crates/organisms/src/systems.rs`'s `producer_growth_system` (the plant/producer branching-growth system) still had the 3rd/last `fastrand` call site named in the original audit — 1 call picking a random existing node to attach a new leaf to (`fastrand::usize(..all_nodes.len())`), 2 calls for the new leaf's spawn offset (`fastrand::f32()`). Confirmed `organisms`' `Cargo.toml` already depends on both `common` and `rand`, so no new dependency was needed.

**Implementation:** Added `mut rng: bevy_ecs::prelude::ResMut<common::SimRng>` to `producer_growth_system`'s signature; replaced the node-pick with `rng.gen_range(0..all_nodes.len())` and both offset draws with `rng.gen::<f32>()`. No `#[allow(clippy::too_many_arguments)]` needed (5 params, under the default threshold). Confirmed via grep no test or other call site outside `crates/app/src/simulation.rs:233`'s production `run_system_once` call needed updating.

**Additional cleanup, directly caused by this change (not scope creep):** after this fix, `grep -rn "fastrand::" crates` returned zero live call sites anywhere in the workspace (only a doc-comment mention of the historical bug, from this same edit). Both `ecology` and `organisms` `Cargo.toml` files still declared `fastrand` as a dependency with nothing left using it — removed both. This is the same class of finding as the `analytics` crate's dead `egui_plot` dependency noted elsewhere in this roadmap's audit; leaving a dependency this exact milestone made dead would be sloppy, not neutral.

**Verification:** `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` — all clean, including after the `Cargo.toml` dependency removals (confirming no transitive/hidden reliance on `fastrand` remained). `cargo test --workspace` — 0 failures across all 28 crates. 1 new regression test added to `crates/organisms/src/systems.rs`: `producer_growth_system_is_deterministic_for_a_given_seed`, proving two independently-constructed, identically-seeded `World`s produce an identical new-leaf spawn position.

**Remaining limitations, disclosed:** Epic A's other two decisions — the `SimulationScheduler` dead-code disposition and the `autosave_interval_ticks` no-op config field — remain untouched, reserved for the next milestone(s). No population-level measurement of how removing RNG non-determinism changes long-run outcomes has been performed — again, that's Epic M's job, not this milestone's.

**Roadmap correction status:** none needed — this milestone matched Epic A's original scope exactly (the dependency cleanup is a direct, mechanical consequence of the fix, not new scope).

**All 3 originally-identified `fastrand::` determinism-breaking call sites (§1/§3.8/§4 of this document) are now fixed.** Recommend proceeding to the next Epic A milestone (A3 — decide and act on `SimulationScheduler`/`autosave_interval_ticks`, per the epic's original Decision text) unchanged, no roadmap correction needed.

### Epic A, Milestone A3 — `autosave_interval_ticks` and `SimulationScheduler` dispositions (Epic A now closed)

**Re-audit before implementing:** re-confirmed both findings still held exactly as originally audited. `autosave_interval_ticks` (`crates/config/src/lib.rs`): declared, defaulted to `3600`, present in the checked-in `data/default.ron`, but `grep -rn "autosave_interval_ticks" crates/app/src` returned zero hits — no system reads it; autosave is 100% manual (`MenuAction::SaveState`, confirmed via `crates/app/src/events.rs`'s `"autosave.bin"` string, which is unrelated — a manual-save filename, not a periodic-autosave mechanism). `SimulationScheduler` (`crates/app/src/app.rs`): re-confirmed constructed at `PhylonApp::new` and stored as a field, re-confirmed zero calls to its `.advance()`/`.step()` methods anywhere in `app`'s source, re-confirmed `main.rs`'s own doc comment already admitted this in plain language.

**Decision made (choosing between the roadmap's own two named options for each, per approval condition 2 — a refinement, not a contradiction, of Epic A's original either/or framing):**

- **`autosave_interval_ticks`: removed**, not wired up. Building a real periodic-autosave system (save path/rotation policy, error handling, a tick-loop hook, user-facing feedback) is feature work — disproportionate to a bug-fix-scoped epic, and explicitly named in the roadmap as the more conservative of the two options. Removed the field from `ResearchConfig`, its `Default` impl value, and its entry in `data/default.ron`; added a doc-comment note on `ResearchConfig` explaining why (a future periodic-autosave feature, if a real need emerges, is a `research`/`app` feature epic, not this one). Confirmed safe for existing `.ron` files: neither `PhylonConfig` nor `ResearchConfig` sets `#[serde(deny_unknown_fields)]`, so a stale on-disk `autosave_interval_ticks` key in someone's own saved config is silently ignored, not a hard parse failure.
- **`SimulationScheduler`: removed from the live app**, not wired up to replace the hand-written tick loop. Replacing `simulation.rs`'s 30+ explicit, individually-ordered `run_system_once` calls with the scheduler's boxed-closure `SystemOrder` dispatch would be a large, materially riskier rewrite of the actual tick pipeline — exactly the kind of disproportionate, undirected redesign approval condition 4 prohibits absent a defect in that pipeline itself (none was found; the defect was purely "an unused field costs a constructor call and misleads readers"). Chose the narrower interpretation of "remove `SimulationScheduler` entirely": removed the field, its construction, and its struct-literal entry from `PhylonApp`/`app.rs`, and removed `app`'s now-unused dependency on the `scheduler` crate. The `scheduler` crate itself (`crates/scheduler/`) is untouched and remains a workspace member — it still has its own tests and the `scheduler_throughput` benchmark, and nothing in this audit found the crate's own contents defective, only its (non-)integration into the live app. Corrected 4 stale, copy-pasted doc-comment headers (`app.rs`, `render.rs`, `events.rs`, `main.rs`) that all claimed the event loop "advances the scheduler on each `AboutToWait`" — false even before this milestone, since `main.rs`'s own doc comment already contradicted it.

**Implementation:** `crates/config/src/lib.rs` — removed the field/default/doc references; `data/default.ron` — removed the corresponding key. `crates/app/src/app.rs` — removed the `scheduler` field, its construction, its struct-literal entry, the `use scheduler::SimulationScheduler` import, and corrected 2 stale doc-comment claims (the module header's step list, and `PhylonApp`'s own "how it happens" paragraph). `crates/app/src/{render,events,main}.rs` — corrected the same stale doc-comment header. `crates/app/Cargo.toml` — removed the now-unused `scheduler` workspace dependency.

**Verification:** `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` — all clean. `cargo test --workspace` — 0 failures across all 28 crates. 1 new regression test added to `crates/config/src/lib.rs`: `real_default_ron_file_still_loads`, which loads the actual checked-in `data/default.ron` (not just an in-memory `Default`) via `PhylonConfig::load`, proving the field removal didn't break the one real config file this repository ships — a more direct proof than exercising `Default` alone would have been.

**Remaining limitations, disclosed:** no periodic-autosave system exists — this is an honest, disclosed absence (not a silent gap, given the doc-comment note now on `ResearchConfig`), not a fix. The `scheduler` crate's own tests/benchmark were not re-run as part of this milestone (they're independent of `app` and untouched by this change) — `cargo test --workspace` already covers them and they passed. No attempt was made to decide whether `scheduler` should eventually be deleted outright or genuinely adopted — that remains an open, undecided question for a future milestone if one ever needs it, not resolved here.

**Roadmap correction status:** Epic A's original Decision text posed both remaining items as an "either/or" without picking a side; this milestone picks a side for each, with reasoning recorded above. This is documented as approval condition 2 requires — a superseding clarification, not a rewrite of Epic A's Context/Reason/Dependencies/Risk/Future-Trigger, none of which changed.

**Epic A (Determinism & Architectural Integrity Repair) is now complete: A1, A2, A3 all done.** All 3 `fastrand` sites fixed (plus the independently-discovered hazard-lifecycle defect), `autosave_interval_ticks` and `SimulationScheduler` both resolved. Recommend proceeding to the next epic in priority order (§11): **Epic J — UI/UX Debt Cleanup**, unchanged, no roadmap correction needed.

### Epic J, Milestone J1 — remove the 2 stray dead Tools-menu buttons

**Re-audit before implementing:** re-confirmed `crates/ui/src/plugins/menu.rs`'s Tools menu still had the exact "Screenshot"/"Recording" small buttons this roadmap's audit named — both showing a `"Not yet implemented"` tooltip and calling only `ui.close_menu()`, no `MenuAction` pushed. Re-confirmed the real, working equivalents still exist elsewhere: `toolbar.rs` pushes `MenuAction::TakeScreenshot`/`ToggleRecording` from real toolbar buttons, and `Ctrl+Shift+S`/`Ctrl+Shift+R` shortcuts already fire the same actions.

**Implementation:** deleted both dead buttons and their separator from the Tools menu outright — they were pure duplicates of working functionality, not partial implementations, so removal (not implementation) was the correct side of Epic J's own either/or framing for this item.

**Verification:** part of this milestone's combined build/clippy/fmt/test pass below (J1-J2 were verified together as one compile unit before moving to J3).

### Epic J, Milestone J2 — implement `FocusSelection` for real

**Re-audit before implementing:** confirmed `MenuAction::FocusSelection`'s handler in `crates/app/src/events.rs` was still a bare `tracing::warn!(...)` stub. Found the exact reusable pattern before writing anything: `crates/app/src/render.rs`'s existing "Camera Tracking" step already does `self.world.ecs.query::<&physics::ParticleNode>().get(&self.world.ecs, tracked)` to read a followed entity's live position each frame, lerping `camera_pos` toward it. Also found (and deliberately did not touch) a pre-existing, slightly odd double-click branch in `viewport.rs` that pushes `FocusSelection` only when *nothing* is selected — outside this milestone's scope per approval condition 4 (not a verified defect this milestone set out to fix, and implementing `FocusSelection` to simply no-op on `None` makes that branch harmless either way).

**Decision:** implemented as a one-shot camera snap, deliberately distinct from `tracked_entity`'s continuous per-frame follow (that's what double-clicking a real selection already does) — reading `selected_entity`'s current `ParticleNode` position once and assigning it directly to `camera_pos`, doing nothing if no entity is selected. No new subsystem needed; this was the one dead action among the seven where "implement for real" was actually proportionate to a cleanup epic.

**Implementation:** `crates/app/src/events.rs` — replaced the stub with the query-and-snap logic described above, reusing the exact `query::<&physics::ParticleNode>().get(...)` idiom already established.

**Verification:** see the combined J1-J3 verification below.

### Epic J, Milestone J3 — remove the remaining 6 dead `MenuAction`s (and a newly-found, fully-dead `WorkbenchCommand` enum)

**Re-audit before implementing:** re-confirmed all 6 handlers (`Undo`, `Redo`, `DuplicateSelection`, `SpawnPaste`, `JoinSelection`, `GrabSelection`) were still bare `tracing::warn!(...)` stubs, and traced every place each was advertised: `shortcuts.rs` (Ctrl+Z/Ctrl+Y and unmodified G/C/V/J keys), `menu.rs` (Edit menu's Undo/Redo/"Duplicate Selected"), `dialogs.rs`'s Keybinds dialog (an "Editing" section listing all of them, plus "C" in the "Selection" section), and `command_palette.rs` (Undo/Redo only). For each, checked whether a real implementation would be proportionate to a UI-cleanup epic:

- **Undo/Redo** would need a genuine command-history stack — a real architectural feature, not a menu wire-up.
- **DuplicateSelection** would need to clone a selected organism's genome/diet/category through the same spawn path births use, plus decide lineage semantics for the clone (new lineage? child of itself?) — a real biological/architectural decision, not a UI fix.
- **SpawnPaste** would need a genome-clipboard mechanism that doesn't exist anywhere (`CopyEntityId`, the only existing "copy" action, copies a debug ID string for the user's own reference, not a spawnable payload).
- **JoinSelection**/**GrabSelection** would need, respectively, real spring-joining-between-organisms semantics and a mouse-drag input state machine — both genuine interactive-editing features.

All six were judged disproportionate to this epic and **removed** (not implemented) from every place they were advertised, per Epic J's own decision text ("either implement it for real, or remove it from every menu/dialog/shortcut/keybind that advertises it").

**A new, previously-undiscovered piece of dead code was found during this same re-audit:** `crates/ui/src/state.rs` declared a ~90-line `WorkbenchCommand` enum — a fully parallel command catalog (its own `Undo`/`Redo`/`DuplicateSelected`/`FocusSelection`/etc. variants) — re-exported from `lib.rs` but, confirmed via a workspace-wide search, never constructed, matched, or consumed anywhere at all. This reads as an early design sketch superseded by `MenuAction` + the command palette's own tuple list, left behind rather than deleted. Since it's thematically identical dead-action-catalog debt to what this milestone was already removing, and its deletion is a purely mechanical, zero-risk removal (not a redesign), it was folded into this same milestone rather than logged for a separate one.

**Implementation:**

- `crates/ui/src/types.rs` — removed the 6 `MenuAction` variants.
- `crates/app/src/events.rs` — removed the 6 corresponding match arms.
- `crates/ui/src/shortcuts.rs` — removed the `undo`/`redo` `KeyboardShortcut` fields and their `Default` values and `consume_all` dispatch; removed the G/C/V/J dead-action key checks, keeping X (`DeleteSelection`) and F (`ToggleStationary`), which are real.
- `crates/ui/src/plugins/menu.rs` — removed the Edit menu's Undo/Redo buttons and "Duplicate Selected" button (kept "Delete Selected").
- `crates/ui/src/plugins/dialogs.rs` — removed the corresponding Keybinds dialog entries ("C" from "Selection"; the whole "Editing" section collapsed to its one remaining real entry, "F" → "Toggle Stationary").
- `crates/ui/src/plugins/command_palette.rs` — removed the `("Undo", ...)`/`("Redo", ...)` catalog entries.
- `crates/ui/src/state.rs` — removed the entire `WorkbenchCommand` enum; `crates/ui/src/lib.rs` — removed it from the re-export list.

**Verification (covers J1, J2, and J3 together — all three were built/tested as one pass since they touch overlapping files):** `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` — all clean. `cargo test --workspace` — 0 failures across all 28 crates. A workspace-wide grep for every removed identifier (`MenuAction::Undo`, `::Redo`, `::DuplicateSelection`, `::SpawnPaste`, `::JoinSelection`, `::GrabSelection`, `WorkbenchCommand`) after the edits returned zero hits outside this milestone's own explanatory code comments — confirmed no dangling reference survived anywhere.

**Remaining limitations, disclosed:**

- No automated test exercises `FocusSelection`'s new behavior directly — `events.rs` has no existing test harness for `MenuAction` dispatch (it depends on a live `PhylonApp`/ECS-world combination this session has no established unit-test seam for), the same class of limitation this project has consistently disclosed for UI/App-composition-root code rather than force a disproportionate test harness into existence for one small fix.
- The `viewport.rs` double-click branch that pushes `FocusSelection` only when nothing is selected (noted during J2's re-audit) was deliberately left untouched — it's at most a minor, pre-existing UX oddity, not a verified defect this milestone set out to fix, per approval condition 4.
- The six removed actions represent real, plausible future features (undo/redo, organism duplication, clipboard-based spawning, spring-joining, drag-to-move) — removing their dead stubs doesn't mean they're rejected ideas, just that building them is out of proportion for a debt-cleanup epic; each would need its own scoped epic if ever wanted.

**Roadmap correction status:** none needed — J1-J3 matched Epic J's original scope; the `WorkbenchCommand` finding is additive evidence folded into this milestone's execution log (per approval condition 2), not a change to Epic J's original Context/Decision/Reason/Dependencies/Risk/Future-Trigger text.

**Epic J's "dead controls" work (J1-J3) is complete.** Remaining Epic J items — app-preferences persistence, the Carnivore/Omnivore colorblind color shift, and the ADR-P5-08 decorative selection pulse — are still open. Recommend proceeding to the next Epic J milestone (J4 — app-preferences persistence) unchanged, no roadmap correction needed.

### Epic J, Milestone J4 — app-preferences persistence

**Re-audit before implementing:** re-confirmed no settings/preferences persistence mechanism exists anywhere (grep for `confy`/settings-file patterns returned nothing, matching Phase 5's own SX-9a disclosure). Identified the smallest, clearest set of `WorkbenchState` fields a person would actually expect to survive a restart: `high_contrast` and `ui_scale` (both real, existing accessibility settings toggled in `sidebar.rs`'s Settings tab — a checkbox and a slider, confirmed via direct read) and `show_onboarding_hints` (Phase 5, SX-9a's own disclosed session-scoped limitation). Deliberately did not attempt to persist anything else in `WorkbenchState` — camera position, selection, panel layout, etc. are legitimately session state, not preferences.

Found that `high_contrast`/`ui_scale` are mutated directly (`ui.checkbox(&mut state.high_contrast, ...)`, a slider) with no `MenuAction` round-trip through `app` — meaning there is no existing per-toggle hook to save from. Rather than invent a dirty-tracking/change-notification mechanism (disproportionate for a cosmetic-preference save), chose to sync-and-save at the two real process-exit paths this app already has: `MenuAction::Quit`'s handler and the `winit::WindowEvent::CloseRequested` handler — both confirmed via direct read of `events.rs`.

**Decision:** new `crates/app/src/preferences.rs` module — a `Preferences { high_contrast, ui_scale, onboarding_seen }` struct, serialized as `.ron` (mirroring `research::ExperimentManifest`'s own `ron::ser::to_string_pretty`/`ron::de::from_str` pattern) to `data/preferences.ron` — the same relative-to-cwd convention every other on-disk artifact in this app already uses. Deliberately a separate file from `config::PhylonConfig` (`data/default.ron`), not a new section added to it: `PhylonConfig` describes one simulation *experiment's* setup, loaded once and (until this milestone) never written back; conflating it with live-mutating cosmetic UI state would blur that boundary. A missing/corrupt preferences file falls back to `Default` with a logged warning rather than a hard error — deliberately more forgiving than `PhylonConfig::load`'s behavior, since nothing about the simulation itself depends on these values.

**Implementation:**

- `crates/app/src/preferences.rs` (new) — `Preferences` struct + `Default`, `load`/`save` (RON, matching the `research` crate's established pattern), `preferences_path()`.
- `crates/app/src/main.rs` — added `pub mod preferences;`.
- `crates/app/src/app.rs` — added a `preferences: Preferences` field to `PhylonApp`; `PhylonApp::new` now loads it before constructing `WorkbenchState` and applies `high_contrast`/`ui_scale` onto the initial state; added a `save_preferences()` helper method that syncs the live `WorkbenchState` values back into `preferences` and writes them to disk.
- `crates/app/src/events.rs` — `MenuAction::StartSimulation`'s handler now gates `show_onboarding_hints = true` on `!self.preferences.onboarding_seen` (marking it seen and saving immediately when first shown, not deferred to exit, so an early crash/kill doesn't make the hint reappear next launch); both `MenuAction::Quit` and `WindowEvent::CloseRequested` now call `self.save_preferences()` before exiting.
- `crates/app/Cargo.toml` — added `serde`/`ron` as direct dependencies (both already workspace dependencies used elsewhere; `app` needed its own direct declaration to call them).
- `crates/ui/src/state.rs` — updated `show_onboarding_hints`'s doc comment to describe the new persisted-`onboarding_seen` gating, replacing the now-stale "session-scoped only" framing.

**Verification:** `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` — all clean. `cargo test --workspace` — 0 failures across all 28 crates (`app` crate's own test count rose from 9 to 12). 3 new regression tests added to `crates/app/src/preferences.rs`: `load_missing_file_returns_default`, `save_then_load_round_trips` (a real save-to-a-temp-path-then-load-back round trip, not just exercising `Default`), and `load_corrupt_file_returns_default_not_a_panic` (proves a genuinely malformed `.ron` file degrades gracefully rather than crashing the app at startup).

**Remaining limitations, disclosed:**

- `high_contrast`/`ui_scale` are only persisted at the two exit paths this app has (`Quit`, window close) — a hard process kill (task-manager kill, `SIGKILL`, a crash) between toggling a setting and a graceful exit loses that specific change. `onboarding_seen` doesn't have this gap (saved immediately when set). This is a deliberate, disclosed proportionality choice, not an oversight — building a full change-notification/dirty-tracking system for two cosmetic settings would be disproportionate to this cleanup epic.
- No settings UI exists to edit `data/preferences.ron` directly, nor any in-app "reset preferences" action — a user who wants to reset must delete the file by hand. Not built here since nothing in this milestone's scope named a need for it.
- Preferences are global (one file, not per-experiment or per-profile) — consistent with them being cosmetic/accessibility settings, not simulation configuration, but worth naming explicitly since `config::PhylonConfig` (by contrast) is loaded per-experiment.

**Roadmap correction status:** none needed — matched Epic J's original decision text for this item exactly ("a minimal `.ron`-based settings file mirroring `config::PhylonConfig`'s own pattern").

**Epic J now has 2 items remaining: the Carnivore/Omnivore colorblind color shift, and the ADR-P5-08 decorative selection pulse.** Recommend proceeding to the next Epic J milestone unchanged, no roadmap correction needed.

### Epic J, Milestone J5 — Carnivore/Omnivore colorblind color fix

**Re-audit before implementing:** re-read `docs/design/accessibility.md`'s Deuteranopia table and confirmed it still matched `ecology::Diet::standard_color()` exactly (`Carnivore` `#F05454`, `Omnivore` `#FFB703`, both converging to near-identical yellow-olive under the documented simulation) — no drift between the doc and source.

**Measured, not guessed, per approval condition 3:** no colorblindness-simulation tool exists anywhere in this codebase (confirmed via grep). Wrote a throwaway example, `crates/ecology/examples/deuteranopia_check.rs` (deleted after use, matching this project's own "measure honestly, then delete" convention from Phase 3), implementing a standard Machado et al. (2009) deuteranopia simulation matrix. **A real measurement bug was caught and fixed before trusting any result:** the tool's first version fed `Diet::standard_color()`'s output (linear-space RGB) directly into the simulation matrix without first converting to display sRGB — its "normal color" hex output didn't match `docs/design/accessibility.md`'s documented values at all, which is what caught the error. Also caught a second, subtler issue: the tool's initial gamma functions used the precise piecewise sRGB transfer function, but this codebase's own `theme::linear_to_srgb` (confirmed via direct read) uses a plain gamma-2.2 power law — switched to match exactly, so the final chosen linear value renders as intended through the app's own color pipeline, not a subtly-different tool-internal approximation.

**Iterative measurement, both directions tested:** the accessibility doc's own recommended direction ("shift Omnivore's hue toward orange-red-adjacent") was tried first (3 candidates: darker amber, burnt orange/brown, deep amber-brown) — all 3 measurably made the Carnivore/Omnivore simulated-color distance **worse** (33.5-77.9 vs. the original 87.4), confirming that direction converges harder with red under deuteranopia rather than separating from it. Reversed direction: shifting toward a fully saturated, high-lightness bright yellow (away from red, exploiting the lightness channel deuteranopes retain full discrimination on) measurably improved separation.

**Decision:** `Diet::Omnivore`'s `standard_color()` changed from `[1.0, 0.482, 0.0]` (`#FFB703` amber) to `[1.0, 0.737972, 0.0]` (`#FFDE00` bright yellow) — the best-measured candidate, improving simulated-color distance from Carnivore (+43%), Producer (+35%), and Decomposer (+8%), at the cost of a small reduction vs. Herbivore (-7%, from 230.6 to 213.6 — still an enormous margin next to the Producer/Carnivore pair's own already-safe 6.7).

**Implementation:** `crates/ecology/src/lib.rs` — updated `Diet::standard_color()`'s `Omnivore` arm and its comment explaining the change and measurement. `docs/design/accessibility.md` — updated the Deuteranopia table and its narrative from "flagged, not fixed" to a full account of the fix, including the measured direction-reversal finding. `docs/design/colors.md` — updated the palette table's Omnivore hex and the accessibility note's "currently unlanded" phrasing. No UI-crate changes were needed — `theme::chart_color()` re-derives from `Diet::standard_color()` on every call rather than caching a literal (the whole point of that design, confirmed during Phase 5), so the new color propagates to the viewport, status bar, and Metrics charts automatically.

**Verification:** `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` — all clean. `cargo test --workspace` — 0 failures across all 28 crates (`ecology`'s own test count rose from 22 to 23). A new *permanent* regression test (`omnivore_color_is_not_the_old_amber_and_stays_visibly_distinct_from_carnivore`, distinct from the deleted throwaway measurement tool) asserts the color isn't the old amber and stays at least a minimum Euclidean distance from Carnivore in linear RGB — a cheap guard against silently reverting this fix or picking a replacement that's trivially close to Carnivore again, without re-running a full deuteranopia simulation on every CI run.

**Remaining limitations, disclosed:**

- The simulated-color "distance" metric used throughout is a simple Euclidean distance in simulated sRGB space, not a rigorous perceptual metric like CIEDE2000 — adequate for a go/no-go design decision (as the original accessibility audit's own methodology already was), not a claim of precise perceptual measurement.
- Only Deuteranopia (red-green, the most common form) was checked, matching the original audit's own scope — Protanopia/Tritanopia were not separately re-verified against the new Omnivore color.
- This changes the simulation's own visual identity (organism skin color in the viewport), not just a UI chrome token, exactly as `accessibility.md`/`colors.md` both already flagged as needing explicit sign-off before landing — implemented now under this milestone's explicit approval, but worth naming that this is a more visible change than most of this epic's other items.

**Roadmap correction status:** none needed — matched Epic J's original decision text for this item; the direction-reversal finding is additive measurement evidence recorded here, not a change to the epic's Context/Reason/Dependencies/Risk/Future-Trigger.

**Epic J now has 1 item remaining: the ADR-P5-08 decorative selection pulse.** Recommend proceeding to the final Epic J milestone unchanged, no roadmap correction needed.

### Epic J, Milestone J6 — ADR-P5-08 decorative selection-highlight pulse (Epic J now complete)

**Re-audit before implementing:** re-confirmed `crates/app/src/render.rs`'s selection-highlight submission still computed `let pulse = 0.6 + 0.4 * (self.total_sim_time * 3.0).sin();` and fed it as the highlight's alpha (`[1.0, 1.0, 1.0, pulse]`) — an unconditional wall-clock sine oscillation, exactly as ADR-P5-08 recorded, unchanged since Phase 5.

**Decision:** chose ADR-P5-08's first option (a static, non-animated outline) over its second (driving intensity from Health fraction or a discrete `TimedEffects` flash). Reason: the ADR itself flagged that a Health-driven alternative "would need to be reconciled with Health's own existing disk encoding (SX-1c) to avoid a second, competing Health signal," and `docs/design/biological_visual_language.md`'s numeric priority hierarchy places Selection at Priority 1 — the highest, above Health's Priority 2 — so borrowing Health's signal into Selection's alpha would blur an ordering this project has been deliberate about maintaining. A `TimedEffects`-based discrete flash was also considered but rejected as disproportionate: it would need new selection-change-detection logic and coordination with a GPU/SDF rendering path that doesn't currently go through the `TimedEffects`/egui overlay system anything else uses — a real architectural change with no verified defect in the underlying plumbing to justify it (approval condition 4). A static fixed alpha satisfies "no decorative animation" with the smallest possible change.

**Implementation:** `crates/app/src/render.rs` — replaced the sine expression with `let pulse = 1.0;` (full alpha, not the oscillation's ~0.6 average — chosen specifically because Selection being Priority 1 means any replacement "must remain unambiguous and undiminished," per ADR-P5-08's own requirement, and full alpha is strictly more visible than the old animation's average brightness, not less).

**Verification:** `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` — all clean. `cargo test --workspace` — 0 failures across all 28 crates (unchanged test count — this is a one-line constant-value change with no new logic branch to test).

**Remaining limitations, disclosed:**

- No automated test covers this change — `render.rs`'s GPU-submission code has no unit-test harness in this codebase (the same standing limitation disclosed for every rendering-adjacent milestone all through Phases 3-5); correctness here was verified by direct code inspection (the expression is now a literal constant, not a computation that could hide a bug) rather than a runtime test.
- No visual confirmation that the fixed-alpha outline actually reads well in the live app — the recurring "no screen-capture/automation driver" limitation this whole project has disclosed since Phase 3.
- The alternative Health-driven or `TimedEffects`-flash approaches ADR-P5-08 also named are not implemented — this is a deliberate, disclosed choice (see Decision above), not an oversight; either could still be revisited later if a concrete need for a livelier selection signal emerges.

**Roadmap correction status:** none needed — matches ADR-P5-08's own first recommended option exactly.

**Epic J (UI/UX Debt Cleanup) is now fully complete: J1-J6 all done.** Every item audited in Phase 6's original findings — 2 stray dead buttons, 7 dead `MenuAction`s (1 implemented, 6 removed) plus a newly-found dead `WorkbenchCommand` enum, app-preferences persistence, the Carnivore/Omnivore colorblind collision, and the ADR-P5-08 decorative pulse — has been resolved. Per the estimated implementation order (§11), the next epic is **Epic C — Regional Brains**, which already has a complete, approval-ready sub-roadmap (`PHASE4_EPIC1_NEURAL_ROADMAP.md`) sitting unimplemented. Recommend proceeding to Epic C's first milestone (N1a) unchanged, no roadmap correction needed — but this is a larger, multi-milestone epic switch, so awaiting explicit direction before starting it.

### Epic C, Milestone N1a — region plumbing (`RegionId`/`Brain::node_regions`)

**Re-audit before implementing:** re-read `PHASE4_EPIC1_NEURAL_ROADMAP.md` in full and re-verified its central claims directly against current source, since it was written in an earlier phase: confirmed `brain::Brain` (`crates/brain/src/lib.rs`) still has no region/position/segment field (`id, nodes, synapses, input_count, output_count, winner_take_all, plasticity_enabled, external_override` — exactly as audited); confirmed `Brain::new` is the sole construction path (a workspace-wide search for a direct `Brain { ... }` struct literal found zero hits — everything, including all test fixtures, goes through `Brain::new` and its builder methods), meaning a default assigned inside `new()` covers every call site with no other changes required; confirmed `nodes`/`synapses`' mutation methods (`reindex_synapses`, `prune_weak_synapses`) only ever reorder/retain `synapses`, never `nodes` — so a `node_regions` vec parallel to `nodes` can never desync from it without extra synchronization logic.

**A real consequence found during re-audit, not in the sub-roadmap's own text (written before `storage::SchemaVersion` existed in its current form):** `brain::Brain` is embedded directly via bincode in `storage::snapshot::SnapshotNode.brain` — confirmed via direct read of `crates/storage/src/snapshot.rs`. Adding a field to `Brain` changes its bincode positional layout, exactly the same situation `SchemaVersion::CURRENT`'s own doc comment already documents happened once before (bumped 1→2 for Epic 8's `winner_take_all`/`plasticity_enabled` additions). This needed the same treatment here — folded into this milestone since it's a direct, mechanical consequence of the exact field this milestone adds, not separate scope.

**Implementation:**

- `crates/brain/src/lib.rs` — added `RegionId` (`Central` default / `Ganglion(usize)`, `#[derive(Default)]` with `#[default] Central`, per ADR-N1-01: pure CPU-side wiring metadata, not embedded in the `#[repr(C)]`/`Pod` `CtrnnNode` GPU type). Added `Brain::node_regions: Vec<RegionId>`, initialized in `Brain::new` as `vec![RegionId::Central; nodes.len()]` — no signature change, so every existing call site is unaffected and every existing organism's brain topology is bit-for-bit identical to before this milestone.
- `crates/storage/src/lib.rs` — bumped `SchemaVersion::CURRENT` from `2` to `3`, documenting the break in the same style as the prior 1→2 bump (no migration path, matching this project's established, consistent policy).

**Verification:** `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` — all clean. `cargo test --workspace` — 0 failures across all 28 crates, confirming the sub-roadmap's own N1a testing requirement ("existing brain-wiring tests... byte-for-byte unaffected") is satisfied — nothing regressed. 2 new tests added to `crates/brain/src/lib.rs`: `brain_new_defaults_every_node_region_to_central` (asserts `node_regions.len() == nodes.len()` and every entry is `Central`, the sub-roadmap's own named N1a test requirement) and `region_id_default_is_central` (guards the derived `Default` impl against silent drift from what `Brain::new` actually assigns).

**Remaining limitations, disclosed:**

- No behavior change yet — `node_regions` is populated but not read by anything (wiring still ignores it entirely). This is N1a's own explicit, correct scope, not an oversight — N1b is where region actually starts affecting which synapses get wired.
- The schema-version bump means any `.phylon` save file from before this milestone will fail to load — expected, disclosed, consistent with this project's standing no-migration-path policy, not a regression specific to this change.
- `Ganglion(usize)`'s `usize` is a developmental-graph position, but nothing yet validates that position actually corresponds to a real `SegmentType::Ganglion` segment — that validation is N1c's job (finding real Ganglion positions via `DevelopmentalGraph`), not meaningful to build ahead of N1c per the "infra before behavior" pattern this milestone itself follows.

**Roadmap correction status:** the sub-roadmap's own text is followed exactly for N1a's actual scope; the `SchemaVersion` bump is additive evidence (a real consequence the original document didn't anticipate, since it predates certain storage-crate details) recorded here per approval condition 2, not a change to N1a's Goal/Dependencies/Risk/Effort in §3's milestone table.

**N1a is complete.** Per the sub-roadmap's own ordering, N1b (region-bound wiring — "the real behavior change," Medium-High risk) is next, depending on N1a. Awaiting explicit direction before starting N1b.

### Epic C, Milestone N1b — region-bound wiring (the real behavior change)

**Re-audit before implementing:** re-read `growth_system`'s brain-wiring block (`crates/organisms/src/systems.rs`) directly and confirmed it still matched the sub-roadmap's description exactly — a flat double loop over `0..total_nodes` × `input_count..total_nodes`, querying `expressed_brain_cppn` with each pair's raw normalized index, keeping any synapse with `weight.abs() > 0.01`. No region concept anywhere in the loop, consistent with N1a having only added `Brain::node_regions` as a post-construction field, never consulted during wiring itself.

**A real sequencing question surfaced by this re-audit, resolved before writing any code:** the sub-roadmap's own N1b test list names "a test proving cross-region synapse density is measurably lower than intra-region density for a fixture genome with an explicit Ganglion segment" — but Ganglion detection is explicitly N1c's job (per the milestone table, N1c depends on both N1a *and* N1b), so no real genome can produce a non-`Central` region until N1c lands. Resolved by testing the region-aware *wiring rule* directly (a pure function taking an explicit `same_region: bool`), independent of genome/CPPN machinery — proving the rule itself is correct without needing N1c's real detection to exist first. This is consistent with the sub-roadmap's own principle of not pre-building against a not-yet-real shape (the same reasoning it already applied to defer N2's design).

**Implementation:** `crates/organisms/src/systems.rs` — extracted the wiring decision into a new pure function, `should_wire_synapse(weight: f32, same_region: bool) -> bool`, using the existing `0.01` threshold when `same_region` is true and a new, stricter `0.5` threshold when false (two new named constants: `SYNAPSE_WEIGHT_THRESHOLD`, `CROSS_REGION_SYNAPSE_WEIGHT_THRESHOLD`). The wiring loop now builds a local `node_regions: Vec<brain::RegionId>` (every entry `Central` — N1c's future job is to replace this one line with real `DevelopmentalGraph`-based Ganglion detection, nothing else in the loop needs to change), consults `node_regions[i] == node_regions[j]` per pair, and calls `should_wire_synapse` instead of the old inline threshold check. The same `node_regions` vector is written into the constructed `Brain.node_regions` after `Brain::new` (which still only knows how to default to all-`Central` itself, per N1a) — establishing the exact plumbing N1c needs without requiring a second change to this call site later.

**Verification:** `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` — all clean. `cargo test --workspace` — 0 failures across all 28 crates; critically, **every pre-existing `organisms` test passed completely unchanged** (48→56 total is only new additions, no existing test needed modification), itself direct evidence of graceful degradation. 5 new tests added to `crates/organisms/src/systems.rs`, matching the sub-roadmap's own 3 named N1b requirements:

- `should_wire_synapse_same_region_matches_the_pre_n1b_single_threshold_rule` — asserts `should_wire_synapse(w, true)` equals the literal old `w.abs() > 0.01` expression across 11 representative weights (including both sides of the threshold) — a direct mathematical proof of "no observable change when every node is Central," not an indirect snapshot comparison.
- `should_wire_synapse_cross_region_requires_a_stronger_weight_than_same_region` — proves the actual new behavior: a weight that wires within a region fails to wire across one.
- `growth_system_produces_a_brain_with_uniformly_central_regions_today` — an integration-level check that a real organism grown from a real genome ends up with a non-empty, uniformly-`Central` `Brain::node_regions`, confirming the new plumbing (built during wiring, written into the constructed `Brain`) actually reaches the real component, not just the pure-function tests.
- `growth_system_brain_wiring_is_deterministic_for_the_same_genome` — the sub-roadmap's own named determinism test; expected to trivially pass (no RNG or `HashMap` iteration exists anywhere in this wiring path), but verified directly rather than assumed.

**Remaining limitations, disclosed:**

- Cross-region sparsification (`CROSS_REGION_SYNAPSE_WEIGHT_THRESHOLD = 0.5`) has zero observable effect on any organism today, since nothing assigns a non-`Central` region yet — this is N1b's own correct, disclosed scope (the sub-roadmap explicitly separates "the wiring rule exists" from "the wiring rule does something," with the latter gated on N1c), not an incomplete implementation.
- The specific threshold value (`0.5`, chosen as "substantially stronger" than `0.01` without a principled derivation) is a reasonable placeholder, not a tuned constant — matching this project's own established pattern of flagging placeholder rates honestly (e.g. Phase 4's `TRANSPORT_RATE`/`ENDOCRINE_RATE`) rather than pretending a first guess is calibrated. Real tuning only becomes meaningful once N1c produces organisms with actual cross-region pairs to observe.
- The "hub node" mechanism the sub-roadmap's parenthetical example named ("only through designated hub nodes near a Ganglion") was not built — a flat stricter-threshold rule was chosen instead, as the simplest mechanism satisfying "sparser cross-region wiring" without inventing additional undefined concepts (what makes a node a "hub") ahead of N1c's real anatomy. If N1c's real-world results show this is insufficient, a hub-based refinement remains available as a documented follow-on, not foreclosed by this choice.

**Roadmap correction status:** the sub-roadmap's own N1b Goal/Dependencies/Risk/Effort stand unchanged; the sequencing clarification (testing the rule directly rather than via a not-yet-real Ganglion-bearing genome) is recorded here as approval condition 2 requires, not a scope change.

**N1b is complete.** Per the sub-roadmap's own ordering, N1c (Ganglion becomes a real neural anchor — Medium risk, depends on N1a and N1b) is next and is the final N1 milestone. Awaiting explicit direction before starting N1c.

### Epic C, Milestone N1c — Ganglion becomes a real neural anchor (N1 now complete)

**Re-audit before implementing:** re-confirmed `DevelopmentalGraph`'s actual query surface (`root`, `children_of`, `node_at_position`) directly against `crates/organisms/src/developmental_graph.rs` before assuming anything about it, and confirmed no "graph distance between two nodes" helper existed yet — the sub-roadmap's own wording ("nearest... by body-graph distance, not Euclidean") needed one, and P4-F1 deliberately only shipped `root`/`children_of`/`node_at_position` as its "small, generic, non-biology-specific" surface, not a distance metric.

**A real design gap in the sub-roadmap's own text, resolved before writing code:** N1c's description says hidden nodes are anchored "to the nearest [Ganglion]," but hidden CTRNN nodes are abstract units with no body position of their own to measure distance *from* — unlike input nodes (real sensors) or output nodes (real effectors). Resolved by anchoring each hidden node to an evenly-spread target position along the body axis (its own index among just the hidden nodes, scaled across `crate::MAX_SEGMENTS`), then finding whichever real decoded Ganglion is nearest to *that* target position by graph distance. This is a genuine design decision this milestone made, not a pre-specified mechanic — documented explicitly rather than silently picked.

**Implementation:**

- `crates/organisms/src/developmental_graph.rs` — added `DevelopmentalGraph::index_at_position` (same lookup as `node_at_position`, returning an index instead of a reference, since `graph_distance` needs indices) and `DevelopmentalGraph::graph_distance` (a plain BFS over the undirected tree formed by `parent` links — deliberately unoptimized, matching P4-F1's own "small generic primitive, not a specialized structure" principle, and more than fast enough for a graph capped at `MAX_SEGMENTS * 3` nodes).
- `crates/organisms/src/systems.rs` — extracted the full N1c assignment logic into a new pure function, `assign_hidden_node_regions(graph, input_count, hidden_count, total_nodes) -> Vec<RegionId>`, called from `growth_system`'s wiring block in place of N1b's placeholder all-`Central` vector. Collects every decoded `SegmentType::Ganglion` position from the real `DevelopmentalGraph`; for each hidden node, computes its target position, looks up the nearest Ganglion by `graph_distance` (ties broken by lower body position, keeping the result fully deterministic), and assigns `RegionId::Ganglion(position)` — or `Central` if no Ganglion exists, or if no real segment was ever decoded at the exact target position (e.g. pruned by apoptosis; a disclosed fallback, not a guess).

**Verification:** `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` — all clean. `cargo test --workspace` — 0 failures across all 28 crates; again, **every pre-existing test passed completely unchanged** (56→60 in `organisms` is purely additions). 4 new tests, matching the sub-roadmap's own 2 named N1c requirements plus 2 supporting `graph_distance` tests:

- `graph_distance_on_a_straight_spine_is_the_hop_count` / `graph_distance_through_a_shared_branch_point_is_not_the_index_difference` (`developmental_graph.rs`) — the latter specifically proves two fin branches sharing a torso parent are 2 graph-hops apart even though their raw node indices differ by only 1, directly proving distance comes from real tree structure, not index arithmetic.
- `assign_hidden_node_regions_anchors_hidden_nodes_to_the_nearest_ganglion_by_graph_distance` (`systems.rs`) — N1c's first named requirement, using a hand-built `DevelopmentalGraph` fixture with one explicit Ganglion segment (bypassing genome/CPPN decoding entirely, mirroring N1b's own test strategy, since no genome has ever been observed to decode a Ganglion).
- `assign_hidden_node_regions_splits_hidden_nodes_between_two_ganglia_by_distance` (`systems.rs`) — N1c's second named requirement, with two Ganglion segments (positions 2 and 12 of 15) confirming hidden nodes actually split between both (3 anchor to the nearer one, 1 to the farther one) rather than all coincidentally picking the same one.

**Remaining limitations, disclosed:**

- No real genome has ever been observed to decode a `SegmentType::Ganglion` segment — every organism grown from every genome fixture in this test suite still ends up with uniformly `Central` regions (confirmed by the existing, unchanged `growth_system_produces_a_brain_with_uniformly_central_regions_today` test). N1c's real detection logic is now genuinely wired and unit-tested, but has never been observed to fire on a real, evolved organism. Whether real evolutionary pressure ever produces a Ganglion segment at all is an open, unverified question this milestone doesn't answer.
- The "evenly-spread target position" scheme for hidden nodes is this milestone's own design choice, not dictated by the sub-roadmap — a reasonable, deterministic, testable resolution of a real gap in N1c's original wording, but one specific choice among others that could have been made (e.g., weighting by CPPN-evaluated node properties instead of raw index). Documented as a decision, not a discovery.
- A hidden node whose target position was pruned by apoptosis falls back to `Central` rather than searching nearby positions — a simple, disclosed limitation, not a sophisticated nearest-valid-position search.

**Roadmap correction status:** N1c's Goal/Dependencies/Risk/Effort stand as originally written; the hidden-node-anchoring mechanism (undefined in the sub-roadmap's own text) is recorded here as a real design decision this milestone made, per approval condition 2.

**N1c is complete — Epic C (Regional Brains, N1a/N1b/N1c) is now fully implemented**, exactly as `PHASE4_EPIC1_NEURAL_ROADMAP.md` scoped it. Per that same document (§4), **N2 (axon guidance/neuron migration/pruning) is deliberately left unscoped**, pending its own re-audit now that N1's actual shape exists in code — not designed speculatively here. Per Phase 6's own priority order (§11), the next epic is **Epic D — Reaction-Diffusion Morphogens**, which (like Epic C) already has a complete, approval-ready sub-roadmap (`PHASE4_EPIC4_MORPHOGEN_ROADMAP.md`) sitting unimplemented. Awaiting explicit direction before starting Epic D or N2's re-audit.

### Epic D, Milestone D1a — Intra-organism morphogen field (ADR-D1-01's graph-based half)

**Re-audit before implementing:** re-read `PHASE4_EPIC4_MORPHOGEN_ROADMAP.md` in full, then re-confirmed its architectural claims directly against current source before trusting them — `genetics::morphogen.rs`'s `ap_position`/`distance_from_head_gradient`/`external_inputs_for_position` were unchanged as described, but the sub-roadmap's `develop_at_position` reference turned out stale: development had moved to `crates/genetics/src/develop.rs`, and — the significant find — **`develop_at_position_with_life_stage` (Phase 4, P4-L1, ADR-P4-03) already exists** and is not mentioned anywhere in the D1 sub-roadmap (written earlier in Phase 4, before P4-L1 landed). It already implements exactly the seam D1a's own text proposed inventing from scratch: an extra scalar folded additively into every regulatory gene's external input, with `0.0` reproducing `develop_at_position`'s original output exactly (proven by its own `life_stage_signal_zero_matches_develop_at_position` regression test). Also re-confirmed `transport_system`/`endocrine_diffusion_system`'s exact edge-walking shape and the GPU diffusion field's real 4-layer structure (`FieldLayer::{Pheromones,Energy,O2,CO2}`) directly against `crates/organisms/src/transport.rs`/`endocrine.rs` and `crates/diffusion/src/lib.rs` — both still accurate as described.

**Decision (a scope simplification the sub-roadmap's author couldn't have known about, not a deviation from ADR-D1-01's intent):** rather than adding a second, parallel field-reading parameter to `genetics::develop_at_position`, this milestone reuses the existing `life_stage_signal` channel — `organisms::systems::growth_system` sums the new field reading and `life_stage_signal` together before the one call to `develop_at_position_with_life_stage`. This means **`genetics::develop_at_position`/`simulate_growth_timeline` are untouched by D1a** — no `genetics`-crate signature change, no new call-site updates across `growth_system`/`spawning`/`simulate_growth_timeline` (the exact ripple D1a's own sub-roadmap text anticipated needing). ADR-D1-01 itself (the graph-vs-GPU-field split) is not superseded, only its D1a row's originally-imagined mechanism is simplified given evidence that didn't exist when it was written.

**Implementation:**

- New module `crates/organisms/src/morphogen_field.rs` — `MorphogenLevel { concentration: f32 }`, a per-segment `Component` (mirroring `ChemicalEconomy`/`HormoneLevel`'s existing per-segment pattern rather than a per-organism `Vec` indexed by graph position, since this crate already has two precedents for "per-segment component + Body-Graph-edge-walking system"). `morphogen_diffusion_system` walks the same parent-before-child edges `transport_system`/`endocrine_diffusion_system` use, but relaxes bidirectionally (unlike `endocrine_diffusion_system`'s fixed-source one-directional model) **and** applies a uniform per-tick decay (`MORPHOGEN_DECAY_RATE = 0.1`) — the "reaction" term neither existing system needs, since neither models a signal that fades on its own. `MORPHOGEN_SEED_CONCENTRATION = 1.0` is the emission source: growth itself, not a separate system — every newly-spawned segment (head in `spawning::spawn_organism`; every spine and fin node in `growth_system`) is seeded at this value.
- `growth_system` gained a `morphogen_query: Query<&MorphogenLevel>` param; before each position's decode, reads `state.parent_spine_node`'s current concentration (`0.0` if `None`, i.e. for the very first grown position) and adds it to `life_stage_signal`.
- Registered `organisms::morphogen_diffusion_system` in `crates/app/src/simulation.rs`'s tick schedule, immediately after `endocrine_diffusion_system` (same "circulation" family, same Body Graph edges) — confirmed via search that this is the only schedule registration point in the codebase (no second batch/headless runner duplicates it).

**A real regression this milestone's own change caused, found and fixed, not left broken:** `systems::tests::simulate_growth_timeline_matches_a_real_growth_system_run` (Phase 3 M13's cross-check for ADR-P3-09/ADR-P4-01's "the graph is a pure function of genome+position, so it's safe not to persist it" claim) started failing (12 vs. 14 grown segments) once `growth_system` began folding a nonzero field reading into decode. Root cause: that test's fixture never runs `morphogen_diffusion_system`, so every segment after the first reads its parent's *undecayed* seed concentration (`1.0`) every tick — a real, reproducible, and expected divergence, not a bug, since `simulate_growth_timeline` correctly never models field state at all. This is precisely the divergence `PHASE4_EPIC4_MORPHOGEN_ROADMAP.md` §3.3/D1c anticipated, just arriving one milestone earlier than the sub-roadmap assumed. Resolved (not deferred, since a broken pre-existing regression test cannot be left failing per this roadmap's own verification discipline) by re-scoping the test in place: exact node-for-node equality is retired; the test now asserts (a) the head and first grown position — both still genuinely field-free on both sides, since `parent_spine_node` is `None` until the first spine node exists — still match exactly, and (b) both graphs still complete to a sane, bounded, non-empty shape. The doc comment explains why, and explicitly defers the *quantified* divergence measurement D1c's sub-roadmap text calls for to that milestone, rather than duplicating it here.

**Verification:** `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` — all clean. `cargo test --workspace` — 0 failures across all 28 crates. `organisms` crate: 60→64 tests (4 new: 3 in `morphogen_field.rs` covering diffusion-toward-tail, decay-toward-zero-when-isolated, and determinism; 1 in `systems.rs`, `growth_system_decode_changes_when_the_tips_morphogen_level_is_nonzero`, D1a's own named testing requirement — reuses `genetics::develop::nonzero_life_stage_signal_can_change_the_decode`'s exact hand-built sensitive CPPN and signal magnitude to prove `growth_system` itself, not just the underlying `genetics` function, is sensitive to a nonzero field reading). One pre-existing test re-scoped as described above (not silently weakened — the divergence is disclosed in its own doc comment).

**Remaining limitations, disclosed:**

- Diffusion/decay rate constants (`MORPHOGEN_DIFFUSION_RATE = 0.3`, `MORPHOGEN_DECAY_RATE = 0.1`) are untuned placeholders, same status as `TRANSPORT_RATE`/`ENDOCRINE_RATE` — no biological calibration has been attempted (that is Epic M's job, once enough of Epic D/E/F exists to calibrate against).
- The field signal only ever influences growth *while an organism is actively growing* (every position after the first reads whatever concentration currently sits on the previous tip) — a mature, fully-grown organism's `MorphogenLevel` components still exist and still diffuse/decay every tick (harmless, since nothing reads them post-growth), but this milestone adds no new consumer of that steady-state value. Whether one is worth adding is a future-work question, not decided here.
- D1b (the inter-organism/environmental half of ADR-D1-01 — a 5th world-space GPU diffusion layer) and D1c (the formal quantified-divergence test) remain unimplemented, exactly as scoped.

**Roadmap correction status:** ADR-D1-01 stands as written (the graph-vs-GPU split is confirmed correct). D1a's *mechanism* is simplified from "new `genetics` parameter" to "reuse the existing `life_stage_signal` channel," recorded here as a superseding implementation note, not a rewrite, per approval condition 4. D1c's scope is unchanged, but this entry discloses that part of its concern (test divergence) was necessarily addressed now rather than deferred.

Per Phase 6's priority order, the next steps are **D1b** (the GPU-touching layer) or **D1c** (the quantified-divergence test, now easier since D1a already re-scoped the one test it broke) — both remain part of the already-approved Epic D sub-roadmap. Awaiting explicit direction on which to take next.

### Epic D, Milestone D1b — Inter-organism/environmental morphogen layer (ADR-D1-01's GPU-touching half)

**Re-audit before implementing:** re-read `PHASE4_EPIC4_MORPHOGEN_ROADMAP.md` §1.2's architecture claims and re-confirmed each directly against current source. `crates/gpu/src/diffusion_pipeline.rs`/`diffusion_step.wgsl` were unchanged as described (4-layer ping-pong `texture_2d_array`, 5-point Neumann-reflecting Laplacian, `[0,1000]` clamp). **One claim was stale**, however: the sub-roadmap's §1.2 states `DiffusionComputePipeline` is "instantiated exactly twice total (main field + a separate hazard field)". Tracing every call site (`crates/app/src/app.rs:506` and `:617`) found these are the *windowed* and *headless* GPU-init paths for the **same single main field** — alternative code paths for one pipeline, not two simultaneous instances. `diffusion::CpuHazardFieldState` (the actual hazard field `ecology::catastrophe_system` reads/writes) has no GPU pipeline behind it at all — it's pure-CPU (`splat`/`clear` operate directly on its own `Vec<f32>`, confirmed via a full-repo search for any GPU reference near it, finding none). This doesn't change D1b's design (there is still exactly one real 4-layer GPU field, and expanding it to 5 is exactly what ADR-D1-01 calls for) — it only corrects which existing infrastructure D1b actually extends, recorded here per approval condition 3 rather than silently proceeding on the stale premise.

**Implementation:**

- `crates/diffusion/src/lib.rs` — added `FieldLayer::Morphogen = 4`; updated `CpuFieldState::sample`'s doc comment to document all 5 layers.
- `crates/gpu/src/diffusion_pipeline.rs` — added `pub const LAYER_COUNT: u32 = 5`, replacing every previously-hardcoded `4` (texture `depth_or_array_layers`, per-layer view `Vec` capacity/loop, compute-dispatch z-extent, `DiffusionUniforms::layers` array size, staging-buffer size, the second `copy_texture_to_buffer` `Extent3d`) with either `LAYER_COUNT` or `LAYER_COUNT as usize` — a single named constant so this shape can't drift out of sync again, matching this milestone's own risk note in §5 of the sub-roadmap.
- `crates/gpu/src/diffusion_step.wgsl` — `array<LayerConfig, 4>` → `array<LayerConfig, 5>`; the bounds guard `layer >= 4u` → `layer >= 5u`.
- `crates/app/src/simulation.rs` — added a 5th "Layer 4: Morphogen" emitter-gathering block, mirroring the existing O2/CO2 blocks' shape exactly: queries every `(ParticleNode, organisms::MorphogenLevel)`, skips zero/negative concentrations, and emits proportional to concentration. This naturally emits only near real developmental activity without a separate "is this organism growing" query — a mature organism's segments' `MorphogenLevel` decays toward zero once nothing reseeds it (D1a), so they stop contributing on their own.
- `crates/organisms/src/systems.rs` — `growth_system` gained an `Option<Res<diffusion::CpuFieldState>>` param (`Option`, not `Res`, so every existing test `World` that never inserts this resource keeps working unchanged — a missing field reads as "no environmental signal yet," the same neutral baseline a real run has before the GPU's first successful readback). Moved the `spawn_pos` computation earlier in the function (the formula itself is unchanged) so a new `environmental_signal` — the Morphogen layer sampled at that position — can be computed before the decode call, then summed into the same `life_stage_signal + field_signal` channel D1a already established. Unlike `field_signal` (this organism's own intra-organism signal), `environmental_signal` can carry a real contribution from *other* developing organisms nearby — the actual inter-organism coupling ADR-P3-03's reversal trigger named.

**Verification:** `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings` (required one `#[allow(clippy::too_many_arguments)]` on `growth_system`, now at 8 params — same precedent as `catastrophe_system` in Epic A), `cargo fmt --all -- --check` — all clean. `cargo test --workspace` — 0 failures across all 28 crates, including doc-tests. New tests:

- `diffusion::tests::cpu_field_state_samples_the_morphogen_layer_independently_of_the_other_4_layers` — D1b's own named CPU-side requirement: writes distinct recognizable values into all 5 layers at the same grid cell, confirms `sample` returns each layer's own value with no cross-channel bleed.
- `gpu::diffusion_pipeline::tests::diffusion_pipeline_steps_all_5_layers_and_the_morphogen_layer_stays_isolated` — this milestone's own GPU-validation requirement, honored literally rather than satisfied by `cargo test` alone: a **real** headless `wgpu` device and adapter (mirroring `app::app::init_gpu_headless`'s exact request shape), a real `DiffusionComputePipeline`, a real compute dispatch with a single emitter targeted only at layer 4, and a real GPU→CPU readback — confirming all 5 layers round-trip (texture depth, dispatch z-extent, and uniform array size all actually agree; a mismatch between any of these would corrupt data or panic inside the shader, not just fail a value comparison) and that the emission produced activity at layer 4 without leaking into layers 0-3.
- **Beyond `cargo test`:** ran the real app binary headlessly for 200 real ticks (`research.headless: true`, `max_ticks: 200`, both temporarily set in `data/default.ron` for this run only and reverted via `git checkout` immediately after — confirmed via `git status`/`git diff` that no unrelated changes leaked). Exit code 0, log shows "Headless run completed (reached max_ticks = 200)" with no wgpu validation errors, no panics, and no bind-group/texture-mismatch errors from the diffusion pipeline specifically. (An unrelated pre-existing `bevy_ecs` `B0003` double-despawn warning appeared once during the run — not touched by this milestone's diff, not investigated further here, noted only for completeness.)

**Remaining limitations, disclosed:**

- The new layer's `diffusion_rate`/`decay_rate` (`0.3`/`0.1`, matching D1a's own intra-organism constants) are untuned placeholders, same status as every other layer's rates — no biological calibration attempted (Epic M's job).
- No organism has ever been observed to actually receive a nonzero `environmental_signal` from *another* organism's emission in practice (never run at real population scale with two organisms developing close enough together, in the same tick, for one's emission to reach another before decaying) — the mechanism is real, tested at the pipeline level, and wired end-to-end, but its actual triggering in an evolving population is an open, unverified question, exactly like N1c's disclosed Ganglion-detection limitation.
- Per the sub-roadmap's own non-goals (§3.2): no visualization of the new layer was added (the existing heatmap's `ActiveHeatmap` enum match in `render.rs` is untouched — adding a 5th layer to the texture array doesn't affect it, confirmed by inspection, since that match dispatches on named enum variants, not a layer-count-driven loop).

**Roadmap correction status:** ADR-D1-01 stands as written. D1b's own architecture premise (§1.2, "instantiated exactly twice") is corrected per the re-audit finding above — recorded as a superseding note, not a rewrite, per approval condition 4.

**D1b is complete.** Per the sub-roadmap's own dependency table, **D1c** (the `simulate_growth_timeline`/live-run quantified-divergence test) depends on both D1a and D1b, both now landed — D1c is unblocked. Awaiting explicit direction before starting D1c or moving on to Epic E.

### Epic D, Milestone D1c — `simulate_growth_timeline` reconciliation (Epic D now complete)

**Re-audit before implementing:** re-read `PHASE4_EPIC4_MORPHOGEN_ROADMAP.md` §3, D1c's row: "update its doc comment to explicitly scope it as genome + zero-field-input analytic replay... add a comparison test that quantifies... how far a live run's decode diverges... so future regressions are caught by magnitude, not just a binary pass/fail." Re-confirmed against current source that D1a had already done *part* of this job as a necessary side effect of fixing the regression it caused (re-scoping `simulate_growth_timeline_matches_a_real_growth_system_run`'s assertions and doc comment) — but D1a's own execution-log entry explicitly deferred the *quantified* magnitude measurement to D1c, and `simulate_growth_timeline`'s own doc comment (in `developmental_graph.rs`) had not yet been updated to state its new scope. Both remaining gaps are exactly D1c's named job, confirmed nothing else in Epic D's implementation shifted since D1b landed.

**Implementation:**

- `crates/organisms/src/developmental_graph.rs` — added a scope note to `simulate_growth_timeline`'s doc comment stating explicitly that it calls the zero-field-input `genetics::develop_at_position` (never `develop_at_position_with_life_stage`), and is therefore "a lower-bound / zero-field reference reconstruction, not a live-run-identical prediction" whenever D1a/D1b's field state is nonzero — pointing at the new quantified test (below) rather than re-deriving the reasoning inline.
- `crates/organisms/src/systems.rs` — added `real_run_field_signal_divergence_from_the_pure_replay_is_bounded_and_quantified`, reusing the exact same M8 apoptosis-fixture CPPN as `simulate_growth_timeline_matches_a_real_growth_system_run` (a direct continuation of that test's own finding). Quantifies divergence two ways rather than asserting a single equal/unequal outcome:
  1. **Node-count delta**, asserted `> 0` (this fixture must still show real divergence — a regression detector for "the fixture stopped exercising the effect") **and** `< MAX_SEGMENTS` (an upper sanity bound — a regression detector for "the field signal now dominates decode so badly the body plan collapses or explodes," which a bare non-zero check would miss entirely).
  2. **First divergent position**, asserted `>= 2` when a divergence exists at all — positions 0 (head) and 1 (grown before any `MorphogenLevel` exists to read from) must stay field-free and identical on both sides; this bound would catch a future change that accidentally made an *earlier* position field-sensitive.
- Cleaned up a stray leftover comment fragment (`// node-for-node.`) in `simulate_growth_timeline_matches_a_real_growth_system_run`'s doc comment left over from D1a's edit, and updated that comment to point forward at this milestone's new test by name instead of restating D1c's job description in two places.

**Verification:** `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` — all clean. `cargo test --workspace` — 0 failures across all 28 crates, including doc-tests. `organisms` crate: 64→65 tests (1 new).

**Remaining limitations, disclosed:**

- The node-count-delta bounds (`> 0`, `< MAX_SEGMENTS`) are deliberately loose — wide enough to not be a tuning-sensitivity trap (this project's own untuned-placeholder rates could shift the exact delta at any time without that being a real regression), narrow enough to still catch a genuine blow-up. This is a documented design tradeoff, not an oversight: a tighter bound pinned to today's exact empirical value (14 vs. 12) would make this test brittle to legitimate future rate tuning (Epic M).
- This test exercises one fixture genome, not a distribution — it demonstrates the divergence is bounded and measurable for a genome known to exercise real pruning, not that every possible genome's divergence stays within these bounds. A broader statistical sweep (many random genomes) is future work, not claimed here.

**Roadmap correction status:** D1c's Goal/Dependencies/Risk/Effort stand as originally written; no correction needed.

**D1c is complete — Epic D (Reaction-Diffusion Morphogens, D1a/D1b/D1c) is now fully implemented**, exactly as `PHASE4_EPIC4_MORPHOGEN_ROADMAP.md` scoped it, including ADR-D1-01's graph-vs-GPU split and both re-audit-driven corrections (the already-existing `develop_at_position_with_life_stage` seam in D1a; the stale "two pipelines" claim in D1b). Per that same document (§4), **P4-D2 (Temporal Gene Expression) is deliberately left unscoped**, pending its own re-audit now that D1's actual shape exists in code — not designed speculatively here. Per Phase 6's own priority order (§11), the next epic is **Epic E — Physiology Calibration & Save/Load Coverage**. Awaiting explicit direction before starting Epic E or D2's re-audit.

### Epic E — Save/Load Physiology & Graph Coverage (workstream 2 of 2)

**Re-audit before implementing — a materially bigger finding than the epic's own Context paragraph described:** Epic E's Context names 4 components missing from `storage::SimulationSnapshot` (`HormoneLevel`, `SegmentInfection`, `SegmentImmunity`, `DevelopmentalGraph`). Re-auditing `crates/storage/src/snapshot.rs` directly before writing any code found the real gap is far larger: **none** of `metabolism::{ChemicalEconomy, Age, Metabolism, Health, Hydration, BodyTemperature}` or `brain::Neuromodulators` were covered either, and none of the 8 total missing types derived `Serialize` at all. Concretely, this meant a reloaded organism's `metabolism_system` query (which requires `ChemicalEconomy`+`Age`+`Metabolism`) never matched it again post-restore — a "loaded" organism looked visually identical but had silently and completely stopped metabolizing, aging, or being capable of dying of starvation. This is a substantially larger scope than the epic described, so — per approval condition 1 ("do not silently expand scope") — this finding was surfaced to the user directly before any implementation began; the user chose **full coverage** over a narrower first pass. This entry documents that as an explicit scope correction, not a silent expansion.

**Implementation:**

- Added `Serialize`/`Deserialize` directly to 11 existing component types, in their own home crates (matching this codebase's existing pattern for `organisms::{Generation,SpawnTick,LifeStage}`/`OrganismColor`, rather than duplicating their shape into storage-side wrapper structs): `metabolism::{ChemicalEconomy, Age, Metabolism, Health, Hydration, BodyTemperature}` (new `serde` dependency added to the `metabolism` crate, which had none before), `brain::HormoneLevel`, `ecology::disease::{Infection, SegmentInfection, SegmentImmunity}`, `organisms::morphogen_field::MorphogenLevel`, and `genetics::DevelopmentalOutputs` (needed for the graph, below).
- `brain::Neuromodulators` was deliberately **not** given a direct derive — its private `last_atp` bookkeeping field has no public accessor, and adding one purely for serialization felt like the wrong seam to add to a crate that otherwise has zero storage-awareness. Instead, `SnapshotNode` stores just its 3 public channels (`neuromodulator_channels: Option<[f32; 3]>`); `restore_world` reconstructs via `Neuromodulators::new(restored_atp)` and overwrites the 3 channels — a documented, minor precision cost (one tick's stale `last_atp` baseline) rather than a correctness break.
- `crates/storage/src/snapshot.rs` — added `metabolism` as a new dependency; extended `SnapshotNode` with ~15 new optional fields (reusing the real domain types directly wherever they now derive `Serialize`, not bespoke wrappers) plus a new `SnapshotDevelopmentalNode`/`developmental_graph: Option<Vec<SnapshotDevelopmentalNode>>` pair for the persistent Body Graph, whose `entity: Option<Entity>` field is remapped through the exact same stable-`u64`-id scheme `SnapshotSpring` already established for cross-entity references.
- `SimulationSnapshot::from_world` restructured: the original single 7-tuple query now only builds the `nodes` skeleton plus an `entity_to_index` map; ~13 additional small single-purpose queries (one per new component type) then fill in each `SnapshotNode`'s new fields by index. This is a deliberate structural choice, not incidental — folding all ~20 optional components into one query tuple would exceed Bevy's practical `WorldQuery` tuple-arity ceiling.
- `SimulationSnapshot::restore_world` mirrors this: inserts each present optional component during the main per-node spawn loop, then does a **second pass** specifically for `DevelopmentalGraph` (deferred because a graph node's `entity_id` may reference a sibling segment not yet spawned in per-node order — the graph can only be attached once every entity in `id_map` exists).

**Verification:** `cargo build --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` — all clean. `cargo test --workspace` — 0 failures across all 28 crates, including doc-tests. New test: `storage::snapshot::tests::save_then_load_round_trip_preserves_previously_lost_physiology_and_graph_state` — builds a 2-entity organism (head + one body segment) carrying every newly-covered component, snapshots it, restores into a fresh `World`, and confirms each value survived by querying the *restored* world (not just inspecting the intermediate snapshot, which would only prove `from_world` and miss a `restore_world` bug) — including confirming the restored `DevelopmentalGraph`'s `entity` references now point at real, distinct, freshly-spawned entities rather than stale or `None` values. Fixed one pre-existing test (`export_organisms_csv_writes_header_and_rows`) that constructed a `SnapshotNode` literal predating this milestone's new fields.

**Remaining limitations, disclosed:**

- `organisms::components::GrowthState` (an organism mid-growth) is still not covered — saving/loading mid-growth is a real, disclosed gap, not silently assumed away; an organism saved while still growing will resume as if growth had already completed (whatever segments existed at save time become its final body plan). Scoped out as a separate, smaller future gap, not part of "physiology" in the sense this milestone's re-audit finding was about.
- `last_atp`'s one-tick reseed-on-restore imprecision (above) is a documented, minor cost, not a hidden one.
- This is workstream 2 of Epic E's two independent workstreams. **Workstream 1 (calibration of placeholder physiological rate constants) is not started** — the epic's own text frames these as independent, either-can-proceed-alone tracks.

**Roadmap correction status:** Epic E's Context paragraph understated workstream 2's real scope (4 named components vs. the actual ~15); this is recorded here as the correction, following the same discipline as every prior re-audit finding this phase. The Decision/Reason/Dependencies/Risk/Future-Trigger text otherwise stands as written.

**Workstream 2 is complete.** Per Phase 6's own priority order, the next epic is **Epic F — Life-Cycle & Multi-Generation Depth** (priority 6), or Epic E's own workstream 1 (calibration). Awaiting explicit direction on which to take next.
