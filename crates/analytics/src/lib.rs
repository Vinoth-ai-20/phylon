use common::Tick;
use events::{DeathCause, PhylonEvent};
use std::collections::VecDeque;
use world::PhylonWorld;

pub struct SimulationStats {
    pub max_history: usize,
    pub population_history: VecDeque<(f64, f64)>, // (tick, count)
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
            population_history: VecDeque::with_capacity(max_history),
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

        // Exact count of all active entities in the world
        self.current_population = world.ecs.len() as usize;

        // Record population every tick
        self.population_history
            .push_back((tick.0 as f64, self.current_population as f64));
        if self.population_history.len() > self.max_history {
            self.population_history.pop_front();
        }
    }
}
