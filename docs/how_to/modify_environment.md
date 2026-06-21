# How to Modify the Environment

The Phylon environment consists of a continuous 2D spatial grid governed by strict physics and chemical diffusion rules. This guide explains how to alter the environmental constraints.

## Modifying Chemical Hotspots

The environment features chemical diffusion fields (e.g., pheromones, food scent, hazard markers). At the start of a simulation, "Hotspots" (Emitters) are spawned to seed these fields.

Open `crates/app/src/app.rs` and locate the "Spawn Resource Hotspots" section (around line 680).

```rust
// Spawn 20 random chemical emitters
for _ in 0..20 {
    let px = rng.gen_range(-1000.0..1000.0);
    let py = rng.gen_range(-1000.0..1000.0);
    world.spawn(diffusion::Emitter {
        position: common::Vec2::new(px, py),
        value: rng.gen_range(5.0..20.0), // Intensity of the emission
        radius: rng.gen_range(50.0..150.0), // Spatial radius of effect
    });
}
```

You can change the loop count, adjust the bounding box (`-1000.0..1000.0`), or create specific geometric patterns for the emitters.

## Modifying Physics Global Constraints

The physics engine relies on a symplectic Euler integrator with strict positional constraints. The global constraints (e.g., spatial bounds, grid sizes) are initialized in the `EnvironmentManager`.

Locate the physics initialization in `app.rs`:

```rust
// Adjust the maximum spatial bounds of the simulation
let env_manager = environment::EnvironmentManager::new(2000.0, 2000.0);
world.insert_resource(env_manager);
```

> [!WARNING]
> Increasing the spatial bounds significantly may require you to tune the GPU compute thread groups in `crates/gpu/src/diffusion_pipeline.rs` to ensure the diffusion grid fully covers the new boundaries.

## Modifying Drag and Damping

To alter how "thick" or viscous the fluid medium feels to the organisms, you must modify the `PhysicsConfig` resource.

```rust
world.insert_resource(physics::PhysicsConfig {
    linear_damping: 0.95,  // Lower values = thicker fluid (more drag)
    angular_damping: 0.90, // Lower values = harder to rotate
    gravity: common::Vec2::new(0.0, 0.0), // You can add gravity here!
});
```
