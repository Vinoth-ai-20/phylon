//! Quorum sensing / biofilm density-scaling aggregation: densely clustered
//! organisms of the same diet gain a physical consequence (reduced
//! hydration loss) from clustering, on top of whatever emergent
//! quorum-*sensing* behavior evolution finds via the ambient signal field.

use bevy_ecs::prelude::*;
use common::Vec3;

/// Global tunables for biofilm density protection.
#[derive(Resource, Debug, Clone)]
pub struct BiofilmConfig {
    /// Radius within which same-`Diet` neighbors count toward cluster density.
    pub cluster_radius: f32,
    /// Minimum same-`Diet` neighbor count within `cluster_radius` for an
    /// organism to be considered part of a biofilm.
    pub min_neighbors_for_biofilm: usize,
    /// The `Hydration::loss_rate` every organism spawns with (see
    /// `organisms::spawning::spawn_organism`) — the baseline this system
    /// scales from every tick, rather than progressively shrinking whatever
    /// value is currently stored (which would compound indefinitely).
    pub baseline_loss_rate: f32,
    /// Multiplier applied to `baseline_loss_rate` while part of a biofilm
    /// (`< 1.0` — the extracellular matrix slows water loss).
    pub protection_factor: f32,
}

impl Default for BiofilmConfig {
    fn default() -> Self {
        Self {
            cluster_radius: 40.0,
            min_neighbors_for_biofilm: 3,
            baseline_loss_rate: 0.0001,
            protection_factor: 0.4,
        }
    }
}

/// # Biofilm Aggregation System
///
/// ## 1. What Happens
/// Every tick, organisms with at least `min_neighbors_for_biofilm` same-`Diet`
/// neighbors within `cluster_radius` get their `Hydration::loss_rate` scaled
/// down by `protection_factor`; isolated organisms are reset to the
/// unscaled baseline. Quorum sensing/biofilm density scaling, per the
/// spec's Microbial/Cellular section.
///
/// ## 2. Why It Happens
/// `sensing_system` already feeds organisms the ambient signal field as a
/// brain input, so evolution can already learn quorum-*sensing* behaviors
/// on its own (an emergent solution, not a scripted one — consistent with
/// this codebase's "no top-down behavioral scripts" architecture). What's
/// missing is the density-scaling *consequence* the spec asks for:
/// extracellular matrix secretion physically protecting a dense cluster
/// from desiccation. This system is that consequence, not a replacement for
/// brain-driven aggregation behavior.
///
/// ## 3. How It Happens
/// `loss_rate` is always recomputed from `baseline_loss_rate`, never
/// multiplied against its own previous value — every organism spawns with
/// the same fixed baseline (see `spawning::spawn_organism`), so this stays
/// correct without needing a second stored-baseline field per organism.
/// Broad-phase neighbor lookup via `spatial::UniformGrid`, matching this
/// crate's other social systems (see `social::flocking_system`).
pub fn biofilm_system(
    config: Res<BiofilmConfig>,
    mut query: Query<(
        Entity,
        &ecology::Diet,
        &physics::ParticleNode,
        &mut metabolism::Hydration,
    )>,
) {
    let snapshot: Vec<(Entity, ecology::Diet, Vec3)> = query
        .iter()
        .map(|(e, d, n, _)| (e, d.clone(), n.position))
        .collect();

    if snapshot.is_empty() {
        return;
    }

    let mut grid = spatial::UniformGrid::new(config.cluster_radius.max(1.0)).unwrap();
    for (e, _, pos) in &snapshot {
        let _ = grid.insert(*e, *pos);
    }

    let mut neighbor_counts: std::collections::HashMap<Entity, usize> =
        std::collections::HashMap::with_capacity(snapshot.len());
    for (e, diet, pos) in &snapshot {
        let count = grid
            .query_radius(*pos, config.cluster_radius)
            .into_iter()
            .filter(|&other| {
                other != *e
                    && snapshot
                        .iter()
                        .any(|(oe, od, _)| *oe == other && od == diet)
            })
            .count();
        neighbor_counts.insert(*e, count);
    }

    for (e, _diet, _node, mut hydration) in query.iter_mut() {
        let count = neighbor_counts.get(&e).copied().unwrap_or(0);
        hydration.loss_rate = if count >= config.min_neighbors_for_biofilm {
            config.baseline_loss_rate * config.protection_factor
        } else {
            config.baseline_loss_rate
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;

    fn sample_hydration() -> metabolism::Hydration {
        metabolism::Hydration {
            level: 1.0,
            loss_rate: 0.0001,
        }
    }

    #[test]
    fn dense_cluster_gets_protection_factor_applied() {
        let mut world = World::new();
        world.insert_resource(BiofilmConfig {
            min_neighbors_for_biofilm: 2,
            ..BiofilmConfig::default()
        });

        let center = world
            .spawn((
                ecology::Diet::Decomposer,
                physics::ParticleNode::new(Vec3::new(0.0, 0.0, 0.0), 1.0, 0, 1),
                sample_hydration(),
            ))
            .id();
        for i in 0..2 {
            world.spawn((
                ecology::Diet::Decomposer,
                physics::ParticleNode::new(Vec3::new(5.0 + i as f32, 0.0, 0.0), 1.0, 0, i + 2),
                sample_hydration(),
            ));
        }

        world.run_system_once(biofilm_system);

        let config = BiofilmConfig::default();
        let expected = config.baseline_loss_rate * config.protection_factor;
        assert_eq!(
            world
                .get::<metabolism::Hydration>(center)
                .unwrap()
                .loss_rate,
            expected
        );
    }

    #[test]
    fn isolated_organism_keeps_baseline_loss_rate() {
        let mut world = World::new();
        world.insert_resource(BiofilmConfig::default());

        let lone = world
            .spawn((
                ecology::Diet::Decomposer,
                physics::ParticleNode::new(Vec3::new(0.0, 0.0, 0.0), 1.0, 0, 1),
                sample_hydration(),
            ))
            .id();

        world.run_system_once(biofilm_system);

        let config = BiofilmConfig::default();
        assert_eq!(
            world.get::<metabolism::Hydration>(lone).unwrap().loss_rate,
            config.baseline_loss_rate
        );
    }

    #[test]
    fn different_diet_neighbors_do_not_count_toward_cluster() {
        let mut world = World::new();
        world.insert_resource(BiofilmConfig {
            min_neighbors_for_biofilm: 1,
            ..BiofilmConfig::default()
        });

        let lone = world
            .spawn((
                ecology::Diet::Decomposer,
                physics::ParticleNode::new(Vec3::new(0.0, 0.0, 0.0), 1.0, 0, 1),
                sample_hydration(),
            ))
            .id();
        world.spawn((
            ecology::Diet::Herbivore,
            physics::ParticleNode::new(Vec3::new(5.0, 0.0, 0.0), 1.0, 0, 2),
            sample_hydration(),
        ));

        world.run_system_once(biofilm_system);

        let config = BiofilmConfig::default();
        assert_eq!(
            world.get::<metabolism::Hydration>(lone).unwrap().loss_rate,
            config.baseline_loss_rate
        );
    }

    #[test]
    fn loss_rate_resets_when_no_longer_clustered() {
        // Verifies the "always recompute from baseline" design decision:
        // scaling down then back up must return to the exact baseline, not
        // a compounded value.
        let mut world = World::new();
        world.insert_resource(BiofilmConfig {
            min_neighbors_for_biofilm: 1,
            ..BiofilmConfig::default()
        });

        let e1 = world
            .spawn((
                ecology::Diet::Decomposer,
                physics::ParticleNode::new(Vec3::new(0.0, 0.0, 0.0), 1.0, 0, 1),
                sample_hydration(),
            ))
            .id();
        let e2 = world
            .spawn((
                ecology::Diet::Decomposer,
                physics::ParticleNode::new(Vec3::new(5.0, 0.0, 0.0), 1.0, 0, 2),
                sample_hydration(),
            ))
            .id();

        world.run_system_once(biofilm_system);
        let config = BiofilmConfig::default();
        assert_eq!(
            world.get::<metabolism::Hydration>(e1).unwrap().loss_rate,
            config.baseline_loss_rate * config.protection_factor
        );

        // Move e2 far away and re-run.
        world.get_mut::<physics::ParticleNode>(e2).unwrap().position =
            Vec3::new(10_000.0, 0.0, 0.0);
        world.run_system_once(biofilm_system);

        assert_eq!(
            world.get::<metabolism::Hydration>(e1).unwrap().loss_rate,
            config.baseline_loss_rate
        );
    }
}
