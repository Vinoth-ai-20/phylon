# Phylon

Phylon is a research-grade, high-performance artificial life laboratory built in Rust.

![CI](https://github.com/Vinoth-ai-20/phylon/actions/workflows/ci.yml/badge.svg)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust 1.80+](https://img.shields.io/badge/rust-1.80+-lightgray.svg)](https://www.rust-lang.org)

Phylon simulates massive populations of neural-driven organisms within a continuous, deterministic physics environment. It leverages a data-oriented Entity-Component-System (ECS) architecture tightly coupled with GPU compute shaders to solve chemical diffusion and rigid-body mechanics at scale. The system enforces bit-exact reproducibility across platforms by adhering to fixed-timestep updates and explicitly avoiding floating-point non-determinism in critical simulation paths.

## Architecture

The simulation state is strictly partitioned between a CPU-authoritative logic layer and a GPU-accelerated compute layer. The core logic runs on a lock-free, multithreaded ECS driven by `hecs` and `rayon`, coordinating neural network inferences and behavioral systems across a bounded 2D or Toroidal topology. The physics layer implements a Symplectic Euler integrator, while environmental chemical diffusion is processed via discrete Laplacian operators dispatched to the GPU as WGSL compute passes. The workspace is divided into 29 decoupled crates forming a strict directed acyclic graph, ensuring rapid compilation and enforced boundary encapsulation between rendering, simulation, and data analytics.

## Performance Targets

The engine is engineered to maintain a strict 60 Hz tick rate under the following load parameters:

| Metric | Target |
|--------|--------|
| Active Organisms | 100,000 |
| Spatial Chunk Resolution | 256x256 units |
| Max Active Chunks | 512 |
| Neural Network Topology | 3-layer, dynamic synapses |
| Chemical Diffusion Fields | 3 independent channels (Food, Toxin, Pheromone) |

## Getting Started

To build the core workspace and application binaries locally:

```bash
cargo build --release
```

To run the deterministic test suite:

```bash
cargo test
```

## Running the Simulation

```bash
cargo run --release --bin phylon
```

## Current Status

Phase 8 is complete. The workspace features SQLite database integration via `storage` to persist massive simulation telemetry asynchronously, a headless command-line orchestrator (`phylon-research`), and a live-reloading God-Mode API embedded with `rhai` via the `plugins` crate. The biological features include Continuous-Time Recurrent Neural Networks (CTRNN) with Hebbian learning, heritable sexual/asexual/facultative reproduction, multi-sector vision, speciation mechanics, and communicable disease spread.

Phase 7 introduced procedurally generated SDF visuals and Multiple Render Targets (MRT) for movement trails, replacing simple colored squares with biologically plausible forms. Phase 8 overhauled the `egui` interface into a complete application shell, implementing a persistent menu bar, non-blocking asynchronous progress overlays, centralized UI state, and global keyboard shortcuts. Phase 9 (Network & Multiplayer) is slated next.

## Documentation

Comprehensive architectural design documents, crate dependency graphs, and technical specifications are available in the [`docs/`](docs/) directory.

## Contributing

We accept pull requests that align with the core architectural constraints. Please ensure that all new crates maintain the strict acyclic dependency rules and that simulation changes respect the GPU determinism policy. Refer to [CONTRIBUTING.md](CONTRIBUTING.md) for build instructions, branch naming conventions, and validation requirements.

## License

MIT License
