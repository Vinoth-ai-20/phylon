# Phylon

Phylon is a research-grade artificial life laboratory: a 3D, GPU-accelerated simulator where populations of neural-driven, physically-simulated organisms grow, forage, reproduce, and evolve under real metabolic and selective pressure — not scripted, not hand-tuned to "look alive."

![CI](https://github.com/Vinoth-ai-20/phylon/actions/workflows/ci.yml/badge.svg)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust 1.80+](https://img.shields.io/badge/rust-1.80+-lightgray.svg)](https://www.rust-lang.org)

![Phylon simulation viewport](screenshots/phylon_20260709_150900_486.png)

## What this is

An organism in Phylon is not an animation — it's a graph of physically-simulated particle nodes, actuated by a continuous-time recurrent neural network (CTRNN), whose entire body plan and brain wiring are decoded from an evolvable genome each generation. Nothing about an organism's shape, color, or behavior is hand-authored; all of it emerges from selection acting on a genotype-to-phenotype pipeline modeled loosely on real developmental biology (Hox-style combinatorial body-plan codes, a chemical-economy metabolism, disease and injury, life stages).

The project's standing engineering rule is **measure, don't assume** — when something looks broken or looks like it's working, the expected response is a real, direct measurement (a headless run, a printed diagnostic, a comparison against two independent runs), not an inspection-based guess. That discipline runs through the whole codebase and its own documentation; see [Determinism](docs/explanation/determinism.md) for an example of a claim this project chose to correct rather than repeat once it was actually measured.

## Scientific goals

- **Emergent locomotion and behavior** from an evolvable brain (CTRNN) wired by an indirect, geometry-aware encoding (a CPPN), avoiding both hand-coded gaits and the "broken brain" problem where body mutations scramble a directly-encoded brain.
- **Emergent body plans** from a decoded regulatory network, not a template — the same decode pipeline grows a hand-tuned starter species and an evolved 40th-generation descendant.
- **A real chemical economy**, not an abstract single "energy" scalar — organisms trade glucose, oxygen, carbon dioxide, and ATP, with day/night-cycle-driven photosynthesis for producers and predation/foraging for everyone else.
- **Multi-generational, population-scale observation** — batch experiment runs, lineage/species tracking, and deterministic-enough replay for genuine before/after comparison, not just a pretty single-run demo.

## The 3D engine

Phylon's viewport, physics, and organism orientation are fully 3D: a single canonical `Camera3d` (orbit and fly navigation, ray-based picking, box/lasso select), mesh-based organism rendering with physically-based shading, a GPU spatial-hash physics broad-phase, and a body-fixed forward/dorsal orientation frame driving both bilateral symmetry and 3D vision. Chemical diffusion remains a set of 2D world-space planes by deliberate design — see [Camera & Viewport](docs/explanation/camera_and_viewport.md) and [Architecture](docs/explanation/architecture.md) for what's 3D, what's still 2D, and why.

## Architecture

- **Language**: Rust, workspace of 30 independent crates forming a strict acyclic dependency graph — see [Crate Dependency Graph](docs/reference/crate_graph.md).
- **ECS**: `bevy_ecs`, driven by a hand-written, fixed-order per-tick function — not the rest of the Bevy engine, and not a generic scheduler (a `scheduler` crate exists but is not used by the running app).
- **Concurrency**: `rayon` for per-organism CPU work; `wgpu` (Vulkan/Metal/DX12) compute shaders for physics integration and chemical diffusion.
- **Determinism**: seeded RNG (`SimRng`) and fixed timesteps are real and enforced; bit-exact reproducibility of the GPU physics pipeline across separate runs is a known, open gap, not a settled guarantee — see [Determinism](docs/explanation/determinism.md) for the honest current state.

## Features

- Evolvable body plans and brains via three independently-evolvable CPPNs per genome (regulatory network, brain wiring, and a currently-vestigial third network).
- A real metabolism (glucose/O2/CO2/ATP), disease, injury, and a two-stage life cycle.
- Five diet-driven ecological roles (Producer/Herbivore/Carnivore/Omnivore/Decomposer) with real predation, foraging, and decomposition.
- Species/lineage tracking, population and diversity analytics, and a research workbench (Inspector, Neural/GRN viewers, Lineage Explorer, Metrics, Event Log).
- Headless batch experiment runs, deterministic-enough action-based replay, and an embedded `rhai` scripting engine for scenario authoring.
- A WebSocket-based, framework-agnostic reinforcement-learning bridge for external training loops.

## Installation & Build

**Prerequisites:**

- Rust 1.80 or newer ([rustup](https://rustup.rs/)).
- A Vulkan-, Metal-, or DX12-capable GPU and driver (Phylon uses `wgpu` compute shaders for physics and diffusion — a software fallback exists for headless/CI use, but real-time interactive use expects real GPU compute support).

```bash
git clone https://github.com/Vinoth-ai-20/phylon.git
cd phylon
cargo build --release
```

## Running

```bash
cargo run -p app --release
```

Always use `--release` — physics and GPU synchronization overhead makes debug builds visibly stutter. See [Getting Started](docs/tutorials/getting_started.md) for a full walkthrough of what you'll see and how to navigate it.

## Controls

The viewport is a real 3D camera (orbit by default, with an opt-in free-fly mode). Full keybinding table: [Controls](docs/reference/controls.md).

## Project structure

```text
crates/          30 workspace crates — see docs/reference/crate_graph.md
data/            Runtime config (default.ron), the experiment-tracking database, saved preferences
docs/            All current documentation (Diátaxis: tutorials / how-to / explanation / reference,
                 plus design/ for the UI design system and roadmap/ for project history)
screenshots/     Screenshots of the running application
recordings/      Short recordings of the running application
```

There is no other markdown documentation at the repository root by design — everything current lives under `docs/`, and this README is the front door, not a second copy of it.

## Documentation

Full documentation lives in [`docs/`](docs/index.md), organized by the [Diátaxis](https://diataxis.fr/) framework:

- **[Tutorials](docs/tutorials/getting_started.md)** — learning-oriented walkthroughs.
- **[How-To Guides](docs/how_to/add_custom_genomes.md)** — step-by-step tasks (custom genomes, environment tuning, troubleshooting).
- **[Explanation](docs/explanation/architecture.md)** — the scientific and architectural model: architecture, simulation model, genetics & neurobiology, the camera/3D engine, determinism.
- **[Reference](docs/reference/components.md)** — component map, crate graph, controls.
- **[Design System](docs/design/design_system.md)** — the UI's visual/interaction design tokens and component catalog.

For exhaustive API signatures:

```bash
cargo doc --open
```

## Research features

- **Batch runs**: multi-seed headless experiments producing Markdown/RON reports (`research` crate, `app::batch`).
- **Replay**: deterministic-enough, action-based replay of a recorded run (headless-only today; see [Backlog](docs/roadmap/backlog.md) for the deferred in-app timeline).
- **Scripting**: sandboxed `rhai` scenario scripts for automated interventions.
- **Remote control / MARL**: a WebSocket protocol (`network` crate) for driving the simulation from an external reinforcement-learning trainer, independent of any specific ML framework.

## Performance

The GPU physics broad-phase (spatial hash) has been benchmarked directly at 1,000 / 5,000 / 10,000 particle nodes: roughly 321µs / 382µs / 542µs per compute step — comfortably under a 60 Hz frame budget, with sub-linear scaling. This is a measured figure for one subsystem, not a whole-simulation population target; treat any specific "N organisms at 60 Hz" claim as something to verify for your own hardware and scenario rather than a fixed spec.

## GPU requirements

A `wgpu`-compatible backend: Vulkan (Linux/Windows), Metal (macOS), or DX12 (Windows). Headless/CI environments without a real GPU can use a software Vulkan implementation (e.g. Mesa's lavapipe) — see `.github/workflows/ci.yml` for exactly how this project's own CI does it.

## Roadmap

See [Project History](docs/roadmap/history.md) for what's shipped, phase by phase, [Architecture Decisions](docs/roadmap/decisions.md) for the durable "why" behind current design, and [Open Items & Backlog](docs/roadmap/backlog.md) for disclosed, still-open gaps — including a real, measured GPU-determinism limitation and several unscheduled future initiatives.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for build/test commands, branch conventions, and required checks. New crates must preserve the acyclic dependency graph; simulation-affecting changes must go through `SimRng`, never an unseeded RNG. Changes to viewport interaction, the camera, gizmos, or window performance also need a pass through [MANUAL_TESTING.md](MANUAL_TESTING.md) — there's no automated input-injection tooling for the live window in this project's environments.

## Citation

If you use Phylon in academic or research work, please cite the repository:

```bibtex
@software{phylon,
  title  = {Phylon: A GPU-Accelerated Artificial Life Laboratory},
  author = {Phylon Contributors},
  url    = {https://github.com/Vinoth-ai-20/phylon},
  year   = {2026}
}
```

## Acknowledgements

Built on [`wgpu`](https://wgpu.rs/), [`egui`](https://www.egui.rs/), [`bevy_ecs`](https://bevyengine.org/), and [`rayon`](https://github.com/rayon-rs/rayon) — thank you to those communities.

## License

Dual-licensed under either the [MIT License](LICENSE-MIT) or the [Apache License, Version 2.0](LICENSE-APACHE), at your option.
