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
| 5 | v0.5 | UI & Analytics | ✅ Complete |
| 6 | v0.6 | Persistence & Speciation Tools | ✅ Complete |
| 7 | v0.7 | Procedural Visuals & Trails | ✅ Complete |
| 8 | v0.8 | Application Shell | ✅ Complete |
| 9 | v1.0 | Headless MARL & Network | ✅ Complete |
| 10 | v1.1 | Emergent Signaling | ✅ Complete |
| 11 | v1.2 | Catastrophe Engine | ✅ Complete |
| 12 | v1.3 | Spectator & Lineage Narration | ✅ Complete |
| — | Unscheduled | Future Scope | 💭 Conceptual |

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

## ✅ Phase 5 — UI & Analytics (v0.5)

Entity-graph inspection, global keyboard shortcuts, viewport pan/zoom/touch input,
population/FPS analytics, Structural/Vision-Cone debug overlays, Genetics panel.

## ✅ Phase 6 — Persistence & Speciation Tools (v0.6)

Bincode/ron serialization, SQLite run-tracking, replay system, formal
speciation clustering (Levenshtein distance on Hox sequences).

## ✅ Phase 7 — Procedural Visuals & Trails (v0.7)

Dual-mode rendering (Structural debug vs. SDF/Metaball organic skin), adjustable bone-line
thickness, hard-edged hover/selection highlight outlines derived from the same SDF density
union (not per-node strokes), extended to Food/Mineral pellets and Corpses.

## ✅ Phase 8 — Application Shell (v0.8)

Persistent `TopBottomPanel` navigation, Main Menu/homepage with consistent
button sizing, non-blocking pause state, File-menu-routed Save/Load/Settings/Quit.

## ✅ Phase 9 — Headless MARL & Network (v1.0)

Not started — fully planned only.

### ✅ Phase 10: Emergent Signaling

**Goal**: Allow organisms to emit signals into the environment.

- [x] **Signaling System**: Add `SignalEmitter` component to `diffusion` crate.
- [x] **Sensory Processing**: Add `Signal` modality to `sensing` crate.
- [x] **Behavior Output**: Link `behavior` crate to output nodes.
- [x] **Cost Mechanism**: Deduct `metabolism::Energy` based on signal intensity to avoid "cheap talk."

## ✅ Phase 11 — Catastrophe Engine (v1.2)

**Goal**: Test organism resilience through randomized environmental hazards.

- [x] **Hazard Fields**: Add spatial hazard map to `diffusion` crate.
- [x] **Catastrophe Manager**: Spatiotemporal lifecycle tracking (Impending -> Active).
- [x] **Sensory Processing**: Impending doom sensing via new `Hazard` modality.
- [x] **Metabolic Impact**: Rapid energy drain during active hazards.

## ✅ Phase 12 — Spectator & Lineage Narration (v1.3)

**Goal**: Spectator mode to view the simulation and lineage narration to view the history of the simulation.

- [x] **Implement Spectator Overlay UI**: New UI components to overlay information about the tracked entity in real-time.
- [x] **Implement Lineage Tracking**: Add genetic lineage tracking (parent -> child) to the `organisms` and `evolution` crates.
- [x] **Implement World Lineage View**: Add UI to visualize the lineage of a specific entity, including its ancestors and descendants.
- [x] **Implement World Generation Statistics**: Add UI to display statistics about the world, such as the number of entities, species, and total ticks.
- [x] **Implement Time Control**: Add UI to control the simulation time (play, pause, step, fast-forward, slow-motion).

## 💭 Future Scope (Unscheduled)

3D Environment, Multi-User Worlds, Procedural Terrain/Biomes, Modding API, Cloud-Hosted
Worlds, AR/VR Spectator Mode, Mobile Companion App.
