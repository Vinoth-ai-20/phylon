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
    /// When true and `parent_id` is present, connects the new organism's
    /// head node to the parent's head node with a physical spring after
    /// spawning — colonial fragmentation/budding (see
    /// `reproduction::BirthRequest::is_budding`).
    is_budding: bool,
}

impl bevy_ecs::world::Command for SpawnOrganismCommand {
    fn apply(self, world: &mut bevy_ecs::world::World) {
        let (lineage_id, generation) = {
            if let Some(parent_id) = self.parent_id {
                if let Some(tracker) = world.get_resource::<evolution::LineageTracker>() {
                    if let Some(parent_record) =
                        tracker.get_record(common::EntityId(parent_id.to_bits()))
                    {
                        (parent_record.lineage, parent_record.generation + 1)
                    } else {
                        (evolution::LineageId(0), 1)
                    }
                } else {
                    (evolution::LineageId(0), 1)
                }
            } else {
                let mut tracker = world.get_resource_mut::<evolution::LineageTracker>();
                if let Some(ref mut t) = tracker {
                    (t.new_lineage_id(), 0)
                } else {
                    (evolution::LineageId(0), 0)
                }
            }
        };

        // Classified fresh for every organism (not inherited from the
        // parent) — a child's genome can drift far enough from its
        // parent's to found a new species, which is exactly the
        // genetic-distance clustering `SpeciesRegistry` exists to detect.
        // See `evolution::SpeciesRegistry`'s doc comment for the algorithm.
        let species_id =
            if let Some(mut registry) = world.get_resource_mut::<evolution::SpeciesRegistry>() {
                registry.classify(&self.genome)
            } else {
                evolution::SpeciesId(0)
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

        // Colonial fragmentation/budding: physically tether the new
        // organism's head node to its parent's, forming a growing colony
        // instead of an independent dispersed offspring. See
        // `reproduction::BirthRequest::is_budding`'s doc comment.
        if self.is_budding {
            if let Some(parent_id) = self.parent_id {
                if world.get_entity(parent_id).is_some() {
                    world.spawn(physics::Spring {
                        node_a: parent_id,
                        node_b: entity,
                        constraint_type: physics::ConstraintType::Elastic,
                        rest_length: 20.0,
                        base_length: 20.0,
                        stiffness: 10.0,
                        damping: 0.5,
                        actuation_amplitude: 0.0,
                        actuation_phase: 0.0,
                        // Colony links break more easily than intra-body
                        // bones (`2.0` elsewhere) — a colony can still
                        // fragment further under strain.
                        breaking_strain: 1.5,
                        is_fin: 0,
                    });
                }
            }
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

        // Phase 4, P4-E1: the first real `events::PhylonEvent` producer for
        // births/reproduction — mirrors the death-side emission in
        // `process_deaths_system`. `Command::apply` runs directly against
        // `&mut World`, so events are sent via `World::send_event` rather
        // than an `EventWriter` system param.
        let tick = common::Tick(
            world
                .get_resource::<metabolism::GlobalAtmosphere>()
                .map_or(0, |a| a.ticks),
        );
        world.send_event(events::PhylonEvent::OrganismBorn {
            id: common::EntityId(entity.to_bits()),
            tick,
        });
        if let Some(parent_id) = self.parent_id {
            world.send_event(events::PhylonEvent::ReproductionEvent {
                parent: common::EntityId(parent_id.to_bits()),
                child: common::EntityId(entity.to_bits()),
                tick,
            });
        }

        // Phase 4, P4-V1: reproduction is one of this milestone's individual
        // interaction VFX triggers — rendered by `ui::render::render_timed_effects`.
        if let Some(mut timed_effects) = world.get_resource_mut::<events::TimedEffects>() {
            timed_effects.spawn(
                self.position,
                events::TimedEffectKind::FloatingText {
                    text: "Born!".to_string(),
                    color: [0.4, 0.8, 0.5],
                },
                tick.0,
                BIRTH_EFFECT_DURATION_TICKS,
            );
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
            is_budding: event.is_budding,
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

/// # Interaction Event Log System
///
/// ## 1. What Happens
/// The first real consumer of `events::PhylonEvent` (Phase 4, P4-E1) — reads
/// every event published this tick and logs the notable ones (predation
/// deaths) into `analytics::NarrationLog`.
///
/// ## 2. Why It Happens
/// `PhylonEvent` (`crates/events`) was fully designed but never wired into
/// the running app — nothing published or consumed a single event before
/// this milestone. This system proves the wiring works end-to-end: a real
/// event, published by `process_deaths_system`, drained and acted on here.
///
/// ## 3. How It Happens
/// Only `OrganismDied { cause: Predation, .. }` is logged — births and
/// ordinary (non-predation) deaths are common enough that logging every one
/// would flood `NarrationLog` (which already has its own, separate
/// generation-milestone logging for births; see `SpawnOrganismCommand::apply`).
pub fn interaction_event_log_system(
    mut phylon_events: EventReader<events::PhylonEvent>,
    mut log: ResMut<analytics::NarrationLog>,
) {
    for event in phylon_events.read() {
        if let events::PhylonEvent::OrganismDied {
            id,
            cause: events::DeathCause::Predation,
            tick,
        } = event
        {
            log.push_event(tick.0, "Predation", format!("Organism {} was eaten", id.0));
        }
    }
}

/// Expires every `events::TimedEffects` entry whose duration has elapsed —
/// the per-tick half of the P4-E1 timed-effects framework (see
/// `events::TimedEffects::expire`'s doc comment for why expiry is
/// tick-based, not wall-clock).
pub fn expire_timed_effects_system(
    mut effects: ResMut<events::TimedEffects>,
    atmosphere: Res<metabolism::GlobalAtmosphere>,
) {
    effects.expire(atmosphere.ticks);
}

/// Ticks a P4-E1 [`events::TimedEffectKind::FloatingText`] stays active for
/// after being spawned — not biologically tuned, same placeholder status as
/// every other Phase 4 rate/duration constant introduced so far.
const DEATH_EFFECT_DURATION_TICKS: u64 = 90; // ~1.5s at 60Hz

/// Same as [`DEATH_EFFECT_DURATION_TICKS`], for the "Born!" effect (Phase 4,
/// P4-V1).
const BIRTH_EFFECT_DURATION_TICKS: u64 = 90;

/// Traverses the physics spring network to completely remove organisms marked as Dead.
#[allow(clippy::type_complexity)]
pub fn process_deaths_system(
    mut commands: bevy_ecs::prelude::Commands,
    dead_q: bevy_ecs::prelude::Query<
        (
            bevy_ecs::entity::Entity,
            &physics::ParticleNode,
            &metabolism::ChemicalEconomy,
            &metabolism::Age,
            Option<&ecology::Eaten>,
            Option<&ecology::disease::Infection>,
        ),
        bevy_ecs::prelude::With<metabolism::Dead>,
    >,
    spring_q: bevy_ecs::prelude::Query<(bevy_ecs::entity::Entity, &physics::Spring)>,
    mut tracker: Option<bevy_ecs::prelude::ResMut<evolution::LineageTracker>>,
    mut phylon_events: bevy_ecs::prelude::EventWriter<events::PhylonEvent>,
    mut timed_effects: bevy_ecs::prelude::ResMut<events::TimedEffects>,
    atmosphere: bevy_ecs::prelude::Res<metabolism::GlobalAtmosphere>,
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

    for (head, node, chem, age, eaten, infection) in dead_q.iter() {
        if nodes_to_despawn.contains(&head) {
            continue;
        }

        if let Some(ref mut t) = tracker {
            t.register_death(common::EntityId(head.to_bits()), 0, "Died".to_string());
            // TODO: Get actual tick
        }

        // Phase 4, P4-E1/P4-L2: true cause-of-death tracking. Every death
        // that reaches this system was triggered by
        // `metabolism::compute_metabolism`'s `should_die = atp <= 0.0 ||
        // age_ticks >= max_lifespan` — except predation, which kills
        // directly regardless of ATP/age. So for a non-eaten death, the two
        // conditions that guarantee `should_die` give an honest cause
        // hierarchy: predation (a direct kill, checked first) outranks
        // senescence (age alone is sufficient to have caused this death,
        // regardless of ATP), which outranks disease (an active infection
        // was draining this organism's ATP, the same currency starvation
        // depletes — see `ecology::disease_progression_system`), which
        // outranks a plain starvation fallback (must be the case if none of
        // the above applied, since `should_die` guarantees ATP was
        // depleted). `Unknown` is kept only as a defensive fallback for a
        // future death path that doesn't fit this hierarchy — it should be
        // unreachable for any death `compute_metabolism` itself triggers.
        let cause = if eaten.is_some() {
            events::DeathCause::Predation
        } else if age.ticks >= age.max_lifespan {
            events::DeathCause::Senescence
        } else if matches!(
            infection.map(|i| i.state),
            Some(ecology::disease::InfectionState::Infectious)
        ) {
            events::DeathCause::Disease
        } else if chem.atp <= 0.0 {
            events::DeathCause::Starvation
        } else {
            events::DeathCause::Unknown
        };
        phylon_events.send(events::PhylonEvent::OrganismDied {
            id: common::EntityId(head.to_bits()),
            cause,
            tick: common::Tick(atmosphere.ticks),
        });

        // A predation death is the one trigger this milestone demonstrates
        // the new timed-effects framework with — proving `TimedEffects`
        // works end-to-end against a real event, without yet building the
        // rendering that would actually draw it (out of scope for P4-E1;
        // see `events::TimedEffects`'s doc comment).
        if eaten.is_some() {
            timed_effects.spawn(
                node.position,
                events::TimedEffectKind::FloatingText {
                    text: "Eaten!".to_string(),
                    color: [0.8, 0.2, 0.2],
                },
                atmosphere.ticks,
                DEATH_EFFECT_DURATION_TICKS,
            );
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;

    #[derive(Resource, Default)]
    struct Captured(Vec<events::PhylonEvent>);

    fn capture(mut reader: EventReader<events::PhylonEvent>, mut captured: ResMut<Captured>) {
        for event in reader.read() {
            captured.0.push(event.clone());
        }
    }

    fn base_world() -> World {
        let mut world = World::new();
        world.insert_resource(bevy_ecs::event::Events::<events::PhylonEvent>::default());
        world.insert_resource(events::TimedEffects::default());
        world.insert_resource(metabolism::GlobalAtmosphere::default());
        world.insert_resource(Captured::default());
        world
    }

    fn spawn_dead(
        world: &mut World,
        age_ticks: u64,
        max_lifespan: u64,
        atp: f32,
        eaten: bool,
        infectious: bool,
    ) -> Entity {
        let mut entity = world.spawn((
            physics::ParticleNode::new(common::Vec2::new(0.0, 0.0), 1.0, 0, 0),
            metabolism::ChemicalEconomy {
                glucose: 0.0,
                o2: 0.0,
                co2: 0.0,
                atp,
                max_glucose: 100.0,
                max_o2: 100.0,
                max_co2: 100.0,
                max_atp: 100.0,
            },
            metabolism::Age {
                ticks: age_ticks,
                max_lifespan,
            },
            metabolism::Dead,
        ));
        if eaten {
            entity.insert(ecology::Eaten);
        }
        if infectious {
            entity.insert(ecology::disease::Infection {
                state: ecology::disease::InfectionState::Infectious,
                ticks_in_state: 0,
                virulence: 1.0,
                transmissibility: 0.1,
            });
        }
        entity.id()
    }

    fn run_and_capture(world: &mut World) -> Vec<events::PhylonEvent> {
        world.run_system_once(process_deaths_system);
        world.run_system_once(capture);
        world.resource::<Captured>().0.clone()
    }

    #[test]
    fn senescence_outranks_starvation() {
        // Age past max_lifespan AND ATP depleted — should_die's age
        // condition alone is sufficient, so this must report Senescence,
        // not Starvation.
        let mut world = base_world();
        spawn_dead(&mut world, 1000, 1000, 0.0, false, false);
        let events = run_and_capture(&mut world);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            events::PhylonEvent::OrganismDied {
                cause: events::DeathCause::Senescence,
                ..
            }
        ));
    }

    #[test]
    fn disease_outranks_plain_starvation() {
        let mut world = base_world();
        spawn_dead(&mut world, 0, 1000, 0.0, false, true);
        let events = run_and_capture(&mut world);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            events::PhylonEvent::OrganismDied {
                cause: events::DeathCause::Disease,
                ..
            }
        ));
    }

    #[test]
    fn plain_starvation_is_the_fallback() {
        let mut world = base_world();
        spawn_dead(&mut world, 0, 1000, 0.0, false, false);
        let events = run_and_capture(&mut world);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            events::PhylonEvent::OrganismDied {
                cause: events::DeathCause::Starvation,
                ..
            }
        ));
    }

    #[test]
    fn predation_outranks_every_other_cause() {
        // Eaten, AND old, AND infectious, AND starved — predation must
        // still win, since it's a direct kill independent of any of those
        // conditions.
        let mut world = base_world();
        spawn_dead(&mut world, 1000, 1000, 0.0, true, true);
        let events = run_and_capture(&mut world);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            events::PhylonEvent::OrganismDied {
                cause: events::DeathCause::Predation,
                ..
            }
        ));
    }
}
