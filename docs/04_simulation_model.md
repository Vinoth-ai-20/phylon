# Simulation Model

The Phylon simulation progresses through a fixed-step deterministic tick execution model. The `scheduler` manages the sequential update of various subsystems, ensuring consistent reproducibility.

## Single Tick End-to-End Sequence

1. **PrePhysics**: Preparatory updates (e.g., clearing stale spatial structures, registering newly born entities from the last tick into the world).
2. **Physics**: CPU broad-phase spatial bucketing. `rayon` threads process narrow-phase rigid body forces, constraints, and collisions. `world` positions are mutated.
3. **Diffusion**: CPU orchestrates a GPU dispatch. Field grids (nutrients, gases) are updated via compute shaders (WGSL). State remains on GPU; a low-res summary is read back if global ecology logic requires it.
4. **Sensing**: A GPU batch dispatch gathers spatial fields/entities around each organism into sensory inputs. Readback gives each entity its sense data.
5. **Brain**: Neural network forward passes. CPU dispatches `burn` batched inference utilizing GPU tensors. Output actions are read back to the CPU.
6. **Behavior**: `rayon` parallel iterators process brain outputs. Intentions (move, attack, eat, mate) are transformed into desired velocities or metabolic state changes in `world`.
7. **Metabolism**: Entities consume energy, increase age, process respiration. Exhaustion/starvation deaths are published to `events`.
8. **Ecology**: Environmental interactions. Plants gain energy from sunlight field. Fungi distribute nutrients.
9. **Reproduction**: Entities satisfying reproduction criteria trigger birth mechanics (crossover, mutation in `genetics`). Parent entities mutate, new child events are published.
10. **PostTick**: Cleanup phase. Dead entities are removed from `world` and spatial indices.
11. **Analytics**: Observers process the `events` bus (e.g., graphing populations).

## Event Dispatch

Events are not polled; they are published eagerly into the `events` bus via `crossbeam` channels during phase execution (e.g., `DeathEvent` published in Metabolism). At designated synchronization points or in isolated phases (Analytics), the queues are drained and processed.

## GPU Boundaries

- **Dispatches**: Occur strictly within the `Diffusion`, `Sensing`, and `Brain` phases.
- **Readbacks**: Must complete prior to the phase requiring their output. `wgpu` maps staging buffers back to the CPU so that deterministic CPU algorithms (Behavior, Reproduction) operate on concrete numbers. Precision rules ensure cross-platform reproducibility by enforcing standard IEEE-754 floats or fixed-point fallbacks from GPU memory before CPU branching decisions.
