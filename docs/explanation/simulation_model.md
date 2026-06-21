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

## Symplectic Euler Physics

The rigid-body mechanics of organisms are governed by a Symplectic Euler integrator (`crates/physics`).

- Each organism is constructed of **Particle Nodes** (mass points) connected by **Springs**.
- Muscles are represented by actuated springs whose `base_length` is dynamically altered by the organism's neural output.
- The physics engine runs entirely on the GPU (`crates/gpu/src/physics_pipeline.rs`) to ensure determinism and handle tens of thousands of constraints simultaneously.

## Chemical Diffusion

The environment contains continuous chemical layers (Food, Pheromones, Hazards).

- Diffusion is solved discretely across a uniform spatial grid via WGSL compute shaders.
- A discrete Laplacian operator spreads values between adjacent cells each tick, mathematically simulating the physical spread of scent molecules in a fluid medium.
- Evaporation occurs at a constant decay rate to prevent the grid values from saturating to infinity over long periods.

## Data & Analytics

The `analytics` module continuously monitors the ecosystem's health by tracking populations in a ring buffer.

- Population histories are recorded for 8 distinct ecological roles (`Producers`, `Herbivores`, `Carnivores`, `Omnivores`, `Decomposers`, `Food`, `Minerals`, `Corpses`).
- The user interface visualizes these demographic shifts over time via interactive graphs, enabling detailed observation of population crashes, predator-prey cycles, and carrying capacity limits.
