# Phylon — Full Architecture Roadmap

## GPU-accelerated, decentralized soft-body ecosystem simulation

---

## At a Glance

| Phase | Version | Theme | Status |
| --- | --- | --- | --- |
| 0 | v0.0.1 | Foundation | ✅ Complete |
| 1 | v0.1 | Decentralized Physics & Debug Renderer | ✅ Complete |
| 1.5 | v0.15 | Topological Plasticity | ✅ Complete |
| 2 | v0.2 | Fields & Environment | ✅ Complete |
| 2.5 | v0.25 | Closed-Loop Fields | ✅ Complete |
| 3 | v0.3 | CPPN/HyperNEAT Morphology & Passive Ecology | ✅ Complete |
| 3.5 | v0.35 | Sexual Recombination & Drift | ✅ Complete |
| 4 | v0.4 | Actuation, Cognition & Behavior | ✅ Complete |
| 4.5 | v0.45 | Learned Gaits | ✅ Complete |
| 7 | v0.7 | Procedural Visuals & Trails | ✅ Complete |
| 5 | v0.5 | UI & Analytics | 🔧 Wrapping Up |
| 8 | v0.8 | Application Shell | 🔧 Started (overlapping with 5) |
| 6 | v0.6 | Persistence & Speciation Tools | 📋 Next Up |
| 9 | v1.0 | Headless MARL & Network | 📋 Planned |
| 10 | v1.1 | Emergent Signaling | 💭 Speculative |
| 11 | v1.2 | Catastrophe Engine | 💭 Speculative |
| 12 | v1.3 | Spectator & Lineage Narration | 💭 Speculative |
| — | Unscheduled | Future Scope | 💭 Conceptual |

*Note: Phase 7 completed out of sequence — dual-mode rendering (Structural/SDF) and
highlight work landed before Phase 5/8 fully wrapped, which is fine; phases here track
dependency order, not strict chronology.*

---

## ✅ Phase 0 — Foundation (v0.0.1)

Workspace scaffolding, acyclic crate DAG, `bevy_ecs` + `wgpu` baseline, stable window,
fixed-tick scheduler.

## ✅ Phase 1 — Decentralized Physics & Debug Renderer (v0.1)

Graph-based soft-body physics (nodes + spring-constraints), GPU flat-buffer allocation,
instanced-quad rendering.

### ✅ Phase 1.5 — Topological Plasticity (v0.15)

Runtime-mutable graph topology.

## ✅ Phase 2 — Fields & Environment (v0.2)

Diffusion fields (oxygen, pheromones, and the expanded set: Sunlight, CO2, Soil Fertility)
via WGSL compute shaders.

### ✅ Phase 2.5 — Closed-Loop Fields (v0.25)

Closed-loop readback, environment background clear-color fixed, nutrient field rendering
debugged and corrected.

## ✅ Phase 3 — CPPN/HyperNEAT Morphology & Passive Ecology (v0.3)

CPPN-driven Hox/branching morphology replacing the old fixed grid/mesh placeholder —
spine + lateral fin/limb branching genuinely driven by genetics, verified across multiple
distinct organism topologies. Mineral/Corpse/Decomposer nutrient recycling loop and
Ecological Category system (Keystone/Indicator/Endemic/Invasive) implemented, with
genetics-driven color preserved as the authoritative organism color source.

### ✅ Phase 3.5 — Sexual Recombination & Drift (v0.35)

Crossover and mutation operators for Hox/CPPN genomes.

## ✅ Phase 4 — Actuation, Cognition & Behavior (v0.4)

CTRNN brain wired to `muscle_actuation.wgsl` via a CPG (Central Pattern Generator),
confirmed working after the camera-desync bug that was masquerading as movement got fixed.
Raycasting/vision-cone sensing implemented (Left/Center/Right inputs), with a
body-scale-relative self-occlusion radius.

### ✅ Phase 4.5 — Learned Gaits (v0.45)

Brain output drives actuation amplitude/phase directly per tick.

## ✅ Phase 7 — Procedural Visuals & Trails (v0.7)

Dual-mode rendering (Structural debug vs. SDF/Metaball organic skin), adjustable bone-line
thickness, hard-edged hover/selection highlight outlines derived from the same SDF density
union (not per-node strokes), extended to Food/Mineral pellets and Corpses.

## 🔧 Phase 5 — UI & Analytics (v0.5) — Wrapping Up

**Done:** entity-graph inspection (segment tree, not flat list), global keyboard shortcuts
(audited for conflicts, focus-scoped), viewport pan/zoom/touch input, population/FPS
analytics, Structural/Vision-Cone debug overlays, Genetics panel (CPPN topology
visualizer, color swatch), Component Editor (physics tuning sliders).

**Left to finish:** real GPU profiling via `wgpu::QuerySet` timestamps (CPU-timer fallback
already in place as the interim), final pass on remaining pellet/corpse render-mode parity
(confirm visible in both Structural and SDF modes consistently).

## 🔧 Phase 8 — Application Shell (v0.8) — Started, Overlapping with 5

**Done:** persistent `TopBottomPanel` navigation, Main Menu/homepage with consistent
button sizing, non-blocking pause state (no more blocking modal), File-menu-routed
Save/Load/Settings/Quit with two-step confirmation instead of popups.

**Left to finish:** full separation of sim config from UI state (audit needed — confirm
this is actually clean, not just incidentally working), non-blocking async progress
overlays (not yet built at all — needed once Phase 6 save/load does real file I/O that
could take noticeable time).

## 📋 Phase 6 — Persistence & Speciation Tools (v0.6) — Next Up

**Status reality check:** "Open Recent" is currently a mock UI button — no real
`bincode`/`ron` serialization, no SQLite run-tracking, no replay system, no formal
speciation clustering (Levenshtein distance on Hox sequences) yet exists. This is the
actual next phase of substantive work, not a wrap-up item.

## 📋 Phase 9 — Headless MARL & Network (v1.0)

Not started — fully planned only.

## 💭 Phase 10 — Emergent Signaling (v1.1)

## 💭 Phase 11 — Catastrophe Engine (v1.2)

## 💭 Phase 12 — Spectator & Lineage Narration (v1.3)

## 💭 Future Scope (Unscheduled)

3D Environment, Multi-User Worlds, Procedural Terrain/Biomes, Modding API, Cloud-Hosted
Worlds, AR/VR Spectator Mode, Mobile Companion App — unchanged from prior roadmap version.

---

*Reordering note: Phase 7 jumped ahead of 5/8 in actual build order — that's fine
architecturally, since rendering didn't strictly depend on UI shell completion. Phases 5
and 8 should both be closed out before starting Phase 6's real persistence work, since
async progress overlays (Phase 8) are specifically needed to make Phase 6's save/load
operations not freeze the UI.
