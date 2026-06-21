# Getting Started with Phylon

Welcome to Phylon! This tutorial will guide you through the process of setting up your environment, compiling the engine, and running your first artificial life simulation.

## Prerequisites

Before building Phylon, ensure you have the following installed on your system:

1. **Rust Toolchain**: Phylon requires Rust version 1.80 or higher. Install it via [rustup](https://rustup.rs/).
2. **Vulkan SDK**: The engine offloads diffusion and physics computations to the GPU using WGSL compute shaders. You must have a Vulkan-compatible graphics driver installed.

## Step 1: Building the Engine

Phylon is a complex workspace consisting of 30 isolated crates. To build the entire engine and the primary application binary, run the following command in the root directory:

```bash
cargo build --release
```

> [!TIP]
> Always use the `--release` flag when running the simulation. The physics solver and GPU synchronization overhead will cause significant stuttering in unoptimized debug builds.

## Step 2: Running the Simulation

Once compiled, you can launch the simulation with:

```bash
cargo run -p app --release
```

### What You Will See

Upon launch, Phylon will spawn a window with a 2D environment representing an ecosystem.

- **Producers (Green)**: Static entities that form the base of the food chain.
- **Herbivores (White/Blue)**: Swarming organisms that actively seek out and consume producers.
- **Carnivores (Red)**: Apex predators that hunt herbivores.
- **Decomposers (Grey)**: Scavengers that break down corpses to return nutrients to the environment.

At spawn, the organisms' brains (CTRNNs) are randomized. You are observing true, unassisted Darwinian evolution. Over time, organisms that randomly possess viable swimming and hunting patterns will survive, reproduce, and pass those genetic traits to their offspring.

## Step 3: Navigating the UI

The application shell provides several interactive panels to inspect the simulation in real-time.

1. **Top Control Bar**: Pause, resume, and adjust the simulation tick rate.
2. **Left Genetics Panel**: Displays the phylogenetic tree, tracking lineages and speciation events.
3. **Right Inspector Panel**: Click on any organism in the viewport to open its inspector. Here you can view its:
   - **Metabolic State**: Current energy, age, and diet.
   - **Neural Output**: Live visualization of its CTRNN node activations.
   - **Genetic Blueprint**: The organism's Hox Sequence (body plan) and CPPN (brain wiring).

You can also trigger manual mutations on a selected organism using the `Mutate Weights`, `Mutate Add Node`, and `Mutate Add Connection` buttons.

---

## Next Steps

Now that you have the simulation running, you might want to learn how to manipulate the ecosystem. Proceed to the [How-To Guides](../how_to/add_custom_genomes.md) to learn how to inject your own custom species into the simulation!
