# Phylon

A research-grade Artificial Life Laboratory written entirely in Rust.

Phylon is a long-term scientific simulation platform for studying emergent
complexity, evolutionary dynamics, and ecosystem behavior. It is not a game
or a demo — it is a programmable sandbox for computational biology, physics,
and multi-agent learning research.

## Status

> 🚧 Phase 0 — Foundation (active development)

The workspace is being scaffolded. Nothing runs yet.

## Vision

- 100,000+ organisms simulated in real time
- GPU-accelerated field diffusion (gases, heat, pheromones, nutrients)
- Evolution via NEAT with full genome and lineage tracking
- Research-grade reproducibility: seeded RNG, snapshot save/load, experiment replay
- Professional inspector UI for genomes, neural brains, and ecosystem analytics

Inspired by Lenia, The Bibites, Avida, Tierra, and EvoLife.

## Tech Stack

- **Language:** Rust (pure, no Python, no JS)
- **Rendering:** wgpu + winit
- **GUI:** egui
- **ECS:** hecs
- **ML/Neural:** burn (NEAT, CTRNN, Hebbian)
- **Persistence:** SQLite via sqlx
- **Scripting:** Rhai
- **Parallelism:** rayon (compute) + tokio (I/O)

## Getting Started

```bash
git clone https://github.com/<your-username>/phylon.git
cd phylon
cargo build
cargo run
```

> Requires Rust stable. Install via [rustup.rs](https://rustup.rs).

## Documentation

Full architecture, design decisions, and phase roadmap live in `docs/` (generated during Phase 0).

The project specification is in `PHYLON_PROMPT.md`.

## License

MIT
