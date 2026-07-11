# Crate Dependency Graph

Phylon is aggressively divided into dozens of independent crates. This structure enforces architectural boundaries and maximizes incremental compilation speeds. The dependency graph forms a strict Directed Acyclic Graph (DAG).

**Correction:** an earlier revision of this document listed only 20 of the workspace's 30 crates and claimed `world` wraps `hecs`. Both corrected below, verified directly against `Cargo.toml`'s `members` list (30 entries) and `world`'s own doc comment (`bevy_ecs::world::World`, not `hecs` — this workspace has never depended on `hecs`).

## Core Dependency Stack

The stack is organized from bottom to top. Crates higher up the stack depend on the crates below them. Crates at the same level generally do not depend on each other.

### Level 0: Foundation

- `common`: Foundational math vectors, global Entity IDs, and shared utilities.
- `config`: Compile-time and run-time constants (Grid sizes, tick rates).

### Level 1: Data & State

- `storage`: Bincode serialization and file I/O for saving/loading lineages.
- `events`: The lock-free global event bus.
- `spatial`: Spatial indexing structures (uniform grid, `Octree`) for efficient entity neighborhood queries — shared by `ecology`'s foraging broad-phase and `sensing`.

### Level 2: Core Simulation Primitives

- `physics`: Spatial partitioning and Symplectic Euler integration.
- `diffusion`: Grid-based discrete Laplacian diffusion.
- `genetics`: CPPN mutation engine and the regulatory-network body-plan decode (positional Hox-code, not a stored sequence — see [Genetics & Neurobiology](../explanation/genetics_and_neurobiology.md)).
- `brain`: CTRNN execution math.
- `ecology`: Diets, Food Pellets, and Corpses.
- `metabolism`: Energy tracking and starvation logic.
- `environment`: Biome classification and procedural terrain/resource-fertility generation.

### Level 3: Organism Systems

- `sensing`: Translates physics and ecology into flat float vectors for the brain.
- `behavior`: Reads brain outputs and actuates physics springs.
- `reproduction`: Handles crossover, mutation, and genome validation.
- `evolution`: Selection pressure, speciation, lineage tracking, fitness metrics, and hybridization barriers — emergent, not an explicit fitness function.
- `learning`: Reinforcement-learning interface contracts (observation/action spaces, policy API) exposed to external RL trainers — deliberately independent of any specific ML framework.

### Level 4: Orchestration

- `gpu`: Manages the `wgpu` state, initializing compute pipelines for physics and diffusion.
- `organisms`: The top-level ECS manager, tying together components from Levels 2 and 3 into coherent Entities.
- `world`: A thin wrapper around `bevy_ecs::world::World` and the global resource registry. **Not `hecs`** — this workspace has always used `bevy_ecs`.
- `scheduler`: **Not used by the live app** (Phase 6, Epic A removed it as the app's driver; `app::simulation::update_simulation` drives every real tick directly). Retained deliberately as a benchmark fixture (`benchmarks`' `scheduler_throughput`) and integration-test target (`tests`' `scheduler_integrates_with_event_bus`) — see the crate's own module doc comment (Phase 7, W1a) for the full decision record.
- `analytics`: Metrics collection, population history, diversity indices, spatial heatmaps, lineage tracking, and research report generation — a pure consumer of the event bus, never mutates simulation state.
- `research`: Experiment manifests, batch-run configuration, and report generation/comparison for research workflows.
- `network`: `tokio-tungstenite` WebSocket server for remote simulation control and multi-user collaboration sessions.
- `plugins`: Embedded `rhai` scripting engine for scenario authoring and scripted interventions — deliberately depends on nothing but `rhai`/`common`/`thiserror`, no simulation-domain crates.

### Level 5: Application Shell

- `ui`: `egui` panels for the Inspector, Lineage Graph, and Simulation Controls.
- `rendering`: Maps ECS states to `wgpu` instanced geometry.
- `app`: The main application loop — the workspace's composition root, the only crate permitted to depend on everything (see `app/src/main.rs`'s own doc comment).

### Not part of the runtime stack

- `tests`: Cross-crate integration tests, built and run by `cargo test`, not linked into any runtime binary.
- `benchmarks`: `criterion` benchmark harness for Phylon subsystems, run via `cargo bench`, not linked into any runtime binary.

---

> [!TIP]
> If you are adding a new core feature (e.g., a new sensor type), you should implement the logic in the appropriate low-level crate (e.g., `crates/sensing`) and only expose the necessary public interface to the higher-level crates (e.g., `crates/organisms` and `crates/app`).
