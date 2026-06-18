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
| 3 | v0.3 | Hox Genetics & Passive Ecology | ✅ Complete |
| 3.5 | v0.35 | Sexual Recombination & Drift | ✅ Complete |
| 4 | v0.4 | Actuation, Cognition & Behavior | 🔧 In Progress |
| 4.5 | v0.45 | Learned Gaits | 💭 Speculative |
| 5 | v0.5 | UI & Analytics | 📋 Planned |
| 6 | v0.6 | Persistence & Speciation Tools | 📋 Planned |
| 7 | v0.7 | Procedural Visuals & Trails | 📋 Planned |
| 8 | v0.8 | Application Shell | 📋 Planned |
| 9 | v1.0 | Headless MARL & Network | 📋 Planned |
| 10 | v1.1 | Emergent Signaling | 💭 Speculative |
| 11 | v1.2 | Catastrophe Engine | 💭 Speculative |
| 12 | v1.3 | Spectator & Lineage Narration | 💭 Speculative |
| — | Unscheduled | Future Scope | 💭 Conceptual |

---

## Phase 0 — Foundation (v0.0.1) ✅

Workspace scaffolding, acyclic crate DAG, `bevy_ecs` + `wgpu` baseline, stable window, fixed-tick scheduler.
**Crates**: `common`, `config`, `events`, `scheduler`, `app`

## Phase 1 — Decentralized Physics & Debug Renderer (v0.1) 🔧

ECS `world` active. Graph-based soft-body physics (nodes + spring-constraints). GPU flat-buffer allocation. Instanced-quad rendering via `bytemuck`.
**Crates**: `world`, `spatial`, `physics`, `rendering`

### Phase 1.5 — Topological Plasticity (v0.15) 💭

Runtime-mutable graph topology — budding new nodes (growth) and severing edges (injury/decay) instead of a fixed spawn-time graph. Sets up continuous growth for Phase 3.
**Crates**: `physics`, `world`

## Phase 2 — Fields & Environment (v0.2) 📋

`diffusion` fields (oxygen, pheromones) via WGSL compute shaders on a low-res grid, upsampled at render time. Async CPU readback explicitly deferred.
**Crates**: `diffusion`, `environment`, `gpu`

### Phase 2.5 — Closed-Loop Fields (v0.25) 💭

Double-buffered staging-belt readback so the CPU can sample gradients without stalling the GPU. Adds diurnal/seasonal modulation of diffusion constants.
**Crates**: `diffusion`, `gpu`

## Phase 3 — Hox Genetics & Passive Ecology (v0.3) 📋

Procedural growth via Hox sequence (Zygote → Head/Torso/Muscle/Tail) with mechanical differentiation. Passive collision-eating. Asexual cloning only. Hard population caps to prevent OOM.
**Crates**: `organisms`, `genetics`, `reproduction`, `metabolism`, `ecology`

### Phase 3.5 — Sexual Recombination & Drift (v0.35) 💭

Crossover between parent Hox sequences plus segment duplication/deletion events. Spatially isolated populations drift genetically — visible proto-speciation before Phase 6's formal clustering exists.
**Crates**: `genetics`, `reproduction`, `ecology`

## Phase 4 — Actuation, Cognition & Behavior (v0.4) 📋

Raycasting `sensing` with delayed async GPU readback. `brain` forward passes via `burn`. Compute-shader muscle actuation (sine-wave rest-length modulation). Transition from passive eating to active foraging.
**Crates**: `sensing`, `brain`, `behavior`, `actuation`, `evolution`

### Phase 4.5 — Learned Gaits (v0.45) 💭

Brain output drives phase/amplitude/frequency of muscle actuation directly — gait becomes a heritable trait instead of a hardcoded sine wave.
**Crates**: `brain`, `actuation`, `evolution`

## Phase 5 — UI & Analytics (v0.5) 📋

Full `egui` integration: decentralized entity-graph inspection, population analytics, Hox sequence viewer, compute-shader profiling.
**Crates**: `ui`, `analytics`

## Phase 6 — Persistence & Speciation Tools (v0.6) 📋

Serialize/deserialize via `bincode`/`ron`. SQLite run-tracking. Functional replay system. Formal speciation tracking (Levenshtein clustering on Hox sequences). God-mode interventions.
**Crates**: `storage`, `research`, `plugins`

## Phase 7 — Procedural Visuals & Trails (v0.7) 📋

Instanced quads upgraded to SDF-based shaders parameterized by Hox genetics + segment type. MRT handles decay and pheromone trail rendering.
**Crates**: `rendering`, `shaders`

## Phase 8 — Application Shell (v0.8) 📋

Persistent `egui` TopBottomPanel navigation. Full separation of sim config from UI state. Non-blocking async progress overlays. Global keyboard shortcuts.
**Crates**: `ui`, `app`

## Phase 9 — Headless MARL & Network (v1.0) 📋

Fully headless simulation mode for MARL experience generation. Efficient state-as-arrays extraction. WebSocket remote-control API. Multi-client observation of a single headless server.
**Crates**: `network`, `marl_interface`

## Phase 10 — Emergent Signaling (v1.1) 💭

Pheromone *emission pattern* (not just concentration) becomes heritable — pulse trains, frequency, duration. `analytics` tracks mutual information between signal and nearby behavior across generations, surfacing genuine emergent alarm-calls/mate-signals.
**Crates**: `diffusion`, `genetics`, `analytics`

## Phase 11 — Catastrophe Engine (v1.2) 💭

Scripted/randomized global events — droughts, floods, localized extinction pulses — routed through `events` and exposed in the Phase 6 god-mode panel. Functions as an evolutionary-robustness stress test.
**Crates**: `events`, `ecology`, `plugins`

## Phase 12 — Spectator & Lineage Narration (v1.3) 💭

Read-only web viewer over the Phase 9 WebSocket API. LLM pass over the Phase 6 SQLite lineage DB periodically narrates notable evolutionary events (splits, extinctions, trait shifts).
**Crates**: `network`, `storage`, `narrator` *(new — thin Anthropic API wrapper)*

---

## Future Scope (Unscheduled, Beyond v1.3) 💭

These aren't sequenced yet — they're directions worth keeping on the radar as the core sim matures.

- **3D Environment & Volumetric Physics** — move from the 2D instanced-quad plane to a true volumetric world: octree spatial partitioning, 3D spring-graphs, depth-aware diffusion fields.
- **Multi-User / Collaborative Worlds** — multiple humans god-moding the same persistent world simultaneously, with a shared intervention log so interventions don't silently collide.
- **Procedural Terrain & Biomes** — heightmaps, water bodies, and biome-specific diffusion/metabolism modifiers so evolution has *geography* to push against, not just a flat plane.
- **Modding API & Plugin Marketplace** — expose `plugins` (from Phase 6) as a stable public API for custom organism behaviors, shaders, or god-mode events authored by the community.
- **Cloud-Hosted Persistent Worlds** — always-on servers (Minecraft-realm style) where the ecosystem keeps evolving between sessions, with the Phase 9 network layer as the entry point.
- **AR/VR Spectator Mode** — a volumetric viewer for the Phase 12 web client, letting you walk through the ecosystem at table-top or room scale rather than watching it on a flat screen — a natural extension once Phase 12's spectator layer exists.
- **Mobile Companion App** — lightweight read-only client for checking on a running world (population graphs, notable lineage events) without needing the full desktop app open.

---

*This map folds the original Phase 0–9 architecture doc together with the speculative 1.5/2.5/3.5/4.5 sub-phases and the 10–12 post-1.0 arcs. Treat the `.5` phases and Phase 10+ as a backlog to formalize, not a committed schedule.*
