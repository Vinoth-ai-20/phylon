# Phylon Documentation

Welcome to the official documentation for **Phylon**, a research-grade, high-performance artificial life laboratory built in Rust.

This documentation is organized following the [Diátaxis framework](https://diataxis.fr/), which divides content into four distinct quadrants based on user needs.

## 1. Tutorials (Learning-Oriented)
*Start here if you are new to Phylon.*
- [Getting Started](tutorials/getting_started.md) - Installation, building, and running your first simulation.

## 2. How-To Guides (Problem-Oriented)
*Step-by-step guides for specific tasks.*
- [Adding Custom Genomes](how_to/add_custom_genomes.md) - Define and spawn a new species.
- [Modifying the Environment](how_to/modify_environment.md) - Configure physics and chemical diffusion hotspots.
- [Troubleshooting](how_to/troubleshooting.md) - Common errors and their solutions.

## 3. Explanation (Understanding-Oriented)
*Deep-dive theoretical discussions on how Phylon works.*
- [Architecture & Concurrency](explanation/architecture.md) - ECS, crate graph, and multi-threading.
- [Simulation Model](explanation/simulation_model.md) - Physics, diffusion, metabolism, and ecology.
- [Genetics & Neurobiology](explanation/genetics_and_neurobiology.md) - Regulatory networks, CPPNs, and CTRNNs.
- [Camera & Viewport (3D Engine)](explanation/camera_and_viewport.md) - The 3D camera, rendering pipeline, and navigation model.
- [Determinism](explanation/determinism.md) - What is and isn't guaranteed to reproduce bit-for-bit today.

## 4. Reference (Information-Oriented)
*Quick lookups and technical overviews.*
- [Component Overview](reference/components.md) - High-level map of the Entity-Component-System logic.
- [Crate Dependency Graph](reference/crate_graph.md) - Workspace architecture and boundaries.
- [Controls](reference/controls.md) - Keyboard and mouse bindings.
- **Rust API Docs:** For exhaustive method signatures, structs, and enumerations, run `cargo doc --open` in the root workspace.

## 5. Design System (Information-Oriented)
*The permanent source of truth for the workbench UI's visual and interaction design — implemented in `crates/ui/src/theme.rs`.*
- [Design System Overview](design/design_system.md) - Principles and how the other design docs relate.
- [Typography](design/typography.md) - Type scale, numerals, capitalization.
- [Colors](design/colors.md) - Every color token and its meaning.
- [Spacing](design/spacing.md) - The 4/8/12/16/24/32/48 scale.
- [Layout & Docking](design/layout.md) - Panel ratios, docking model, window management.
- [Component Catalog](design/components.md) - Every reusable widget, fully specified.
- [Iconography](design/iconography.md) - Icon sizes and semantic meaning.
- [Accessibility](design/accessibility.md) - Colorblind safety, focus, keyboard navigation.
- [Biological Visual Language](design/biological_visual_language.md) - The canonical mapping from simulation state to viewport signal (health, disease, behavior, death, etc.).

## 6. Roadmap & History (Information-Oriented)

*What shipped, why, and what's still open — condensed from the project's own phase-by-phase development record.*

- [Project History](roadmap/history.md) - A phase-by-phase summary of what was built, in order.
- [Architecture Decisions](roadmap/decisions.md) - Durable ADRs extracted from the phase archive, organized by topic.
- [Open Items & Backlog](roadmap/backlog.md) - Disclosed, still-open gaps and unscheduled future work.
- [Phase 9 — Workbench UX, Performance & Optimization](roadmap/phase9_workbench_performance.md) - Active roadmap: measured FPS/rendering bottlenecks and camera/input architecture findings, not yet implemented.

---

> **Note to researchers:** Phylon's determinism guarantees are real but partial — see [Determinism](explanation/determinism.md) for exactly what's covered today (fixed timesteps, seeded RNG, sorted ECS iteration) and what is a known, open gap (bit-exact reproducibility of the GPU physics pipeline across separate runs of the same seed).
