# Phylon Documentation

Welcome to the official documentation for **Phylon**, a research-grade, high-performance artificial life laboratory built in Rust.

This documentation is organized following the [Diátaxis framework](https://diataxis.fr/), which divides content into four distinct quadrants based on user needs:

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
- [Architecture & Concurrency](explanation/architecture.md) - ECS, Crate Graph, and multi-threading.
- [Simulation Model](explanation/simulation_model.md) - Physics, Diffusion, Metabolism, and Ecology.
- [Genetics & Neurobiology](explanation/genetics_and_neurobiology.md) - Hox sequences, CPPNs, and CTRNNs.
- [GPU Determinism](explanation/determinism.md) - How we guarantee bit-exact reproducibility across platforms.

## 4. Reference (Information-Oriented)
*Quick lookups and technical overviews.*
- [Component Overview](reference/components.md) - High-level map of the Entity-Component-System logic.
- [Crate Dependency Graph](reference/crate_graph.md) - Workspace architecture and boundaries.
- **Rust API Docs:** For exhaustive method signatures, structs, and enumerations, run cargo doc --open in the root workspace.

---

> **Note to Ecologists and RSEs:** Phylon is designed to enforce bit-exact reproducibility while simulating massive populations. Understanding the strict boundaries between the CPU-authoritative logic layer and the GPU compute layer is critical for adding new ecological mechanics.
