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

/// An organism's current life stage. Every organism starts `Juvenile`;
/// `organisms::life_cycle::life_stage_system` promotes it to `Adult` once it
/// clears a maturity age threshold — at which point growth becomes
/// re-entrant (see that system's doc comment). Only two stages exist today,
/// deliberately: this is a life-stage state machine, not tied to a specific
/// number of stages, and two is the minimum that makes "a transition"
/// meaningful at all.
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
    /// `0.0` for `Juvenile` reproduces `develop_at_position`'s baseline
    /// decode exactly; `Adult`'s value is an untuned placeholder, chosen
    /// only to be clearly nonzero.
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
/// `genetics::develop_at_position` and materialises the result as a spine
/// node (and, if the position branches, a lateral fin pair). When growth is
/// complete the brain is wired and `GrowthState` is removed.
///
/// The Body Graph itself is tracked in a sibling `crate::DevelopmentalGraph`
/// component on the same entity, not a field here — `growth_system` writes
/// to that component directly, so it survives this component's removal
/// instead of being dropped along with it. See `crate::developmental_graph`'s
/// module doc for why the Body Graph must be a separate, persistent
/// component.
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
    /// Position for the next spine node. A full `Vec3`, though `z` stays
    /// `0.0` at every construction site today: the `forward`/`dorsal`
    /// fin-placement math is genuinely 3D-capable (see
    /// `crate::bilateral_fin_direction`), but nothing yet gives growth a
    /// mechanism to actually leave the Z=0 plane (that would be a
    /// deliberate biological behavior change, not a math limitation).
    pub current_pos: Vec3,
    /// Distance between adjacent spine nodes.
    pub segment_length: f32,
    /// The list of actuated spring effectors built so far.
    pub effectors: Vec<bevy_ecs::entity::Entity>,
    /// Set once a decoded segment is `SegmentType::Tail` — growth stops
    /// after that segment even if `next_segment_index` hasn't reached
    /// `organisms::MAX_SEGMENTS` yet; no special-cased length, just an
    /// emergent stopping condition from the decode itself.
    pub is_organism_complete: bool,
    /// Body-fixed forward (direction-of-travel) unit vector — computed once
    /// at spawn/resume and reused, rather than re-deriving `(cos, sin, 0)`
    /// at every use site. Still confined to the growth plane (`z == 0.0`)
    /// at every construction site in this crate today: the fin-placement
    /// math (`crate::bilateral_fin_direction`) is genuinely 3D-capable, but
    /// nothing introduces a mechanism for `forward` to actually tilt out of
    /// plane (that would be a deliberate biological behavior change).
    pub forward: Vec3,
    /// Body-fixed dorsal ("up") reference — together with `forward`,
    /// disambiguates "left fin" vs. "right fin" via
    /// `organisms::bilateral_fin_direction(dorsal, forward)`, a proper 3D
    /// cross product that's well-defined given two independent reference
    /// vectors (a naive 2D-only `perp = Vec2::new(-dir.y, dir.x)`
    /// construction has no direct 3D generalization). Every construction
    /// site in this crate sets this to `Vec3::Z`, which reproduces the
    /// equivalent 2D fin placement exactly.
    pub dorsal: Vec3,
}
