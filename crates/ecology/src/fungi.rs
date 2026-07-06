use bevy_ecs::prelude::*;
use rand::Rng;

use crate::{Corpse, Diet, MineralPellet, ResourceSpatialGrids};

/// The on-contact eat radius `foraging_system` already uses for
/// [`Diet::Decomposer`] — this module's remote siphon only applies beyond
/// it, so a corpse is never double-fed from both systems in the same tick.
const CONTACT_EAT_RADIUS: f32 = 20.0;

/// Global tunables for the fungal nutrient-redistribution network.
#[derive(Resource, Debug, Clone)]
pub struct FungalNetworkConfig {
    /// Maximum range a decomposer's mycelial network can reach beyond
    /// `CONTACT_EAT_RADIUS`.
    pub siphon_radius: f32,
    /// Fraction of a corpse's *remaining* energy drawn per tick at zero
    /// distance; falls off linearly to `0` at `siphon_radius`.
    pub siphon_rate: f32,
    /// Fraction of siphoned energy that reaches the decomposer directly;
    /// the rest is redistributed as fresh soil nutrients elsewhere in the
    /// network's reach (see [`fungal_network_system`]'s doc comment).
    pub decomposer_share: f32,
}

impl Default for FungalNetworkConfig {
    fn default() -> Self {
        Self {
            siphon_radius: 300.0,
            siphon_rate: 0.01,
            decomposer_share: 0.6,
        }
    }
}

/// # Fungal Nutrient Network System
///
/// ## 1. What Happens
/// Every [`Diet::Decomposer`] organism remotely siphons a small fraction of
/// every corpse's remaining energy within `siphon_radius` (beyond
/// `foraging_system`'s on-contact eat radius) each tick. Most of what's
/// siphoned feeds the decomposer directly; the rest respawns as a fresh
/// [`MineralPellet`] at a random point within the network's reach.
///
/// ## 2. Why It Happens
/// `foraging_system`'s existing Decomposer behavior is eat-on-contact: a
/// decomposer standing on a corpse gets its full value instantly. Real
/// fungal mycelial networks work differently — they extract nutrients from
/// organic matter at one point and transport them through the network to
/// enrich soil elsewhere, often meters from the decomposition site. This
/// system is the distance half of that picture that eat-on-contact can't
/// express: **redistribution**, not just remote feeding — most of a
/// decomposer's own energy still comes from direct contact (unaffected by
/// this system); this module is what makes fungal decomposition actually
/// spread nutrients outward instead of concentrating them wherever the
/// decomposer happens to be standing.
///
/// ## 3. How It Happens
/// Falloff is linear in distance: `siphoned = corpse.energy_value *
/// siphon_rate * (1 - dist / siphon_radius)`. A corpse drained below a
/// small residue is despawned (mirroring `corpse_decay_system`'s full-decay
/// cleanup). Uses [`common::SimRng`] for the redistribution point, never an
/// unseeded RNG.
pub fn fungal_network_system(
    mut commands: Commands,
    config: Res<FungalNetworkConfig>,
    mut sim_rng: ResMut<common::SimRng>,
    mut organism_query: Query<(
        &Diet,
        &mut metabolism::ChemicalEconomy,
        &physics::ParticleNode,
    )>,
    mut corpse_query: Query<(Entity, &mut Corpse)>,
    resource_grids: Res<ResourceSpatialGrids>,
) {
    for (diet, mut chem, node) in organism_query.iter_mut() {
        if *diet != Diet::Decomposer || chem.atp <= 0.0 {
            continue;
        }

        for corpse_entity in resource_grids
            .corpses
            .query_radius(node.position, config.siphon_radius)
        {
            let Ok((_, mut corpse)) = corpse_query.get_mut(corpse_entity) else {
                continue;
            };
            let dist = node.position.distance(corpse.position);
            if dist <= CONTACT_EAT_RADIUS || dist > config.siphon_radius {
                continue;
            }

            let falloff = 1.0 - (dist / config.siphon_radius);
            let siphoned = corpse.energy_value * config.siphon_rate * falloff;
            if siphoned <= 0.0 {
                continue;
            }
            corpse.energy_value -= siphoned;

            let to_decomposer = siphoned * config.decomposer_share;
            let to_redistribute = siphoned - to_decomposer;

            chem.glucose = (chem.glucose + to_decomposer).min(chem.max_glucose);

            if to_redistribute > 0.01 {
                let angle = sim_rng.gen_range(0.0..std::f32::consts::TAU);
                let dist_out = sim_rng.gen_range(0.0..config.siphon_radius);
                let offset = common::Vec2::new(angle.cos(), angle.sin()) * dist_out;
                commands.spawn(MineralPellet {
                    position: node.position + offset,
                    energy_value: to_redistribute,
                });
            }

            if corpse.energy_value <= 0.1 {
                commands.entity(corpse_entity).despawn();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;

    fn sample_chem() -> metabolism::ChemicalEconomy {
        metabolism::ChemicalEconomy {
            glucose: 0.0,
            o2: 0.0,
            co2: 0.0,
            atp: 50.0,
            max_glucose: 1_000_000.0,
            max_o2: 0.0,
            max_co2: 0.0,
            max_atp: 100.0,
        }
    }

    #[test]
    fn siphons_remote_corpse_energy_into_decomposer_glucose() {
        let mut world = World::new();
        world.insert_resource(FungalNetworkConfig::default());
        world.insert_resource(common::SimRng::from_seed(1));

        let corpse_pos = common::Vec2::new(100.0, 0.0);
        let corpse = world
            .spawn(Corpse {
                position: corpse_pos,
                energy_value: 1000.0,
                decay_timer: 1000,
                max_decay: 1000,
            })
            .id();

        let mut grids = ResourceSpatialGrids::new(50.0);
        let _ = grids.corpses.insert(corpse, corpse_pos);
        world.insert_resource(grids);

        world.spawn((
            Diet::Decomposer,
            sample_chem(),
            physics::ParticleNode::new(common::Vec2::new(0.0, 0.0), 1.0, 0, 1),
        ));

        world.run_system_once(fungal_network_system);

        let mut q = world.query::<(&Diet, &metabolism::ChemicalEconomy)>();
        let (_, chem) = q.iter(&world).next().unwrap();
        assert!(chem.glucose > 0.0);

        let remaining_corpse = world.get::<Corpse>(corpse).unwrap();
        assert!(remaining_corpse.energy_value < 1000.0);
    }

    #[test]
    fn ignores_corpses_within_contact_radius_already_handled_by_foraging() {
        let corpse_pos = common::Vec2::new(5.0, 0.0); // well within CONTACT_EAT_RADIUS
        let mut world = World::new();
        world.insert_resource(FungalNetworkConfig::default());
        world.insert_resource(common::SimRng::from_seed(1));

        let corpse = world
            .spawn(Corpse {
                position: corpse_pos,
                energy_value: 1000.0,
                decay_timer: 1000,
                max_decay: 1000,
            })
            .id();

        let mut grids = ResourceSpatialGrids::new(50.0);
        let _ = grids.corpses.insert(corpse, corpse_pos);
        world.insert_resource(grids);

        world.spawn((
            Diet::Decomposer,
            sample_chem(),
            physics::ParticleNode::new(common::Vec2::new(0.0, 0.0), 1.0, 0, 1),
        ));

        world.run_system_once(fungal_network_system);

        let remaining_corpse = world.get::<Corpse>(corpse).unwrap();
        assert_eq!(remaining_corpse.energy_value, 1000.0);
    }

    #[test]
    fn ignores_corpses_beyond_siphon_radius() {
        let mut world = World::new();
        world.insert_resource(FungalNetworkConfig {
            siphon_radius: 50.0,
            ..FungalNetworkConfig::default()
        });
        world.insert_resource(common::SimRng::from_seed(1));

        let corpse_pos = common::Vec2::new(500.0, 0.0);
        let corpse = world
            .spawn(Corpse {
                position: corpse_pos,
                energy_value: 1000.0,
                decay_timer: 1000,
                max_decay: 1000,
            })
            .id();

        let mut grids = ResourceSpatialGrids::new(50.0);
        let _ = grids.corpses.insert(corpse, corpse_pos);
        world.insert_resource(grids);

        world.spawn((
            Diet::Decomposer,
            sample_chem(),
            physics::ParticleNode::new(common::Vec2::new(0.0, 0.0), 1.0, 0, 1),
        ));

        world.run_system_once(fungal_network_system);

        let remaining_corpse = world.get::<Corpse>(corpse).unwrap();
        assert_eq!(remaining_corpse.energy_value, 1000.0);
    }
}
