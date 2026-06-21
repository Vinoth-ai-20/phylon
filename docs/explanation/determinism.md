# GPU Determinism Policy

Phylon enforces strict, bit-exact deterministic execution. This guarantees that a simulation started with ng_seed=1234 will produce the exact same ecological outcomes across Windows, macOS, and Linux.

## The Challenge of Floating Point Math

CPUs and GPUs often handle floating-point rounding differently, breaking determinism. Furthermore, multithreaded operations (like parallel collision detection) can resolve in arbitrary order.

## How Phylon Achieves Determinism

1. **Fixed Timesteps**: The engine uses a strict fixed Delta Time (DT = 0.016). We never use variable framerates for simulation logic.
2. **GPU Strictness**: All mathematical operations on the GPU use strict IEEE-754 semantics. Fused Multiply-Add (FMA) instructions are disabled in the WGSL shaders where possible, or mathematically guaranteed to produce identical results regardless of hardware architecture.
3. **Sorted ECS Iteration**: When systems must aggregate inputs (e.g., multiple organisms eating from the same resource), the inputs are sorted by EntityId prior to processing, guaranteeing a deterministic resolution order regardless of the Rayon thread execution timeline.
