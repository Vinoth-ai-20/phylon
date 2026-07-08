use crate::components::{GrowthState, OrganismColor};
use crate::developmental_graph::DevelopmentalGraph;
use bevy_ecs::prelude::{Commands, Entity, Query};
use common::Vec2;
use rand::Rng;

/// Base weight-magnitude threshold for wiring a synapse between two nodes
/// in the same neural region (`brain::RegionId`) — unchanged from the
/// single-threshold rule that existed before Phase 6, Epic C's N1b.
const SYNAPSE_WEIGHT_THRESHOLD: f32 = 0.01;

/// Cross-region synapses require a substantially stronger CPPN-evaluated
/// weight to be wired at all — the "sparser cross-region wiring" N1b adds,
/// per `PHASE4_EPIC1_NEURAL_ROADMAP.md`'s N1b goal ("cross-region pairs are
/// wired more sparsely... replacing pure all-pairs-via-index-CPPN with a
/// rule that actually reflects anatomy"). Every node still defaults to
/// `brain::RegionId::Central` for any organism with no decoded
/// `SegmentType::Ganglion` segment (every organism observed so far, since
/// nothing has evolved one yet — N1c, Phase 6, Epic C, added the real
/// detection logic, but finding an actual Ganglion in a real population is
/// a separate, unverified question) — so `same_region` is still `true` for
/// every node pair in practice today, and this constant has no observed
/// effect yet, exactly the graceful-degradation property both N1b's and
/// N1c's own test requirements ask for.
const CROSS_REGION_SYNAPSE_WEIGHT_THRESHOLD: f32 = 0.5;

/// Decides whether a CPPN-evaluated synapse weight is strong enough to
/// wire, given whether the source/target nodes share a neural region.
/// Extracted as its own pure function (Phase 6, Epic C, N1b) so the
/// region-aware wiring rule is unit-testable independent of
/// `growth_system`'s full genome/CPPN machinery.
fn should_wire_synapse(weight: f32, same_region: bool) -> bool {
    let threshold = if same_region {
        SYNAPSE_WEIGHT_THRESHOLD
    } else {
        CROSS_REGION_SYNAPSE_WEIGHT_THRESHOLD
    };
    weight.abs() > threshold
}

/// Assigns every brain node's neural region for one organism (Phase 6,
/// Epic C, N1c) — anchoring each *hidden* CTRNN node to the nearest decoded
/// `genetics::SegmentType::Ganglion` position, by real graph-topological
/// distance (`DevelopmentalGraph::graph_distance`), if the organism has one
/// or more Ganglion segments; `brain::RegionId::Central` otherwise (every
/// organism observed so far, since nothing has evolved a Ganglion segment
/// yet). Input and output nodes are never reassigned — they already have a
/// real identity (a sensor or an effector) that a Ganglion anchor wouldn't
/// add anything to; only hidden nodes are abstract CTRNN units with no
/// inherent body position, which is exactly what this function gives them
/// one for.
///
/// A hidden node has no position of its own, so it's anchored to an
/// evenly-spread target position along the body axis — its own index among
/// just the hidden nodes, scaled across `crate::MAX_SEGMENTS` — then
/// matched to whichever decoded Ganglion is nearest to that target
/// position by graph distance (ties broken by the lower body position, to
/// stay fully deterministic). If no real segment was ever decoded at the
/// exact target position (e.g. pruned by apoptosis), that hidden node
/// falls back to `Central` rather than guessing — a real, disclosed
/// limitation, not silently papered over.
fn assign_hidden_node_regions(
    graph: &DevelopmentalGraph,
    input_count: usize,
    hidden_count: usize,
    total_nodes: usize,
) -> Vec<brain::RegionId> {
    let ganglion_positions: Vec<usize> = graph
        .nodes
        .iter()
        .filter(|n| n.role == genetics::SegmentType::Ganglion)
        .map(|n| n.position)
        .collect();

    (0..total_nodes)
        .map(|i| {
            let is_hidden = i >= input_count && i < input_count + hidden_count;
            if !is_hidden || ganglion_positions.is_empty() {
                return brain::RegionId::Central;
            }
            let hidden_index = i - input_count;
            let target_position = hidden_index * crate::MAX_SEGMENTS / hidden_count.max(1);
            let Some(target_index) = graph.index_at_position(target_position) else {
                return brain::RegionId::Central;
            };
            ganglion_positions
                .iter()
                .filter_map(|&pos| {
                    graph.index_at_position(pos).map(|ganglion_index| {
                        (pos, graph.graph_distance(target_index, ganglion_index))
                    })
                })
                .min_by_key(|&(pos, distance)| (distance, pos))
                .map(|(pos, _)| brain::RegionId::Ganglion(pos))
                .unwrap_or(brain::RegionId::Central)
        })
        .collect()
}

/// # Embryonic Growth & Morphogenesis System
///
/// ## 1. What Happens
/// The `growth_system` executes the procedural embryogenesis of an organism. Over multiple ticks,
/// it decodes each body position through the regulatory network (`genetics::develop_at_position`,
/// Phase 3 M4) and iteratively spawns the `ParticleNode`s and `Spring` constraints that make up
/// the organism's physical body.
///
/// ## 2. Why It Happens
/// Spawning an entire complex multi-body organism in a single tick with perfect physics stability
/// is mathematically impossible due to spring rest-length violations. By growing the organism
/// one segment at a time (like a biological bud), the PBD physics solver has time to gently
/// relax the tension, preventing numeric explosions ("fly-off").
///
/// ## 3. How It Happens
/// The system acts as a state machine tracked by `GrowthState`:
/// 1. Every $N$ ticks, it decodes the next body position via `genetics::develop_at_position`.
/// 2. It spawns a new node at exactly $RestLength$ away from the `parent_spine_node`.
/// 3. It attaches a `Rigid`, `Elastic`, or `Passive` spring to the parent, depending on the
///    decoded segment type.
/// 4. If the decoded position branches, it sprouts orthogonal fin nodes and connects them with
///    `Elastic` muscles.
/// 5. Once growth is complete (either `organisms::MAX_SEGMENTS` is reached, or a decoded segment
///    is `Tail`), it wires the `Brain` CTRNN topology.
#[allow(clippy::too_many_arguments)]
pub fn growth_system(
    mut commands: Commands,
    atmosphere: bevy_ecs::prelude::Res<metabolism::GlobalAtmosphere>,
    mut query: Query<(
        Entity,
        &mut GrowthState,
        &mut DevelopmentalGraph,
        Option<&crate::components::LifeStage>,
    )>,
    node_query: Query<&physics::ParticleNode>,
    spring_query: Query<&physics::Spring>,
    chem_query: Query<&metabolism::ChemicalEconomy>,
    morphogen_query: Query<&crate::morphogen_field::MorphogenLevel>,
    // `Option` (not `Res`) so that every existing headless/unit-test `World`
    // that never inserts this resource keeps working unchanged — a missing
    // field reads as "no environmental signal yet" (0.0), the same neutral
    // baseline a real run has before the GPU's first successful readback.
    cpu_field: Option<bevy_ecs::prelude::Res<diffusion::CpuFieldState>>,
) {
    use genetics::SegmentType;
    use physics::{ParticleNode, Spring};

    for (entity, mut state, mut graph, life_stage) in query.iter_mut() {
        // Phase 4, P4-L1: an organism without a `LifeStage` component (there
        // shouldn't be any post-P4-L1, but this keeps the query total rather
        // than panicking on an edge case) developmentally behaves as
        // `Juvenile` — signal `0.0`, `develop_at_position`'s original
        // behavior.
        let life_stage_signal = life_stage
            .copied()
            .unwrap_or_default()
            .developmental_signal();
        // Expressed (dominance-resolved for diploid genomes — see
        // `Genome::expressed_brain_cppn`/`expressed_regulatory_cppn`) once
        // per organism, not per query, since growth/brain-wiring below
        // calls `.evaluate(...)`/`develop_at_position(...)` many times per
        // tick. `morph_cppn` is deliberately not queried here any more —
        // Phase 3 M4 moved segment-identity decoding entirely onto
        // `regulatory_cppn` (see ADR-P3-02); `morph_cppn` remains a crossed/
        // mutated/distance-compared locus but has no growth-time consumer
        // left, a known, documented piece of technical debt.
        let expressed_regulatory_cppn = state.genome.expressed_regulatory_cppn();
        let expressed_brain_cppn = state.genome.expressed_brain_cppn();

        let is_finished =
            state.next_segment_index >= crate::MAX_SEGMENTS || state.is_organism_complete;

        if state.ticks_until_next_bud > 0 && !is_finished {
            state.ticks_until_next_bud -= 1;
            continue;
        }

        if is_finished {
            // ── Wire the brain once the body is fully grown ──────────────────
            // 6 standard inputs + 1 Signal input + 1 Hazard input + 1 Pacemaker
            let input_count = 9;
            // effectors + 1 SignalEmitter output
            let output_count = state.effectors.len() + 1;

            let hidden_count = 4;
            let total_nodes = input_count + hidden_count + output_count;

            let mut nodes = Vec::new();
            let mut synapses = Vec::new();

            for i in 0..total_nodes {
                let mut bias = 0.0;
                let mut time_constant = 0.5;
                let mut activation = 1; // Tanh

                if i < input_count {
                    time_constant = 1.0;
                    activation = 7; // Linear
                } else {
                    // Evolve node properties via Brain CPPN
                    if !expressed_brain_cppn.nodes.is_empty() {
                        let w_inputs = [
                            (i as f32) / (total_nodes as f32),
                            (i as f32) / (total_nodes as f32),
                        ];
                        let w_outputs = expressed_brain_cppn.evaluate(&w_inputs);
                        if w_outputs.len() >= 3 {
                            bias = w_outputs[1] * 1.5;
                            // Time constant must be strictly positive and low enough to allow fast 2 Hz oscillations.
                            time_constant = w_outputs[2].abs().clamp(0.1, 2.0);
                        }
                    }
                }

                nodes.push(brain::CtrnnNode {
                    state: 0.0,
                    time_constant,
                    bias,
                    activation,
                    first_synapse: 0,
                    synapse_count: 0,
                });
            }

            // Find fins for Braitenberg wiring
            let mut left_fin_idx = None;
            let mut right_fin_idx = None;
            for (out_idx, &effector_entity) in state.effectors.iter().enumerate() {
                if let Ok(spring) = spring_query.get(effector_entity) {
                    if spring.is_fin == 1 {
                        if left_fin_idx.is_none() {
                            left_fin_idx = Some(input_count + hidden_count + out_idx);
                        } else if right_fin_idx.is_none() {
                            right_fin_idx = Some(input_count + hidden_count + out_idx);
                        }
                    }
                }
            }

            // Phase 6, Epic C (N1c): every node's neural region, parallel
            // to `nodes`. Built here (not left to `Brain::new`'s own
            // default) so the wiring loop below can consult it while
            // deciding which synapses to keep, and so the exact same
            // vector becomes the constructed `Brain`'s `node_regions`
            // afterward. See `assign_hidden_node_regions`'s own doc
            // comment for the actual Ganglion-anchoring logic — extracted
            // as its own function so it's unit-testable against a
            // hand-built `DevelopmentalGraph` fixture, independent of
            // genome/CPPN decoding (mirroring N1b's `should_wire_synapse`
            // extraction for the same reason).
            let node_regions =
                assign_hidden_node_regions(&graph, input_count, hidden_count, total_nodes);

            for i in 0..total_nodes {
                // Connections can only target hidden and output nodes (not inputs)
                for j in input_count..total_nodes {
                    let mut weight = 0.0;

                    // Neocortex: Evolved CPPN Weights
                    if !expressed_brain_cppn.nodes.is_empty() {
                        let w_inputs = [
                            (i as f32) / (total_nodes as f32),
                            (j as f32) / (total_nodes as f32),
                        ];
                        let w_outputs = expressed_brain_cppn.evaluate(&w_inputs);
                        if !w_outputs.is_empty() {
                            weight += w_outputs[0] * 1.5;
                        }
                    }

                    let same_region = node_regions[i] == node_regions[j];
                    if should_wire_synapse(weight, same_region) {
                        synapses.push(brain::CtrnnSynapse {
                            source: i as u32,
                            target: j as u32,
                            weight,
                            _padding: 0,
                        });
                    }
                }
            }

            let initial_atp = chem_query.get(entity).map(|c| c.atp).unwrap_or(0.0);

            let mut brain = brain::Brain::new(
                brain::BrainId(0),
                nodes,
                synapses,
                input_count,
                output_count,
            );
            // `Brain::new` already defaults every node to `RegionId::Central`
            // (N1a), so this is a no-op today — written explicitly so N1c's
            // real Ganglion-detected `node_regions` (computed above, for the
            // wiring loop's own use) propagates into the constructed `Brain`
            // once that milestone replaces the all-`Central` vector above
            // with real detection, without needing a second change here.
            brain.node_regions = node_regions;

            commands.entity(entity).insert((
                brain,
                brain::Neuromodulators::new(initial_atp),
                sensing::SensoryState::new(input_count),
                behavior::MotorSystem {
                    effectors: state.effectors.clone(),
                },
                diffusion::SignalEmitter::default(),
            ));
            commands.entity(entity).remove::<GrowthState>();
            continue;
        }

        // ── Grow the next body position ───────────────────────────────────────
        // Decoded via the regulatory network, not read from a stored gene —
        // the same `develop_at_position` call handles every position,
        // including the head node `spawning::spawn_organism` already built
        // (Phase 3 M4; see ADR-P3-02).
        //
        // Phase 6, Epic D (D1a): the growing tip's own current
        // `morphogen_field::MorphogenLevel` (seeded at spawn, spread and
        // decayed each tick by `morphogen_diffusion_system`) is folded into
        // the exact same additive signal `develop_at_position_with_life_stage`
        // already uses for `life_stage_signal` — re-auditing `genetics`
        // before this milestone found that function already implements the
        // "extra scalar folded into every gene's external input, 0.0
        // reproduces the original" seam D1a's own sub-roadmap proposed
        // building from scratch, so no new `genetics`-crate parameter is
        // added here.
        let field_signal = state
            .parent_spine_node
            .and_then(|tip| morphogen_query.get(tip).ok())
            .map(|level| level.concentration)
            .unwrap_or(0.0);

        // ── Where the next segment will actually form ───────────────────────
        // Computed here (rather than after the decode, as before D1b) purely
        // because Epic D, D1b's environmental signal needs a world position
        // to sample *before* calling `develop_at_position_with_life_stage`
        // — the formula itself is unchanged from pre-D1b.
        let spawn_pos = if let Some(prev_entity) = state.parent_spine_node {
            if let Ok(parent_node) = node_query.get(prev_entity) {
                parent_node.position
                    + Vec2::new(state.heading.cos(), state.heading.sin()) * -state.segment_length
            } else {
                state.current_pos
            }
        } else {
            state.current_pos
        };

        // Phase 6, Epic D (D1b, ADR-D1-01's inter-organism/environmental
        // half): samples the world-space GPU diffusion field's Morphogen
        // layer at the position the next segment is about to form at.
        // Unlike `field_signal` (this organism's own intra-organism
        // signal), this can carry a contribution from *other* developing
        // organisms nearby — the actual inter-organism coupling
        // ADR-P3-03's reversal trigger named. Folded into the same
        // additive channel as `life_stage_signal`/`field_signal` for the
        // same reason D1a gave: that channel already exists, is already
        // tested, and 0.0 (no nearby signal) reproduces the pre-D1
        // baseline exactly.
        let environmental_signal = cpu_field
            .as_ref()
            .map(|field| field.sample(spawn_pos, diffusion::FieldLayer::Morphogen as u32))
            .unwrap_or(0.0);

        let outputs = genetics::develop_at_position_with_life_stage(
            &expressed_regulatory_cppn,
            state.next_segment_index,
            crate::MAX_SEGMENTS,
            life_stage_signal + field_signal + environmental_signal,
        );

        // Phase 3 M8 (DEF-002): a position marked for apoptosis is pruned
        // before organogenesis — it is never spawned, as if it had never
        // formed (germ-line-protected positions can never reach this
        // branch; see `genetics::decode_apoptosis`). Bookkeeping mirrors
        // the normal "advance state" step at the end of a tick, minus
        // spawning and minus updating `parent_spine_node`/`graph`, so the
        // next real segment attaches directly to the last real one — no
        // visible gap, the position simply never existed.
        if outputs.apoptosis {
            state.next_segment_index += 1;
            state.ticks_until_next_bud = state.base_bud_interval;
            let offset =
                Vec2::new(state.heading.cos(), state.heading.sin()) * -state.segment_length;
            state.current_pos += offset;
            continue;
        }

        // Phase 3 M6: the decode-to-physics mapping lives in
        // `developmental_graph::compile_segment` now — independently
        // testable, and reusable by future research panels — rather than
        // inline match arms here.
        let compiled = crate::compile_segment(outputs.segment_type);
        let seg_u32 = compiled.particle_segment_type;
        let stiffness = compiled.stiffness;

        // Phase 4, P4-F2: every body segment gets its own small physiology
        // pool, not just the head — `metabolism::ChemicalEconomy` is reused
        // verbatim (not a new type) at a deliberately smaller scale (see
        // `ChemicalEconomy::segment_default`'s doc comment). This is
        // additive: `metabolism_system`'s query also requires `&Age`/
        // `&Metabolism`, which no non-head segment carries, so this cannot
        // change any existing organism-level metabolism/reproduction/
        // foraging behavior — confirmed by this crate's full test suite
        // still passing unmodified.
        let spine_node = commands
            .spawn((
                ParticleNode::new(spawn_pos, 1.0, seg_u32, entity.index()),
                OrganismColor(outputs.pigment),
                metabolism::ChemicalEconomy::segment_default(),
                // Phase 4, P4-F4: every non-head segment also gets a
                // `HormoneLevel` — `organisms::endocrine_diffusion_system`
                // relaxes it toward its structural parent's channel
                // reading each tick (see that system's doc comment).
                brain::HormoneLevel::default(),
                // Phase 4, P4-F5: every non-head segment also gets its own
                // infection severity/immune resistance —
                // `organisms::segment_infection_system` spreads the
                // organism-wide `Infection` (if any) out into these.
                ecology::disease::SegmentInfection::healthy(),
                ecology::disease::SegmentImmunity::baseline(),
                // Phase 5, SX-2d: reuses the existing `SpawnTick` component
                // (previously only attached to an organism's head at
                // creation, `organisms::spawning`) rather than adding a new
                // type — `crates/app/src/render.rs` reads this to fade/scale
                // a just-formed segment in over a short fixed window.
                crate::components::SpawnTick(atmosphere.ticks),
                // Phase 6, Epic D (D1a): every newly-grown segment starts as
                // the organism's new growing tip — see `morphogen_field`'s
                // doc comment for why this is the emission/"reaction" term
                // `morphogen_diffusion_system` then spreads and decays.
                crate::morphogen_field::MorphogenLevel {
                    concentration: crate::morphogen_field::MORPHOGEN_SEED_CONCENTRATION,
                },
            ))
            .id();

        // Body Graph (Phase 3, M6; persistent as of Phase 4, ADR-P4-01):
        // this spine node's parent is always the most recently pushed
        // non-branch node — i.e. the last spine node, which is exactly
        // `graph.nodes.len() - 1` immediately before this push, since
        // branch nodes (below) never become anyone's structural parent.
        let parent_graph_index = graph.nodes.len().checked_sub(1);
        let current_position = state.next_segment_index;
        graph.push(
            outputs.segment_type,
            outputs,
            parent_graph_index,
            false,
            current_position,
            Some(spine_node),
        );

        // ── Connect to previous spine node with a Rigid bone ─────────────────
        if let Some(prev) = state.parent_spine_node {
            let constraint_type = compiled.constraint_type;

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
                            physics::ConstraintType::Elastic => outputs.actuation_amplitude,
                            _ => 0.0, // Rigid / Passive spine bones never actuate
                        },
                        actuation_phase: outputs.actuation_phase,
                        breaking_strain: 2.0,
                        is_fin: 0,
                    },
                    OrganismColor(outputs.pigment),
                ))
                .id();

            if constraint_type == physics::ConstraintType::Elastic
                || constraint_type == physics::ConstraintType::Rotational
            {
                state.effectors.push(s);
            }
        }

        // ── Branch: sprout bilateral fin pair if the decode's branch output
        // fires. Only Torso and Muscle segments can branch (not Head or Tail).
        let branch_eligible = crate::can_branch(outputs.segment_type);
        if branch_eligible && outputs.branches && state.parent_spine_node.is_some() {
            // This branch's parent is the spine node just pushed above —
            // its index is the graph's current last entry.
            let spine_graph_index = graph.nodes.len() - 1;
            let current_position = state.next_segment_index;

            let fin_spread = state.segment_length * 0.75;
            let dir = Vec2::new(state.heading.cos(), state.heading.sin());
            let perp = Vec2::new(-dir.y, dir.x);

            let f_up_pos = spawn_pos + perp * fin_spread;
            let f_dn_pos = spawn_pos + perp * -fin_spread;

            let f_up = commands
                .spawn((
                    ParticleNode::new(f_up_pos, 0.5, 4, entity.index()),
                    OrganismColor(outputs.pigment),
                    metabolism::ChemicalEconomy::segment_default(),
                    brain::HormoneLevel::default(),
                    ecology::disease::SegmentInfection::healthy(),
                    ecology::disease::SegmentImmunity::baseline(),
                    crate::components::SpawnTick(atmosphere.ticks),
                    crate::morphogen_field::MorphogenLevel {
                        concentration: crate::morphogen_field::MORPHOGEN_SEED_CONCENTRATION,
                    },
                ))
                .id();
            let f_dn = commands
                .spawn((
                    ParticleNode::new(f_dn_pos, 0.5, 4, entity.index()),
                    OrganismColor(outputs.pigment),
                    metabolism::ChemicalEconomy::segment_default(),
                    brain::HormoneLevel::default(),
                    ecology::disease::SegmentInfection::healthy(),
                    ecology::disease::SegmentImmunity::baseline(),
                    crate::components::SpawnTick(atmosphere.ticks),
                    crate::morphogen_field::MorphogenLevel {
                        concentration: crate::morphogen_field::MORPHOGEN_SEED_CONCENTRATION,
                    },
                ))
                .id();

            graph.push(
                SegmentType::Fin,
                outputs,
                Some(spine_graph_index),
                true,
                current_position,
                Some(f_up),
            );
            graph.push(
                SegmentType::Fin,
                outputs,
                Some(spine_graph_index),
                true,
                current_position,
                Some(f_dn),
            );

            // Attach fins via Rigid hinges to the spine
            commands.spawn((
                Spring {
                    node_a: spine_node,
                    node_b: f_up,
                    constraint_type: physics::ConstraintType::Rigid,
                    rest_length: fin_spread,
                    base_length: fin_spread,
                    stiffness: 20.0,
                    damping: 0.5,
                    actuation_amplitude: 0.0,
                    actuation_phase: 0.0,
                    breaking_strain: 2.0,
                    is_fin: 1,
                },
                OrganismColor(outputs.pigment),
            ));
            commands.spawn((
                Spring {
                    node_a: spine_node,
                    node_b: f_dn,
                    constraint_type: physics::ConstraintType::Rigid,
                    rest_length: fin_spread,
                    base_length: fin_spread,
                    stiffness: 20.0,
                    damping: 0.5,
                    actuation_amplitude: 0.0,
                    actuation_phase: 0.0,
                    breaking_strain: 2.0,
                    is_fin: 1,
                },
                OrganismColor(outputs.pigment),
            ));

            // Attach Elastic muscle to the previous spine node
            let prev_spine = state.parent_spine_node.unwrap();
            let muscle_rest_len =
                (state.segment_length * state.segment_length + fin_spread * fin_spread).sqrt();

            let sf_up = commands
                .spawn((
                    Spring {
                        node_a: prev_spine,
                        node_b: f_up,
                        constraint_type: physics::ConstraintType::Elastic,
                        rest_length: muscle_rest_len,
                        base_length: muscle_rest_len,
                        stiffness: 25.0,
                        damping: 0.9,
                        actuation_amplitude: outputs.actuation_amplitude,
                        actuation_phase: 0.0,
                        breaking_strain: 2.0,
                        is_fin: 0,
                    },
                    OrganismColor(outputs.pigment),
                ))
                .id();
            state.effectors.push(sf_up);

            let sf_dn = commands
                .spawn((
                    Spring {
                        node_a: prev_spine,
                        node_b: f_dn,
                        constraint_type: physics::ConstraintType::Elastic,
                        rest_length: muscle_rest_len,
                        base_length: muscle_rest_len,
                        stiffness: 25.0,
                        damping: 0.9,
                        actuation_amplitude: outputs.actuation_amplitude,
                        actuation_phase: std::f32::consts::PI, // Opposing phase → flap
                        breaking_strain: 2.0,
                        is_fin: 0,
                    },
                    OrganismColor(outputs.pigment),
                ))
                .id();
            state.effectors.push(sf_dn);
        }

        // Advance state — current_pos still updated as a fallback reference.
        state.parent_spine_node = Some(spine_node);
        let offset = Vec2::new(state.heading.cos(), state.heading.sin()) * -state.segment_length;
        state.current_pos += offset;
        state.next_segment_index += 1;
        state.ticks_until_next_bud = state.base_bud_interval;
        if outputs.segment_type == SegmentType::Tail {
            state.is_organism_complete = true;
        }
    }
}

/// # Unbounded Plant Growth System
///
/// ## 1. What Happens
/// The `producer_growth_system` handles the continuous, post-embryonic structural expansion
/// of `Producer` organisms (plants/autotrophs) as long as they have surplus resources.
///
/// ## 2. Why It Happens
/// Unlike animals (which have a fixed Hox body plan to ensure locomotion symmetry), plants
/// grow fractally based on resource availability. This creates physical obstruction and
/// biomass aggregation in the ecosystem, providing dynamic maze-like structures for herbivores.
///
/// ## 3. How It Happens
/// If a Producer's $Glucose$ and $ATP$ exceed the `growth_cost` threshold, it pays the metabolic tax:
///
/// $$ Glucose_{new} = Glucose - 5000.0 $$
/// $$ ATP_{new} = ATP - 2000.0 $$
///
/// The system traverses the existing plant graph (BFS) and randomly attaches a new leaf node
/// via an `Elastic` spring, inherently biasing growth upwards against gravity.
pub fn producer_growth_system(
    mut commands: Commands,
    atmosphere: bevy_ecs::prelude::Res<metabolism::GlobalAtmosphere>,
    mut rng: bevy_ecs::prelude::ResMut<common::SimRng>,
    mut query: Query<(
        Entity,
        &ecology::Diet,
        &mut metabolism::ChemicalEconomy,
        &mut metabolism::Metabolism,
        &physics::ParticleNode,
    )>,
    spring_q: Query<&physics::Spring>,
) {
    // Threshold to grow a new node
    let growth_cost = 5000.0;
    let branch_cost_atp = 2000.0;
    // Ceiling on structural mass — without this a producer's own growth
    // accelerates its future CO2 demand indefinitely (mass feeds directly
    // into `co2_needed = 4.0 * mass * sunlight` in photosynthesis_system),
    // outrunning any amount the atmosphere can replenish.
    let max_producer_mass = 300.0;

    // We need adjacency map to find all nodes of an organism starting from head.
    let mut adj: std::collections::HashMap<Entity, Vec<Entity>> = std::collections::HashMap::new();
    for spring in spring_q.iter() {
        adj.entry(spring.node_a).or_default().push(spring.node_b);
        adj.entry(spring.node_b).or_default().push(spring.node_a);
    }

    for (head_entity, diet, mut chem, mut metabolism, head_node) in query.iter_mut() {
        if *diet == ecology::Diet::Producer
            && chem.glucose > chem.max_glucose * 0.8
            && chem.glucose >= growth_cost
            && chem.atp > branch_cost_atp + 500.0
            && atmosphere.co2 > 50.0
            && metabolism.mass < max_producer_mass
        {
            chem.glucose -= growth_cost;
            chem.atp -= branch_cost_atp;
            metabolism.mass += 5.0; // Increase mass
            chem.max_glucose += 2000.0;
            chem.max_o2 += 1000.0;
            chem.max_atp += 2000.0;
            chem.max_co2 += 1000.0;

            // Find a random node to attach to
            let mut all_nodes = vec![head_entity];
            let mut queue = std::collections::VecDeque::new();
            let mut visited = std::collections::HashSet::new();

            queue.push_back(head_entity);
            visited.insert(head_entity);

            while let Some(curr) = queue.pop_front() {
                if let Some(neighbors) = adj.get(&curr) {
                    for &n in neighbors {
                        if visited.insert(n) {
                            queue.push_back(n);
                            all_nodes.push(n);
                        }
                    }
                }
            }

            // Pick a random node from the plant body
            let target_node = all_nodes[rng.gen_range(0..all_nodes.len())];

            let offset = common::Vec2::new(
                (rng.gen::<f32>() - 0.5) * 20.0,
                rng.gen::<f32>() * 20.0 + 5.0, // Upward bias
            );

            let new_node_id = commands
                .spawn((
                    physics::ParticleNode::new(
                        head_node.position + offset,
                        1.0,
                        1,
                        head_entity.index(),
                    ),
                    crate::components::OrganismColor([0.2, 0.9, 0.2]), // Bright green new leaf
                    // Phase 5, SX-2d: same fade-in-on-spawn treatment as
                    // `growth_system`'s new segments — a plant sprouting a
                    // new leaf is the same kind of "growth" event.
                    crate::components::SpawnTick(atmosphere.ticks),
                ))
                .id();

            commands.spawn((
                physics::Spring {
                    node_a: target_node,
                    node_b: new_node_id,
                    constraint_type: physics::ConstraintType::Elastic,
                    rest_length: 20.0,
                    base_length: 20.0,
                    stiffness: 10.0,
                    damping: 0.5,
                    actuation_amplitude: 0.0,
                    actuation_phase: 0.0,
                    breaking_strain: 5.0,
                    is_fin: 0,
                },
                crate::components::OrganismColor([0.2, 0.9, 0.2]),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;

    /// Runs `growth_system` repeatedly against a single organism's
    /// `GrowthState` until the `Brain`/`GrowthState`-removal branch fires,
    /// or a fixed iteration ceiling is hit (a bug that stalls growth should
    /// fail this test loudly, not hang it).
    fn run_growth_to_completion(world: &mut World, entity: Entity) {
        for _ in 0..(crate::MAX_SEGMENTS * 40) {
            if world.get::<GrowthState>(entity).is_none() {
                return;
            }
            world.run_system_once(growth_system);
        }
        panic!("growth_system did not complete within the iteration ceiling");
    }

    fn spawn_growth_entity(world: &mut World, genome: genetics::Genome) -> Entity {
        world
            .spawn((
                metabolism::ChemicalEconomy {
                    glucose: 1000.0,
                    o2: 1000.0,
                    co2: 0.0,
                    atp: 1000.0,
                    max_glucose: 1000.0,
                    max_o2: 1000.0,
                    max_co2: 1000.0,
                    max_atp: 1000.0,
                },
                GrowthState {
                    genome,
                    next_segment_index: 1,
                    ticks_until_next_bud: 0,
                    base_bud_interval: 0,
                    parent_spine_node: None,
                    current_pos: Vec2::new(0.0, 0.0),
                    segment_length: 20.0,
                    effectors: Vec::new(),
                    is_organism_complete: false,
                    heading: 0.0,
                },
                DevelopmentalGraph::new(),
            ))
            .id()
    }

    #[test]
    fn growth_system_completes_for_a_default_genome_without_panicking() {
        let mut world = World::new();
        world.insert_resource(metabolism::GlobalAtmosphere::default());
        let genome = genetics::Genome::new_minimal(genetics::GenomeId(1), common::EntityId(0));
        let entity = spawn_growth_entity(&mut world, genome);
        run_growth_to_completion(&mut world, entity);
        assert!(world.get::<brain::Brain>(entity).is_some());
    }

    /// Phase 6, Epic C (N1b)'s own named test requirement: `should_wire_synapse`'s
    /// same-region branch must behave *identically* to the single-threshold
    /// rule that existed before N1b, for any weight — proving the region-aware
    /// wiring rule is mathematically a no-op whenever every node is
    /// `RegionId::Central` (true for every organism today, since N1c hasn't
    /// landed to assign anything else). This is a direct proof of "graceful
    /// degradation," not an indirect golden-topology snapshot.
    #[test]
    fn should_wire_synapse_same_region_matches_the_pre_n1b_single_threshold_rule() {
        let weights: [f32; 11] = [
            -2.0, -0.5, -0.011, -0.01, -0.005, 0.0, 0.005, 0.01, 0.011, 0.5, 2.0,
        ];
        for weight in weights {
            let old_rule = weight.abs() > 0.01;
            assert_eq!(
                should_wire_synapse(weight, true),
                old_rule,
                "same-region wiring decision changed for weight {weight} — this must stay identical to the pre-N1b rule"
            );
        }
    }

    /// The other half of N1b's own named test requirement: cross-region
    /// pairs must be wired more sparsely than same-region pairs for the
    /// same weight — the actual new behavior N1b adds, tested directly
    /// since nothing in this milestone's own scope produces a real
    /// cross-region organism yet (that's N1c).
    #[test]
    fn should_wire_synapse_cross_region_requires_a_stronger_weight_than_same_region() {
        // A weight that would wire within the same region but is too weak
        // to cross a region boundary — proving cross-region density is
        // measurably lower for any weight distribution with mass in this range.
        let weight = 0.2;
        assert!(should_wire_synapse(weight, true));
        assert!(!should_wire_synapse(weight, false));
    }

    /// Integration-level confirmation of the same graceful-degradation
    /// property: a real organism grown from a real genome ends up with a
    /// `Brain::node_regions` that is uniformly `Central` and non-empty —
    /// proving N1b/N1c's new plumbing (`node_regions` built during wiring,
    /// then written into the constructed `Brain`) actually reaches the real
    /// component, not just the pure-function unit tests below. Still
    /// uniformly `Central` even with N1c's real Ganglion-detection logic
    /// active, since `genetics::Genome::new_minimal`'s CPPN has never been
    /// observed to decode a `SegmentType::Ganglion` segment (a real,
    /// disclosed limitation of the fixture, not a hidden assumption).
    #[test]
    fn growth_system_produces_a_brain_with_uniformly_central_regions_today() {
        let mut world = World::new();
        world.insert_resource(metabolism::GlobalAtmosphere::default());
        let genome = genetics::Genome::new_minimal(genetics::GenomeId(1), common::EntityId(0));
        let entity = spawn_growth_entity(&mut world, genome);
        run_growth_to_completion(&mut world, entity);

        let brain = world.get::<brain::Brain>(entity).unwrap();
        assert_eq!(brain.node_regions.len(), brain.nodes.len());
        assert!(brain
            .node_regions
            .iter()
            .all(|region| *region == brain::RegionId::Central));
    }

    /// Builds a minimal `DevelopmentalGraph` fixture directly (bypassing
    /// genome/CPPN decoding, mirroring N1b's own `should_wire_synapse` test
    /// strategy), with one explicit `SegmentType::Ganglion` segment — since
    /// nothing in `growth_system` itself can decode a real one yet (that
    /// would require reverse-engineering a genome that happens to produce
    /// one, which is out of scope here). Proves N1c's own first named test
    /// requirement: a hidden node's assigned region must match the nearest
    /// Ganglion *by body-graph distance*, not by coincidentally matching
    /// raw node index.
    fn linear_fixture_graph_with_ganglion_at(position: usize) -> DevelopmentalGraph {
        use genetics::{DevelopmentalOutputs, SegmentType};
        let outputs = |t: SegmentType| DevelopmentalOutputs {
            segment_type: t,
            branches: false,
            actuation_amplitude: 0.0,
            actuation_phase: 0.0,
            pigment: [0.5, 0.5, 0.5],
            apoptosis: false,
        };
        let mut graph = DevelopmentalGraph::new();
        let mut last = graph.push(
            SegmentType::Head,
            outputs(SegmentType::Head),
            None,
            false,
            0,
            None,
        );
        for pos in 1..crate::MAX_SEGMENTS {
            let role = if pos == position {
                SegmentType::Ganglion
            } else {
                SegmentType::Torso
            };
            last = graph.push(role, outputs(role), Some(last), false, pos, None);
        }
        graph
    }

    #[test]
    fn assign_hidden_node_regions_anchors_hidden_nodes_to_the_nearest_ganglion_by_graph_distance() {
        // A single Ganglion near the tail (position 10 of 0..15) — every
        // hidden node's evenly-spread target position should end up
        // anchored to this one Ganglion, since it's the only candidate,
        // regardless of raw index proximity.
        let graph = linear_fixture_graph_with_ganglion_at(10);
        let input_count = 9;
        let hidden_count = 4;
        let output_count = 2;
        let total_nodes = input_count + hidden_count + output_count;

        let regions = assign_hidden_node_regions(&graph, input_count, hidden_count, total_nodes);

        assert_eq!(regions.len(), total_nodes);
        // Inputs and outputs must never be reassigned.
        for region in &regions[0..input_count] {
            assert_eq!(*region, brain::RegionId::Central);
        }
        for region in &regions[input_count + hidden_count..total_nodes] {
            assert_eq!(*region, brain::RegionId::Central);
        }
        // Every hidden node has exactly one Ganglion candidate, so all must
        // anchor to it.
        for region in &regions[input_count..input_count + hidden_count] {
            assert_eq!(*region, brain::RegionId::Ganglion(10));
        }
    }

    /// N1c's second named test requirement: with two Ganglion segments,
    /// hidden nodes must split between them correctly by graph distance —
    /// nodes whose target position is nearer the head-side Ganglion get
    /// anchored there, and nodes nearer the tail-side Ganglion get anchored
    /// to that one instead.
    #[test]
    fn assign_hidden_node_regions_splits_hidden_nodes_between_two_ganglia_by_distance() {
        use genetics::{DevelopmentalOutputs, SegmentType};
        let outputs = |t: SegmentType| DevelopmentalOutputs {
            segment_type: t,
            branches: false,
            actuation_amplitude: 0.0,
            actuation_phase: 0.0,
            pigment: [0.5, 0.5, 0.5],
            apoptosis: false,
        };
        let mut graph = DevelopmentalGraph::new();
        let mut last = graph.push(
            SegmentType::Head,
            outputs(SegmentType::Head),
            None,
            false,
            0,
            None,
        );
        for pos in 1..crate::MAX_SEGMENTS {
            let role = if pos == 2 || pos == 12 {
                SegmentType::Ganglion
            } else {
                SegmentType::Torso
            };
            last = graph.push(role, outputs(role), Some(last), false, pos, None);
        }
        let _ = last;

        let input_count = 9;
        let hidden_count = 4;
        let output_count = 2;
        let total_nodes = input_count + hidden_count + output_count;

        let regions = assign_hidden_node_regions(&graph, input_count, hidden_count, total_nodes);
        let hidden_regions = &regions[input_count..input_count + hidden_count];

        // Hidden index 0 targets position 0 (nearest the head-side Ganglion
        // at 2); hidden index 3 targets position 3*15/4=11 (nearest the
        // tail-side Ganglion at 12). Both real Ganglia must actually be
        // used by *some* hidden node — proving this is a real split, not
        // every node coincidentally picking the same one.
        assert!(hidden_regions.contains(&brain::RegionId::Ganglion(2)));
        assert!(hidden_regions.contains(&brain::RegionId::Ganglion(12)));
    }

    /// N1b's own named determinism test requirement — matching every other
    /// Phase 4/6 milestone's discipline. `growth_system`'s brain-wiring loop
    /// has no RNG/`HashMap` involved (pure `Vec` iteration over a
    /// deterministic CPPN evaluation), so this is expected to trivially
    /// pass — but "expected to" is not the same as "verified," which is the
    /// whole point of writing it down as a real test rather than an
    /// assumption.
    #[test]
    fn growth_system_brain_wiring_is_deterministic_for_the_same_genome() {
        fn wire_once() -> (Vec<brain::CtrnnNode>, Vec<brain::CtrnnSynapse>) {
            let mut world = World::new();
            world.insert_resource(metabolism::GlobalAtmosphere::default());
            let genome = genetics::Genome::new_minimal(genetics::GenomeId(1), common::EntityId(0));
            let entity = spawn_growth_entity(&mut world, genome);
            run_growth_to_completion(&mut world, entity);
            let brain = world.get::<brain::Brain>(entity).unwrap();
            (brain.nodes.clone(), brain.synapses.clone())
        }

        let (nodes_a, synapses_a) = wire_once();
        let (nodes_b, synapses_b) = wire_once();

        assert_eq!(nodes_a.len(), nodes_b.len());
        for (a, b) in nodes_a.iter().zip(nodes_b.iter()) {
            assert_eq!(a.bias, b.bias);
            assert_eq!(a.time_constant, b.time_constant);
            assert_eq!(a.activation, b.activation);
        }
        assert_eq!(synapses_a.len(), synapses_b.len());
        for (a, b) in synapses_a.iter().zip(synapses_b.iter()) {
            assert_eq!(a.source, b.source);
            assert_eq!(a.target, b.target);
            assert_eq!(a.weight, b.weight);
        }
    }

    #[test]
    fn developmental_graph_survives_growth_completion_and_removal_of_growth_state() {
        // Phase 4, ADR-P4-01's whole point: unlike pre-Phase-4 behavior
        // (where the graph was nested in `GrowthState` and discarded the
        // moment it was removed), the persistent `DevelopmentalGraph`
        // sibling component must still be present, non-empty, and
        // unchanged after `GrowthState` is gone.
        let mut world = World::new();
        world.insert_resource(metabolism::GlobalAtmosphere::default());
        let genome = genetics::Genome::new_minimal(genetics::GenomeId(1), common::EntityId(0));
        let entity = spawn_growth_entity(&mut world, genome);
        run_growth_to_completion(&mut world, entity);

        assert!(
            world.get::<GrowthState>(entity).is_none(),
            "GrowthState must actually be gone for this test to prove anything"
        );
        let graph = world
            .get::<DevelopmentalGraph>(entity)
            .expect("DevelopmentalGraph must survive GrowthState's removal");
        assert!(
            !graph.nodes.is_empty(),
            "the persisted graph must contain the nodes grown during this run"
        );

        // Stays stable afterward too — running the (now no-op, since
        // GrowthState is gone) system again must not mutate or clear it.
        let node_count_before = graph.nodes.len();
        world.run_system_once(growth_system);
        let node_count_after = world.get::<DevelopmentalGraph>(entity).unwrap().nodes.len();
        assert_eq!(node_count_before, node_count_after);
    }

    /// Phase 5, SX-2d: every newly-grown segment must carry a `SpawnTick`
    /// matching the *current* tick, not `0` or the head's own spawn tick —
    /// `crates/app/src/render.rs` reads this to fade/scale the segment in
    /// over a short window rather than popping it in at full size.
    #[test]
    fn growth_system_tags_new_segments_with_the_current_tick() {
        let mut world = World::new();
        world.insert_resource(metabolism::GlobalAtmosphere {
            ticks: 12_345,
            ..Default::default()
        });
        let genome = genetics::Genome::new_minimal(genetics::GenomeId(1), common::EntityId(0));
        let head = world
            .spawn(physics::ParticleNode::new(Vec2::new(0.0, 0.0), 1.0, 0, 0))
            .id();
        world.entity_mut(head).insert(metabolism::ChemicalEconomy {
            glucose: 1000.0,
            o2: 1000.0,
            co2: 0.0,
            atp: 1000.0,
            max_glucose: 1000.0,
            max_o2: 1000.0,
            max_co2: 1000.0,
            max_atp: 1000.0,
        });
        world.entity_mut(head).insert((
            GrowthState {
                genome,
                next_segment_index: 1,
                ticks_until_next_bud: 0,
                base_bud_interval: 0,
                parent_spine_node: Some(head),
                current_pos: Vec2::new(0.0, 0.0),
                segment_length: 20.0,
                effectors: Vec::new(),
                is_organism_complete: false,
                heading: 0.0,
            },
            DevelopmentalGraph::new(),
        ));

        world.run_system_once(growth_system);

        let mut newly_grown =
            world.query::<(&physics::ParticleNode, &crate::components::SpawnTick)>();
        let found = newly_grown
            .iter(&world)
            .any(|(_, spawn_tick)| spawn_tick.0 == 12_345);
        assert!(
            found,
            "at least one newly-grown segment must be tagged with the current tick"
        );
    }

    #[test]
    fn growth_system_produces_more_than_one_particle_node() {
        let mut world = World::new();
        world.insert_resource(metabolism::GlobalAtmosphere::default());
        let genome = genetics::Genome::new_minimal(genetics::GenomeId(1), common::EntityId(0));
        // The head node itself isn't spawned by `growth_system` (that's
        // `spawning::spawn_organism`'s job) — spawn a stand-in head so
        // `parent_spine_node`-dependent spring/branch logic has something
        // to attach to, matching how `spawn_organism` seeds this state.
        let head = world
            .spawn(physics::ParticleNode::new(Vec2::new(0.0, 0.0), 1.0, 0, 0))
            .id();
        world.entity_mut(head).insert(metabolism::ChemicalEconomy {
            glucose: 1000.0,
            o2: 1000.0,
            co2: 0.0,
            atp: 1000.0,
            max_glucose: 1000.0,
            max_o2: 1000.0,
            max_co2: 1000.0,
            max_atp: 1000.0,
        });
        world.entity_mut(head).insert((
            GrowthState {
                genome,
                next_segment_index: 1,
                ticks_until_next_bud: 0,
                base_bud_interval: 0,
                parent_spine_node: Some(head),
                current_pos: Vec2::new(0.0, 0.0),
                segment_length: 20.0,
                effectors: Vec::new(),
                is_organism_complete: false,
                heading: 0.0,
            },
            DevelopmentalGraph::new(),
        ));
        run_growth_to_completion(&mut world, head);

        let node_count = world.query::<&physics::ParticleNode>().iter(&world).count();
        assert!(
            node_count > 1,
            "expected growth_system to spawn at least one body segment beyond the head"
        );
    }

    #[test]
    fn growth_system_pushes_one_body_graph_node_per_segment() {
        // Phase 3 M6 (persistent as of Phase 4, ADR-P4-01): the sibling
        // `DevelopmentalGraph` component should accumulate exactly one
        // `DevelopmentalNode` per non-branching tick, tracking
        // `next_segment_index`'s growth 1:1 (this fixture's genome
        // deterministically never branches — `Cppn::new()`'s empty
        // regulatory network always decodes `branches: false`).
        let mut world = World::new();
        world.insert_resource(metabolism::GlobalAtmosphere::default());
        let genome = genetics::Genome::new_minimal(genetics::GenomeId(1), common::EntityId(0));
        let entity = spawn_growth_entity(&mut world, genome);

        world.run_system_once(growth_system);
        let after_one = world.get::<DevelopmentalGraph>(entity).unwrap().nodes.len();
        assert_eq!(after_one, 1);

        world.run_system_once(growth_system);
        let after_two = world.get::<DevelopmentalGraph>(entity).unwrap().nodes.len();
        assert_eq!(after_two, 2);
        assert_eq!(
            world.get::<DevelopmentalGraph>(entity).unwrap().nodes[1].parent,
            Some(0)
        );
    }

    #[test]
    fn growth_system_records_a_real_entity_and_chemical_economy_per_segment() {
        // Phase 4, P4-F2: every `DevelopmentalNode` grown by `growth_system`
        // (not `simulate_growth_timeline`'s pure reconstruction) must carry
        // `Some(entity)` pointing at the real `ParticleNode` entity spawned
        // for that position, and that entity must carry its own
        // `metabolism::ChemicalEconomy` pool — proving the graph index can
        // be used to look up live physiological state, not just anatomy.
        let mut world = World::new();
        world.insert_resource(metabolism::GlobalAtmosphere::default());
        let genome = genetics::Genome::new_minimal(genetics::GenomeId(1), common::EntityId(0));
        let entity = spawn_growth_entity(&mut world, genome);

        world.run_system_once(growth_system);
        world.run_system_once(growth_system);

        let graph = world.get::<DevelopmentalGraph>(entity).unwrap();
        assert_eq!(graph.nodes.len(), 2);
        let spine_entity = graph.nodes[1]
            .entity
            .expect("non-head segment should record its live entity");
        assert!(spine_entity != entity);
        assert!(world
            .get::<metabolism::ChemicalEconomy>(spine_entity)
            .is_some());
    }

    #[test]
    fn growth_system_decode_changes_when_the_tips_morphogen_level_is_nonzero() {
        // Phase 6, Epic D (D1a)'s own named testing requirement: a nonzero
        // field reading must actually change decode output vs. a zero
        // baseline. Reuses `genetics::develop::nonzero_life_stage_signal_can_change_the_decode`'s
        // exact hand-built sensitive CPPN and signal magnitude (5.0), since
        // both life-stage and morphogen signals enter the same additive
        // channel (see this crate's `morphogen_field` module doc comment).
        use genetics::cppn::DEFAULT_MUTATION_RATE;
        use genetics::{Cppn, CppnConnection, CppnNode};

        let sensitive_cppn = Cppn {
            nodes: vec![
                CppnNode {
                    activation: brain::ActivationFn::Linear,
                    bias: 0.0,
                    layer: 0,
                },
                CppnNode {
                    activation: brain::ActivationFn::Linear,
                    bias: 0.0,
                    layer: 1,
                },
            ],
            connections: vec![CppnConnection {
                source: 0,
                target: 1,
                weight: 10.0,
                enabled: true,
                innovation: 0,
                mutation_rate: DEFAULT_MUTATION_RATE,
            }],
        };

        let grow_second_segment_with_tip_concentration = |tip_concentration: f32| {
            let mut world = World::new();
            world.insert_resource(metabolism::GlobalAtmosphere::default());
            let mut genome =
                genetics::Genome::new_minimal(genetics::GenomeId(1), common::EntityId(0));
            genome.regulatory_cppn = sensitive_cppn.clone();
            let entity = spawn_growth_entity(&mut world, genome);

            // Grow the first segment (graph index 0 — `spawn_growth_entity`
            // starts from an empty graph with no head node of its own):
            // `parent_spine_node` is `None` at this point, so `field_signal`
            // is 0.0 for both runs — identical so far.
            world.run_system_once(growth_system);
            let graph = world.get::<DevelopmentalGraph>(entity).unwrap();
            let spine_entity = graph.nodes[0].entity.unwrap();

            // Directly control the tip's concentration rather than relying
            // on `morphogen_diffusion_system` (not run in this test) — an
            // isolated, deterministic proof that `growth_system` itself is
            // sensitive to this reading, independent of the diffusion
            // system's own dynamics (covered separately in
            // `morphogen_field`'s own tests).
            world
                .get_mut::<crate::morphogen_field::MorphogenLevel>(spine_entity)
                .unwrap()
                .concentration = tip_concentration;

            world.run_system_once(growth_system);
            let graph = world.get::<DevelopmentalGraph>(entity).unwrap();
            graph.nodes[1].outputs
        };

        let baseline = grow_second_segment_with_tip_concentration(0.0);
        let with_field = grow_second_segment_with_tip_concentration(5.0);
        assert_ne!(baseline, with_field);
    }

    #[test]
    fn growth_system_prunes_an_apoptotic_position_without_spawning_it() {
        // Phase 3 M8 (DEF-002): a hand-built regulatory_cppn found (via a
        // throwaway scan, not guessed) to decode position 1 as `Ganglion`
        // (non-Germinal) with the apoptosis signal firing. `next_segment_index`
        // must still advance past the pruned position, but no `ParticleNode`/
        // graph entry should exist for it.
        use genetics::cppn::DEFAULT_MUTATION_RATE;
        use genetics::{Cppn, CppnConnection, CppnNode};

        let regulatory_cppn = Cppn {
            nodes: vec![
                CppnNode {
                    activation: brain::ActivationFn::Linear,
                    bias: 0.0,
                    layer: 0,
                },
                CppnNode {
                    activation: brain::ActivationFn::Linear,
                    bias: 0.0,
                    layer: 0,
                },
                CppnNode {
                    activation: brain::ActivationFn::Sine,
                    bias: -3.0,
                    layer: 1,
                },
            ],
            connections: vec![CppnConnection {
                source: 0,
                target: 2,
                weight: 5.0,
                enabled: true,
                innovation: 0,
                mutation_rate: DEFAULT_MUTATION_RATE,
            }],
        };
        let mut genome = genetics::Genome::new_minimal(genetics::GenomeId(1), common::EntityId(0));
        genome.regulatory_cppn = regulatory_cppn;

        // Sanity-check the fixture's premise directly before relying on it
        // inside the system-level assertion below.
        let outputs =
            genetics::develop_at_position(&genome.regulatory_cppn, 1, crate::MAX_SEGMENTS);
        assert!(
            outputs.apoptosis,
            "fixture must decode position 1 as apoptotic"
        );
        assert_ne!(outputs.segment_type, genetics::SegmentType::Germinal);

        let mut world = World::new();
        world.insert_resource(metabolism::GlobalAtmosphere::default());
        let entity = spawn_growth_entity(&mut world, genome);

        world.run_system_once(growth_system);

        let state = world.get::<GrowthState>(entity).unwrap();
        assert_eq!(
            state.next_segment_index, 2,
            "index must still advance past a pruned position"
        );
        assert_eq!(
            world.get::<DevelopmentalGraph>(entity).unwrap().nodes.len(),
            0,
            "a pruned position must not be recorded in the graph"
        );
        assert_eq!(
            world.query::<&physics::ParticleNode>().iter(&world).count(),
            0,
            "a pruned position must never spawn a ParticleNode"
        );
    }

    #[test]
    fn simulate_growth_timeline_matches_a_real_growth_system_run() {
        // Phase 3 M13's whole justification for not persisting the Body
        // Graph (see `developmental_graph`'s doc comment) is that
        // `simulate_growth_timeline` faithfully predicts what a real
        // `growth_system` run actually builds. This test is the direct
        // proof: same genome (the M8 apoptosis fixture, which exercises
        // real pruning — the simplest all-`Cppn::new()` fixture never
        // prunes anything, so it wouldn't stress this claim), predicted
        // timeline vs. the graph a real run actually produces, compared.
        //
        // Phase 6, Epic D (D1a) note — re-audited and intentionally not
        // fully restored to a byte-for-byte match: `growth_system` now folds
        // the growing tip's own `morphogen_field::MorphogenLevel` into every
        // segment's decode (see ADR-D1-01), which `simulate_growth_timeline`
        // deliberately does not model (it remains the pure genome+position
        // reconstruction). This test's world never runs
        // `morphogen_diffusion_system`, so every segment after the first
        // reads its parent's *undecayed* seed concentration — a real,
        // reproducible divergence, not a bug. Exact node-for-node equality
        // is retired as of this milestone; the two checks below (the
        // field-free root and first segment still match exactly, and both
        // graphs still complete to a sane, non-empty shape) are what
        // remains a meaningful regression guard here. D1c's own job — a
        // *quantified* measure of how far a live run diverges from the pure
        // replay, so future regressions are caught by magnitude, not just
        // pass/fail — is `real_run_field_signal_divergence_from_the_pure_replay_is_bounded_and_quantified`,
        // below, using this exact same fixture.
        use genetics::cppn::DEFAULT_MUTATION_RATE;
        use genetics::{Cppn, CppnConnection, CppnNode};

        let regulatory_cppn = Cppn {
            nodes: vec![
                CppnNode {
                    activation: brain::ActivationFn::Linear,
                    bias: 0.0,
                    layer: 0,
                },
                CppnNode {
                    activation: brain::ActivationFn::Linear,
                    bias: 0.0,
                    layer: 0,
                },
                CppnNode {
                    activation: brain::ActivationFn::Sine,
                    bias: -3.0,
                    layer: 1,
                },
            ],
            connections: vec![CppnConnection {
                source: 0,
                target: 2,
                weight: 5.0,
                enabled: true,
                innovation: 0,
                mutation_rate: DEFAULT_MUTATION_RATE,
            }],
        };
        let mut genome = genetics::Genome::new_minimal(genetics::GenomeId(1), common::EntityId(0));
        genome.regulatory_cppn = regulatory_cppn.clone();

        let predicted = crate::simulate_growth_timeline(&regulatory_cppn);

        // Real run: seed the graph with the head node exactly as
        // `spawning::spawn_organism` does, then step growth_system to
        // completion. As of Phase 4 (ADR-P4-01) the graph is a persistent
        // sibling component, not nested in `GrowthState` — it's no longer
        // necessary to capture it before `GrowthState` disappears, since it
        // simply survives that removal now; just read it once growth ends.
        let mut world = World::new();
        world.insert_resource(metabolism::GlobalAtmosphere::default());
        let head_outputs = genetics::develop_at_position(&regulatory_cppn, 0, crate::MAX_SEGMENTS);
        let mut graph = crate::DevelopmentalGraph::new();
        graph.push(
            head_outputs.segment_type,
            head_outputs,
            None,
            false,
            0,
            None,
        );
        let entity = world
            .spawn((
                metabolism::ChemicalEconomy {
                    glucose: 1000.0,
                    o2: 1000.0,
                    co2: 0.0,
                    atp: 1000.0,
                    max_glucose: 1000.0,
                    max_o2: 1000.0,
                    max_co2: 1000.0,
                    max_atp: 1000.0,
                },
                GrowthState {
                    genome,
                    next_segment_index: 1,
                    ticks_until_next_bud: 0,
                    base_bud_interval: 0,
                    parent_spine_node: None,
                    current_pos: Vec2::new(0.0, 0.0),
                    segment_length: 20.0,
                    effectors: Vec::new(),
                    is_organism_complete: head_outputs.segment_type == genetics::SegmentType::Tail,
                    heading: 0.0,
                },
                graph,
            ))
            .id();

        for _ in 0..(crate::MAX_SEGMENTS * 40) {
            if world.get::<GrowthState>(entity).is_none() {
                break;
            }
            world.run_system_once(growth_system);
        }
        let last_graph = world.get::<DevelopmentalGraph>(entity).unwrap();

        // Both graphs still complete to a sane, bounded, non-empty shape.
        assert!(!predicted.nodes.is_empty());
        assert!(!last_graph.nodes.is_empty());
        assert!(last_graph.nodes.len() <= crate::MAX_SEGMENTS * 3); // spine + bilateral fins

        // The head (position 0) and the first grown segment (position 1)
        // are decoded before any `MorphogenLevel` exists to read from
        // (`state.parent_spine_node` is `None` until the first spine node is
        // spawned), so these two positions are still field-free on both
        // sides and must still match exactly.
        for (p, r) in predicted.nodes.iter().zip(last_graph.nodes.iter()).take(2) {
            assert_eq!(p.position, r.position);
            assert_eq!(p.role, r.role);
            assert_eq!(p.is_branch, r.is_branch);
        }
    }

    #[test]
    fn real_run_field_signal_divergence_from_the_pure_replay_is_bounded_and_quantified() {
        // Phase 6, Epic D (D1c): `PHASE4_EPIC4_MORPHOGEN_ROADMAP.md` §3.1's
        // own named requirement — a comparison test that *quantifies* how
        // far a live run's decode diverges from `simulate_growth_timeline`'s
        // pure zero-field replay, for a fixture genome with nonzero field
        // input, "so future regressions are caught by magnitude, not just a
        // binary pass/fail." Reuses the exact same M8 apoptosis-fixture CPPN
        // as `simulate_growth_timeline_matches_a_real_growth_system_run`
        // (same genome, same setup), so this is a direct continuation of
        // that test's own finding, not a separate discovery.
        use genetics::cppn::DEFAULT_MUTATION_RATE;
        use genetics::{Cppn, CppnConnection, CppnNode};

        let regulatory_cppn = Cppn {
            nodes: vec![
                CppnNode {
                    activation: brain::ActivationFn::Linear,
                    bias: 0.0,
                    layer: 0,
                },
                CppnNode {
                    activation: brain::ActivationFn::Linear,
                    bias: 0.0,
                    layer: 0,
                },
                CppnNode {
                    activation: brain::ActivationFn::Sine,
                    bias: -3.0,
                    layer: 1,
                },
            ],
            connections: vec![CppnConnection {
                source: 0,
                target: 2,
                weight: 5.0,
                enabled: true,
                innovation: 0,
                mutation_rate: DEFAULT_MUTATION_RATE,
            }],
        };
        let mut genome = genetics::Genome::new_minimal(genetics::GenomeId(1), common::EntityId(0));
        genome.regulatory_cppn = regulatory_cppn.clone();

        let predicted = crate::simulate_growth_timeline(&regulatory_cppn);

        let mut world = World::new();
        world.insert_resource(metabolism::GlobalAtmosphere::default());
        let head_outputs = genetics::develop_at_position(&regulatory_cppn, 0, crate::MAX_SEGMENTS);
        let mut graph = crate::DevelopmentalGraph::new();
        graph.push(
            head_outputs.segment_type,
            head_outputs,
            None,
            false,
            0,
            None,
        );
        let entity = world
            .spawn((
                metabolism::ChemicalEconomy {
                    glucose: 1000.0,
                    o2: 1000.0,
                    co2: 0.0,
                    atp: 1000.0,
                    max_glucose: 1000.0,
                    max_o2: 1000.0,
                    max_co2: 1000.0,
                    max_atp: 1000.0,
                },
                GrowthState {
                    genome,
                    next_segment_index: 1,
                    ticks_until_next_bud: 0,
                    base_bud_interval: 0,
                    parent_spine_node: None,
                    current_pos: Vec2::new(0.0, 0.0),
                    segment_length: 20.0,
                    effectors: Vec::new(),
                    is_organism_complete: head_outputs.segment_type == genetics::SegmentType::Tail,
                    heading: 0.0,
                },
                graph,
            ))
            .id();

        for _ in 0..(crate::MAX_SEGMENTS * 40) {
            if world.get::<GrowthState>(entity).is_none() {
                break;
            }
            world.run_system_once(growth_system);
        }
        let last_graph = world.get::<DevelopmentalGraph>(entity).unwrap();

        // Magnitude 1: total node-count delta. Empirically 14 (predicted)
        // vs. 12 (real) for this fixture at the time D1a landed — asserted
        // here as a bounded range, not an exact pinned number, since the
        // point is catching an implausible blow-up (e.g. a future change
        // that makes the field signal dominate decode so badly the body
        // plan collapses to 1 segment or explodes past `MAX_SEGMENTS`), not
        // pinning today's precise value as sacred.
        let node_count_delta = (predicted.nodes.len() as i64 - last_graph.nodes.len() as i64).abs();
        assert!(
            node_count_delta > 0,
            "expected D1a/D1b's field signal to visibly change the grown timeline length for this fixture — if this now passes with delta 0, the fixture may have stopped exercising real divergence"
        );
        assert!(
            node_count_delta < crate::MAX_SEGMENTS as i64,
            "divergence magnitude implausibly large ({node_count_delta} segments) — this should catch a future regression that makes the field signal dominate decode, not just confirm any divergence exists"
        );

        // Magnitude 2: the first position where the two timelines actually
        // disagree. Must be position 2 or later — position 0 (head) and
        // position 1 (grown before any `MorphogenLevel` exists to read
        // from) are field-free on both sides and must never be where the
        // divergence starts.
        let first_divergent_position = predicted
            .nodes
            .iter()
            .zip(last_graph.nodes.iter())
            .position(|(p, r)| p.role != r.role || p.position != r.position);
        if let Some(index) = first_divergent_position {
            assert!(
                index >= 2,
                "divergence must not start before position 2 — positions 0 and 1 are field-free on both sides by construction, got first divergence at index {index}"
            );
        }
    }

    /// Phase 6, Epic A (milestone A2): `producer_growth_system` used
    /// unseeded `fastrand::` for both its target-node pick and its spawn
    /// offset. Same fixed seed, run twice from independently-constructed
    /// `World`s, must now produce an identical new-node offset — proving the
    /// `fastrand`→`SimRng` migration preserved determinism rather than
    /// breaking it.
    #[test]
    fn producer_growth_system_is_deterministic_for_a_given_seed() {
        fn run_once() -> common::Vec2 {
            let mut world = World::new();
            world.insert_resource(common::SimRng::from_seed(99));
            world.insert_resource(metabolism::GlobalAtmosphere {
                co2: 1000.0,
                ..Default::default()
            });

            let head_pos = common::Vec2::new(0.0, 0.0);
            world.spawn((
                ecology::Diet::Producer,
                metabolism::ChemicalEconomy {
                    glucose: 100_000.0,
                    o2: 0.0,
                    co2: 0.0,
                    atp: 100_000.0,
                    max_glucose: 100_000.0,
                    max_o2: 100_000.0,
                    max_co2: 100_000.0,
                    max_atp: 100_000.0,
                },
                metabolism::Metabolism {
                    mass: 1.0,
                    base_rate: 1.0,
                    is_plant: true,
                },
                physics::ParticleNode::new(head_pos, 1.0, 0, 0),
            ));

            world.run_system_once(producer_growth_system);

            let mut query = world.query::<(&OrganismColor, &physics::ParticleNode)>();
            query
                .iter(&world)
                .map(|(_, node)| node.position)
                .find(|&pos| pos != head_pos)
                .expect("producer_growth_system should have spawned a new leaf node")
        }

        assert_eq!(run_once(), run_once());
    }
}
