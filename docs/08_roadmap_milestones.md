# Roadmap & Milestones

This document defines the core implementation phases, milestones, and acceptance criteria for Phylon.

## Phase 0: Foundation (v0.0.1)

- **Status**: Complete.
- **Acceptance Criteria**: Workspace scaffolding, crate DAG established without cycles. Core configurations (`config`, `common`), fixed-tick execution (`scheduler`), and basic `app` loop open a stable window. Zero warnings.
- **Key Crates**: `common`, `config`, `events`, `scheduler`, `app`.

## Phase 1: World Core & Debug Renderer (v0.1)

- **Status**: Complete.
- **Acceptance Criteria**: ECS `world` active. `physics` implemented with Symplectic Euler. `rendering` can draw a minimal colored dot representation of entities.
- **Key Crates**: `world`, `spatial`, `physics`, `rendering`.

## Phase 2: Fields & Environment (v0.2)

- **Status**: Complete.
- **Acceptance Criteria**: `diffusion` fields active via WGSL compute shaders. `environment` biomes and chunk terrain loaded and visible.
- **Key Crates**: `diffusion`, `environment`, `gpu`.

## Phase 3: Biology & Ecology (v0.3)

- **Status**: Complete.
- **Acceptance Criteria**: Organism life-cycle (`metabolism`, `reproduction`, `genetics`). Basic food chain logic (`ecology`). Death and birth occur.
- **Key Crates**: `organisms`, `genetics`, `reproduction`, `metabolism`, `ecology`.

## Phase 4: Cognition & Behavior (v0.4)

- **Status**: Complete.

- **Acceptance Criteria**: `sensing` raycasting working. `brain` forward passes operational via `burn`. Entities react to sensory input to make decisions (`behavior`).
- **Key Crates**: `sensing`, `brain`, `behavior`, `evolution`.

## Phase 5: UI & Analytics (v0.5)

- **Status**: Complete.
- **Acceptance Criteria**: `egui` fully integrated for inspecting entities, analyzing populations, and profiling performance.
- **Key Crates**: `ui`, `analytics`.

## Phase 6: Persistence & Research Tools (v0.6)

- **Status**: Complete.

- **Acceptance Criteria**: Complete serialize/deserialize via `bincode` (or `ron`). SQLite DB tracks runs. Replay system functional. God-mode interventions available.
- **Key Crates**: `storage`, `research`, `plugins`.

## Phase 7: Procedural Visuals & Trails (v0.7)

- **Status**: Complete.
- **Acceptance Criteria**: Entities are rendered with procedurally generated SDF-based visuals parameterized by genetics. MRT rendering handles decay trails. Food is rendered visually via a distinct pass.
- **Key Crates**: `rendering`, `shaders`.

## Phase 8: Application Shell (v0.8)

*Current Phase*

- **Status**: Complete.
- **Acceptance Criteria**: Persistent `egui` TopBottomPanel acts as the primary user navigation. Full state management separates simulation configuration from UI state. Modals and non-blocking asynchronous progress overlays work. Keyboard shortcuts control global behaviors like pausing and full-screen.
- **Key Crates**: `ui`, `app`.

## Phase 9: Network & Multiplayer (v1.0)

- **Acceptance Criteria**: WebSocket remote control API active. Multiple clients can observe a headless simulation server.
- **Key Crates**: `network`.
