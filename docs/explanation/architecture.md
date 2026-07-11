# Architecture & Concurrency

Phylon's architecture is built around three core principles: **high-performance data-oriented design**, **strict boundary encapsulation**, and **as much determinism as is actually verified** (see [Determinism](determinism.md) for the precise, honest scope of that guarantee).

## The Tick Loop

Phylon decouples simulation logic entirely from wall-clock rendering. The simulation advances in discrete `u64` ticks, driven by a hand-written per-tick function (`app::simulation::update_simulation`) that calls every simulation system directly, in a fixed order, once per tick — not by a generic scheduler abstraction.

A `scheduler` crate does exist in the workspace (a `bevy_ecs`-based scheduling abstraction, with its own tests and a benchmark), but **it is not used by the running application** — it was removed from the live app's tick-driving path early in the project's history and is kept only as a benchmark fixture and integration-test target. Don't describe the app as "using a scheduler"; describe it as a fixed-order tick function.

- Operations that require time integration (physics, metabolic burn) use a fixed `dt` rather than a wall-clock delta.
- Structural ECS changes (spawning/despawning) are deferred via `bevy_ecs::system::Commands` during the parallel phase, to avoid lock contention across `rayon` worker threads.

## The Entity-Component-System (ECS)

Phylon's ECS layer is `bevy_ecs` (specifically `bevy_ecs::world::World`, used directly — not the rest of the Bevy engine, and not `hecs`; the workspace has never depended on `hecs`).

- **Entities**: organisms (each a small graph of particle nodes — head plus body segments), mineral pellets, food pellets, corpses.
- **Components**: flat, contiguous arrays of data (`ParticleNode`, `SensoryState`, `Brain`, `Metabolism`, …).
- **Systems**: isolated logic blocks iterating specific component signatures, advancing state by one tick.

This gives high CPU cache coherency and lets heavy per-organism work (sensing, reproduction, metabolism) scale linearly across cores via `rayon`.

## The Crate Graph

Phylon is divided into 30 independent Rust crates forming a strict Directed Acyclic Graph — see [Crate Dependency Graph](../reference/crate_graph.md) for the full, level-by-level breakdown. At a glance:

- **Core simulation primitives**: `genetics` (CPPNs, regulatory-network body-plan decode), `brain` (CTRNN), `metabolism`, `sensing`, `ecology`, `physics`, `diffusion`.
- **3D engine**: `spatial` (uniform grid + `Octree`), `gpu` (compute-only — physics integration and chemical diffusion, zero rendering), `rendering` (mesh-based organism rendering, GPU-driven field/clip-plane overlays), `ui` (the `Camera3d`-driven viewport and every egui panel).
- **Application**: `app` — the composition root, the only crate permitted to depend on everything.

## 3D Simulation Space

Organism and physics state live in 3D (`common::Vec3` positions, a body-fixed `forward`/`dorsal` orientation frame replacing an earlier 2D scalar heading). Chemical diffusion fields remain deliberately 2D (a bounded set of world-space planes, not a volumetric texture) — this was a measured tradeoff, not an oversight: a volumetric diffusion field would cost roughly two orders of magnitude more GPU memory/bandwidth for a benefit that hasn't been demonstrated as necessary. See [Camera & Viewport](camera_and_viewport.md) for the 3D camera and rendering pipeline this space is viewed and interacted through.

## Concurrency Model

1. **CPU simulation phase**: `rayon` parallelizes per-organism work (sensing, behavior evaluation prep, metabolism); structural ECS mutations are deferred via `Commands`.
2. **GPU synchronization phase**: physics (particle/spring integration, spatial-hash broad-phase collision) and chemical diffusion are dispatched as WGSL compute passes on `wgpu` (Vulkan/Metal/DX12 backend, selected by the platform).
3. **Rendering phase**: the application shell reads updated state and issues instanced `wgpu` draw calls (mesh-based capsule rendering with a physically-based shading model) without holding locks on simulation state.
