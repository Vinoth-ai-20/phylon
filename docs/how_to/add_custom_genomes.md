# How to Add Custom Genomes

Phylon allows you to define and spawn completely custom species into the ecosystem. This guide explains how to create a new genetic blueprint and inject a population into the starting simulation state.

## Step 1: Define the Hox Sequence (Morphology)

The physical structure (morphology) of an organism is dictated by its `HoxSequence`. You can use predefined factories or define your own sequence of `HoxGene` segments (e.g., Head, Torso, Muscle, Tail).

Open `crates/app/src/app.rs` and locate the genome definitions (around line 604).

```rust
use genetics::{Genome, GenomeId, HoxSequence};
use common::EntityId;

// Example: Create a new custom "Titan" worm with 6 torso segments
let titan_genome = Genome::new_hox_driven(
    GenomeId(10),       // Ensure this ID is unique!
    EntityId(0),        // Origin entity (0 for root founders)
    HoxSequence::worm(6, [0.153, 0.224, 0.110]), // Deep Forest Green (#27391C)
);
```

### Color Palette Mappings

When defining custom genomes, it is recommended to use the standard simulation palette to visually separate ecological roles:

- **Producers**: `[0.290, 0.871, 0.502]` (#4ADE80)
- **Herbivores (Worm)**: `[0.937, 0.663, 0.522]` (#EFA985)
- **Herbivores (Branchy)**: `[0.282, 0.792, 0.894]` (#48CAE4)
- **Carnivores (Fish)**: `[0.941, 0.329, 0.329]` (#F05454)
- **Omnivores**: `[0.702, 0.533, 0.922]` (#B388EB)
- **Decomposers**: `[0.831, 0.639, 0.451]` (#D4A373)

## Step 2: Spawn the Population

Once the base genome is defined, you need to spawn a population of individuals.

In `crates/app/src/app.rs`, locate the "Spawn Populations" section. Use the `spawn_pop` helper closure. You must specify the base genome, the ecological diet, and the number of individuals to spawn.

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
