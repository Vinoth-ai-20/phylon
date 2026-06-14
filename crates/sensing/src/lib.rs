use common::EntityId;
use genetics::Genome;
use hecs::World;
use organisms::{Energy, Health};
use physics::{Heading, Position, Velocity};
use serde::{Deserialize, Serialize};
use spatial::UniformGrid;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Observation {
    pub data: [f32; 12],
}

impl Observation {
    pub fn new() -> Self {
        Self { data: [0.0; 12] }
    }
}

pub fn process_sensing(
    world: &World,
    grid: &UniformGrid,
    field_grid: &[[f32; 4]],
    grid_width: u32,
    grid_height: u32,
    foods: &std::collections::HashMap<EntityId, (hecs::Entity, common::Vec2, f32, bool)>,
) {
    puffin::profile_function!();

    for (entity, (pos, heading, _vel, energy, health, genome, obs)) in world
        .query::<(
            &Position,
            &Heading,
            &Velocity,
            &Energy,
            &Health,
            &Genome,
            &mut Observation,
        )>()
        .into_iter()
    {
        obs.data[0] = energy.0 / 200.0;
        obs.data[1] = health.0 / 100.0;

        let center_cell = grid.pos_to_cell(pos.0);
        let cell_size = grid.cell_size();
        let search_range = (genome.vision_depth / cell_size).ceil() as i32;

        let mut closest_food_dist = f32::MAX;
        let mut closest_food_angle = 0.0;

        let heading_vec = common::Vec2::new(heading.0.cos(), heading.0.sin());
        let mut sector_left_prey = 0.0;
        let mut sector_center_prey = 0.0;
        let mut sector_right_prey = 0.0;

        for dx in -search_range..=search_range {
            for dy in -search_range..=search_range {
                let cell = common::IVec2::new(center_cell.x + dx, center_cell.y + dy);
                for &neighbor_id in grid.query_cell(cell) {
                    let nid = hecs::Entity::from_bits(neighbor_id.0).unwrap();
                    if nid == entity {
                        continue;
                    }

                    if let Ok(n_pos) = world.get::<&Position>(nid) {
                        let to_neighbor = n_pos.0 - pos.0;
                        let dist = to_neighbor.length();
                        if dist < genome.vision_depth {
                            let angle_to_neighbor =
                                to_neighbor.normalize_or_zero().angle_to(heading_vec);

                            if angle_to_neighbor.abs() <= genome.vision_cone_angle / 2.0 {
                                let sector_width = genome.vision_cone_angle / 3.0;
                                let val = 1.0 - (dist / genome.vision_depth);

                                if angle_to_neighbor < -sector_width / 2.0 {
                                    sector_left_prey += val;
                                } else if angle_to_neighbor > sector_width / 2.0 {
                                    sector_right_prey += val;
                                } else {
                                    sector_center_prey += val;
                                }
                            }
                        }
                    }
                }
            }
        }

        for (_, food_pos, _, is_consumed) in foods.values() {
            if *is_consumed {
                continue;
            }
            let to_food = *food_pos - pos.0;
            let dist = to_food.length();
            if dist < genome.vision_depth && dist < closest_food_dist {
                closest_food_dist = dist;
                closest_food_angle = to_food.normalize_or_zero().angle_to(heading_vec);
            }
        }

        obs.data[2] = if closest_food_dist < f32::MAX {
            1.0 - (closest_food_dist / genome.vision_depth)
        } else {
            0.0
        };
        obs.data[3] = closest_food_angle / std::f32::consts::PI;

        obs.data[4] = sector_left_prey.min(1.0);
        obs.data[5] = sector_center_prey.min(1.0);
        obs.data[6] = sector_right_prey.min(1.0);

        let half_w = grid_width as f32 / 2.0;
        let half_h = grid_height as f32 / 2.0;
        let gx = (pos.0.x + half_w).floor() as i32;
        let gy = (pos.0.y + half_h).floor() as i32;

        if gx >= 0 && gx < grid_width as i32 && gy >= 0 && gy < grid_height as i32 {
            let idx = (gy as u32 * grid_width + gx as u32) as usize;
            let field = &field_grid[idx];
            obs.data[7] = field[0];
            obs.data[8] = field[1];
            obs.data[9] = field[2];
            obs.data[10] = field[3];
        } else {
            obs.data[7] = 0.0;
            obs.data[8] = 0.0;
            obs.data[9] = 0.0;
            obs.data[10] = 0.0;
        }

        obs.data[11] = 0.0;
    }
}

pub trait FieldSampler {
    fn sample(&self, pos: common::Vec2) -> [f32; 4];
}
