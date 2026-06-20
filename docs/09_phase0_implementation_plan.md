# Phase 0 Implementation Plan

This document is the authoritative checklist for Phase 0. Each item corresponds
to a file created or a command run. Completed items are checked off.

---

## Pre-Code Documents

- [x] `docs/01_architecture.md` — Three-layer boundary, data flow, interface patterns
- [x] `docs/02_crate_dependency_graph.md` — Directed acyclic dependency graph
- [x] `docs/03_directory_tree.md` — Complete workspace directory blueprint
- [x] `docs/04_simulation_model.md` — Tick execution model and system ordering
- [x] `docs/05_physics_and_diffusion.md` — Integrator and PDE formulation
- [x] `docs/06_cpu_gpu_split.md` — CPU vs GPU responsibility table
- [x] `docs/07_gpu_determinism_policy.md` — Reproducibility guarantees
- [x] `docs/08_roadmap_milestones.md` — Versioned milestone definitions
- [x] `docs/09_phase0_implementation_plan.md` — This document

---

## Tooling and CI

- [x] `rust-toolchain.toml` — Pin stable toolchain with rustfmt + clippy components
- [x] `rustfmt.toml` — 100-char width, crate-grouped imports, Unix newlines
- [x] `.clippy.toml` — cognitive-complexity-threshold = 30
- [x] `.github/workflows/ci.yml` — fmt check, clippy -D warnings, build, test, doc

---

## Root Workspace

- [x] `Cargo.toml` — workspace manifest, `[workspace.dependencies]` with all pinned
      crates, `resolver = "2"`, release and dev profiles

---

## Crate Creation (30 crates total)

### `cargo new --lib` (29 library crates)

- [x] `crates/common`
- [x] `crates/config`
- [x] `crates/events`
- [x] `crates/scheduler`
- [x] `crates/world`
- [x] `crates/spatial`
- [x] `crates/physics`
- [x] `crates/diffusion`
- [x] `crates/organisms`
- [x] `crates/genetics`
- [x] `crates/evolution`
- [x] `crates/reproduction`
- [x] `crates/behavior`
- [x] `crates/metabolism`
- [x] `crates/sensing`
- [x] `crates/brain`
- [x] `crates/learning`
- [x] `crates/environment`
- [x] `crates/ecology`
- [x] `crates/gpu`
- [x] `crates/rendering`
- [x] `crates/ui`
- [x] `crates/analytics`
- [x] `crates/storage`
- [x] `crates/research`
- [x] `crates/network`
- [x] `crates/plugins`
- [x] `crates/tests`
- [x] `crates/benchmarks`

### `cargo new --bin` (1 binary crate)

- [x] `crates/app`

---

## Foundation Crate Implementations

### `crates/common` — Fully implemented

- [x] `EntityId(u64)` — globally unique, `NULL` sentinel, `Display`
- [x] `ChunkId(i32, i32)` — chunk grid coordinates, `as_ivec2`, `Display`
- [x] `Tick(u64)` — ordered, `ZERO`, `next()`, `elapsed_since()`, `Display`
- [x] `SimLength`, `SimMass`, `SimEnergy`, `SimTime` — newtype wrappers
- [x] `pub use glam::{Vec2, IVec2}` — math re-exports
- [x] `PhylonError` trait — `Error + Send + Sync + 'static`
- [x] `PhylonResult<T>` — `Result<T, Box<dyn PhylonError>>`
- [x] Unit tests for all types
- [x] `cargo build -p common` → ✅

### `crates/config` — Fully implemented

- [x] `PhysicsIntegrator` enum — `SymplecticEuler`, `VelocityVerlet`
- [x] `SimulationConfig` — all fields per spec, `validate()`
- [x] `RenderConfig` — window size, vsync, overlay opacity
- [x] `ResearchConfig` — experiment ID, autosave, headless, max_ticks
- [x] `PhylonConfig` — root struct, `load(Option<&Path>)`, `tick_duration()`
- [x] `ConfigError` — typed thiserror enum implementing `PhylonError`
- [x] `data/default.ron` — valid RON file matching all struct defaults
- [x] Unit tests for validation and load
- [x] `cargo build -p config` → ✅

### `crates/events` — Fully implemented

- [x] `DeathCause` enum — 8 variants
- [x] `FieldType` enum — 9 field types
- [x] `PhylonEvent` enum — 5 variants with `tick()` accessor
- [x] `EventBus` — bounded crossbeam channel, `publish()`, `drain()`
- [x] `EventBusError` — `ChannelFull` variant
- [x] Unit tests for publish, drain, full channel, pending count
- [x] `cargo build -p events` → ✅

### `crates/scheduler` — Fully implemented

- [x] `SystemOrder` enum — 11 phases in canonical order, `all_ordered()`
- [x] `SimulationScheduler` — accumulator-based fixed timestep
- [x] `step()` — deterministic single-tick advance for headless/test
- [x] `advance(max_ticks)` — wall-clock-paced frame method with lag warning
- [x] `register(order, system_fn)` — sorted insertion for determinism
- [x] `TickStats` — per-tick timing breakdown
- [x] `tracing` spans for every phase
- [x] Unit tests for ordering, registration, execution, error propagation
- [x] `cargo build -p scheduler` → ✅

---

## Skeleton Crates (all with Cargo.toml + lib.rs + test)

- [x] `crates/spatial` — `UniformGrid`, `SpatialError`
- [x] `crates/world` — `World`, `WorldError`
- [x] `crates/physics` — `ParticleNode`, `PhysicsError`
- [x] `crates/diffusion` — `FieldKind`, `DiffusionSystem`
- [x] `crates/genetics` — `GenomeId`, `Ploidy`, `Genome`
- [x] `crates/organisms` — `DietType`, `SpatialComponents`, `BiologicalComponents`
- [x] `crates/evolution` — `LineageId`, `SpeciesId`, `LineageRecord`
- [x] `crates/reproduction` — `ReproductionStrategy`, `PendingBirth`
- [x] `crates/behavior` — `MotorAction`
- [x] `crates/metabolism` — `BASE_METABOLIC_COST`, `RespirationMode`, `MetabolismSystem`
- [x] `crates/sensing` — `SensorModality` (10 modalities)
- [x] `crates/brain` — `BrainId`, `ActivationFn`, `Brain`
- [x] `crates/learning` — `PolicyProvider` trait, `ObservationVector`, `ActionVector`
- [x] `crates/environment` — `Biome`, `ClimateZone`
- [x] `crates/ecology` — `InteractionKind`, `EcologyInteraction`
- [x] `crates/gpu` — `GpuContext`, `GpuError`
- [x] `crates/rendering` — `Renderer`, `RenderError`
- [x] `crates/ui` — `UiContext`, `UiError`
- [x] `crates/analytics` — `PopulationSample`, `AnalyticsAccumulator`
- [x] `crates/storage` — `SchemaVersion`, `StorageError`, `StorageManager`
- [x] `crates/research` — `ExperimentManifest`
- [x] `crates/network` — `NetworkServer`, `NetworkError`
- [x] `crates/plugins` — `PluginEngine`, `PluginError`
- [x] `crates/tests` — integration tests (scheduler + event bus)
- [x] `crates/benchmarks` — criterion scheduler throughput benchmark

---

## Application Binary

### `crates/app/src/main.rs` — Implemented

- [x] CLI entry point with `anyhow::Result`
- [x] `tracing_subscriber` init with `RUST_LOG` env-filter
- [x] `PhylonConfig::load("data/default.ron")` with fallback
- [x] `winit 0.30` `ApplicationHandler` implementation
- [x] `wgpu` surface initialisation (adapter → device → queue → surface config)
- [x] `RedrawRequested` → `scheduler.advance()` → clear frame → `present()`
- [x] `CloseRequested` → `event_loop.exit()`
- [x] Resize handler with surface reconfiguration
- [x] `ControlFlow::Poll` for continuous simulation loop

---

## Data and Assets

- [x] `data/default.ron` — valid RON config matching all struct defaults
- [x] `shaders/diffusion/` — directory created (WGSL files: Phase 3)
- [x] `shaders/rendering/` — directory created (WGSL files: Phase 1)
- [x] `shaders/sensing/` — directory created (WGSL files: Phase 4)
- [x] `shaders/neural/` — directory created (WGSL files: Phase 6)
- [x] `assets/` — directory created (populated in Phase 7)
- [x] `examples/` — directory created (populated as phases progress)

---

## Build Verification

- [x] `cargo check --all` → Exit 0, all 30 crates check cleanly
- [x] `cargo build` → Full workspace builds successfully
- [ ] `cargo test --all` → All tests pass (running)
- [ ] `cargo clippy --all-lib -- -D warnings` → Zero warnings (running)
- [ ] `cargo fmt --check` — formatting verified
- [ ] `cargo doc --no-deps` — docs compile cleanly

---

## Phase 0 Acceptance Criteria Status

| Criterion | Status |
|-----------|--------|
| All 9 `docs/` documents exist and are complete | ✅ |
| `cargo build` succeeds with zero errors across full workspace | ✅ |
| `cargo clippy -- -D warnings` produces zero warnings | 🔄 Running |
| `cargo test` passes all tests | 🔄 Running |
| `cargo doc --no-deps` compiles without errors | 📋 Pending |
| The `app` binary opens a stable window | ✅ Code complete |
| `data/default.ron` is a valid, loadable config file | ✅ |
| `docs/09_phase0_implementation_plan.md` has every item checked off | 🔄 In progress |

## License

This document is dual-licensed under the MIT License and the Apache License, Version 2.0.
