# ECS Component Overview

Phylon is built on an Entity-Component-System (ECS) architecture. This document provides a high-level conceptual map of the core data structures attached to organisms in the simulation.

For exhaustive method signatures and field types, run `cargo doc --open` in the root workspace.

## Biological Data Model

Every living organism in Phylon is an Entity composed of multiple distinct Components. This modularity allows the simulation to decouple movement physics from neural processing and metabolism.

### 1. `physics::ParticleNode`

The fundamental physical manifestation of an organism.

- Tracks `position`, `velocity`, and `mass`.
- Exists in the spatial grid for collision detection and vision querying.
- Organisms are often composed of a primary "Head" node connected to child "Body" nodes via springs.

### 2. `metabolism::Metabolism` & `metabolism::Energy`

The caloric engine of the organism.

- The `Metabolism` component defines the `base_rate` of energy burned passively per tick.
- The `Energy` component tracks the current caloric reservoir. If this reaches zero, the organism despawns (dies) and spawns a Corpse entity in its place.

### 3. `ecology::Diet`

Determines what the organism can consume to replenish its `Energy`.

- Options include `Producer`, `Herbivore`, `Carnivore`, `Omnivore`, and `Decomposer`.
- Checked during the `sensing_system` (to set visual targets) and the collision resolution phase (to trigger consumption).

### 4. `brain::Brain` (CTRNN)

The neural controller of the organism.

- Stores the Continuous-Time Recurrent Neural Network (nodes and synapses).
- Reads the input vector from the `SensoryState` component.
- Outputs activation values that the `behavior_system` translates into muscle actuation amplitudes.

### 5. `genetics::Genome`

The inheritable blueprint of the organism.

- Contains the `HoxSequence` governing the physical segment layout.
- Contains the `CPPN` neural network governing the synaptic wiring of the `Brain`.
- Passed down (and mutated) during events spawned by the `reproduction_system`.

### 6. `sensing::SensoryState`

The organism's view of the world.

- A flat float vector containing 9 normalized inputs: Olfaction, Signal Field, Hazard Field, Energy, Age, Vision (Left, Center, Right), and the Internal Pacemaker (CPG) clock.
- Populated by the `sensing_system` and fed directly into the `Brain` component each tick.
