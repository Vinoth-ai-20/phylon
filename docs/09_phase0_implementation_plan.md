# Phase 0 Implementation Plan

This is the exact ordered list of files to be created during Phase 0 scaffolding. Check off each item as completed.

- [x] `docs/01_architecture.md`: Architecture rules and data flow.
- [x] `docs/02_crate_dependency_graph.md`: Crates DAG and boundaries.
- [x] `docs/03_directory_tree.md`: Complete directory tree.
- [x] `docs/04_simulation_model.md`: Execution model for a single tick.
- [x] `docs/05_physics_and_diffusion.md`: Physics integrators and fields.
- [x] `docs/06_cpu_gpu_split.md`: Division of labor between CPU and GPU.
- [x] `docs/07_gpu_determinism_policy.md`: Reproducibility rules.
- [x] `docs/08_roadmap_milestones.md`: Phased milestone definitions.
- [x] `docs/09_phase0_implementation_plan.md`: This file.

## Workspace Roots
- [ ] `Cargo.toml`: Workspace manifest defining 29 crates and shared dependencies.
- [ ] `.github/workflows/ci.yml`: CI pipeline definition.
- [ ] `rustfmt.toml`: Formatting rules.
- [ ] `.clippy.toml`: Linter rules.
- [ ] `rust-toolchain.toml`: Toolchain pin.

## Core Crates Implementation
- [ ] `crates/common/Cargo.toml` and `src/lib.rs`: Math, unique IDs, simulation units, error types.
- [ ] `crates/config/Cargo.toml` and `src/lib.rs`: Config loader.
- [ ] `data/default.ron`: Default configuration file.
- [ ] `crates/events/Cargo.toml` and `src/lib.rs`: Typed event bus and variants.
- [ ] `crates/scheduler/Cargo.toml` and `src/lib.rs`: Fixed-tick simulator and system orders.
- [ ] `crates/app/Cargo.toml` and `src/main.rs`: winit + wgpu surface initialization loop.

## Skeleton Crates Setup
- [ ] Create 24 empty skeleton crates in `crates/` (world, spatial, physics, diffusion, organisms, genetics, evolution, reproduction, behavior, metabolism, sensing, brain, learning, environment, ecology, gpu, rendering, ui, analytics, storage, research, network, plugins, tests, benchmarks).
- [ ] Each skeleton crate must have `Cargo.toml` pointing to workspace dependencies.
- [ ] Each skeleton crate must have `src/lib.rs` with documentation and a placeholder unit test.
