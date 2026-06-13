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

        if phase == SystemOrder::Physics {
            let dt = self.tick_duration.as_secs_f32();
            physics::symplectic_euler_integration(&mut world.ecs, dt);
            physics::world_bounds_collision(&mut world.ecs, common::Vec2::new(1000.0, 1000.0));
            world.update_spatial_index();
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
