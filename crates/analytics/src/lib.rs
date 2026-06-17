use common::Tick;
use events::{DeathCause, PhylonEvent};
use std::collections::VecDeque;
use world::PhylonWorld;

pub struct SimulationStats {
    pub max_history: usize,
    pub history: VecDeque<(f64, f64, f64, f64)>, // (tick, population, avg_energy, total_food)
    pub deaths_by_starvation: u64,
    pub deaths_by_predation: u64,
    pub deaths_by_age: u64,
    pub total_births: u64,
    pub current_population: usize,
}

impl SimulationStats {
    pub fn new(max_history: usize) -> Self {
        Self {
            max_history,
            history: VecDeque::with_capacity(max_history),
            deaths_by_starvation: 0,
            deaths_by_predation: 0,
            deaths_by_age: 0,
            total_births: 0,
            current_population: 0,
        }
    }

    pub fn process_events(&mut self, events: &[PhylonEvent], _tick: Tick) {
        puffin::profile_scope!("analytics::process_events");

        for event in events {
            match event {
                PhylonEvent::OrganismBorn { .. } => {
                    self.total_births += 1;
                }
                PhylonEvent::DeathEvent { reason, .. } => match reason {
                    DeathCause::Starvation => self.deaths_by_starvation += 1,
                    DeathCause::Predation => self.deaths_by_predation += 1,
                    DeathCause::Age => self.deaths_by_age += 1,
                    _ => {}
                },
                _ => {}
            }
        }
    }

    pub fn update_metrics(&mut self, world: &PhylonWorld, tick: Tick) {
        puffin::profile_scope!("analytics::update_metrics");

        // Exact count of all living organisms
        self.current_population = world.ecs.query::<&organisms::Organism>().iter().count();

        let mut total_energy = 0.0;
        for (_, energy) in world.ecs.query::<&organisms::Energy>().iter() {
            total_energy += energy.0 as f64;
        }
        let avg_energy = if self.current_population > 0 {
            total_energy / self.current_population as f64
        } else {
            0.0
        };

        let total_food = world.ecs.query::<&organisms::FoodPellet>().iter().count() as f64;

        self.history.push_back((
            tick.0 as f64,
            self.current_population as f64,
            avg_energy,
            total_food,
        ));

        if self.history.len() > self.max_history {
            self.history.pop_front();
        }
    }
}
