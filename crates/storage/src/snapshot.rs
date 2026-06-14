use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use world::PhylonWorld;

pub const SNAPSHOT_VERSION: u32 = 2; // Phase 6 version

#[derive(Serialize, Deserialize)]
pub struct SnapshotHeader {
    pub version: u32,
}

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

/// Saves the ECS state to a file. Format is chosen by extension (.ron or .bin).
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

    let header = SnapshotHeader {
        version: SNAPSHOT_VERSION,
    };

    let path = path.as_ref();
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("ron");

    let mut file = File::create(path).context("Failed to create snapshot file")?;

    if ext == "bin" {
        bincode::serialize_into(&mut file, &header)?;
        bincode::serialize_into(&mut file, &snapshot)?;
    } else {
        #[derive(Serialize)]
        struct CombinedSnapshot<'a> {
            header: &'a SnapshotHeader,
            data: &'a WorldSnapshot,
        }
        let combined = CombinedSnapshot {
            header: &header,
            data: &snapshot,
        };
        ron::ser::to_writer_pretty(&mut file, &combined, ron::ser::PrettyConfig::default())?;
    }

    Ok(())
}

/// Loads the ECS state from a file.
pub fn load_world(world: &mut PhylonWorld, path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("ron");

    let mut file = File::open(path).context("Failed to open snapshot file")?;

    let snapshot: WorldSnapshot;

    if ext == "bin" {
        let header: SnapshotHeader = bincode::deserialize_from(&mut file)?;
        if header.version != SNAPSHOT_VERSION {
            anyhow::bail!(
                "Incompatible bincode save file: expected version {}, found {}",
                SNAPSHOT_VERSION,
                header.version
            );
        }
        snapshot = bincode::deserialize_from(&mut file)?;
    } else {
        #[derive(Deserialize)]
        struct CombinedSnapshot {
            header: SnapshotHeader,
            data: WorldSnapshot,
        }
        let mut content = String::new();
        file.read_to_string(&mut content)?;

        if let Ok(combined) = ron::from_str::<CombinedSnapshot>(&content) {
            if combined.header.version != SNAPSHOT_VERSION {
                anyhow::bail!(
                    "Incompatible RON save file: expected version {}, found {}",
                    SNAPSHOT_VERSION,
                    combined.header.version
                );
            }
            snapshot = combined.data;
        } else {
            // Fallback for v1 legacy snapshots
            snapshot = ron::from_str::<WorldSnapshot>(&content)?;
        }
    }

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
