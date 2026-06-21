# Phylon

Phylon is a research-grade, high-performance artificial life laboratory built in Rust.

![CI](https://github.com/Vinoth-ai-20/phylon/actions/workflows/ci.yml/badge.svg)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust 1.80+](https://img.shields.io/badge/rust-1.80+-lightgray.svg)](https://www.rust-lang.org)

Phylon simulates massive populations of neural-driven organisms within a continuous, deterministic physics environment. It leverages a data-oriented Entity-Component-System (ECS) architecture tightly coupled with GPU compute shaders to solve chemical diffusion and rigid-body mechanics at scale. The system enforces bit-exact reproducibility across platforms by adhering to fixed-timestep updates and explicitly avoiding floating-point non-determinism in critical simulation paths.

## Architecture

The simulation state is strictly partitioned between a CPU-authoritative logic layer and a GPU-accelerated compute layer. The core logic runs on a lock-free, multithreaded ECS driven by `hecs` and `rayon`, coordinating neural network inferences and behavioral systems across a bounded 2D or Toroidal topology. The physics layer implements a Symplectic Euler integrator, while environmental chemical diffusion is processed via discrete Laplacian operators dispatched to the GPU as WGSL compute passes. The workspace is divided into 30 decoupled crates forming a strict directed acyclic graph, ensuring rapid compilation and enforced boundary encapsulation between rendering, simulation, and data analytics.

## Performance Targets

The engine is engineered to maintain a strict 60 Hz tick rate under the following load parameters:

| Metric | Target |
| -------- | -------- |
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

To run pre-commit checks:

```bash
cargo fmt;
cargo fmt --all -- --check;
cargo clippy --all-targets --all-features -- -D warnings;
cargo test --workspace;
cargo doc --no-deps --document-private-items;
cargo build --all-targets;
```

## Running the Simulation

```bash
cargo run --release --bin phylon
```

## Running the simulation in release mode

```bash
cargo run -p app --release
```

## Running the simulation in debug mode

```bash
cargo run -p app
```

## Current Status

Phases 0 through 11 are functionally complete. The workspace features the foundational skeletal infrastructure across 30 decoupled crates, and complete implementations for decentralized soft-body physics, chemical diffusion fields, CPPN/HyperNEAT morphology, sexual recombination, CTRNN brains with learned gaits, a comprehensive UI & analytics suite, speciation persistence tools, procedural visuals, the application shell, headless MARL networking, emergent signaling, and the catastrophe engine. The simulation is now prepared for speculative phases like Spectator & Lineage Narration.

## Documentation

Comprehensive documentation, organized using the Diátaxis framework (Tutorials, How-to guides, Explanation, Reference), is available in the [`docs/`](docs/index.md) directory.

For an exhaustive API reference, run:

```bash
cargo doc --open
```

## Contributing

We accept pull requests that align with the core architectural constraints. Please ensure that all new crates maintain the strict acyclic dependency rules and that simulation changes respect the GPU determinism policy. Refer to [CONTRIBUTING.md](CONTRIBUTING.md) for build instructions, branch naming conventions, and validation requirements.

## License

This project is dual-licensed under either the [MIT License](LICENSE-MIT) or the [Apache License, Version 2.0](LICENSE-APACHE), at your option.
