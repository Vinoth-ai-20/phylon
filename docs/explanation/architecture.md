# Architecture & Concurrency

Phylon's architecture is designed around three core principles: **High-Performance Data-Oriented Design**, **Strict Boundary Encapsulation**, and **Bit-Exact Reproducibility**.

## The Custom Deterministic Tick Scheduler

Many Rust game engines (like Bevy) struggle with cross-platform reproducibility because of floating-point drift over standard real-time clocks (e.g., `bevy::time::Time`).

To bypass this, Phylon entirely decouples the simulation logic from the visual rendering pipeline. Instead of relying on real-time deltas, Phylon uses a **Custom Deterministic Tick Scheduler**.

- The simulation advances in discrete `u64` ticks, completely independent of the frame rate.
- Operations that require time integration (like physics and metabolic burn) use fixed constants rather than wall-clock `dt`.
- This guarantees that tick $N$ on a Windows machine will have the exact same mathematical state as tick $N$ on a headless Linux cluster.

## The Entity-Component-System (ECS)

At the heart of the CPU logic is a lock-free, multithreaded ECS leveraging custom integration pathways heavily inspired by `hecs` and `bevy_ecs`.

- **Entities**: Organisms, Mineral Pellets, Food Pellets, and Corpses.
- **Components**: Flat, contiguous arrays of data (e.g., `ParticleNode`, `SensoryState`, `Brain`, `Metabolism`).
- **Systems**: Isolated logic blocks that iterate over specific Component signatures, advancing the state by one discrete tick.

This architecture ensures high CPU cache coherency and allows heavy processing tasks (like the `sensing_system` and `reproduction_system`) to scale linearly across CPU cores.

## The Crate Graph

Phylon is divided into 30 independent Rust crates forming a strict **Directed Acyclic Graph (DAG)**. This prevents circular dependencies, drastically improves incremental compilation times, and enforces strong domain boundaries.

- **Core Logic**: `genetics`, `behavior`, `metabolism`, `sensing`, `ecology`
- **Engine**: `physics`, `diffusion`, `gpu`
- **Application**: `app` (composition root), `ui`, `rendering`

## Concurrency Model

1. **CPU Simulation Phase**: `rayon` parallelizes organism behaviors. During this phase, structural changes to the ECS (like spawning or despawning entities) are deferred using `bevy_ecs::system::Commands` to prevent lock contention across threads.
2. **GPU Synchronization Phase**: All biological data is serialized into flat buffer arrays and dispatched to the GPU (Vulkan/Metal/DX12) as WGSL compute passes. The GPU solves the heavy matrix math for the physics integration and Laplacian chemical diffusion.
3. **Rendering Phase**: The application shell reads the updated states and pushes instanced `wgpu` meshes to the screen without holding locks on the core simulation state.
