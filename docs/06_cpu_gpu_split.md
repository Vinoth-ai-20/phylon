# CPU/GPU Split Strategy

The guiding principle of Phylon is: **CPU is authoritative; GPU is an accelerator.**
The GPU maintains zero canonical state; it only holds transient data during tick calculations.

## Subsystem Split

| Subsystem | Execution Layer | Justification |
|-----------|-----------------|---------------|
| Organism State & Genetics | CPU | Core logic, mutations, and branching logic are highly heterogeneous and require strict seeded RNG determinism. Not suited for SIMT execution. |
| Broad-phase Physics | CPU | Uniform grids/quadtrees are traversed easily on CPU using `rayon` with excellent cache locality. |
| Narrow-phase Physics | CPU | Resolving constraints (springs, collisions) between arbitrary pairs is complex to synchronize lock-free on GPU but trivial via CPU domain-decomposition. |
| Ecology & Reproduction | CPU | Heavy branching, object creation, memory allocation. Strict sequential guarantees required for event generation. |
| Field Diffusion | GPU | Massively data-parallel uniform grid operations. Perfectly suited for WGSL compute pipelines. |
| Neural Network Inference | GPU | Batch forward passes over large matrices (via `burn`). Classic GPU acceleration workload. |
| Sensory Raycasting & Sampling | GPU | Thousands of agents casting visual cones or sampling 2D grid fields is inherently parallel and reads from GPU-resident field textures anyway. |
| Rendering | GPU | Native visual pipeline via `wgpu`. |

## Data Transfer Strategy

**Uploads (CPU -> GPU):**

- Organism positions and properties are batched into dense SoA (Structure of Arrays) buffers and uploaded once per tick to the staging buffers for `Sensing` and `Rendering`.
- Field source emissions (e.g., organism exhaling CO2) are sparse-uploaded to modify the field state buffers before the `Diffusion` dispatch.

**Readbacks (GPU -> CPU):**

- After `Sensing` and `Brain` inference, output buffers containing action intents (floats) are mapped back to the CPU via `wgpu::Buffer::map_async`. The scheduler yields the current thread (or blocks synchronously, given the fixed tick architecture) until the readback completes.
- Field gradients at organism locations are read back sparsely.
- Massive field data is *never* read back entirely unless requested by a save state or heavy analytics snapshot. The CPU only reads what it needs to execute the next behavioral logic step.
