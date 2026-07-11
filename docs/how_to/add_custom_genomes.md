# How to Add Custom Genomes

Phylon allows you to define and spawn completely custom species into the ecosystem. This guide explains how to create a new genetic blueprint and inject a population into the starting simulation state.

There is no `HoxSequence`, `HoxGene`, or `Genome::new_hox_driven` in the current codebase — an earlier design was retired early in the project's history. The current mechanism is described below; see [Genetics & Neurobiology](../explanation/genetics_and_neurobiology.md) for the full decode model.

## Step 1: Define the Regulatory Seed

A starter genome is an ordinary `genetics::Genome` whose `regulatory_cppn` was hand-tuned rather than evolved. `crates/app/src/app.rs`'s `seed_ecosystem` function is where every built-in starter species is defined — search it for `RegulatorySeedWeights` to find the existing examples to copy from.

```rust
use genetics::{Genome, GenomeId, Cppn};
use common::EntityId;

// A starter genome is 3 CPPNs: brain, morph (currently unused at growth
// time), and regulatory (drives body-plan decode).
let titan_genome = Genome::seed(
    GenomeId(10),               // must be unique
    EntityId(0),                // origin entity, 0 for a founder
    seed_brain_cppn(),          // shared brain-wiring seed, or your own
    Cppn::new(),                // morph_cppn — currently inert, minimal is fine
    seed_regulatory_cppn(RegulatorySeedWeights {
        output_bias: -4.0,
        hox_weight: 8.0,
        differentiation_weight: 3.0,
        effector_weight: 3.0,
        pigment_weight: 1.0,
        sine_coarse_weight: 2.0,
        sine_fine_weight: 1.0,
    }),
);
```

**Before trusting a new weight combination, measure it — don't assume it decodes a viable body.** The regulatory network's decode is genuinely sensitive to these weights: it's possible to produce a genome whose body apoptoses almost entirely, or one with zero actuatable muscle segments, while still compiling and running without error. At minimum, check across the position range your species will grow to:

```rust
for pos in 1..organisms::MAX_SEGMENTS {
    let out = genetics::develop_at_position(&regulatory_cppn, pos, organisms::MAX_SEGMENTS);
    println!("{pos}: {:?} apoptosis={}", out.segment_type, out.apoptosis);
}
```

Confirm the resulting sequence has several non-apoptotic segments and at least one real `Muscle` segment with a nonzero `actuation_amplitude`. This exact kind of check caught a real bug in two of the built-in starter species — see [Architecture Decisions](../roadmap/decisions.md).

### Color

Pigment is **emergent** — decoded per-segment from three "Pigment"-role regulatory genes, never stored as RGB on the genome. There is no fixed color-literal table to copy from for a custom genome; if you want a starting population to lean toward a particular hue, that comes from how the Pigment genes' regulatory-network weights happen to respond across the body, the same way Hox/Effector weights shape body plan. `ecology::Diet::standard_color()` is the fallback/reference palette used for diet-based UI elements (charts, legends) — it is not what an individual organism's skin necessarily renders as.

## Step 2: Spawn the Population

Once the base genome is defined, spawn a population with the `spawn_pop` helper closure (also in `seed_ecosystem`):

```rust
// Spawn 50 Titans that act as Carnivores
spawn_pop(&titan_genome, ecology::Diet::Carnivore, 50);
```

> [!NOTE]
> `spawn_pop` mutates each non-`Producer` individual's genome a small number of times before spawning, to give the founder population genetic diversity rather than a clone army. The mutation rate used here matters: too aggressive, and it can degrade the same body-plan viability checked in Step 1 across the whole population. If you change it, re-measure the population's actuatable-effector rate directly (a real headless run), the same way the current rate was chosen.

## Step 3: Compile and Observe

```bash
cargo run -p app --release
```

You should now see your custom "Titan" organisms interacting within the ecosystem!
