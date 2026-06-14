use common::Vec2;
use organisms::Energy;
use physics::Position;
use std::cell::RefCell;
use std::rc::Rc;
use world::PhylonWorld;

#[derive(Clone)]
pub enum PluginCommand {
    SpawnFood { x: f32, y: f32, energy: f32 },
    KillRadius { x: f32, y: f32, radius: f32 },
    FloodField { channel: i64, value: f32 },
}

#[derive(Clone, Default)]
pub struct GodModeApi {
    commands: Rc<RefCell<Vec<PluginCommand>>>,
}

impl GodModeApi {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn_food(&mut self, x: f64, y: f64, energy: f64) {
        self.commands.borrow_mut().push(PluginCommand::SpawnFood {
            x: x as f32,
            y: y as f32,
            energy: energy as f32,
        });
    }

    pub fn kill_radius(&mut self, x: f64, y: f64, radius: f64) {
        self.commands.borrow_mut().push(PluginCommand::KillRadius {
            x: x as f32,
            y: y as f32,
            radius: radius as f32,
        });
    }

    pub fn flood_field(&mut self, channel: i64, value: f64) {
        self.commands.borrow_mut().push(PluginCommand::FloodField {
            channel,
            value: value as f32,
        });
    }

    pub fn drain(&mut self) -> Vec<PluginCommand> {
        self.commands.borrow_mut().drain(..).collect()
    }
}

pub fn apply_commands(world: &mut PhylonWorld, commands: Vec<PluginCommand>) {
    let mut structural_changes = false;

    for cmd in commands {
        match cmd {
            PluginCommand::SpawnFood { x, y, energy } => {
                world.spawn((
                    organisms::FoodPellet,
                    Position(Vec2::new(x, y)),
                    Energy(energy),
                ));
                structural_changes = true;
            }
            PluginCommand::KillRadius { x, y, radius } => {
                let center = Vec2::new(x, y);
                let r2 = radius * radius;
                let mut to_despawn = Vec::new();
                for (e, pos) in world.ecs.query::<&Position>().iter() {
                    if pos.0.distance_squared(center) <= r2 {
                        to_despawn.push(e);
                    }
                }
                if !to_despawn.is_empty() {
                    structural_changes = true;
                }
                for e in to_despawn {
                    let _ = world.ecs.despawn(e);
                }
            }
            PluginCommand::FloodField { channel, value } => {
                if (0..4).contains(&channel) {
                    for cell in &mut world.field_grid {
                        cell[channel as usize] = value;
                    }
                }
            }
        }
    }

    if structural_changes {
        world.update_spatial_index();
    }
}
