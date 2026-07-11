# Troubleshooting

This guide covers common issues you may encounter while building or running Phylon.

## Vulkan/GPU Errors

Phylon relies heavily on `wgpu` and Vulkan for its compute shaders.

**Error:** `Unrecognized present mode` or `wgpu_hal::vulkan::conv` warnings.

- **Cause**: Your graphics driver does not strictly support the requested vsync or present mode, and `wgpu` is falling back to a default.
- **Solution**: This is a harmless warning. If it causes visual tearing, update your GPU drivers.

**Error:** `Failed to acquire next swap chain texture`

- **Cause**: The application window was resized to an invalid dimension (e.g., minimized to 0x0) while the GPU was attempting to present a frame.
- **Solution**: Restart the application. Avoid minimizing the window during heavy compute phases.

## ECS Despawn Errors

**Error:** `error[B0003]: Could not despawn entity Entity ... because it doesn't exist in this World.`

- **Cause**: In highly concurrent physics simulations, two systems (e.g., predator eating prey, and prey starving simultaneously) might attempt to despawn the same entity in the same tick. Bevy's ECS logs this warning when it encounters a double-despawn.
- **Solution**: This is a harmless warning. To minimize it, Phylon utilizes `commands.get_entity(entity)` checks before calling `.despawn()`. If you still see this, it simply means an entity died exactly as something else interacted with it (e.g., during collision resolution).

## Simulation Stuttering or Freezing

**Issue:** The simulation runs at 5 FPS instead of the target 60 FPS.

- **Cause**: You likely ran the application in debug mode (`cargo run`).
- **Solution**: Always compile and run Phylon with the `--release` flag (`cargo run --release`). The ECS iteration overhead and GPU data syncing are significantly optimized in release builds.

## Organisms Not Moving

**Issue:** Organisms spawn and grow correctly but never actuate — brain outputs look non-zero, but nothing moves.

Some of this is expected: a freshly-spawned population's brains (CTRNNs) are randomized, and most random neural configurations don't produce a coherent locomotion pattern — waiting a few hundred ticks for reproduction and selection to favor viable brains is normal.

But if you observe this at the *population* level for an extended run (most or all organisms, indefinitely, not just early on), check the actual root cause rather than assuming it's "just evolution hasn't found a gait yet": this project has directly measured and fixed two structural causes of exactly this symptom before —

1. **Zero actuatable muscle segments.** A body position only becomes a physically-actuated spring if it decoded as a `Muscle` segment (see [Genetics & Neurobiology](../explanation/genetics_and_neurobiology.md)) — an organism can have a perfectly normal-looking brain and body and still have *no spring capable of moving at all*. Add a print of `motor.effectors.len()` for a sample of organisms; if it's consistently `0`, the bug is in body-plan decode or founder-genome tuning, not the brain.
2. **Founder-population mutation dosage.** If every founder genome is mutated too aggressively before it's ever spawned, it can wash out an otherwise-viable regulatory-network tuning across most of the population — this has happened in this project's history and was root-caused by measuring the actual post-mutation actuatable-effector rate directly, not by inspecting the mutation code and guessing.

**Solution**: Measure — sample real organisms' `effectors.len()` and CTRNN outputs directly in a headless run — before assuming this is "just evolution."

## Physics Explosions

**Issue:** Organisms suddenly stretch to infinity or disappear, and the terminal prints `NaN` values.

- **Cause**: A physics singularity occurred. This usually happens if a custom body plan spawns two particle nodes precisely on top of each other, or if you set `linear_damping` too close to `1.0` in `PhysicsConfig`.
- **Solution**: Ensure your custom body plans have logical spacing. Reset `linear_damping` to `0.95`.
