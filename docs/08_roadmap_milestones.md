# Roadmap & Milestones

This document defines the core implementation phases, milestones, and acceptance criteria for Phylon.

## Phase 0: Foundation (v0.0.1)
*Current Phase*
- **Acceptance Criteria**: Workspace scaffolding, crate DAG established without cycles. Core configurations (`config`, `common`), fixed-tick execution (`scheduler`), and basic `app` loop open a stable window. Zero warnings.
- **Key Crates**: `common`, `config`, `events`, `scheduler`, `app`.

## Phase 1: World Core & Debug Renderer (v0.1)
- **Acceptance Criteria**: ECS `world` active. `physics` implemented with Symplectic Euler. `rendering` can draw a minimal colored dot representation of entities.
- **Key Crates**: `world`, `spatial`, `physics`, `rendering`.

## Phase 2: Fields & Environment (v0.2)
- **Acceptance Criteria**: `diffusion` fields active via WGSL compute shaders. `environment` biomes and chunk terrain loaded and visible.
- **Key Crates**: `diffusion`, `environment`, `gpu`.

## Phase 3: Biology & Ecology (v0.3)
- **Acceptance Criteria**: Organism life-cycle (`metabolism`, `reproduction`, `genetics`). Basic food chain logic (`ecology`). Death and birth occur.
- **Key Crates**: `organisms`, `genetics`, `reproduction`, `metabolism`, `ecology`.

## Phase 4: Cognition & Behavior (v0.4)
- **Acceptance Criteria**: `sensing` raycasting working. `brain` forward passes operational via `burn`. Entities react to sensory input to make decisions (`behavior`).
- **Key Crates**: `sensing`, `brain`, `behavior`, `evolution`.

## Phase 5: UI & Analytics (v0.5)
- **Acceptance Criteria**: `egui` fully integrated for inspecting entities, analyzing populations, and profiling performance.
- **Key Crates**: `ui`, `analytics`.

## Phase 6: Persistence & Research Tools (v0.6)
- **Acceptance Criteria**: Complete serialize/deserialize via `bincode`. SQLite DB tracks runs. Replay system functional. God-mode interventions available.
- **Key Crates**: `storage`, `research`, `plugins`.

## Phase 7: Network & Multiplayer (v1.0)
- **Acceptance Criteria**: WebSocket remote control API active. Multiple clients can observe a headless simulation server.
- **Key Crates**: `network`.
