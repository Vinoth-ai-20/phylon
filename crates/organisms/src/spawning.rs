use crate::components::{Generation, GrowthState, OrganismColor, SpawnTick};
use common::Vec2;

/// Spawns an organism's zygote based on its genome.
pub fn spawn_organism(
    world: &mut bevy_ecs::world::World,
    genome: &genetics::Genome,
    start_pos: Vec2,
    diet: ecology::Diet,
    category: ecology::EcologicalCategory,
    generation: u32,
    spawn_tick: u64,
) -> bevy_ecs::entity::Entity {
    use physics::ParticleNode;
    use rand::Rng;

    let segment_length = 20.0;
    let heading = rand::thread_rng().gen_range(0.0..std::f32::consts::TAU);

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
    let head_node = world.spawn_empty().id();
    world.entity_mut(head_node).insert((
        ParticleNode::new(start_pos, 1.0, head_seg_u32, head_node.index_u32()),
        OrganismColor(color),
    ));

    // Attach biology to the head node.
    world.entity_mut(head_node).insert((
        metabolism::ChemicalEconomy {
            glucose: 1500.0, // Yolk reserve for embryogenesis
            o2: 100.0,       // Small initial breath
            co2: 0.0,
            atp: 1500.0, // Initial energy for first heartbeats/synapses
            max_glucose: 20000.0,
            max_o2: 2000.0,
            max_co2: 2000.0,
            max_atp: 20000.0,
        },
        metabolism::Age {
            ticks: 0,
            max_lifespan: 10000,
        },
        metabolism::Metabolism {
            mass: 2.0, // Initial mass of the head node
            base_rate: 0.01,
            is_plant: diet == ecology::Diet::Producer,
        },
        Generation(generation),
        SpawnTick(spawn_tick),
        diet,
        category,
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
            current_pos: start_pos + Vec2::new(heading.cos(), heading.sin()) * -segment_length,
            segment_length,
            effectors: Vec::new(),
            color,
            heading,
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

    head_node
}
