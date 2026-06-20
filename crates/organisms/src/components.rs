use bevy_ecs::prelude::Component;
use common::{EntityId, SimEnergy, SimLength, Vec2};
use serde::{Deserialize, Serialize};

/// Spatial components of an organism: position, velocity, and collision radius.
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct SpatialComponents {
    /// Current position in the simulation world.
    pub position: Vec2,
    /// Current velocity vector.
    pub velocity: Vec2,
    /// Collision and sensing radius.
    pub radius: SimLength,
}

/// Biological state components: energy, age, and diet.
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct BiologicalComponents {
    /// Current energy reserve. Organism dies when this reaches zero.
    pub energy: SimEnergy,
    /// Age in simulation ticks.
    pub age_ticks: u64,
    /// The organism's dietary strategy.
    pub diet: ecology::Diet,
    /// The organism's special ecological trait/category.
    pub category: ecology::EcologicalCategory,
    /// Parent entity ID (null if initial spawn).
    pub parent: EntityId,
}

/// The base color of an organism's skin, driven by genetics.
#[derive(Component, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct OrganismColor(pub [f32; 3]);

/// The generational distance from the initial population (Generation 0).
#[derive(Component, Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct Generation(pub u32);

/// The absolute simulation tick when this entity was spawned.
#[derive(Component, Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct SpawnTick(pub u64);

/// Tracks the sequential growth of an organism from its Hox genome.
///
/// Each tick one gene from the `HoxSequence` is materialised as a spine node.
/// When the sequence is exhausted the brain is wired and `GrowthState` is removed.
#[derive(Component, Debug, Clone)]
pub struct GrowthState {
    /// The genome driving growth (Hox sequence embedded within).
    pub genome: genetics::Genome,
    /// Index of the next gene to build (0 = head already spawned, grows from 1).
    pub next_segment_index: usize,
    /// Ticks remaining until the next segment buds.
    pub ticks_until_next_bud: u64,
    /// The interval between buds.
    pub base_bud_interval: u64,
    /// The single spine node spawned by the previous gene — to attach the next one to.
    pub parent_spine_node: Option<bevy_ecs::entity::Entity>,
    /// Position for the next spine node.
    pub current_pos: Vec2,
    /// Distance between adjacent spine nodes.
    pub segment_length: f32,
    /// The list of actuated spring effectors built so far.
    pub effectors: Vec<bevy_ecs::entity::Entity>,
    /// Skin colour for this organism.
    pub color: [f32; 3],
}
