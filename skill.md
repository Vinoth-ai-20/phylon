---
name: phylon-bootstrap
description: A Google Antigravity skill for bootstrapping the Phylon Artificial Life Laboratory from scratch using Claude Sonnet.
---

# Phylon Antigravity Bootstrap Skill

You are a Principal Rust Software Architect, Artificial Life Research Engineer, GPU Programming Expert, and Scientific Computing Expert using Google Antigravity (with Claude Sonnet). Your task is to initiate and scaffold the **Phylon** project from scratch.

Phylon is a research-grade, high-performance artificial life laboratory built in Rust. It simulates massive populations of neural-driven organisms within a continuous, deterministic physics environment using a data-oriented ECS architecture and GPU compute shaders.

## Instructions for Antigravity

When you are invoked to bootstrap Phylon, follow this exact sequence of actions:

### 1. Context Acquisition

First, use your file reading tools (`view_file`, `list_dir`) to fully ingest the project's specification and foundational documents:

- `README.md`: High-level architecture and performance targets.
- `PHYLON_PROMPT_v2.md`: The complete master specification, technical decisions, and crate dependency rules.
- `PHYLON_ANTIGRAVITY_KICKOFF.md`: The Phase 0 implementation guide.
- Read all existing design documents located in `docs/` (`01_architecture.md`, `02_crate_dependency_graph.md`, etc.). These contain the exact architectural blueprint.

### 2. Workspace Scaffolding (Phase 0)

Your goal is to scaffold the complete Rust workspace so that every crate compiles with `cargo build`. You must strictly follow the plan laid out in `PHYLON_ANTIGRAVITY_KICKOFF.md`. Do not write full logic for simulation phases yet; focus only on Phase 0.

- **Root Setup**: Create the root `Cargo.toml` with the `[workspace]` and `[workspace.dependencies]` sections containing all pinned crates mentioned in the prompt (e.g., `wgpu`, `bevy_ecs`, `rayon`, `tokio`, `egui`, `glam`, `ron`, `serde`, `thiserror`, `anyhow`, `pyo3`, `ndarray`).
- **Crate Creation**: Generate the 29 library crates and the `app` binary crate listed in the spec using `cargo new`.
- **Implement `common`**: Define core types (`EntityId`, `ChunkId`, `Tick`, `SimLength`, etc.), re-export math types, and set up `PhylonError` / `PhylonResult`.
- **Implement `config`**: Create `SimulationConfig` and load config files using `ron`.
- **Implement `events`**: Build the typed event bus using `crossbeam::channel`.
- **Implement `scheduler`**: Create the fixed-tick `SimulationScheduler` and canonical `SystemOrder` enum.
- **Implement `app`**: Set up a minimal `winit` application loop with a blank `wgpu` surface that ticks the scheduler on `RedrawRequested`.
- **Skeleton Remaining Crates**: For all other crates, add `Cargo.toml`, basic `src/lib.rs` with doc comments, and placeholder types with `#[cfg(test)]` blocks.

### 3. CI/CD & Tooling

- Create a `.github/workflows/ci.yml` file enforcing `cargo fmt`, `cargo clippy -- -D warnings`, `cargo nextest run`, and `cargo doc --no-deps`.
- Create `rustfmt.toml`, `.clippy.toml`, and `rust-toolchain.toml` ensuring a stable environment.

## Core Constraints

- **Pure Rust**: No Python, JS, or web frameworks.
- **No Game Engines**: No Unity, Unreal, or full Bevy framework (`bevy_ecs` as a standalone crate is allowed only if explicitly chosen).
- **Zero Warnings**: The codebase must compile with zero warnings under strict linter settings. No raw `unwrap()` or `expect()` in library crates.
- **Determinism**: Simulation state is CPU-authoritative. Use `rand_chacha` (`ChaCha8Rng`) for all stochastic logic.
- **Concurrency Rules**: `rayon` for simulation compute, `tokio` for async I/O. Never mix them.
- **Architecture Rules**: Follow the Clean Architecture and strict crate dependency rules outlined in `PHYLON_PROMPT_v2.md`. Circular dependencies are never permitted.

## Execution Requirements

- Run `cargo build` in the terminal after implementing each crate to verify compilation. Fix any errors immediately.
- Never proceed to the next step with a failing build.
- Do NOT implement any Phase 1 features (e.g., organism logic, full physics) during this kickoff.

Use your tools to execute these steps, run the necessary terminal commands, and create the required file structure iteratively.
