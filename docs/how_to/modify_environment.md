# How to Modify the Environment

Organisms and their physics are simulated in 3D; chemical diffusion fields remain 2D world-space planes (see [Architecture](../explanation/architecture.md)). This guide explains how to alter the environmental constraints.

## Modifying Chemical Hotspots

The environment features chemical diffusion fields (pheromones, energy, O2, CO2, morphogen). At the start of a simulation, "Hotspots" (Emitters) are spawned to seed these fields.

Open `crates/app/src/app.rs`'s `seed_ecosystem` function and locate the emitter-spawning loop (search for `diffusion::Emitter` — exact line numbers drift as the file grows, so search rather than trust a cached line number).

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

The physics engine relies on a symplectic Euler integrator with strict positional constraints. The global constraints (spatial bounds, world seed, toroidal wrapping) are initialized via `environment::EnvironmentManager`.

Locate the environment initialization in `app.rs` (search for `EnvironmentManager::new`):

```rust
// EnvironmentManager::new(seed, toroidal, width, height)
let env_manager = environment::EnvironmentManager::new(rng_seed, false, 2000.0, 2000.0);
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
    gravity: 0.0,          // scalar, applied along -Z; 0.0 for a neutral-buoyancy medium
    ..Default::default()
});
```

Check `crates/physics/src/lib.rs`'s `PhysicsConfig` definition for the full current field list before copying this snippet verbatim — fields get added over time and this example may not be exhaustive.
