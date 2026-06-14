//! The fixed-tick deterministic execution scheduler for Phylon.

use common::Tick;
use std::time::{Duration, Instant};
use tracing::{span, Level};

/// The canonical order of systems executing within a single tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SystemOrder {
    PrePhysics,
    Physics,
    Diffusion,
    Sensing,
    Brain,
    Behavior,
    Metabolism,
    Ecology,
    Reproduction,
    PostTick,
    Analytics,
}

/// The main orchestrator for tick advancement.
pub struct SimulationScheduler {
    pub current_tick: Tick,
    pub tick_rate: u32,
    tick_duration: Duration,
    last_tick_end: Instant,
}

impl SimulationScheduler {
    pub fn new(tick_rate: u32) -> Self {
        Self {
            current_tick: Tick(0),
            tick_rate,
            tick_duration: Duration::from_secs_f64(1.0 / tick_rate as f64),
            last_tick_end: Instant::now(),
        }
    }

    /// Advance the simulation by exactly one tick without maintaining real-time rate (fast forward).
    pub fn tick(&mut self, world: &mut world::PhylonWorld) {
        puffin::profile_function!();
        self.current_tick.0 += 1;
        let tick_span = span!(Level::TRACE, "tick", tick = self.current_tick.0);
        let _enter = tick_span.enter();

        self.run_phase(SystemOrder::PrePhysics, world);
        self.run_phase(SystemOrder::Physics, world);
        self.run_phase(SystemOrder::Diffusion, world);
        self.run_phase(SystemOrder::Sensing, world);
        self.run_phase(SystemOrder::Brain, world);
        self.run_phase(SystemOrder::Behavior, world);
        self.run_phase(SystemOrder::Metabolism, world);
        self.run_phase(SystemOrder::Ecology, world);
        self.run_phase(SystemOrder::Reproduction, world);
        self.run_phase(SystemOrder::PostTick, world);
        self.run_phase(SystemOrder::Analytics, world);
    }

    /// Advance the simulation by exactly one tick.
    pub fn tick_loop(&mut self, world: &mut world::PhylonWorld) {
        puffin::profile_function!();

        // Maintain fixed tick rate
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_tick_end);
        if elapsed < self.tick_duration {
            std::thread::sleep(self.tick_duration - elapsed);
        }
        self.last_tick_end = Instant::now();

        self.tick(world);
    }

    /// Run a specific phase in the system order.
    fn run_phase(&self, phase: SystemOrder, world: &mut world::PhylonWorld) {
        let phase_name = match phase {
            SystemOrder::PrePhysics => "PrePhysics",
            SystemOrder::Physics => "Physics",
            SystemOrder::Diffusion => "Diffusion",
            SystemOrder::Sensing => "Sensing",
            SystemOrder::Brain => "Brain",
            SystemOrder::Behavior => "Behavior",
            SystemOrder::Metabolism => "Metabolism",
            SystemOrder::Ecology => "Ecology",
            SystemOrder::Reproduction => "Reproduction",
            SystemOrder::PostTick => "PostTick",
            SystemOrder::Analytics => "Analytics",
        };
        puffin::profile_scope!(phase_name);
        let _phase_span = span!(Level::TRACE, "phase", name = %phase_name).entered();

        if phase == SystemOrder::Sensing {
            let mut foods = std::collections::HashMap::new();
            for (entity, (pos, _fp)) in world
                .ecs
                .query_mut::<(&physics::Position, &organisms::FoodPellet)>()
            {
                foods.insert(
                    common::EntityId(entity.to_bits().get()),
                    (entity, pos.0, 10.0, false),
                );
            }
            sensing::process_sensing(
                &world.ecs,
                &world.spatial_index,
                &world.field_grid,
                world.grid_width,
                world.grid_height,
                &foods,
            );
        } else if phase == SystemOrder::Brain {
            brain::process_brain(&mut world.ecs);
        } else if phase == SystemOrder::Behavior {
            behavior::process_behavior(&mut world.ecs);
        } else if phase == SystemOrder::Physics {
            let dt = self.tick_duration.as_secs_f32();
            physics::symplectic_euler_integration(&mut world.ecs, dt);
            physics::world_bounds_collision(&mut world.ecs, common::Vec2::new(1000.0, 1000.0));
            world.update_spatial_index();
        } else if phase == SystemOrder::Metabolism {
            metabolism::process_metabolism(&mut world.ecs, &world.event_bus);
            ecology::process_gas_exchange(
                &mut world.ecs,
                &mut world.field_grid,
                world.grid_width,
                world.grid_height,
            );
            ecology::process_disease(
                &mut world.ecs,
                &world.spatial_index,
                phylon_config::PhylonConfig::default().simulation.rng_seed,
                self.current_tick.0,
            );
        } else if phase == SystemOrder::Ecology {
            ecology::spawn_food(
                &mut world.ecs,
                phylon_config::PhylonConfig::default().simulation.rng_seed,
                self.current_tick.0,
            );
            ecology::process_foraging(
                &mut world.ecs,
                &world.spatial_index,
                &world.event_bus,
                &world.field_grid,
                256,
                256,
            );
        } else if phase == SystemOrder::Reproduction {
            reproduction::process_reproduction(
                &mut world.ecs,
                &world.spatial_index,
                &world.event_bus,
                phylon_config::PhylonConfig::default().simulation.rng_seed,
                self.current_tick.0,
            );
        } else if phase == SystemOrder::PostTick {
            let mut events = world.event_bus.drain::<events::PhylonEvent>();

            // Collect deaths and births
            let mut deaths = Vec::new();
            let mut births = Vec::new();

            for e in &events {
                match e {
                    events::PhylonEvent::DeathEvent { id, reason } => deaths.push((*id, *reason)),
                    events::PhylonEvent::BirthEvent {
                        parent,
                        genome,
                        initial_energy,
                        position,
                    } => {
                        births.push((*parent, genome.clone(), *initial_energy, *position));
                    }
                    _ => {}
                }
            }

            // Process deaths FIRST
            for (dead_id, reason) in deaths {
                let entity = hecs::Entity::from_bits(dead_id.0).unwrap();
                let _ = world.ecs.despawn(entity);
                events.push(events::PhylonEvent::OrganismDied {
                    id: dead_id,
                    cause: reason,
                    tick: self.current_tick.0,
                });
            }

            // Process births
            for (parent_id, genome, energy, pos) in births {
                let species = world.species_registry.assign_species(&genome, 15.0);

                let mut generation = 0;
                if let Some(pid) = parent_id {
                    let parent_entity = hecs::Entity::from_bits(pid.0).unwrap();
                    if let Ok(parent_gen) = world.ecs.get::<&organisms::Generation>(parent_entity) {
                        generation = parent_gen.0 + 1;
                    }
                }

                let mut builder = hecs::EntityBuilder::new();
                builder.add(organisms::Organism);
                builder.add(organisms::Age(0));
                builder.add(organisms::Generation(generation));
                builder.add(organisms::Energy(energy));
                builder.add(organisms::Health::default());
                builder.add(genome.clone());
                builder.add(species);
                builder.add(physics::Position(pos));
                builder.add(physics::Velocity(common::Vec2::ZERO));
                builder.add(physics::Acceleration(common::Vec2::ZERO));
                builder.add(physics::Heading::default());
                builder.add(physics::Mass(1.0));
                builder.add(physics::Radius(genome.size));
                builder.add(reproduction::ReproductionCooldown(100));
                builder.add(sensing::Observation::new());
                builder.add(brain::Intention::new());
                builder.add(brain::BrainState::default());
                builder.add(brain::LearnedWeights {
                    data: genome.brain_weights.clone(),
                });

                let id = world.spawn(builder.build());

                events.push(events::PhylonEvent::OrganismBorn {
                    id: common::EntityId(id.to_bits().get()),
                    parent_id,
                    generation,
                    tick: self.current_tick.0,
                });
            }

            world.last_events = events;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_advancement() {
        let mut scheduler = SimulationScheduler::new(60);
        let mut world = world::PhylonWorld::default();
        assert_eq!(scheduler.current_tick, Tick(0));
        scheduler.tick_loop(&mut world);
        assert_eq!(scheduler.current_tick, Tick(1));
    }
}
