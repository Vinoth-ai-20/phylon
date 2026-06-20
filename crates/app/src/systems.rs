use bevy_ecs::prelude::*;
use common;
use ecology;
use genetics;
use metabolism;
use organisms;
use physics;
use reproduction;

struct SpawnOrganismCommand {
    genome: genetics::Genome,
    position: common::Vec2,
    diet: ecology::Diet,
    category: ecology::EcologicalCategory,
}

impl bevy_ecs::world::Command for SpawnOrganismCommand {
    fn apply(self, world: &mut bevy_ecs::world::World) {
        organisms::spawn_organism(
            world,
            &self.genome,
            self.position,
            self.diet,
            self.category,
            0,
            0,
        );
    }
}

pub fn process_births_system(
    mut commands: Commands,
    mut events: EventReader<reproduction::BirthRequest>,
) {
    for event in events.read() {
        commands.add(SpawnOrganismCommand {
            genome: event.genome.clone(),
            position: event.position,
            diet: event.diet.clone(),
            category: event.category.clone(),
        });
    }
}

/// Traverses the physics spring network to completely remove organisms marked as Dead.
pub fn process_deaths_system(
    mut commands: bevy_ecs::prelude::Commands,
    dead_q: bevy_ecs::prelude::Query<
        (
            bevy_ecs::entity::Entity,
            &physics::ParticleNode,
            &metabolism::Energy,
        ),
        bevy_ecs::prelude::With<metabolism::Dead>,
    >,
    spring_q: bevy_ecs::prelude::Query<(bevy_ecs::entity::Entity, &physics::Spring)>,
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

    for (head, node, energy) in dead_q.iter() {
        if nodes_to_despawn.contains(&head) {
            continue;
        }

        // Spawn a corpse entity at the position of the dead organism
        commands.spawn(ecology::Corpse {
            position: node.position,
            energy_value: energy.max, // Corpse yields the organism's max potential energy
            decay_timer: 1800,        // About 30 seconds at 60 FPS
            max_decay: 1800,
        });

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
        commands.entity(n).despawn();
    }
    for s in springs_to_despawn {
        commands.entity(s).despawn();
    }
}
