use common::{IVec2, Vec2};
use genetics::Genome;
use hecs::World;
use organisms::{Energy, FoodPellet};
use physics::{Heading, Position, Velocity};
use serde::{Deserialize, Serialize};
use spatial::UniformGrid;

/// Interface to sample environmental fields at specific locations.
pub trait FieldSampler {
    fn sample(&self, pos: Vec2) -> [f32; 4];
}

/// Component storing the current sensory observation of an organism.
/// Data format: [food_distance, food_angle, current_speed, energy_level, oxygen, carbon, scent, temp]
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Observation {
    pub data: [f32; 8],
}

impl Observation {
    pub fn new() -> Self {
        Self { data: [0.0; 8] }
    }
}

/// Gathers spatial information and populates the Observation component for all organisms.
pub fn process_sensing(world: &mut World, grid: &UniformGrid, field: &dyn FieldSampler) {
    puffin::profile_function!();

    // Collect food positions beforehand to satisfy borrow checker
    let mut food_positions = rustc_hash::FxHashMap::default();
    for (entity, (_, pos)) in world.query::<(&FoodPellet, &Position)>().iter() {
        food_positions.insert(common::EntityId(entity.to_bits().get()), pos.0);
    }

    for (entity, (pos, heading, vel, energy, genome, obs)) in world.query_mut::<(
        &Position,
        &Heading,
        &Velocity,
        &Energy,
        &Genome,
        &mut Observation,
    )>() {
        let mut nearest_dist_sq = genome.sense_radius * genome.sense_radius;
        let mut nearest_food_pos: Option<Vec2> = None;

        let center_cell = grid.pos_to_cell(pos.0);

        // Rough estimate of cells to check based on cell size
        let cell_size = grid.cell_size();
        let search_range = (genome.sense_radius / cell_size).ceil() as i32;

        for dx in -search_range..=search_range {
            for dy in -search_range..=search_range {
                let cell = IVec2::new(center_cell.x + dx, center_cell.y + dy);

                for &neighbor_id in grid.query_cell(cell) {
                    if neighbor_id.0 == entity.to_bits().get() {
                        continue;
                    }

                    if let Some(&food_pos) = food_positions.get(&neighbor_id) {
                        let diff = food_pos - pos.0;
                        let dist_sq = diff.length_squared();

                        if dist_sq < nearest_dist_sq {
                            nearest_dist_sq = dist_sq;
                            nearest_food_pos = Some(food_pos);
                        }
                    }
                }
            }
        }

        // Compute observation values
        let food_distance = if nearest_food_pos.is_some() {
            nearest_dist_sq.sqrt()
        } else {
            genome.sense_radius
        };

        let food_angle = if let Some(f_pos) = nearest_food_pos {
            let diff = f_pos - pos.0;
            let absolute_angle = f32::atan2(diff.y, diff.x);

            // Relative angle: difference between absolute angle and heading
            let mut rel_angle = absolute_angle - heading.0;

            // Normalize to [-PI, PI]
            while rel_angle > std::f32::consts::PI {
                rel_angle -= std::f32::consts::TAU;
            }
            while rel_angle < -std::f32::consts::PI {
                rel_angle += std::f32::consts::TAU;
            }

            rel_angle
        } else {
            0.0 // No food seen
        };

        let current_speed = vel.0.length();
        let field_vals = field.sample(pos.0);

        obs.data[0] = food_distance;
        obs.data[1] = food_angle;
        obs.data[2] = current_speed;
        obs.data[3] = energy.0;
        obs.data[4] = field_vals[0]; // Oxygen
        obs.data[5] = field_vals[1]; // Carbon
        obs.data[6] = field_vals[2]; // Scent
        obs.data[7] = field_vals[3]; // Temp
    }
}
