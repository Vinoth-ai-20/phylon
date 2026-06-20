# Crate Dependency Graph

Phylon is structured into highly modular crates with strict dependency rules to prevent cycles and enforce architectural boundaries.

## Dependency Rules

- `common` has zero internal dependencies.
- Circular dependencies are strictly forbidden.
- Simulation crates never depend on output/I/O crates (`rendering`, `ui`, `storage`).
- `app` is the composition root and depends on everything.

## Graph Summary

| Crate | Direct Dependencies | Purpose |
|-------|---------------------|---------|
| `common` | `glam`, `thiserror`, `serde` | Shared types, IDs, math, basic errors. |
| `events` | `common`, `crossbeam` | Typed event bus system. |
| `config` | `common`, `ron`, `serde`, `config` | Configuration loading. |
| `spatial` | `common`, `glam` | Spatial indexing (grid, quadtree). |
| `world` | `common`, `events`, `spatial` | Central ECS state, entity registry. |
| `physics` | `common`, `world`, `spatial` | Forces, collision, rigid body. |
| `diffusion` | `common`, `world`, `spatial`, `gpu` | PDE field diffusion. |
| `brain` | `common`, `burn` | Neural structures, networks. |
| `sensing` | `common`, `world`, `spatial`, `brain` | Vision, olfaction, hearing. |
| `metabolism` | `common`, `world` | Energy, age, respiration. |
| `behavior` | `common`, `world`, `brain`, `sensing` | Movement, action selection. |
| `genetics` | `common`, `rand` | Genome, mutations. |
| `reproduction` | `common`, `world`, `genetics`, `events` | Birth, replication. |
| `evolution` | `common`, `genetics`, `world` | Selection, lineage tracking. |
| `organisms` | `common`, `world`, `genetics`, `brain`, `metabolism`, `sensing` | Organism archetype definitions. |
| `ecology` | `common`, `world`, `organisms`, `events` | Food web, predation, disease. |
| `environment` | `common`, `world`, `spatial`, `diffusion` | Terrain, climate, biomes. |
| `scheduler` | `common`, `events`, `world`, `physics`, `diffusion`, `ecology`, `behavior`, `metabolism`, `sensing` | Deterministic fixed-tick executor. |
| `gpu` | `common`, `wgpu` | Compute device management. |
| `rendering` | `common`, `world`, `gpu`, `wgpu` | Visual output pipeline. |
| `ui` | `common`, `events`, `rendering`, `egui`, `egui-wgpu` | Inspector and charts. |
| `analytics` | `common`, `events`, `world`, `sqlx` | Metrics, diversity, graphs. |
| `storage` | `common`, `world`, `serde`, `bincode`, `sqlx` | Save/load, snapshotting, SQLite. |
| `research` | `common`, `config`, `scheduler`, `analytics`, `storage` | Experiment automation. |
| `network` | `common`, `tokio`, `tokio-tungstenite`, `serde_json` | Remote WebSocket control. |
| `plugins` | `common`, `rhai`, `world` | Embedded scripting and scenarios. |
| `app` | `winit`, `tracing`, `puffin`, + **ALL OF THE ABOVE** | Main loop, window, composition root. |

## ASCII Dependency Tree (Simplified)

```
app
├── winit, tracing
├── config ───────> common
├── scheduler ────> world, events, physics, diffusion, ecology, behavior
├── ui ───────────> rendering, events, common
├── rendering ────> gpu, world, common
├── storage ──────> world, common
├── network ──────> tokio, common
├── research ─────> scheduler, analytics, storage
└── world
    ├── events ───> common
    ├── spatial ──> common
    └── common
```

*Note: Edges flow downwards to dependencies. Cycles are impossible.*

## License

This document is dual-licensed under the MIT License and the Apache License, Version 2.0.
