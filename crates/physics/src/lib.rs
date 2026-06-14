//! Rigid-body dynamics and core simulation components.

use common::Vec2;
use hecs::World;
use serde::{Deserialize, Serialize};

// ----------------------------------------------------------------------------
// Core Physical Components
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position(pub Vec2);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Velocity(pub Vec2);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Acceleration(pub Vec2);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Mass(pub f32);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Radius(pub f32);

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Heading(pub f32);

// ----------------------------------------------------------------------------
// Physics Systems
// ----------------------------------------------------------------------------

/// Applies Symplectic Euler integration to all entities with physics components.
pub fn symplectic_euler_integration(world: &mut World, dt: f32) {
    puffin::profile_function!();

    for (_, (pos, vel, acc)) in
        world.query_mut::<(&mut Position, &mut Velocity, &mut Acceleration)>()
    {
        vel.0 += acc.0 * dt;
        vel.0 *= 0.95; // Simple linear friction
        pos.0 += vel.0 * dt;
        acc.0 = Vec2::ZERO; // Reset acceleration
    }
}

/// Populates a `spatial::UniformGrid` with all physical entities.
pub fn update_spatial_grid(world: &World, grid: &mut spatial::UniformGrid) {
    puffin::profile_function!();
    grid.clear();
    for (entity, pos) in world.query::<&Position>().iter() {
        grid.insert(common::EntityId(entity.to_bits().get()), pos.0);
    }
}

/// Resolves collisions between all physical entities using the spatial grid for broad-phase.
pub fn circle_circle_collision(world: &mut World, grid: &spatial::UniformGrid, restitution: f32) {
    puffin::profile_function!();

    // Since hecs doesn't allow multiple mutable borrows easily via queries,
    // we extract the data, process collisions, and write back.
    // In a real ECS we'd use unsafe or specific batching, but for Phase 2 we can collect pairs.

    // We will do a simple single-pass resolution.
    let mut displacements = rustc_hash::FxHashMap::default();
    let mut velocity_changes = rustc_hash::FxHashMap::default();

    for (entity_a, (pos_a, vel_a, radius_a, mass_a)) in world
        .query::<(&Position, &Velocity, &Radius, &Mass)>()
        .iter()
    {
        let cell_a = grid.pos_to_cell(pos_a.0);
        let id_a = common::EntityId(entity_a.to_bits().get());

        // Check 3x3 neighborhood
        for dx in -1..=1 {
            for dy in -1..=1 {
                let neighbor_cell = common::IVec2::new(cell_a.x + dx, cell_a.y + dy);
                for &entity_id_b in grid.query_cell(neighbor_cell) {
                    if id_a.0 >= entity_id_b.0 {
                        continue;
                    } // Avoid double-checking and self-checking
                    let entity_b = hecs::Entity::from_bits(entity_id_b.0).unwrap();

                    if let Ok(mut q_b) =
                        world.query_one::<(&Position, &Velocity, &Radius, &Mass)>(entity_b)
                    {
                        if let Some((pos_b, vel_b, radius_b, mass_b)) = q_b.get() {
                            let diff = pos_b.0 - pos_a.0;
                            let dist_sq = diff.length_squared();
                            let combined_radius = radius_a.0 + radius_b.0;

                            if dist_sq < combined_radius * combined_radius && dist_sq > 0.0001 {
                                let dist = dist_sq.sqrt();
                                let normal = diff / dist;
                                let overlap = combined_radius - dist;

                                // Positional correction (mass proportional)
                                let total_mass = mass_a.0 + mass_b.0;
                                let ratio_a = mass_b.0 / total_mass;
                                let ratio_b = mass_a.0 / total_mass;

                                *displacements.entry(entity_a).or_insert(Vec2::ZERO) -=
                                    normal * overlap * ratio_a;
                                *displacements.entry(entity_b).or_insert(Vec2::ZERO) +=
                                    normal * overlap * ratio_b;

                                // Velocity resolution
                                let rel_vel = vel_b.0 - vel_a.0;
                                let vel_along_normal = rel_vel.dot(normal);

                                if vel_along_normal < 0.0 {
                                    let j = -(1.0 + restitution) * vel_along_normal;
                                    let j = j / (1.0 / mass_a.0 + 1.0 / mass_b.0);
                                    let impulse = normal * j;

                                    *velocity_changes.entry(entity_a).or_insert(Vec2::ZERO) -=
                                        impulse / mass_a.0;
                                    *velocity_changes.entry(entity_b).or_insert(Vec2::ZERO) +=
                                        impulse / mass_b.0;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Apply changes
    for (entity, (pos, vel)) in world.query_mut::<(&mut Position, &mut Velocity)>() {
        if let Some(disp) = displacements.get(&entity) {
            pos.0 += *disp;
        }
        if let Some(dv) = velocity_changes.get(&entity) {
            vel.0 += *dv;
        }
    }
}

/// A simple system to bounce entities off chunk borders (Phase 1 testing).
pub fn world_bounds_collision(world: &mut World, bounds: Vec2) {
    puffin::profile_function!();

    let half_bounds = bounds * 0.5;

    for (_, (pos, vel, radius)) in world.query_mut::<(&mut Position, &mut Velocity, &Radius)>() {
        if pos.0.x - radius.0 < -half_bounds.x {
            pos.0.x = -half_bounds.x + radius.0;
            vel.0.x *= -1.0;
        } else if pos.0.x + radius.0 > half_bounds.x {
            pos.0.x = half_bounds.x - radius.0;
            vel.0.x *= -1.0;
        }

        if pos.0.y - radius.0 < -half_bounds.y {
            pos.0.y = -half_bounds.y + radius.0;
            vel.0.y *= -1.0;
        } else if pos.0.y + radius.0 > half_bounds.y {
            pos.0.y = half_bounds.y - radius.0;
            vel.0.y *= -1.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symplectic_euler() {
        let mut world = World::new();
        let e = world.spawn((
            Position(Vec2::ZERO),
            Velocity(Vec2::new(1.0, 0.0)),
            Acceleration(Vec2::new(0.0, 2.0)),
        ));

        symplectic_euler_integration(&mut world, 0.5);

        let mut query = world
            .query_one::<(&Position, &Velocity, &Acceleration)>(e)
            .unwrap();
        let (p, v, a) = query.get().unwrap();

        // v1 = (v0 + a*dt) * 0.95 = ((1.0, 0.0) + (0.0, 1.0)) * 0.95 = (0.95, 0.95)
        assert_eq!(v.0, Vec2::new(0.95, 0.95));
        // p1 = p0 + v1*dt = (0.0, 0.0) + (0.475, 0.475) = (0.475, 0.475)
        assert_eq!(p.0, Vec2::new(0.475, 0.475));
        // a1 = 0
        assert_eq!(a.0, Vec2::ZERO);
    }
}
