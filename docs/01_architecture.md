# Phylon Architecture

Phylon is a research-grade Artificial Life Laboratory built entirely in Rust. It utilizes a three-layer boundary system to maintain a clean architecture, separating the core simulation from the application loop and output systems.

## Three-Layer Boundary

1. **Simulation Core (`world`, `physics`, `diffusion`, `organisms`, `ecology`, etc.)**
   The core simulation is strictly deterministic, running on the CPU using `rayon` for parallelism and utilizing the GPU via `wgpu` strictly as a compute accelerator for massive data-parallel tasks (like diffusion and neural inference). The simulation layer has no knowledge of how it is rendered or stored.

2. **Application Orchestration (`app`, `config`, `scheduler`, `events`)**
   The application layer manages the execution of the simulation. It drives the main event loop, handles the deterministic fixed-timestep updates, loads configurations, and acts as the central hub for the event bus.

3. **Output & I/O (`rendering`, `ui`, `storage`, `network`, `analytics`)**
   The output layer acts on the state of the simulation. Rendering visualizes the state asynchronously or via interpolation. The UI provides real-time telemetry overlays and debugging tools. Storage persists snapshots. Network enables remote observation and control. Analytics and telemetry (via `puffin`) process and record simulation performance and demographic data.

## Organism Representation & Memory Layout

Instead of monolithic structs, organisms in Phylon are represented as **ECS-Backed Dynamic Graphs**. 
- **Nodes** are individual `bevy_ecs` entities (acting as cells or particles) possessing mass, energy, and biological components.
- **Edges** are relational components linking nodes, acting as physical structures (particle-spring constraints for soft-body physics) or biological channels (energy/signal transfer).
This allows organisms to be modular, destructible, and highly evolvable.

To maintain GPU and CPU cache efficiency, `bevy_ecs` archetypes provide contiguous flat-buffer allocation. When entities are deleted, the ECS natively compacts memory, solving the severe fragmentation issues common in dynamic graph simulations without requiring custom garbage collectors.

## Data Flow: Tick to Rendered Frame

1. **Tick Advancement**: The `scheduler` initiates a tick based on the fixed timestep.
2. **CPU Simulation Phases**: Simulation systems (`physics`, `behavior`, `ecology`) run via `bevy_ecs` schedules. The standalone `bevy_ecs` executor handles data-parallelism over the `world` state automatically based on component access. State changes and significant actions trigger events on the `events` bus.
3. **GPU Compute Dispatch**: For fields (`diffusion`) and neural networks (`brain`), the CPU dispatches batched operations to the GPU.
4. **GPU Readback (Pipelined)**: To avoid stalling the `bevy_ecs` scheduler, readbacks (e.g., organism neural outputs or field gradients) are pipelined. The CPU uses the previous tick's readback if possible, or double-buffered staging with fixed precision policies to inform the next phase of CPU logic without blocking.
5. **State Finalization**: The tick concludes, and the canonical CPU state is finalized in the `bevy_ecs` World.
6. **Frame Presentation**: Between ticks or aligned with them, the `app` requests a redraw. `rendering` reads the latest canonical world state (and interpolated positions for smooth visuals), dispatches rendering pipelines, and `ui` draws the interface over it.

## Interfaces & Crate Communications

- **Traits & Types**: Domain models use newtype patterns and generic traits.
- **Channels**: Communication between the compute-heavy simulation (`rayon`) and asynchronous I/O (`tokio`) occurs exclusively via lock-free channels (`crossbeam`).
- **Events**: Cross-domain simulation actions (e.g., an organism dying, reproducing, or a field spiking) are published to the `events` bus. The `scheduler` consumes these during the `PostTick` phase to mutate the ECS, and persists them in the `world` state so decoupled systems like `analytics` or UI layers can react without destructive reads.

## License

This document is dual-licensed under the MIT License and the Apache License, Version 2.0.
