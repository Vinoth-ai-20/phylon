# ECS Component Overview

Phylon is built on an Entity-Component-System (ECS) architecture. This document provides a high-level conceptual map of the core data structures attached to organisms in the simulation.

For exhaustive method signatures and field types, run `cargo doc --open` in the root workspace.

## Biological Data Model

Every living organism in Phylon is an Entity composed of multiple distinct Components. This modularity allows the simulation to decouple movement physics from neural processing and metabolism.

### 1. `physics::ParticleNode`

The fundamental physical manifestation of an organism.

- Tracks `position`/`velocity` (both `common::Vec3` — the simulation is 3D), `mass`, and `segment_type`.
- Exists in the spatial index (`spatial::Octree` on the CPU side, a GPU spatial hash on the physics-compute side) for collision detection and vision querying.
- Organisms are often composed of a primary "Head" node connected to child "Body" nodes via springs.

### 2. `metabolism::Metabolism` & `metabolism::ChemicalEconomy`

The caloric and chemical engine of the organism.

- The `Metabolism` component defines the `base_rate` of energy burned passively per tick.
- The `ChemicalEconomy` component tracks 4 metabolic pools: `Glucose`, `O2`, `CO2`, and `ATP`.
- If `ATP` or `Glucose` reaches zero, the organism despawns (dies) and spawns a `Corpse` entity in its place.

### 3. `metabolism::Age`

Tracks the organism's lifespan in ticks. Used to cull organisms that survive too long to prevent stagnation in evolutionary fitness landscapes.

### 4. `ecology::Diet`

Determines what the organism can consume to replenish its `Glucose`.

- Options include `Producer`, `Herbivore`, `Carnivore`, `Omnivore`, and `Decomposer`.
- Checked during the `sensing_system` (to set visual targets) and the collision resolution phase (to trigger consumption).

### 5. `brain::Brain` (CTRNN)

The neural controller of the organism.

- Stores the Continuous-Time Recurrent Neural Network (nodes and synapses).
- Reads the input vector from the `SensoryState` component.
- Outputs activation values that the `behavior_system` translates into muscle actuation amplitudes.

### 6. `genetics::Genome`

The inheritable blueprint of the organism — exactly three CPPNs, no separate body-plan-sequence type:

- `regulatory_cppn` — decoded per body position into a segment type, branching decision, actuation, and pigment (see [Genetics & Neurobiology](../explanation/genetics_and_neurobiology.md)). This is what actually determines body-plan layout today.
- `brain_cppn` — governs the synaptic wiring of the `Brain`.
- `morph_cppn` — crossed/mutated/part of the speciation-distance calculation, but not read at growth time (disclosed technical debt).
- Passed down (and mutated) during events spawned by the `reproduction_system`.

### 7. `sensing::SensoryState`

The organism's view of the world.

- A flat float vector of **15 normalized inputs**: 3 scalar channels (Olfaction, Energy/ATP, Age) plus a 9-value vision representation (a binned azimuth × elevation cone around the organism's 3D forward/dorsal frame).
- Populated by the `sensing_system` and fed directly into the `Brain` component each tick.

## Environmental Resources

### `metabolism::GlobalAtmosphere`

A singleton Resource that tracks global environmental parameters such as `ticks` and `sunlight_intensity` ($I_{sun}$), dictating the day/night cycle and driving producer photosynthesis rates.
