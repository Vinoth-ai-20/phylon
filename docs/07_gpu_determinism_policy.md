# GPU Determinism Policy

Phylon strives for research-grade reproducibility, but the reality of floating-point math on highly parallel heterogeneous GPU architectures poses challenges.

## Guarantees

- **CPU State is Canonical**: The entity state (`world`) mutated on the CPU is 100% deterministic.
- **RNG Reproducibility**: All stochastic events (mutations, initial placements) use a seeded `ChaCha8Rng`. A run with the same seed, identical interventions, and identical config will result in the exact same sequence of RNG decisions.
- **Event Log**: External human interventions are serialized into an event log for replay.

## Non-Determinism Points

- **GPU Floating Point Variance**: Different GPUs (or even the same GPU on different driver versions) evaluate transcendental functions, associative reductions, and floating-point accumulation in slightly different orders. 
- Therefore, outputs from GPU fields (`diffusion`) and neural networks (`burn` inference) will exhibit microscopic variance across different hardware.

## Replay Mechanism

To ensure replay works correctly despite GPU variance, Phylon employs strict readback precision policies:
1. **Precision truncation**: GPU readbacks that influence behavioral branching logic (e.g., neural outputs) are truncated or rounded to a fixed-point precision scale on the CPU before being used. This absorbs microscopic variance.
2. **Keyframe Snapshots**: The `research` layer saves deterministic binary snapshots periodically. If a replay diverges, the system detects the divergence and can resume from the nearest canonical snapshot.

For mathematically perfect 100% cross-platform determinism (e.g., for continuous integration testing), a `software_fallback` compute mode can be toggled via `PhylonConfig`, bypassing the GPU entirely at the cost of massive performance degradation.
