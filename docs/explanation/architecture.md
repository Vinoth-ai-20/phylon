# Architecture & Concurrency

Phylon's architecture is designed around two core principles: **High-Performance Data-Oriented Design** and **Strict Boundary Encapsulation**.

## The Entity-Component-System (ECS)

At the heart of the CPU logic is a multithreaded ECS driven by hecs and ayon.
- **Entities**: Organisms, Mineral Pellets, Food Pellets, and Corpses.
- **Components**: Flat, contiguous arrays of data (e.g., ParticleNode, SensoryState, Brain, Metabolism).
- **Systems**: Isolated functions that iterate over specific Component signatures, advancing the logic by one DT (Delta Time) tick.

This architecture ensures high cache coherency and allows systems like sensing_system and eproduction_system to scale linearly across CPU cores.

## The Crate Graph

Phylon is divided into 30 independent Rust crates forming a strict Directed Acyclic Graph (DAG). This prevents circular dependencies and significantly improves incremental compilation times. 
- **Core Logic**: genetics, ehavior, metabolism, sensing
- **Engine**: physics, diffusion, gpu
- **Application**: pp, ui, endering

## Concurrency Model

1. **CPU Simulation Phase**: Rayon parallelizes organism behaviors. During this phase, structural changes to the ECS (like spawning or despawning entities) are deferred using evy_ecs::system::Commands to prevent lock contention.
2. **GPU Synchronization Phase**: All biological data is serialized into flat buffer arrays and dispatched to Vulkan compute shaders for heavy matrix math (Physics, Diffusion, CTRNN evaluation).
3. **Rendering Phase**: The application shell reads the updated states and pushes instanced meshes to the screen.
