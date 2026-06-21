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

## "Twitching Meat Blocks" (Organisms Not Moving)

**Issue:** Organisms spawn but just sit there twitching, eventually starving to death.

- **Cause**: In Phase 16, organisms are spawned with completely randomized neural wiring (via their CPPN). A vast majority of random neural configurations are non-viable.
- **Solution**: This is intended behavior. The simulation is an evolutionary engine. Wait a few hundred ticks; the tiny percentage of organisms that randomly possess a viable swimming pattern will find food, reproduce, and pass on their viable brains.

## Physics Explosions

**Issue:** Organisms suddenly stretch to infinity or disappear, and the terminal prints `NaN` values.

- **Cause**: A physics singularity occurred. This usually happens if you modify `HoxSequence` to spawn two nodes precisely on top of each other, or if you set the `linear_damping` too close to `1.0` in `PhysicsConfig`.
- **Solution**: Ensure your custom body plans have logical spacing. Reset `linear_damping` to `0.95`.
