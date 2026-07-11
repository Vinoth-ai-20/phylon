# Simulation Model

Phylon simulates an ecosystem using strict mathematical models for physics, chemical diffusion, and environmental cycles.

## The Day/Night Cycle (Global Environment)

The simulation is bound to a deterministic, cyclical Day/Night cycle driven directly by the simulation tick count. This acts as the pacing mechanism for all producer metabolisms and drives the visual rendering interpolation.

The sunlight intensity $I_{sun}$ is calculated using a shifted cosine wave, ensuring the simulation starts at High Noon ($1.0$) on tick $0$:

$$ I_{sun}(t) = \frac{\cos(t \cdot f) + 1.0}{2.0} $$

Where:

- $t$ is the current simulation tick.
- $f$ is the diurnal frequency scalar (e.g., configuring a full day to exactly 60 seconds of real-time at 60 Hz).

Visually, the `app` rendering layer directly interpolates the `wgpu` ClearColor of the background based on $I_{sun}(t)$, smoothly shifting the environment from deep navy blue to dark twilight.

## Symplectic Euler Physics (3D)

The rigid-body mechanics of organisms are governed by a Symplectic Euler integrator (`crates/physics`), operating in 3D (`common::Vec3` positions/velocities).

- Each organism is constructed of **Particle Nodes** (mass points, positioned in 3D) connected by **Springs**.
- A spring's `constraint_type` is `Rigid` (structural bone), `Passive` (inert), or `Elastic` — only `Elastic` springs actuate, and only a body position that decoded as a `Muscle` segment (see [Genetics & Neurobiology](genetics_and_neurobiology.md)) ever becomes one.
- Bilateral fin/limb pairs are placed using a body-fixed `forward`/`dorsal` orientation frame (`bilateral_fin_direction = dorsal × forward`), replacing an earlier 2D-only perpendicular-vector trick.
- The physics engine runs entirely on the GPU (`crates/gpu/src/physics_pipeline.rs`), using a spatial-hash broad-phase (not a dense grid — a dense 3D grid at the same resolution as the original 2D grid would cost roughly 128× the memory) for organism-vs-organism steric collision. A CPU implementation of the same integrator exists and is used for unit tests, headless CI, and deterministic-behavior validation — the GPU path is the one the live app always uses.

## Chemical Diffusion

The environment contains five continuous chemical layers, diffused as 2D world-space planes (deliberately not volumetric — see [Architecture](architecture.md) for why): **Pheromones**, **Energy**, **O2**, **CO2**, and **Morphogen** (the inter-organism developmental-signaling layer — see [Genetics & Neurobiology](genetics_and_neurobiology.md)).

- Diffusion is solved discretely across a uniform spatial grid via WGSL compute shaders.
- A discrete Laplacian operator spreads values between adjacent cells each tick, mathematically simulating the physical spread of a substance through a fluid medium.
- Evaporation occurs at a constant decay rate to prevent the grid values from saturating to infinity over long periods.
- A second, independently-instanced diffusion field drives environmental hazards.

## Data & Analytics

The `analytics` module continuously monitors the ecosystem's health by tracking populations in a ring buffer.

- Population histories are recorded for 8 distinct ecological roles (`Producers`, `Herbivores`, `Carnivores`, `Omnivores`, `Decomposers`, `Food`, `Minerals`, `Corpses`).
- The user interface visualizes these demographic shifts over time via interactive graphs, enabling detailed observation of population crashes, predator-prey cycles, and carrying capacity limits.
