# Simulation Model

The Phylon simulation progresses through a fixed-step deterministic tick execution model. The `scheduler` manages the sequential update of various subsystems, ensuring consistent reproducibility.

## Single Tick End-to-End Sequence

1. **PrePhysics**: Preparatory updates (e.g., clearing stale spatial structures, registering newly born entities from the last tick into the world).
2. **Physics**: CPU broad-phase spatial bucketing. `rayon` threads process narrow-phase rigid body forces, constraints, and collisions. `world` positions are mutated.
3. **Diffusion**: CPU orchestrates a GPU dispatch. Field grids (nutrients, gases) are updated via compute shaders (WGSL). State remains on GPU; a low-res summary is read back if global ecology logic requires it.
4. **Sensing**: Gathers spatial fields/entities around each organism into sensory inputs. Readback gives each entity its sense data, including 8-vector inputs like `food_distance`, `food_angle`, and local `[Oxygen, Carbon, Scent, Temp]` field diffusion layers.
5. **Brain**: Neural network forward passes. CPU dispatches batched inference utilizing `ndarray`. Output actions are read back into the `Intention` component.
6. **Behavior**: `rayon` parallel iterators process brain outputs. Intentions (move, attack, eat, mate) are transformed into desired velocities or metabolic state changes in `world`.
7. **Metabolism**: Entities consume energy, increase age, process respiration. Basal metabolic cost scales superlinearly (`mass^1.2`) to constrain unrestrained gigantism. Exhaustion/starvation deaths are published to `events`.
8. **Ecology**: Environmental interactions. A CPU-side spawner periodically distributes `FoodPellet` entities. Organisms forage based on their `Diet`: Herbivores consume `FoodPellet`s, Carnivores actively hunt smaller organisms utilizing the $O(1)$ `spatial_index`, and Scavengers consume pellets while absorbing ambient carbon from diffusion grids.
9. **Reproduction**: Entities satisfying reproduction criteria trigger birth mechanics (crossover, mutation in `genetics`). Parent entities mutate, new child events are published.
10. **PostTick**: Cleanup phase. Dead entities are removed from `world` and spatial indices.
11. **Analytics**: Observers process the `events` bus (e.g., graphing populations).

## Event Dispatch

Events are not polled; they are published eagerly into the `events` bus via `crossbeam` channels during phase execution (e.g., `DeathEvent` published in Metabolism). At designated synchronization points or in isolated phases (Analytics), the queues are drained and processed.

## GPU Boundaries

- **Dispatches**: Occur strictly within the `Diffusion`, `Sensing`, and `Brain` phases.
- **Readbacks**: Must complete prior to the phase requiring their output. `wgpu` maps staging buffers back to the CPU so that deterministic CPU algorithms operate on concrete numbers. Note: for Phase 3 foraging, we explicitly avoid staging buffer readbacks to prevent frame stalls, favoring CPU-side `FoodPellet` entities for immediate ecological pressure. Where readbacks are still used (e.g., sensing), precision rules ensure cross-platform reproducibility.
