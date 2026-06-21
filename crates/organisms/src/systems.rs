use crate::components::{GrowthState, OrganismColor};
use bevy_ecs::prelude::{Commands, Entity, Query};
use common::Vec2;

/// System that builds out the organism's body sequentially, one Hox gene per tick.
///
/// Topology produced:
/// - **Spine**: single node per axial segment, connected end-to-end by `Rigid` bones.
///   No two-node-per-segment pairs, no cross springs, no closed rectangular loops.
/// - **Fins**: when a gene's `branching_signal > 0.0`, two fin nodes are sprouted
///   laterally from the spine node and attached via `Rotational` hinges.
pub fn growth_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut GrowthState)>,
    node_query: Query<&physics::ParticleNode>,
    spring_query: Query<&physics::Spring>,
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
                    // Evolve node properties via CPPN
                    if !state.genome.nodes.is_empty() {
                        let w_inputs = [
                            (i as f32) / (total_nodes as f32),
                            (i as f32) / (total_nodes as f32),
                        ];
                        let w_outputs = state.genome.evaluate(&w_inputs);
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

            for i in 0..total_nodes {
                // Connections can only target hidden and output nodes (not inputs)
                for j in input_count..total_nodes {
                    let mut weight = 0.0;

                    // Neocortex: Evolved CPPN Weights
                    if !state.genome.nodes.is_empty() {
                        let w_inputs = [
                            (i as f32) / (total_nodes as f32),
                            (j as f32) / (total_nodes as f32),
                        ];
                        let w_outputs = state.genome.evaluate(&w_inputs);
                        if !w_outputs.is_empty() {
                            weight += w_outputs[0] * 1.5;
                        }
                    }

                    if weight.abs() > 0.01 {
                        synapses.push(brain::CtrnnSynapse {
                            source: i as u32,
                            target: j as u32,
                            weight,
                            _padding: 0,
                        });
                    }
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
                diffusion::SignalEmitter::default(),
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
                // Step one segment_length in the heading direction from where the
                // parent node actually is right now.
                parent_node.position
                    + Vec2::new(state.heading.cos(), state.heading.sin()) * -state.segment_length
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
        if can_branch && gene.branching_signal > 0.0 && state.parent_spine_node.is_some() {
            let fin_spread = state.segment_length * 0.75;
            let dir = Vec2::new(state.heading.cos(), state.heading.sin());
            let perp = Vec2::new(-dir.y, dir.x);

            let f_up_pos = spawn_pos + perp * fin_spread;
            let f_dn_pos = spawn_pos + perp * -fin_spread;

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
                OrganismColor(state.color),
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
                OrganismColor(state.color),
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
                        actuation_amplitude: gene.actuation_amplitude,
                        actuation_phase: 0.0,
                        breaking_strain: 2.0,
                        is_fin: 0,
                    },
                    OrganismColor(state.color),
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
                        actuation_amplitude: gene.actuation_amplitude,
                        actuation_phase: std::f32::consts::PI, // Opposing phase → flap
                        breaking_strain: 2.0,
                        is_fin: 0,
                    },
                    OrganismColor(state.color),
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
    }
}
