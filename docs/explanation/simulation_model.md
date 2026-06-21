# Simulation Model

Phylon simulates an ecosystem using strict mathematical models for physics, chemical diffusion, and metabolism.

## Symplectic Euler Physics

The rigid-body mechanics of organisms are governed by a Symplectic Euler integrator (crates/physics).
- Each organism is constructed of **Particle Nodes** (mass points) connected by **Springs**.
- Muscles are represented by actuated springs whose ase_length is dynamically altered by the organism's neural output.
- The physics engine runs entirely on the GPU (crates/gpu/src/physics_pipeline.rs) to ensure determinism and handle tens of thousands of constraints simultaneously.

## Chemical Diffusion

The environment contains chemical layers (Food, Pheromone, Hazard).
- Diffusion is solved discretely across a uniform spatial grid.
- We apply a discrete Laplacian operator to spread values between adjacent cells each tick, simulating the physical spread of scent molecules in a fluid medium.
- Evaporation occurs at a constant decay rate to prevent the environment from saturating.

## Metabolism & Ecology

Organisms possess a Metabolism component defining their ase_rate of energy consumption.
- Energy is strictly conserved. An organism must consume entities matching its Diet (e.g., Herbivore eats FoodPellet) to gain Energy.
- If Energy drops to zero, the organism dies and is converted into a Corpse.
- Decomposer organisms eat Corpse entities, closing the ecological loop.
