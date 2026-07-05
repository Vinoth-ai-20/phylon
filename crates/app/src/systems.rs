use bevy_ecs::prelude::*;
use common;
use ecology;
use genetics;
use metabolism;
use organisms;
use physics;
use reproduction;

struct SpawnOrganismCommand {
    parent_id: Option<bevy_ecs::entity::Entity>,
    genome: genetics::Genome,
    position: common::Vec2,
    diet: ecology::Diet,
    category: ecology::EcologicalCategory,
}

impl bevy_ecs::world::Command for SpawnOrganismCommand {
    fn apply(self, world: &mut bevy_ecs::world::World) {
        let (lineage_id, species_id, generation) = {
            if let Some(parent_id) = self.parent_id {
                if let Some(tracker) = world.get_resource::<evolution::LineageTracker>() {
                    if let Some(parent_record) =
                        tracker.get_record(common::EntityId(parent_id.to_bits()))
                    {
                        (
                            parent_record.lineage,
                            parent_record.species,
                            parent_record.generation + 1,
                        )
                    } else {
                        (evolution::LineageId(0), evolution::SpeciesId(0), 1)
                    }
                } else {
                    (evolution::LineageId(0), evolution::SpeciesId(0), 1)
                }
            } else {
                let mut tracker = world.get_resource_mut::<evolution::LineageTracker>();
                if let Some(ref mut t) = tracker {
                    (t.new_lineage_id(), evolution::SpeciesId(0), 0)
                } else {
                    (evolution::LineageId(0), evolution::SpeciesId(0), 0)
                }
            }
        };

        let entity = world.resource_scope::<common::SimRng, _>(|world, mut sim_rng| {
            organisms::spawn_organism(
                world,
                &self.genome,
                self.position,
                self.diet,
                self.category,
                generation as u32,
                0,
                &mut sim_rng.0,
            )
        });

        if let Some(mut tracker) = world.get_resource_mut::<evolution::LineageTracker>() {
            tracker.register_birth(
                common::EntityId(entity.to_bits()),
                self.parent_id.map(|p| common::EntityId(p.to_bits())),
                lineage_id,
                species_id,
                generation,
                0, // TODO: Get actual tick
            );
        }

        if generation > 0 && generation % 5 == 0 {
            if let Some(mut log) = world.get_resource_mut::<analytics::NarrationLog>() {
                log.push_event(
                    0, // TODO: tick
                    "Lineage",
                    format!(
                        "Lineage {} reached generation {}!",
                        lineage_id.0, generation
                    ),
                );
            }
        }
    }
}

pub fn process_births_system(
    mut commands: Commands,
    mut events: EventReader<reproduction::BirthRequest>,
) {
    for event in events.read() {
        commands.add(SpawnOrganismCommand {
            parent_id: event.parent_id,
            genome: event.genome.clone(),
            position: event.position,
            diet: event.diet.clone(),
            category: event.category.clone(),
        });
    }
}

pub fn process_narrative_events_system(
    mut hazard_events: EventReader<ecology::catastrophe::HazardSpawned>,
    mut log: ResMut<analytics::NarrationLog>,
) {
    for event in hazard_events.read() {
        log.push_event(
            0, // TODO: tick
            "Hazard",
            format!(
                "Toxic cloud emerged at ({:.1}, {:.1})",
                event.0.x, event.0.y
            ),
        );
    }
}

/// Traverses the physics spring network to completely remove organisms marked as Dead.
pub fn process_deaths_system(
    mut commands: bevy_ecs::prelude::Commands,
    dead_q: bevy_ecs::prelude::Query<
        (
            bevy_ecs::entity::Entity,
            &physics::ParticleNode,
            &metabolism::ChemicalEconomy,
            Option<&ecology::Eaten>,
        ),
        bevy_ecs::prelude::With<metabolism::Dead>,
    >,
    spring_q: bevy_ecs::prelude::Query<(bevy_ecs::entity::Entity, &physics::Spring)>,
    mut tracker: Option<bevy_ecs::prelude::ResMut<evolution::LineageTracker>>,
) {
    if dead_q.is_empty() {
        return;
    }

    let mut adj: std::collections::HashMap<
        bevy_ecs::entity::Entity,
        Vec<(bevy_ecs::entity::Entity, bevy_ecs::entity::Entity)>,
    > = std::collections::HashMap::new();

    for (s_entity, spring) in spring_q.iter() {
        adj.entry(spring.node_a)
            .or_default()
            .push((spring.node_b, s_entity));
        adj.entry(spring.node_b)
            .or_default()
            .push((spring.node_a, s_entity));
    }

    let mut nodes_to_despawn = std::collections::HashSet::new();
    let mut springs_to_despawn = std::collections::HashSet::new();

    for (head, node, chem, eaten) in dead_q.iter() {
        if nodes_to_despawn.contains(&head) {
            continue;
        }

        if let Some(ref mut t) = tracker {
            t.register_death(common::EntityId(head.to_bits()), 0, "Died".to_string());
            // TODO: Get actual tick and cause
        }

        // Spawn a corpse entity at the position of the dead organism, unless it was eaten whole
        if eaten.is_none() {
            commands.spawn(ecology::Corpse {
                position: node.position,
                energy_value: chem.max_glucose + chem.max_atp, // Corpse yields the organism's max potential energy
                decay_timer: 1800,                             // About 30 seconds at 60 FPS
                max_decay: 1800,
            });
        }

        let mut queue = std::collections::VecDeque::new();
        queue.push_back(head);
        nodes_to_despawn.insert(head);

        while let Some(curr) = queue.pop_front() {
            if let Some(neighbors) = adj.get(&curr) {
                for &(neighbor, s_entity) in neighbors {
                    springs_to_despawn.insert(s_entity);
                    if nodes_to_despawn.insert(neighbor) {
                        queue.push_back(neighbor);
                    }
                }
            }
        }
    }

    for n in nodes_to_despawn {
        if let Some(mut e) = commands.get_entity(n) {
            e.despawn();
        }
    }
    for s in springs_to_despawn {
        if let Some(mut e) = commands.get_entity(s) {
            e.despawn();
        }
    }
}
