//! Rigid-body dynamics and core simulation components.

use common::Vec2;
use hecs::World;

// ----------------------------------------------------------------------------
// Core Physical Components
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position(pub Vec2);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Velocity(pub Vec2);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Acceleration(pub Vec2);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mass(pub f32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Radius(pub f32);

// ----------------------------------------------------------------------------
// Physics Systems
// ----------------------------------------------------------------------------

/// Applies Symplectic Euler integration to all entities with physics components.
/// Note: Symplectic Euler updates Velocity first, then Position using the new Velocity.
pub fn symplectic_euler_integration(world: &mut World, dt: f32) {
    puffin::profile_function!();

    // We can't easily parallelise mutable iterations over hecs without batching,
    // but hecs provides a query mutation mechanism. For now, we iterate sequentially
    // or use chunking. For phase 1, a standard query iteration is sufficient.
    for (_, (pos, vel, acc)) in
        world.query_mut::<(&mut Position, &mut Velocity, &mut Acceleration)>()
    {
        // Update velocity
        vel.0 += acc.0 * dt;
        // Update position using NEW velocity
        pos.0 += vel.0 * dt;
        // Reset acceleration
        acc.0 = Vec2::ZERO;
    }
}

/// A simple system to bounce entities off chunk borders (Phase 1 testing).
pub fn world_bounds_collision(world: &mut World, bounds: Vec2) {
    puffin::profile_function!();

    let half_bounds = bounds * 0.5;

    for (_, (pos, vel, radius)) in world.query_mut::<(&mut Position, &mut Velocity, &Radius)>() {
        // X-axis collision
        if pos.0.x - radius.0 < -half_bounds.x {
            pos.0.x = -half_bounds.x + radius.0;
            vel.0.x *= -1.0;
        } else if pos.0.x + radius.0 > half_bounds.x {
            pos.0.x = half_bounds.x - radius.0;
            vel.0.x *= -1.0;
        }

        // Y-axis collision
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

        // v1 = v0 + a*dt = (1.0, 0.0) + (0.0, 1.0) = (1.0, 1.0)
        assert_eq!(v.0, Vec2::new(1.0, 1.0));
        // p1 = p0 + v1*dt = (0.0, 0.0) + (0.5, 0.5) = (0.5, 0.5)
        assert_eq!(p.0, Vec2::new(0.5, 0.5));
        // a1 = 0
        assert_eq!(a.0, Vec2::ZERO);
    }
}
