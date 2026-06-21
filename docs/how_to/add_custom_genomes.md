# How to Add Custom Genomes

Phylon allows you to define and spawn completely custom species into the ecosystem. This guide explains how to create a new genetic blueprint and inject a population into the starting simulation state.

## Step 1: Define the Hox Sequence (Morphology)

The physical structure (morphology) of an organism is dictated by its `HoxSequence`. You can use predefined factories or define your own sequence of `HoxGene` segments (e.g., Head, Torso, Muscle, Tail).

Open `crates/app/src/app.rs` and locate the genome definitions (around line 620).

```rust
use genetics::{Genome, GenomeId, HoxSequence};
use common::EntityId;

// Example: Create a new custom "Titan" worm with 6 torso segments
let titan_genome = Genome::new_hox_driven(
    GenomeId(10),       // Ensure this ID is unique!
    EntityId(0),        // Origin entity (0 for root founders)
    HoxSequence::worm(6, [0.9, 0.1, 0.1]), // 6 segments, Red color
);
```

## Step 2: Spawn the Population

Once the base genome is defined, you need to spawn a population of individuals.

In `crates/app/src/app.rs`, locate the "Spawn Populations" section (around line 660). Use the `spawn_pop` helper closure. You must specify the base genome, the ecological diet, and the number of individuals to spawn.

```rust
// Spawn 50 Titans that act as Carnivores
spawn_pop(&titan_genome, ecology::Diet::Carnivore, 50);
```

> [!NOTE]
> The `spawn_pop` helper automatically mutates the neural wiring (CPPN) of the base genome for every individual it spawns (unless they are `Diet::Producer`). This ensures your new population is genetically diverse rather than a clone army.

## Step 3: Compile and Observe

Run the simulation:

```bash
cargo run -p app --release
```

You should now see your custom "Titan" organisms interacting within the ecosystem!
