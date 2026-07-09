use bevy_ecs::prelude::*;
use common::Vec3;

/// Global tunables for boids-style flocking.
#[derive(Resource, Debug, Clone)]
pub struct FlockingConfig {
    /// Radius within which same-`Diet` neighbors influence alignment/cohesion.
    pub radius: f32,
    /// Radius within which neighbors trigger separation (always `<= radius`).
    pub separation_radius: f32,
    /// Weight of the alignment term (match neighbors' average velocity).
    pub alignment_weight: f32,
    /// Weight of the cohesion term (steer toward neighbors' average position).
    pub cohesion_weight: f32,
    /// Weight of the separation term (steer away from too-close neighbors).
    pub separation_weight: f32,
    /// Maximum magnitude of the combined steering force applied per tick.
    pub max_force: f32,
}

impl Default for FlockingConfig {
    fn default() -> Self {
        Self {
            radius: 120.0,
            separation_radius: 30.0,
            alignment_weight: 0.4,
            cohesion_weight: 0.3,
            separation_weight: 0.6,
            max_force: 50.0,
        }
    }
}

/// # Flocking System
///
/// ## 1. What Happens
/// Every organism's head node receives a supplemental steering force toward
/// alignment (matching same-`Diet` neighbors' velocity), cohesion (steering
/// toward their average position), and separation (avoiding crowding) —
/// classic boids, scoped to same-`Diet` neighbors so schools/flocks/herds
/// form along ecologically meaningful lines rather than mixed-species mobs.
///
/// ## 2. Why It Happens
/// The spec's Colonial/Social section asks for flocking/huddling as an
/// emergent group behavior. Phylon's architecture otherwise forbids
/// top-down behavioral scripts (locomotion is CTRNN-brain-driven muscle
/// actuation) — flocking is deliberately an **additive nudge to
/// `ParticleNode.force`**, not a replacement for brain control: an
/// organism's own evolved brain still drives its muscles every tick: this
/// system only adds a small physical tendency to move with the group on
/// top of that, the same way wind or current would, rather than seizing
/// control of the organism outright.
///
/// ## 3. How It Happens
/// Broad-phase neighbor lookup via `spatial::UniformGrid`, the same pattern
/// `foraging_system`/`disease_spread_system` already use. Pure computation
/// (reading a snapshot, writing nothing) is separated from the write-back
/// pass for the same reason `metabolism_system` splits its computation —
/// keeps the iteration order (and therefore floating-point summation order)
/// independent of `bevy_ecs`'s internal storage order.
pub fn flocking_system(
    config: Res<FlockingConfig>,
    mut query: Query<(Entity, &ecology::Diet, &mut physics::ParticleNode)>,
) {
    let snapshot: Vec<(Entity, ecology::Diet, Vec3, Vec3)> = query
        .iter()
        .map(|(e, d, n)| (e, d.clone(), n.position, n.velocity))
        .collect();

    if snapshot.is_empty() {
        return;
    }

    let mut grid = spatial::UniformGrid::new(config.radius.max(1.0)).unwrap();
    for (e, _, pos, _) in &snapshot {
        let _ = grid.insert(*e, *pos);
    }

    let mut forces: std::collections::HashMap<Entity, Vec3> =
        std::collections::HashMap::with_capacity(snapshot.len());

    for (e, diet, pos, _vel) in &snapshot {
        let mut avg_velocity = Vec3::ZERO;
        let mut avg_position = Vec3::ZERO;
        let mut separation = Vec3::ZERO;
        let mut neighbor_count = 0u32;

        for other in grid.query_radius(*pos, config.radius) {
            if other == *e {
                continue;
            }
            let Some((_, other_diet, other_pos, other_vel)) =
                snapshot.iter().find(|(oe, ..)| *oe == other)
            else {
                continue;
            };
            if other_diet != diet {
                continue;
            }

            avg_velocity += *other_vel;
            avg_position += *other_pos;
            neighbor_count += 1;

            let dist = pos.distance(*other_pos);
            if dist > 0.0001 && dist < config.separation_radius {
                separation += (*pos - *other_pos) / dist;
            }
        }

        if neighbor_count == 0 {
            continue;
        }

        let n = neighbor_count as f32;
        let alignment = (avg_velocity / n).normalize_or_zero();
        let cohesion = ((avg_position / n) - *pos).normalize_or_zero();
        let separation = separation.normalize_or_zero();

        let force = (alignment * config.alignment_weight
            + cohesion * config.cohesion_weight
            + separation * config.separation_weight)
            .clamp_length_max(config.max_force);

        forces.insert(*e, force);
    }

    for (e, _diet, mut node) in query.iter_mut() {
        if let Some(force) = forces.get(&e) {
            node.force += *force;
        }
    }
}

/// Global tunables for pack (cooperative) hunting.
#[derive(Resource, Debug, Clone)]
pub struct PackHuntingConfig {
    /// Radius within which a `Diet::Carnivore` without a locked target can
    /// adopt a nearby packmate's target.
    pub pack_radius: f32,
}

impl Default for PackHuntingConfig {
    fn default() -> Self {
        Self { pack_radius: 200.0 }
    }
}

/// # Pack Hunting System
///
/// ## 1. What Happens
/// Any `Diet::Carnivore` without a locked prey target that has a nearby
/// packmate (another Carnivore) *with* one adopts that same target,
/// converging the pack onto shared prey instead of hunting independently.
/// Every Carnivore with a locked target (whether self-acquired or adopted)
/// is set to [`behavior::BehaviorState::Hunting`] — the first system that
/// actually assigns that state; before this, it was a declared variant
/// nothing ever set.
///
/// ## 2. Why It Happens
/// `sensing::sensing_system` already gives each organism its own
/// `HeadVision::locked_target` independently. Cooperative/pack hunting per
/// the spec's Colonial/Social section means a pack acts on *shared*
/// information, not just proximity — adopting a packmate's lock is the
/// coordination step that turns independent hunters into a pack.
///
/// ## 3. How It Happens
/// `BehaviorState::Fleeing` (critical-ATP survival stress, set by
/// `behavior::physiological_state_update_system`) is never overridden —
/// starvation takes priority over joining a hunt.
pub fn pack_hunting_system(
    config: Res<PackHuntingConfig>,
    mut query: Query<(
        Entity,
        &ecology::Diet,
        &physics::ParticleNode,
        &mut sensing::HeadVision,
        Option<&mut behavior::BehaviorState>,
    )>,
) {
    let snapshot: Vec<(Entity, ecology::Diet, Vec3, Option<Entity>)> = query
        .iter()
        .map(|(e, d, n, v, _)| (e, d.clone(), n.position, v.locked_target))
        .collect();

    if snapshot.is_empty() {
        return;
    }

    let mut grid = spatial::UniformGrid::new(config.pack_radius.max(1.0)).unwrap();
    for (e, _, pos, _) in &snapshot {
        let _ = grid.insert(*e, *pos);
    }

    let mut adoptions: std::collections::HashMap<Entity, Entity> = std::collections::HashMap::new();
    for (e, diet, pos, locked) in &snapshot {
        if *diet != ecology::Diet::Carnivore || locked.is_some() {
            continue;
        }
        for other in grid.query_radius(*pos, config.pack_radius) {
            if other == *e {
                continue;
            }
            let Some((_, other_diet, _, other_locked)) =
                snapshot.iter().find(|(oe, ..)| *oe == other)
            else {
                continue;
            };
            if *other_diet == ecology::Diet::Carnivore {
                if let Some(target) = other_locked {
                    adoptions.insert(*e, *target);
                    break;
                }
            }
        }
    }

    for (e, diet, _node, mut vision, bstate_opt) in query.iter_mut() {
        if *diet != ecology::Diet::Carnivore {
            continue;
        }
        if let Some(&target) = adoptions.get(&e) {
            vision.locked_target = Some(target);
        }
        if vision.locked_target.is_some() {
            if let Some(mut bstate) = bstate_opt {
                if *bstate != behavior::BehaviorState::Fleeing {
                    *bstate = behavior::BehaviorState::Hunting;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;
    use common::Vec2;

    fn sample_vision() -> sensing::HeadVision {
        sensing::HeadVision {
            range: 250.0,
            fov: std::f32::consts::PI,
            last_forward: Vec2::X,
            self_occlusion_radius: 5.0,
            locked_target: None,
        }
    }

    #[test]
    fn flocking_pushes_isolated_organism_toward_same_diet_neighbors() {
        let mut world = World::new();
        world.insert_resource(FlockingConfig::default());

        let left = world
            .spawn((
                ecology::Diet::Herbivore,
                physics::ParticleNode::new(Vec3::new(0.0, 0.0, 0.0), 1.0, 0, 1),
            ))
            .id();
        let right = world
            .spawn((
                ecology::Diet::Herbivore,
                physics::ParticleNode::new(Vec3::new(50.0, 0.0, 0.0), 1.0, 0, 2),
            ))
            .id();

        world.run_system_once(flocking_system);

        // Cohesion should pull each toward the other, i.e. nonzero forces
        // pointing at each other along the shared axis.
        assert!(world.get::<physics::ParticleNode>(left).unwrap().force.x > 0.0);
        assert!(world.get::<physics::ParticleNode>(right).unwrap().force.x < 0.0);
    }

    #[test]
    fn flocking_ignores_different_diet_neighbors() {
        let mut world = World::new();
        world.insert_resource(FlockingConfig::default());

        world.spawn((
            ecology::Diet::Herbivore,
            physics::ParticleNode::new(Vec3::new(0.0, 0.0, 0.0), 1.0, 0, 1),
        ));
        world.spawn((
            ecology::Diet::Carnivore,
            physics::ParticleNode::new(Vec3::new(50.0, 0.0, 0.0), 1.0, 0, 2),
        ));

        world.run_system_once(flocking_system);

        let mut q = world.query::<&physics::ParticleNode>();
        for node in q.iter(&world) {
            assert_eq!(node.force, Vec3::ZERO);
        }
    }

    #[test]
    fn pack_hunting_adopts_nearby_packmates_locked_target() {
        let mut world = World::new();
        world.insert_resource(PackHuntingConfig::default());

        let prey = world.spawn(()).id();

        world.spawn((
            ecology::Diet::Carnivore,
            physics::ParticleNode::new(Vec3::new(0.0, 0.0, 0.0), 1.0, 0, 1),
            sensing::HeadVision {
                locked_target: Some(prey),
                ..sample_vision()
            },
            behavior::BehaviorState::Idle,
        ));
        let follower = world
            .spawn((
                ecology::Diet::Carnivore,
                physics::ParticleNode::new(Vec3::new(50.0, 0.0, 0.0), 1.0, 0, 2),
                sample_vision(),
                behavior::BehaviorState::Idle,
            ))
            .id();

        world.run_system_once(pack_hunting_system);

        assert_eq!(
            world
                .get::<sensing::HeadVision>(follower)
                .unwrap()
                .locked_target,
            Some(prey)
        );
        assert_eq!(
            *world.get::<behavior::BehaviorState>(follower).unwrap(),
            behavior::BehaviorState::Hunting
        );
    }

    #[test]
    fn pack_hunting_never_overrides_fleeing() {
        let mut world = World::new();
        world.insert_resource(PackHuntingConfig::default());

        let prey = world.spawn(()).id();
        world.spawn((
            ecology::Diet::Carnivore,
            physics::ParticleNode::new(Vec3::new(0.0, 0.0, 0.0), 1.0, 0, 1),
            sensing::HeadVision {
                locked_target: Some(prey),
                ..sample_vision()
            },
            behavior::BehaviorState::Idle,
        ));
        let fleeing = world
            .spawn((
                ecology::Diet::Carnivore,
                physics::ParticleNode::new(Vec3::new(50.0, 0.0, 0.0), 1.0, 0, 2),
                sample_vision(),
                behavior::BehaviorState::Fleeing,
            ))
            .id();

        world.run_system_once(pack_hunting_system);

        assert_eq!(
            *world.get::<behavior::BehaviorState>(fleeing).unwrap(),
            behavior::BehaviorState::Fleeing
        );
    }

    #[test]
    fn pack_hunting_ignores_non_carnivores() {
        let mut world = World::new();
        world.insert_resource(PackHuntingConfig::default());

        let prey = world.spawn(()).id();
        world.spawn((
            ecology::Diet::Carnivore,
            physics::ParticleNode::new(Vec3::new(0.0, 0.0, 0.0), 1.0, 0, 1),
            sensing::HeadVision {
                locked_target: Some(prey),
                ..sample_vision()
            },
            behavior::BehaviorState::Idle,
        ));
        let herbivore = world
            .spawn((
                ecology::Diet::Herbivore,
                physics::ParticleNode::new(Vec3::new(50.0, 0.0, 0.0), 1.0, 0, 2),
                sample_vision(),
                behavior::BehaviorState::Idle,
            ))
            .id();

        world.run_system_once(pack_hunting_system);

        assert_eq!(
            world
                .get::<sensing::HeadVision>(herbivore)
                .unwrap()
                .locked_target,
            None
        );
    }
}
