//! # Phylon Organisms
//!
//! Organism archetype definitions, ECS component bundles, and lifecycle types.
//!
//! Every simulated organism is a set of ECS components. This crate defines
//! the canonical component bundles and the [`DietType`] enum that governs
//! ecological interactions.
//!
//! ## Phase 0 scope
//!
//! Component type declarations and DietType enum. ECS integration: Phase 3.

#![warn(missing_docs)]
#![warn(clippy::all)]

use bevy_ecs::prelude::{Component, Entity, Query};
use common::{EntityId, SimEnergy, SimLength, Vec2};
use serde::{Deserialize, Serialize};

// Diet type is now defined in the ecology crate to avoid duplication.

/// Tools for sandbox mode, presets, and procedural generation.
pub mod sandbox;
pub use sandbox::{PresetDefinition, SandboxTraits};

/// Spatial components of an organism: position, velocity, and collision radius.
#[derive(bevy_ecs::component::Component, Debug, Clone, Serialize, Deserialize)]
pub struct SpatialComponents {
    /// Current position in the simulation world.
    pub position: Vec2,
    /// Current velocity vector.
    pub velocity: Vec2,
    /// Collision and sensing radius.
    pub radius: SimLength,
}

/// Biological state components: energy, age, and diet.
#[derive(bevy_ecs::component::Component, Debug, Clone, Serialize, Deserialize)]
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
#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct OrganismColor(pub [f32; 3]);

/// The generational distance from the initial population (Generation 0).
#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct Generation(pub u32);

/// The absolute simulation tick when this entity was spawned.
#[derive(bevy_ecs::component::Component, Debug, Clone, Copy, Serialize, Deserialize, Default)]
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

/// System that builds out the organism's body sequentially, one Hox gene per tick.
///
/// Topology produced:
/// - **Spine**: single node per axial segment, connected end-to-end by `Rigid` bones.
///   No two-node-per-segment pairs, no cross springs, no closed rectangular loops.
/// - **Fins**: when a gene's `branching_signal > 0.0`, two fin nodes are sprouted
///   laterally from the spine node and attached via `Rotational` hinges.
pub fn growth_system(
    mut commands: bevy_ecs::prelude::Commands,
    mut query: Query<(Entity, &mut GrowthState)>,
    node_query: Query<&physics::ParticleNode>,
) {
    use genetics::SegmentType;
    use physics::{ParticleNode, Spring};

    for (entity, mut state) in query.iter_mut() {
        // Retrieve Hox sequence; fall back to an empty one if none exists.
        let hox_genes = match state.genome.hox.as_ref() {
            Some(h) => h.genes.clone(),
            None => vec![genetics::HoxGene::head(), genetics::HoxGene::tail()],
        };

        // Check if we've processed all genes.
        let is_finished = state.next_segment_index >= hox_genes.len();

        if state.ticks_until_next_bud > 0 && !is_finished {
            state.ticks_until_next_bud -= 1;
            continue;
        }

        if is_finished {
            // ── Wire the brain once the body is fully grown ──────────────────
            let input_count = 6;
            let output_count = state.effectors.len();

            let mut nodes = Vec::new();
            let mut synapses = Vec::new();

            for _ in 0..input_count {
                nodes.push(brain::CtrnnNode {
                    state: 0.0,
                    time_constant: 1.0,
                    bias: 0.0,
                    activation: 7, // Linear
                    first_synapse: 0,
                    synapse_count: 0,
                });
            }
            for _ in 0..output_count {
                nodes.push(brain::CtrnnNode {
                    state: 0.0,
                    time_constant: 0.5,
                    bias: 0.0,
                    activation: 1, // Tanh [-1, 1]
                    first_synapse: 0,
                    synapse_count: 0,
                });
            }
            for i in 0..input_count {
                for j in 0..output_count {
                    let w_inputs = [
                        (i as f32) / (input_count as f32),
                        (j as f32) / (output_count as f32),
                    ];
                    let w_outputs = state.genome.evaluate(&w_inputs);
                    let weight = if !w_outputs.is_empty() {
                        w_outputs[0] * 5.0
                    } else {
                        0.5
                    };
                    synapses.push(brain::CtrnnSynapse {
                        source: i as u32,
                        target: (input_count + j) as u32,
                        weight,
                        _padding: 0,
                    });
                }
            }

            commands.entity(entity).insert((
                brain::Brain::new(
                    brain::BrainId(0),
                    nodes,
                    synapses,
                    input_count,
                    output_count,
                ),
                sensing::SensoryState::new(input_count),
                behavior::MotorSystem {
                    effectors: state.effectors.clone(),
                },
            ));
            commands.entity(entity).remove::<GrowthState>();
            continue;
        }

        // ── Grow the next Hox gene ────────────────────────────────────────────
        let gene = &hox_genes[state.next_segment_index];

        let seg_u32 = match gene.segment {
            SegmentType::Head => 0,
            SegmentType::Torso => 1,
            SegmentType::Muscle => 2,
            SegmentType::Tail => 3,
            SegmentType::Fin => 4,
        };

        let stiffness = match gene.segment {
            SegmentType::Head => 10.0,
            SegmentType::Torso => 15.0,
            SegmentType::Muscle => 8.0,
            SegmentType::Tail => 2.0,
            SegmentType::Fin => 5.0,
        };

        // ── Spawn one spine node adjacent to the actual parent position ────────
        // Using the parent's *live* position (not a pre-calculated grid offset)
        // means the spring starts at exactly rest_length, producing zero initial
        // force and preventing the instability that caused fly-off.
        let spawn_pos = if let Some(prev_entity) = state.parent_spine_node {
            if let Ok(parent_node) = node_query.get(prev_entity) {
                // Step one segment_length in the -X direction from where the
                // parent node actually is right now.
                parent_node.position - Vec2::new(state.segment_length, 0.0)
            } else {
                state.current_pos
            }
        } else {
            state.current_pos
        };

        let spine_node = commands
            .spawn((
                ParticleNode::new(spawn_pos, 1.0, seg_u32),
                OrganismColor(state.color),
            ))
            .id();

        // ── Connect to previous spine node with a Rigid bone ─────────────────
        if let Some(prev) = state.parent_spine_node {
            let constraint_type = match gene.segment {
                SegmentType::Muscle => physics::ConstraintType::Elastic,
                SegmentType::Tail => physics::ConstraintType::Passive,
                _ => physics::ConstraintType::Rigid,
            };

            let s = commands
                .spawn((
                    Spring {
                        node_a: prev,
                        node_b: spine_node,
                        constraint_type,
                        rest_length: state.segment_length,
                        base_length: state.segment_length,
                        stiffness,
                        damping: 0.5,
                        // Spine bones are NEVER actuated — only Elastic (Muscle) connections
                        // drive locomotion through the muscle_actuation shader.
                        // Rigid and Passive bones must have amplitude=0 or the PBD
                        // correction injects ~19 units/s per iteration per tick when
                        // rest_length oscillates, causing runaway fly-off.
                        actuation_amplitude: match constraint_type {
                            physics::ConstraintType::Elastic => gene.actuation_amplitude,
                            _ => 0.0, // Rigid / Passive spine bones never actuate
                        },
                        actuation_phase: gene.actuation_phase,
                        breaking_strain: 2.0,
                        is_fin: 0,
                    },
                    OrganismColor(state.color),
                ))
                .id();

            if constraint_type == physics::ConstraintType::Elastic
                || constraint_type == physics::ConstraintType::Rotational
            {
                state.effectors.push(s);
            }
        }

        // ── Branch: sprout bilateral fin pair if branching_signal > 0 ────────
        // Only Torso and Muscle segments can branch (not Head or Tail).
        let can_branch = matches!(gene.segment, SegmentType::Torso | SegmentType::Muscle);
        if can_branch && gene.branching_signal > 0.0 {
            let fin_spread = state.segment_length * 0.75;

            let f_up_pos = spawn_pos + Vec2::new(0.0, fin_spread);
            let f_dn_pos = spawn_pos + Vec2::new(0.0, -fin_spread);

            let f_up = commands
                .spawn((
                    ParticleNode::new(f_up_pos, 0.5, 4),
                    OrganismColor(state.color),
                ))
                .id();
            let f_dn = commands
                .spawn((
                    ParticleNode::new(f_dn_pos, 0.5, 4),
                    OrganismColor(state.color),
                ))
                .id();

            // Attach fins via Rotational hinges — no cross-links back to spine
            let sf_up = commands
                .spawn((
                    Spring {
                        node_a: spine_node,
                        node_b: f_up,
                        constraint_type: physics::ConstraintType::Rotational,
                        rest_length: fin_spread,
                        base_length: fin_spread,
                        stiffness: 5.0,
                        damping: 0.3,
                        actuation_amplitude: gene.actuation_amplitude,
                        actuation_phase: 0.0,
                        breaking_strain: 2.0,
                        is_fin: 1,
                    },
                    OrganismColor(state.color),
                ))
                .id();
            state.effectors.push(sf_up);

            let sf_dn = commands
                .spawn((
                    Spring {
                        node_a: spine_node,
                        node_b: f_dn,
                        constraint_type: physics::ConstraintType::Rotational,
                        rest_length: fin_spread,
                        base_length: fin_spread,
                        stiffness: 5.0,
                        damping: 0.3,
                        actuation_amplitude: gene.actuation_amplitude,
                        actuation_phase: std::f32::consts::PI, // Opposing phase → flap
                        breaking_strain: 2.0,
                        is_fin: 1,
                    },
                    OrganismColor(state.color),
                ))
                .id();
            state.effectors.push(sf_dn);
        }

        // Advance state — current_pos still updated as a fallback reference.
        state.parent_spine_node = Some(spine_node);
        state.current_pos.x -= state.segment_length;
        state.next_segment_index += 1;
        state.ticks_until_next_bud = state.base_bud_interval;
    }
}

/// Spawns an organism's zygote based on its genome.
pub fn spawn_organism(
    world: &mut bevy_ecs::world::World,
    genome: &genetics::Genome,
    start_pos: Vec2,
    diet: ecology::Diet,
    category: ecology::EcologicalCategory,
    generation: u32,
    spawn_tick: u64,
) {
    use physics::ParticleNode;

    let segment_length = 20.0;

    // Determine color and initial head segment from HoxSequence when available.
    let (color, head_seg_u32) = if let Some(hox) = genome.hox.as_ref() {
        let seg_u32 = match hox.genes.first().map(|g| g.segment) {
            Some(genetics::SegmentType::Head) => 0,
            Some(genetics::SegmentType::Torso) => 1,
            Some(genetics::SegmentType::Muscle) => 2,
            Some(genetics::SegmentType::Tail) => 3,
            _ => 0,
        };
        (hox.color, seg_u32)
    } else {
        ([0.8, 0.4, 0.4], 0u32)
    };

    // Spawn the head node at start_pos (gene index 0).
    let head_node = world
        .spawn((
            ParticleNode::new(start_pos, 1.0, head_seg_u32),
            OrganismColor(color),
        ))
        .id();

    // Attach biology to the head node.
    world.entity_mut(head_node).insert((
        metabolism::Energy {
            current: 100.0,
            max: 200.0,
        },
        metabolism::Age {
            ticks: 0,
            max_lifespan: 10000,
        },
        metabolism::Metabolism {
            mass: 10.0,
            base_rate: 0.05,
        },
        Generation(generation),
        SpawnTick(spawn_tick),
        diet,
        category,
        reproduction::ReproductionStrategy {
            energy_threshold: 180.0,
            energy_cost: 100.0,
            cooldown_ticks: 300,
            current_cooldown: 0,
            mode: reproduction::ReproductionMode::Asexual,
            genome: genome.clone(),
        },
        // GrowthState starts at gene index 1; index 0 (Head) is already built.
        GrowthState {
            genome: genome.clone(),
            next_segment_index: 1,
            ticks_until_next_bud: 30, // ~0.5 s per segment bud at 60 Hz
            base_bud_interval: 30,
            parent_spine_node: Some(head_node),
            current_pos: start_pos - Vec2::new(segment_length, 0.0),
            segment_length,
            effectors: Vec::new(),
            color,
        },
        sensing::HeadVision {
            range: 250.0,
            fov: std::f32::consts::PI * 0.8, // ~144 degrees
            last_forward: common::Vec2::X,
            self_occlusion_radius: genome
                .hox
                .as_ref()
                .map(|hox| hox.genes.len() as f32 * segment_length)
                .unwrap_or(5.0 * segment_length)
                * 1.5, // Add a 50% margin
        },
    ));
}

/// Spawns a deterministic "Proto-Fish" with an instant adult topology.
///
/// This **bypasses** the CPPN/[`GrowthState`] state machine entirely and is
/// intended as a diagnostic fixture for iterating on physics and rendering.
/// The topology is:
///
/// - 5-node rigid spine along the negative-X axis (head at `pos`, tail left).
/// - 2 lateral fin nodes branching from spine node 2 (the middle segment).
/// - Rotational fin springs with opposing actuation phases so the fins flap.
///
/// The head node carries [`metabolism::Energy`], [`metabolism::Age`], and
/// [`metabolism::Metabolism`] components so the inspector sidebar can display
/// biological metrics.
///
/// # CPPN branching backlog note
///
/// The CPPN's `branching_signal` (output index 5) threshold is too rarely
/// exceeded in random genomes. A targeted tuning pass is required — see the
/// Phase 5 implementation plan for details.
pub fn spawn_proto_fish(
    world: &mut bevy_ecs::world::World,
    pos: Vec2,
    diet: ecology::Diet,
    category: ecology::EcologicalCategory,
    generation: u32,
    spawn_tick: u64,
) {
    use physics::{ConstraintType, ParticleNode, Spring};

    // Geometry constants — all in world units
    let segment_len: f32 = 20.0;
    let fin_spread: f32 = 15.0;

    // ── Spine (5 nodes along −X axis, head at pos, tail to the left) ──────
    // Segment types: Head(0), Torso(1), Torso(1), Torso(1), Tail(3)
    let spine_types: [u32; 5] = [0, 1, 1, 1, 3];
    let proto_color = [0.15, 0.72, 0.45]; // The original green used for debug proto-fish

    let spine_nodes: Vec<bevy_ecs::entity::Entity> = spine_types
        .iter()
        .enumerate()
        .map(|(i, &seg_type)| {
            let p = pos + Vec2::new(-(i as f32) * segment_len, 0.0);
            world
                .spawn((
                    ParticleNode::new(p, 1.0, seg_type),
                    OrganismColor(proto_color),
                ))
                .id()
        })
        .collect();

    // Rigid bone springs connecting adjacent spine nodes
    for i in 0..4 {
        world.spawn((
            Spring {
                node_a: spine_nodes[i],
                node_b: spine_nodes[i + 1],
                constraint_type: ConstraintType::Rigid,
                rest_length: segment_len,
                base_length: segment_len,
                stiffness: 20.0,
                damping: 0.5,
                actuation_amplitude: 0.0,
                actuation_phase: 0.0,
                breaking_strain: 5.0,
                is_fin: 0,
            },
            OrganismColor(proto_color),
        ));
    }

    // ── Lateral fins at spine node index 2 (centre of spine) ───────────────
    let fin_root = spine_nodes[2];
    let fin_root_pos = pos + Vec2::new(-2.0 * segment_len, 0.0);

    let f_up_pos = fin_root_pos + Vec2::new(0.0, fin_spread);
    let f_dn_pos = fin_root_pos + Vec2::new(0.0, -fin_spread);

    let f_up = world
        .spawn((
            ParticleNode::new(f_up_pos, 0.5, 4),
            OrganismColor(proto_color),
        ))
        .id();
    let f_dn = world
        .spawn((
            ParticleNode::new(f_dn_pos, 0.5, 4),
            OrganismColor(proto_color),
        ))
        .id();

    // Rotational springs — opposing phases produce a flapping motion
    world.spawn((
        Spring {
            node_a: fin_root,
            node_b: f_up,
            constraint_type: ConstraintType::Rotational,
            rest_length: fin_spread,
            base_length: fin_spread,
            stiffness: 5.0,
            damping: 0.3,
            actuation_amplitude: 8.0,
            actuation_phase: 0.0, // Phase 0
            breaking_strain: 5.0,
            is_fin: 1,
        },
        OrganismColor(proto_color),
    ));
    world.spawn((
        Spring {
            node_a: fin_root,
            node_b: f_dn,
            constraint_type: ConstraintType::Rotational,
            rest_length: fin_spread,
            base_length: fin_spread,
            stiffness: 5.0,
            damping: 0.3,
            actuation_amplitude: 8.0,
            actuation_phase: std::f32::consts::PI, // Opposing phase → flap
            breaking_strain: 5.0,
            is_fin: 1,
        },
        OrganismColor(proto_color),
    ));

    // ── Biological state on the head node ──────────────────────────────────
    world.entity_mut(spine_nodes[0]).insert((
        metabolism::Energy {
            current: 100.0,
            max: 200.0,
        },
        metabolism::Age {
            ticks: 0,
            max_lifespan: 10000,
        },
        metabolism::Metabolism {
            mass: 15.0, // approx mass of 5 spine + 2 fin nodes
            base_rate: 0.05,
        },
        Generation(generation),
        SpawnTick(spawn_tick),
        diet,
        category,
    ));
}
