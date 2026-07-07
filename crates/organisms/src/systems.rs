use crate::components::{GrowthState, OrganismColor};
use crate::developmental_graph::DevelopmentalGraph;
use bevy_ecs::prelude::{Commands, Entity, Query};
use common::Vec2;

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
pub fn growth_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut GrowthState, &mut DevelopmentalGraph)>,
    node_query: Query<&physics::ParticleNode>,
    spring_query: Query<&physics::Spring>,
    chem_query: Query<&metabolism::ChemicalEconomy>,
) {
    use genetics::SegmentType;
    use physics::{ParticleNode, Spring};

    for (entity, mut state, mut graph) in query.iter_mut() {
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

            let initial_atp = chem_query.get(entity).map(|c| c.atp).unwrap_or(0.0);

            commands.entity(entity).insert((
                brain::Brain::new(
                    brain::BrainId(0),
                    nodes,
                    synapses,
                    input_count,
                    output_count,
                ),
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
        let outputs = genetics::develop_at_position(
            &expressed_regulatory_cppn,
            state.next_segment_index,
            crate::MAX_SEGMENTS,
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
                ParticleNode::new(spawn_pos, 1.0, seg_u32, entity.index()),
                OrganismColor(outputs.pigment),
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
            graph.push(
                SegmentType::Fin,
                outputs,
                Some(spine_graph_index),
                true,
                current_position,
            );
            graph.push(
                SegmentType::Fin,
                outputs,
                Some(spine_graph_index),
                true,
                current_position,
            );

            let fin_spread = state.segment_length * 0.75;
            let dir = Vec2::new(state.heading.cos(), state.heading.sin());
            let perp = Vec2::new(-dir.y, dir.x);

            let f_up_pos = spawn_pos + perp * fin_spread;
            let f_dn_pos = spawn_pos + perp * -fin_spread;

            let f_up = commands
                .spawn((
                    ParticleNode::new(f_up_pos, 0.5, 4, entity.index()),
                    OrganismColor(outputs.pigment),
                ))
                .id();
            let f_dn = commands
                .spawn((
                    ParticleNode::new(f_dn_pos, 0.5, 4, entity.index()),
                    OrganismColor(outputs.pigment),
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
            let target_node = all_nodes[fastrand::usize(..all_nodes.len())];

            let offset = common::Vec2::new(
                (fastrand::f32() - 0.5) * 20.0,
                fastrand::f32() * 20.0 + 5.0, // Upward bias
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
        let genome = genetics::Genome::new_minimal(genetics::GenomeId(1), common::EntityId(0));
        let entity = spawn_growth_entity(&mut world, genome);
        run_growth_to_completion(&mut world, entity);
        assert!(world.get::<brain::Brain>(entity).is_some());
    }

    #[test]
    fn developmental_graph_survives_growth_completion_and_removal_of_growth_state() {
        // Phase 4, ADR-P4-01's whole point: unlike pre-Phase-4 behavior
        // (where the graph was nested in `GrowthState` and discarded the
        // moment it was removed), the persistent `DevelopmentalGraph`
        // sibling component must still be present, non-empty, and
        // unchanged after `GrowthState` is gone.
        let mut world = World::new();
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

    #[test]
    fn growth_system_produces_more_than_one_particle_node() {
        let mut world = World::new();
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
        // timeline vs. the graph a real run actually produces, compared
        // node-for-node.
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
        let head_outputs = genetics::develop_at_position(&regulatory_cppn, 0, crate::MAX_SEGMENTS);
        let mut graph = crate::DevelopmentalGraph::new();
        graph.push(head_outputs.segment_type, head_outputs, None, false, 0);
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

        assert_eq!(predicted.nodes.len(), last_graph.nodes.len());
        for (p, r) in predicted.nodes.iter().zip(last_graph.nodes.iter()) {
            assert_eq!(p.position, r.position);
            assert_eq!(p.role, r.role);
            assert_eq!(p.is_branch, r.is_branch);
        }
    }
}
