use bevy_ecs::prelude::Component;
use common::{EntityId, SimEnergy, SimLength, Vec2, Vec3};
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

/// An organism's current life stage (Phase 4, `PHASE4_ROADMAP.md` milestone
/// P4-L1, ADR-P4-03). Every organism starts `Juvenile`; `organisms::life_cycle::life_stage_system`
/// promotes it to `Adult` once it clears a maturity age threshold — at which
/// point growth becomes re-entrant (see that system's doc comment). Only two
/// stages exist for this milestone, deliberately: the roadmap calls for a
/// life-stage state machine, not a specific number of stages, and two is the
/// minimum that makes "a transition" meaningful at all.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LifeStage {
    /// Not yet mature — the stage every organism is spawned in.
    #[default]
    Juvenile,
    /// Mature — reached once `life_stage_system`'s age threshold is crossed.
    Adult,
}

impl LifeStage {
    /// The `develop_at_position_with_life_stage` signal for this stage —
    /// `0.0` for `Juvenile` reproduces `develop_at_position`'s original,
    /// pre-P4-L1 decode exactly; `Adult`'s value is an untuned placeholder,
    /// same status as every other Phase 4 rate/signal constant introduced
    /// this phase, chosen only to be clearly nonzero.
    pub fn developmental_signal(self) -> f32 {
        match self {
            LifeStage::Juvenile => 0.0,
            LifeStage::Adult => 1.0,
        }
    }
}

/// Tracks the sequential growth of an organism from its regulatory genome.
///
/// Each tick, `organisms::growth_system` decodes the next body position via
/// `genetics::develop_at_position` (Phase 3, M4) and materialises the result
/// as a spine node (and, if the position branches, a lateral fin pair).
/// When growth is complete the brain is wired and `GrowthState` is removed.
///
/// As of Phase 4 (`PHASE4_ROADMAP.md`'s ADR-P4-01), the Body Graph itself is
/// tracked in a sibling `crate::DevelopmentalGraph` component on the same
/// entity, not a field here — `growth_system` writes to that component
/// directly, so it survives this component's removal instead of being
/// dropped along with it (the pre-Phase-4 behavior; see ADR-P3-04/ADR-P3-09
/// for why that used to be fine, and ADR-P4-01 for why it no longer is).
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
    /// Position for the next spine node. `Vec3` since Phase 8 (ADR-P8-01)
    /// — `z` stays `0.0` until Epic 8.6's real 3D growth-orientation
    /// redesign; `heading` below is untouched by that same deferral.
    pub current_pos: Vec3,
    /// Distance between adjacent spine nodes.
    pub segment_length: f32,
    /// The list of actuated spring effectors built so far.
    pub effectors: Vec<bevy_ecs::entity::Entity>,
    /// Set once a decoded segment is `SegmentType::Tail` — growth stops
    /// after that segment even if `next_segment_index` hasn't reached
    /// `organisms::MAX_SEGMENTS` yet (Phase 3, M4; no special-cased length,
    /// just an emergent stopping condition from the decode itself).
    pub is_organism_complete: bool,
    /// Heading angle at which this organism spawns.
    pub heading: f32,
}
