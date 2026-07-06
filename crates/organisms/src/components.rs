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

/// Tracks the sequential growth of an organism from its regulatory genome.
///
/// Each tick, `organisms::growth_system` decodes the next body position via
/// `genetics::develop_at_position` (Phase 3, M4) and materialises the result
/// as a spine node (and, if the position branches, a lateral fin pair).
/// When growth is complete the brain is wired and `GrowthState` is removed.
#[derive(Component, Debug, Clone)]
pub struct GrowthState {
    /// The genome driving growth.
    pub genome: genetics::Genome,
    /// Index of the next segment to build (0 = head already spawned, grows from 1).
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
    /// Set once a decoded segment is `SegmentType::Tail` — growth stops
    /// after that segment even if `next_segment_index` hasn't reached
    /// `organisms::MAX_SEGMENTS` yet (Phase 3, M4; no special-cased length,
    /// just an emergent stopping condition from the decode itself).
    pub is_organism_complete: bool,
    /// The Body Graph accumulated so far (Phase 3, M6) — one
    /// `DevelopmentalNode` per decoded position/branch, in growth order.
    /// Transient per ADR-P3-04: dropped along with the rest of
    /// `GrowthState` once growth completes, never persisted or exposed as
    /// its own ECS type.
    pub graph: crate::developmental_graph::DevelopmentalGraph,
    /// Heading angle at which this organism spawns.
    pub heading: f32,
}
