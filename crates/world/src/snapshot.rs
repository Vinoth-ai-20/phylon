use crate::PhylonWorld;
use anyhow::Result;

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::Path;

#[derive(Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub organisms: Vec<OrganismSnapshot>,
    pub foods: Vec<FoodSnapshot>,
}

#[derive(Serialize, Deserialize)]
pub struct OrganismSnapshot {
    pub genome: genetics::Genome,
    pub position: physics::Position,
    pub velocity: physics::Velocity,
    pub acceleration: physics::Acceleration,
    pub mass: physics::Mass,
    pub radius: physics::Radius,
    pub heading: physics::Heading,
    pub energy: organisms::Energy,
    pub health: organisms::Health,
    pub age: organisms::Age,
    pub observation: sensing::Observation,
    pub intention: brain::Intention,
}

#[derive(Serialize, Deserialize)]
pub struct FoodSnapshot {
    pub position: physics::Position,
    pub energy: organisms::Energy,
}

/// Saves the ECS state to a RON file.
pub fn save_world(world: &PhylonWorld, path: impl AsRef<Path>) -> Result<()> {
    let mut snapshot = WorldSnapshot {
        organisms: Vec::new(),
        foods: Vec::new(),
    };

    // Extract Organisms
    for (_, (genome, pos, vel, acc, mass, rad, head, en, hp, age, obs, int)) in world
        .ecs
        .query::<(
            &genetics::Genome,
            &physics::Position,
            &physics::Velocity,
            &physics::Acceleration,
            &physics::Mass,
            &physics::Radius,
            &physics::Heading,
            &organisms::Energy,
            &organisms::Health,
            &organisms::Age,
            &sensing::Observation,
            &brain::Intention,
        )>()
        .iter()
    {
        snapshot.organisms.push(OrganismSnapshot {
            genome: genome.clone(),
            position: *pos,
            velocity: *vel,
            acceleration: *acc,
            mass: *mass,
            radius: *rad,
            heading: *head,
            energy: *en,
            health: *hp,
            age: *age,
            observation: obs.clone(),
            intention: int.clone(),
        });
    }

    // Extract Food
    for (_, (_food, pos, en)) in world
        .ecs
        .query::<(
            &organisms::FoodPellet,
            &physics::Position,
            &organisms::Energy,
        )>()
        .iter()
    {
        snapshot.foods.push(FoodSnapshot {
            position: *pos,
            energy: *en,
        });
    }

    let file = File::create(path)?;
    ron::ser::to_writer_pretty(file, &snapshot, ron::ser::PrettyConfig::default())?;

    Ok(())
}

/// Loads the ECS state from a RON file.
pub fn load_world(world: &mut PhylonWorld, path: impl AsRef<Path>) -> Result<()> {
    let file = File::open(path)?;
    let snapshot: WorldSnapshot = ron::de::from_reader(file)?;

    // Validate genome version
    if let Some(first) = snapshot.organisms.first() {
        if first.genome.version != 1 {
            anyhow::bail!(
                "Incompatible save file: expected genome version 1, found {}",
                first.genome.version
            );
        }
    }

    world.ecs.clear();

    for o in snapshot.organisms {
        world.spawn((
            organisms::Organism,
            o.genome,
            o.position,
            o.velocity,
            o.acceleration,
            o.mass,
            o.radius,
            o.heading,
            o.energy,
            o.health,
            o.age,
            o.observation,
            o.intention,
        ));
    }

    for f in snapshot.foods {
        world.spawn((organisms::FoodPellet, f.position, f.energy));
    }

    world.update_spatial_index();

    Ok(())
}
