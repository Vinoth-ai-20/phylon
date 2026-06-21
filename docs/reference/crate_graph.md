# Crate Dependency Graph

Phylon is aggressively divided into dozens of independent crates. This structure enforces architectural boundaries and maximizes incremental compilation speeds. The dependency graph forms a strict Directed Acyclic Graph (DAG).

## Core Dependency Stack

The stack is organized from bottom to top. Crates higher up the stack depend on the crates below them. Crates at the same level generally do not depend on each other.

### Level 0: Foundation

- `common`: Foundational math vectors, global Entity IDs, and shared utilities.
- `config`: Compile-time and run-time constants (Grid sizes, tick rates).

### Level 1: Data & State

- `storage`: Bincode serialization and file I/O for saving/loading lineages.
- `events`: The lock-free global event bus.

### Level 2: Core Simulation Primitives

- `physics`: Spatial partitioning and Symplectic Euler integration.
- `diffusion`: Grid-based discrete Laplacian diffusion.
- `genetics`: Hox sequencing, CPPN mutation engine.
- `brain`: CTRNN execution math.
- `ecology`: Diets, Food Pellets, and Corpses.
- `metabolism`: Energy tracking and starvation logic.

### Level 3: Organism Systems

- `sensing`: Translates physics and ecology into flat float vectors for the brain.
- `behavior`: Reads brain outputs and actuates physics springs.
- `reproduction`: Handles crossover, mutation, and genome validation.

### Level 4: Orchestration

- `gpu`: Manages the `wgpu` state, initializing compute pipelines for physics and diffusion.
- `organisms`: The top-level ECS manager, tying together components from Levels 2 and 3 into coherent Entities.
- `world`: The `hecs` World wrapper and global resource registry.
- `scheduler`: Manages execution order of all ECS systems.

### Level 5: Application Shell

- `ui`: `egui` panels for the Inspector, Lineage Graph, and Simulation Controls.
- `rendering`: Maps ECS states to `wgpu` instanced geometry.
- `app`: The main application loop linking `scheduler`, `gpu`, and `ui` together.

---

> [!TIP]
> If you are adding a new core feature (e.g., a new sensor type), you should implement the logic in the appropriate low-level crate (e.g., `crates/sensing`) and only expose the necessary public interface to the higher-level crates (e.g., `crates/organisms` and `crates/app`).
