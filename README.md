# Phylon

Phylon is a research-grade, high-performance artificial life laboratory built in Rust. It specializes in simulating continuous-time morphological and neural evolution driven by metabolic constraints.

![CI](https://github.com/Vinoth-ai-20/phylon/actions/workflows/ci.yml/badge.svg)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust 1.80+](https://img.shields.io/badge/rust-1.80+-lightgray.svg)](https://www.rust-lang.org)

Phylon simulates massive populations of neural-driven organisms within a continuous, deterministic physics environment. It leverages a data-oriented Entity-Component-System (ECS) architecture tightly coupled with GPU compute shaders to solve chemical diffusion and rigid-body mechanics at scale. The system enforces **bit-exact reproducibility** across platforms by adhering to custom fixed-timestep updates and explicitly avoiding floating-point non-determinism in critical simulation paths.

![Placeholder: Day/Night Cycle transition from #080616 to #1A1953 with organisms hunting]

## Architecture & Technology Stack

The simulation state is strictly partitioned between a CPU-authoritative logic layer and a GPU-accelerated compute layer.

- **Language**: Rust
- **Concurrency**: Lock-free parallel processing via `rayon`.
- **ECS**: A highly optimized Entity-Component-System implementation heavily influenced by `hecs` and `bevy_ecs`.
- **Rendering & Compute**: Headless, cross-platform shader execution using `wgpu` (Vulkan/Metal/DX12).
- **Workspace**: Divided into a strict 30-crate Directed Acyclic Graph (DAG) ensuring rapid compilation and boundary encapsulation between rendering, simulation, and data analytics.

## Ecology & Simulation

Phylon treats artificial organisms not just as neural networks, but as physical, metabolizing entities existing in a closed-loop chemical economy.

- **Chemical Economy**: All entities trade Glucose, O2, CO2, and ATP. Producers undergo photosynthesis tied directly to the global daylight cycle, while predators rely on predation.
- **Day/Night Cycles**: A deterministic, shifted cosine wave dictates sunlight intensity, creating harsh twilight phases that cull inefficient metabolisms.
- **L-System Morphology**: Bodies (heads, muscles, tails, fins) grow fractally according to `HoxSequence` genetics.

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

## Running the Simulation

```bash
cargo run -p app --release
```

## Documentation

Comprehensive documentation, organized using the Diátaxis framework (Tutorials, How-to guides, Explanation, Reference), is available in the [`docs/`](docs/index.md) directory.

- **[Tutorials](docs/tutorials/getting_started.md)**: Learning-oriented walk-throughs.
- **[How-To Guides](docs/how_to/add_custom_genomes.md)**: Problem-oriented, step-by-step tasks.
- **[Explanation](docs/explanation/architecture.md)**: Deep dives into the scientific and architectural models.
- **[Reference](docs/reference/components.md)**: Technical lookups.

For an exhaustive API reference, run:

```bash
cargo doc --open
```

## Contributing

We accept pull requests that align with the core architectural constraints. Please ensure that all new crates maintain the strict acyclic dependency rules and that simulation changes respect the GPU determinism policy. Refer to [CONTRIBUTING.md](CONTRIBUTING.md) for build instructions, branch naming conventions, and validation requirements.

## License

This project is dual-licensed under either the [MIT License](LICENSE-MIT) or the [Apache License, Version 2.0](LICENSE-APACHE), at your option.
