# Simulation Model

The Phylon simulation progresses through a fixed-step deterministic tick execution model. The `scheduler` manages the sequential update of various subsystems, ensuring consistent reproducibility.

## Single Tick End-to-End Sequence

1. **PrePhysics**: Preparatory updates (e.g., clearing stale spatial structures, registering newly born entities from the last tick into the world).
2. **Physics**: CPU broad-phase spatial bucketing. `rayon` threads process narrow-phase rigid body forces, constraints, and collisions. `world` positions are mutated.
3. **Diffusion**: CPU orchestrates a GPU dispatch. Field grids (nutrients, gases) are updated via compute shaders (WGSL). State remains on GPU; a low-res summary is read back if global ecology logic requires it.
4. **Sensing**: Gathers spatial fields/entities around each organism using a sector-based (Left/Center/Right) vision cone based on evolvable `vision_cone_angle` and `vision_depth`. Readback gives each entity its sense data including neighboring entities and nearest food positions.
5. **Brain**: Continuous-Time Recurrent Neural Networks (CTRNN) forward passes. The network integrates sensory input over time using internal potentials and features Hebbian plasticity where synaptic weights update dynamically during the organism's lifetime, clamped by evolvable limits. Output actions are read back into the `Intention` component.
6. **Behavior**: `bevy_ecs` parallel schedules process brain outputs. Intentions (move, attack, eat, mate) are transformed into desired velocities or metabolic state changes in `world`.
7. **Metabolism**: Entities consume energy, increase age, process respiration. Basal metabolic cost scales superlinearly (`mass^1.2`) to constrain unrestrained gigantism. Exhaustion/starvation deaths are published to `events`.
8. **Ecology**: Environmental interactions including disease transmission via proximity spread (`process_disease`). A CPU-side spawner periodically distributes `FoodPellet` entities. Organisms forage based on their `Diet`.
9. **Reproduction**: Entities satisfying reproduction and energy criteria trigger birth mechanics. Reproduction modes are evolvable: Asexual, Sexual (with crossover), or Facultative (switching dynamically based on population density). The system also evaluates genetic distance to assign distinct `SpeciesId` for speciation tracking, with full lineage persistence written to the SQLite database.
10. **PostTick**: Cleanup phase. Dead entities are removed from `world` and spatial indices.
11. **Analytics**: Observers process the `events` bus (e.g., graphing populations).

## Event Dispatch

Events are not polled; they are published eagerly into the `events` bus via `crossbeam` channels during phase execution (e.g., `DeathEvent` published in Metabolism). At the end of the tick (in the `PostTick` phase), the queue is drained by the `scheduler` to manage entity lifecycles. These events are then persisted in the `world`'s `last_events` buffer so that decoupled systems (like `analytics` or UI components) can observe and process the tick's events non-destructively.

## GPU Boundaries

- **Dispatches**: Occur strictly within the `Diffusion`, `Sensing`, and `Brain` phases.
- **Readbacks (Pipelined)**: We employ asynchronous pipelined readbacks to avoid `bevy_ecs` thread stalls. The CPU logic for the *current* tick may operate on the readback mapped from the *previous* tick for sensing/brain outputs. `wgpu` maps staging buffers back to the CPU so that deterministic CPU algorithms operate on concrete numbers without blocking the main execution graph. Where blocking readbacks are strictly required, precision rules ensure cross-platform reproducibility.
