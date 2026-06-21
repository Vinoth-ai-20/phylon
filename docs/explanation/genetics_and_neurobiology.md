# Genetics & Neurobiology

Phylon implements a state-of-the-art neuro-evolutionary pipeline combining explicit body plans with evolving neural networks.

## Hox Sequences (Morphology)

An organism's physical layout is defined by its HoxSequence.

- A sequence is an ordered list of HoxGene segments (e.g., Head, Torso, Tail).
- The growth_system reads these genes sequentially during the organism's zygote phase, spawning the physical particle nodes and connecting them with joints.
- Specific segments dictate biological properties (e.g., fins generate ctuation_amplitude).

## Dual Compositional Pattern Producing Networks (CPPN)

To avoid pleiotropy issues where physical mutations scramble neural topologies ("broken brain" syndrome), Phylon uses a dual independent CPPN architecture: `brain_cppn` and `morph_cppn`. Both are explicitly tracked by a `GlobalInnovationTracker`.

- **morph_cppn**: If an organism doesn't have a hard-coded HoxSequence, this CPPN dynamically evaluates the sequence of body segments up to a fixed maximum length (15 segments). It outputs the structural class (Head, Torso, Muscle, Fin, Tail), as well as actuation amplitude and phase for locomotion.
- **brain_cppn**: Operates as a neural blueprint. It maps spatial coordinates (the relative positions of two biological segments) to a connection weight. When an organism is born, its brain CPPN is queried for every possible pair of nodes. If it outputs a non-zero value, a synapse is created.
- The evaluation loop cleanly isolates structural `Input` nodes, preventing coordinate erasure.
- This allows evolution to encode complex, repeating geometric symmetries (like bilateral walking gaits) efficiently without breaking when the physical structure evolves.

## Continuous-Time Recurrent Neural Networks (CTRNN)

The resulting brain is a CTRNN.

- A fixed vector of 9 sensory inputs (Olfaction, Signals, Hazards, Energy, Age, 3x Vision, and an Internal Pacemaker clock) feed into the input layer.
- The Internal Pacemaker continuously supplies a ~1 Hz sine wave to the brain, providing the evolutionary algorithm with a foundational rhythm to actuate fin muscles.
- Activation flows through hidden layers governed by differential equations (time constants), allowing the brain to maintain short-term memory.
- The CTRNN nodes are integrated on the GPU via a highly parallel WGSL compute shader.
- To prevent positive feedback loops from causing `NaN` explosions or extreme Tanh saturation, node states are safely clamped between `-10.0` and `10.0` during GPU integration.
- Outputs dictate the contraction and expansion of `Elastic` muscle springs (with highly tuned stiffness/damping), driving the organism's physical locomotion in the anisotropic fluid environment.
