use crate::components::{Generation, GrowthState, LifeStage, OrganismColor, SpawnTick};
use common::Vec3;

/// Spawns an organism's zygote based on its genome.
///
/// `rng` must be the caller's handle onto the single shared `common::SimRng`
/// resource (see its doc comment) — never a fresh `rand::thread_rng()` —
/// so spawn placement is reproducible for a given experiment seed.
#[allow(clippy::too_many_arguments)]
pub fn spawn_organism(
    world: &mut bevy_ecs::world::World,
    genome: &genetics::Genome,
    start_pos: Vec3,
    diet: ecology::Diet,
    category: ecology::EcologicalCategory,
    generation: u32,
    spawn_tick: u64,
    rng: &mut impl rand::Rng,
) -> bevy_ecs::entity::Entity {
    use physics::ParticleNode;

    let segment_length = 20.0;
    let heading = rng.gen_range(0.0..std::f32::consts::TAU);
    let forward = Vec3::new(heading.cos(), heading.sin(), 0.0);

    // Decode the head node (body position 0) through the same regulatory
    // pipeline every later segment uses — no special-cased "head" template.
    // Color is likewise emergent per-position pigmentation, never
    // genome-stored data.
    let expressed_regulatory_cppn = genome.expressed_regulatory_cppn();
    let head_outputs =
        genetics::develop_at_position(&expressed_regulatory_cppn, 0, crate::MAX_SEGMENTS);
    let color = head_outputs.pigment;
    // The same decode-to-physics mapping `growth_system` uses for every
    // later segment (see `developmental_graph::compile_segment`) — no
    // separate head-specific lookup.
    let head_compiled = crate::compile_segment(head_outputs.segment_type);

    // Spawn the head node at start_pos (gene index 0).
    let head_node = world.spawn_empty().id();
    world.entity_mut(head_node).insert((
        ParticleNode::new(
            start_pos,
            1.0,
            head_compiled.particle_segment_type,
            head_node.index(),
        ),
        OrganismColor(color),
    ));

    // The Body Graph's root node: index 0, no parent, not a branch.
    // `entity` is `head_node` itself — the same entity per-segment
    // physiology systems look up via `DevelopmentalGraph::root()`.
    let mut graph = crate::developmental_graph::DevelopmentalGraph::new();
    graph.push(
        head_outputs.segment_type,
        head_outputs,
        None,
        false,
        0,
        Some(head_node),
    );

    // Attach biology to the head node.
    world.entity_mut(head_node).insert((
        metabolism::ChemicalEconomy {
            glucose: 50000.0,
            o2: 10000.0,
            co2: 0.0,
            atp: 50000.0,
            max_glucose: 100000.0,
            max_o2: 10000.0,
            max_co2: 10000.0,
            max_atp: 100000.0,
        },
        metabolism::Age {
            ticks: 0,
            max_lifespan: 10000,
        },
        metabolism::Metabolism {
            mass: 10.0,
            base_rate: 0.005,
            is_plant: diet == ecology::Diet::Producer,
        },
        Generation(generation),
        SpawnTick(spawn_tick),
        diet,
        category,
        // The head starts as the organism's own growing tip, seeded exactly
        // like every later segment `growth_system` spawns — see
        // `morphogen_field`'s doc comment.
        crate::morphogen_field::MorphogenLevel {
            concentration: crate::morphogen_field::MORPHOGEN_SEED_CONCENTRATION,
        },
        reproduction::ReproductionStrategy {
            energy_threshold: 900.0,
            energy_cost: 500.0,
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
            current_pos: start_pos + forward * -segment_length,
            segment_length,
            effectors: Vec::new(),
            is_organism_complete: head_outputs.segment_type == genetics::SegmentType::Tail,
            forward,
            dorsal: Vec3::Z,
        },
        // A sibling component, not nested in `GrowthState` — it survives
        // that component's removal once growth completes, since
        // `growth_system` writes to it directly.
        graph,
        // Every organism starts `Juvenile`;
        // `organisms::life_cycle::life_stage_system` promotes it later.
        LifeStage::Juvenile,
        sensing::HeadVision {
            range: 250.0,
            fov: std::f32::consts::PI * 0.8, // ~144 degrees
            last_forward: common::Vec3::X,
            dorsal: common::Vec3::Z,
            // Body length isn't known ahead of growth (growth decodes
            // segment-by-segment rather than reading a fixed-length
            // sequence) — use the fixed growth ceiling as a safe
            // upper-bound estimate.
            self_occlusion_radius: crate::MAX_SEGMENTS as f32 * segment_length * 1.5,
            locked_target: None,
        },
    ));

    // Attach Genome as a standalone ECS component so Inspector and ExportGenome can query it.
    world.entity_mut(head_node).insert(genome.clone());

    // Attach extended physiology components required by the Inspector.
    world.entity_mut(head_node).insert((
        metabolism::Health {
            current: 100.0,
            max: 100.0,
        },
        metabolism::Hydration {
            level: 1.0,
            loss_rate: 0.0001,
        },
        metabolism::BodyTemperature {
            current: 22.0,
            ideal: 22.0,
        },
        behavior::BehaviorState::Idle,
        behavior::CurrentGoal {
            description: "Initialising".to_string(),
            target_entity: None,
        },
    ));

    head_node
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
/// The head node carries [`metabolism::ChemicalEconomy`], [`metabolism::Age`], and
/// [`metabolism::Metabolism`] components so the inspector sidebar can display
/// biological metrics.
///
/// # CPPN branching backlog note
///
/// The CPPN's `branching_signal` (output index 5) threshold is too rarely
/// exceeded in random genomes. A targeted tuning pass is a possible future
/// improvement.
pub fn spawn_proto_fish(
    world: &mut bevy_ecs::world::World,
    pos: Vec3,
    diet: ecology::Diet,
    category: ecology::EcologicalCategory,
    generation: u32,
    spawn_tick: u64,
    rng: &mut impl rand::Rng,
) {
    use physics::{ConstraintType, ParticleNode, Spring};

    // Geometry constants — all in world units
    let segment_len: f32 = 20.0;
    let fin_spread: f32 = 15.0;
    let heading: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
    let dir = Vec3::new(heading.cos(), heading.sin(), 0.0);

    // ── Spine (5 nodes along −X axis, head at pos, tail to the left) ──────
    // Segment types: Head(0), Torso(1), Torso(1), Torso(1), Tail(3)
    let spine_types: [u32; 5] = [0, 1, 1, 1, 3];
    let proto_color = [0.15, 0.72, 0.45]; // The original green used for debug proto-fish

    let mut head_node_id = 0;
    let spine_nodes: Vec<bevy_ecs::entity::Entity> = spine_types
        .iter()
        .enumerate()
        .map(|(i, &seg_type)| {
            let p = pos + dir * (-(i as f32) * segment_len);
            let ent = world.spawn_empty().id();
            if i == 0 {
                head_node_id = ent.index();
            }
            world.entity_mut(ent).insert((
                ParticleNode::new(p, 1.0, seg_type, head_node_id),
                OrganismColor(proto_color),
            ));
            ent
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
    let fin_root_pos = pos + dir * (-2.0 * segment_len);

    // Same bilateral fin-placement formula `growth_system` uses, with a
    // fixed `Vec3::Z` dorsal (this preset has no per-organism
    // `GrowthState`/dorsal of its own) — equivalent to the simpler 2D-only
    // `Vec3::new(-dir.y, dir.x, 0.0)` formula for this fixed dorsal.
    let perp = crate::bilateral_fin_direction(Vec3::Z, dir);
    let f_up_pos = fin_root_pos + perp * fin_spread;
    let f_dn_pos = fin_root_pos + perp * -fin_spread;

    let f_up = world
        .spawn((
            ParticleNode::new(f_up_pos, 0.5, 4, head_node_id),
            OrganismColor(proto_color),
        ))
        .id();
    let f_dn = world
        .spawn((
            ParticleNode::new(f_dn_pos, 0.5, 4, head_node_id),
            OrganismColor(proto_color),
        ))
        .id();

    // Hinge (Rigid bone)
    world.spawn((
        Spring {
            node_a: fin_root,
            node_b: f_up,
            constraint_type: ConstraintType::Rigid,
            rest_length: fin_spread,
            base_length: fin_spread,
            stiffness: 20.0,
            damping: 0.5,
            actuation_amplitude: 0.0,
            actuation_phase: 0.0,
            breaking_strain: 5.0,
            is_fin: 1,
        },
        OrganismColor(proto_color),
    ));
    world.spawn((
        Spring {
            node_a: fin_root,
            node_b: f_dn,
            constraint_type: ConstraintType::Rigid,
            rest_length: fin_spread,
            base_length: fin_spread,
            stiffness: 20.0,
            damping: 0.5,
            actuation_amplitude: 0.0,
            actuation_phase: 0.0,
            breaking_strain: 5.0,
            is_fin: 1,
        },
        OrganismColor(proto_color),
    ));

    // Muscle (Elastic actuator) connecting to previous spine node
    let prev_spine = spine_nodes[1];
    let muscle_rest_len = (segment_len * segment_len + fin_spread * fin_spread).sqrt();

    world.spawn((
        Spring {
            node_a: prev_spine,
            node_b: f_up,
            constraint_type: ConstraintType::Elastic,
            rest_length: muscle_rest_len,
            base_length: muscle_rest_len,
            stiffness: 5.0,
            damping: 0.3,
            actuation_amplitude: 8.0,
            actuation_phase: 0.0, // Phase 0
            breaking_strain: 5.0,
            is_fin: 0,
        },
        OrganismColor(proto_color),
    ));
    world.spawn((
        Spring {
            node_a: prev_spine,
            node_b: f_dn,
            constraint_type: ConstraintType::Elastic,
            rest_length: muscle_rest_len,
            base_length: muscle_rest_len,
            stiffness: 5.0,
            damping: 0.3,
            actuation_amplitude: 8.0,
            actuation_phase: std::f32::consts::PI, // Opposing phase → flap
            breaking_strain: 5.0,
            is_fin: 0,
        },
        OrganismColor(proto_color),
    ));

    // ── Biological state on the head node ──────────────────────────────────
    world.entity_mut(spine_nodes[0]).insert((
        metabolism::ChemicalEconomy {
            glucose: 10000.0,
            o2: 10000.0,
            co2: 0.0,
            atp: 10000.0,
            max_glucose: 100000.0,
            max_o2: 10000.0,
            max_co2: 10000.0,
            max_atp: 100000.0,
        },
        metabolism::Age {
            ticks: 0,
            max_lifespan: 10000,
        },
        metabolism::Metabolism {
            mass: 15.0, // approx mass of 5 spine + 2 fin nodes
            base_rate: 0.05,
            is_plant: diet == ecology::Diet::Producer,
        },
        Generation(generation),
        SpawnTick(spawn_tick),
        diet,
        category,
    ));
}
