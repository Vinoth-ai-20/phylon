use bevy::{
    prelude::*,
    render::{
        render_resource::{
            BindGroup, CachedComputePipelineId, ComputePipelineDescriptor, PipelineCache,
            ShaderStages, SpecializedComputePipeline, SpecializedComputePipelines,
        },
        renderer::{RenderContext, RenderDevice},
        Extract, ExtractSchedule, RenderApp, RenderSystems,
    },
};
use std::collections::HashMap;

use bevy::window::PrimaryWindow;
use crossbeam_channel::Sender;
use std::sync::atomic::Ordering;

#[derive(Resource)]
pub struct PositionSender(pub Sender<Vec<u8>>);

#[derive(Resource)]
pub struct NodeEntitiesSender(pub Sender<Vec<Entity>>);

#[derive(Resource)]
pub struct BrainDataSender(pub Sender<Vec<u8>>);

/// The plugin that bridges our custom WGPU shaders into Bevy's Render World.
pub struct RenderingPlugin {
    pub gpu_pos_tx: Sender<Vec<u8>>,
    pub gpu_node_entities_tx: Sender<Vec<Entity>>,
    pub brain_data_tx: Sender<Vec<u8>>,
    pub diffusion_data_tx: Sender<Vec<f32>>,
}

#[derive(Resource)]
pub struct DiffusionDataSender(pub Sender<Vec<f32>>);

impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        // Extract the RenderApp from the Main App
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        // 1. Add extraction system
        render_app.add_systems(
            ExtractSchedule,
            (
                extract_simulation_data,
                extract_camera_data,
                extract_brain_data,
                extract_diffusion_data,
                extract_splat_data,
                extract_overlay_config,
            ),
        );

        // Insert our senders
        render_app.insert_resource(PositionSender(self.gpu_pos_tx.clone()));
        render_app.insert_resource(NodeEntitiesSender(self.gpu_node_entities_tx.clone()));
        render_app.insert_resource(BrainDataSender(self.brain_data_tx.clone()));
        render_app.insert_resource(DiffusionDataSender(self.diffusion_data_tx.clone()));
        render_app.init_resource::<PreviousNodeEntities>();

        // 2. Add our Render Systems
        render_app
            .init_resource::<PhylonPipelineCache>()
            .init_resource::<SimulationBindGroups>()
            .init_resource::<BrainBindGroups>()
            .add_systems(
                bevy::render::Render,
                (prepare_simulation_bind_groups, prepare_brain_bind_groups)
                    .in_set(RenderSystems::PrepareBindGroups),
            )
            .add_systems(
                bevy::render::Render,
                (queue_physics_pipelines, queue_brain_pipelines).in_set(RenderSystems::Queue),
            )
            .add_systems(
                bevy::render::Render,
                (
                    dispatch_physics_compute
                        .before(bevy::core_pipeline::core_2d::main_opaque_pass_2d),
                    dispatch_brain_compute
                        .before(bevy::core_pipeline::core_2d::main_opaque_pass_2d),
                    dispatch_diffusion_compute
                        .before(bevy::core_pipeline::core_2d::main_opaque_pass_2d),
                    dispatch_splat_compute
                        .before(bevy::core_pipeline::core_2d::main_opaque_pass_2d),
                    dispatch_field_render
                        .before(bevy::core_pipeline::core_2d::main_transparent_pass_2d),
                    dispatch_splat_render
                        .before(bevy::core_pipeline::core_2d::main_transparent_pass_2d)
                        .after(dispatch_field_render),
                )
                    .in_set(RenderSystems::Render),
            );
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<PhylonPhysicsPipeline>()
            .init_resource::<SpecializedComputePipelines<PhylonPhysicsPipeline>>()
            .init_resource::<PhylonBrainPipeline>()
            .init_resource::<SpecializedComputePipelines<PhylonBrainPipeline>>()
            .init_resource::<SdfRendererResource>()
            .init_resource::<DiffusionRendererResource>()
            .init_resource::<SplatRendererResource>()
            .init_resource::<FieldRendererResource>();
    }
}

#[derive(Resource, Default)]
pub struct ExtractedPhysicsData {
    pub needs_sync: bool,
    pub nodes: Vec<gpu::physics_pipeline::GpuParticleNode>,
    pub springs: Vec<gpu::physics_pipeline::GpuPhysicsSpring>,
    pub node_colors: Vec<[f32; 3]>,
}

#[derive(Resource, Default)]
pub struct PreviousNodeEntities(pub Vec<Entity>);

/// Copies data from the Main World to the Render World
fn extract_simulation_data(
    mut commands: Commands,
    query_nodes: Extract<
        Query<(
            Entity,
            &physics::ParticleNode,
            Option<&organisms::components::OrganismColor>,
        )>,
    >,
    query_springs: Extract<Query<&physics::Spring>>,
    sender: Res<NodeEntitiesSender>,
    mut prev_entities: ResMut<PreviousNodeEntities>,
) {
    let mut entity_to_index = HashMap::new();
    let mut gpu_nodes = Vec::new();
    let mut node_colors = Vec::new();
    let mut node_entities = Vec::new();

    for (entity, node, genome) in query_nodes.iter() {
        entity_to_index.insert(entity, gpu_nodes.len() as u32);
        gpu_nodes.push(gpu::physics_pipeline::GpuParticleNode {
            position: [node.position.x, node.position.y],
            velocity: [node.velocity.x, node.velocity.y],
            force: [node.force.x, node.force.y],
            mass: node.mass,
            organism_id: node.organism_id,
        });
        node_entities.push(entity);

        let mut color = [0.2, 0.8, 0.2]; // Default green
        if let Some(org_color) = genome {
            color = org_color.0;
        }
        node_colors.push(color);
    }

    let mut gpu_springs = Vec::new();
    for spring in query_springs.iter() {
        let Some(&idx_a) = entity_to_index.get(&spring.node_a) else {
            continue;
        };
        let Some(&idx_b) = entity_to_index.get(&spring.node_b) else {
            continue;
        };

        let constraint_type = match spring.constraint_type {
            physics::ConstraintType::Elastic => 0,
            physics::ConstraintType::Rigid => 1,
            physics::ConstraintType::Passive => 2,
            physics::ConstraintType::Rotational => 3,
        };

        gpu_springs.push(gpu::physics_pipeline::GpuPhysicsSpring {
            node_a: idx_a,
            node_b: idx_b,
            constraint_type,
            rest_length: spring.rest_length,
            base_length: spring.base_length,
            stiffness: spring.stiffness,
            damping: spring.damping,
            actuation_amplitude: spring.actuation_amplitude,
            actuation_phase: spring.actuation_phase,
            breaking_strain: spring.breaking_strain,
            is_fin: spring.is_fin,
            _padding_2: 0,
        });
    }

    let _ = sender.0.send(node_entities.clone());

    let needs_sync = prev_entities.0 != node_entities;
    prev_entities.0 = node_entities;

    commands.insert_resource(ExtractedPhysicsData {
        needs_sync,
        nodes: gpu_nodes,
        springs: gpu_springs,
        node_colors,
    });
}

#[derive(Resource, Default)]
pub struct ExtractedCameraData {
    pub position: common::Vec2,
    pub zoom: f32,
    pub screen_size: [f32; 2],
    pub msaa_samples: u32,
}

type CameraExtractQuery<'w> = (
    &'w Camera,
    &'w GlobalTransform,
    Option<&'w bevy::prelude::Projection>,
    Option<&'w Msaa>,
);

pub fn extract_camera_data(
    mut commands: Commands,
    camera_query: Extract<Query<CameraExtractQuery>>,
    window_query: Extract<Query<&Window, With<PrimaryWindow>>>,
) {
    let mut data = ExtractedCameraData {
        position: common::Vec2::ZERO,
        zoom: 1.0,
        screen_size: [800.0, 600.0],
        msaa_samples: Msaa::default().samples(),
    };

    if let Some(window) = window_query.iter().next() {
        data.screen_size = [window.width(), window.height()];
    }

    if let Some((_camera, transform, projection, msaa)) = camera_query.iter().next() {
        let pos = transform.translation();
        data.position = common::Vec2::new(pos.x, pos.y);
        if let Some(bevy::prelude::Projection::Orthographic(proj)) = projection {
            data.zoom = proj.scale;
        }
        if let Some(msaa) = msaa {
            data.msaa_samples = msaa.samples();
        }
    }

    commands.insert_resource(data);
}

// --- PIPELINE & RESOURCES ---

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum PassType {
    ComputeForces,
    Integrate,
    PbdProjection,
    ApplyPbd,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct PhylonPipelineKey {
    pub pass_type: PassType,
    pub drag_enabled: bool,
}

#[derive(Resource)]
pub struct PhylonPhysicsPipeline {
    pub physics_bind_group_layout: bevy::render::render_resource::BindGroupLayout,
    pub physics_desc: bevy::render::render_resource::BindGroupLayoutDescriptor,
    pub shader: Handle<Shader>,
}

impl FromWorld for PhylonPhysicsPipeline {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        let render_device = world.resource::<RenderDevice>();

        let entries = bevy::render::render_resource::BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                bevy::render::render_resource::binding_types::storage_buffer::<[f32; 8]>(false), // ParticleNode (32 bytes)
                bevy::render::render_resource::binding_types::storage_buffer::<[f32; 12]>(false), // Spring (48 bytes)
                bevy::render::render_resource::binding_types::uniform_buffer::<[f32; 4]>(false), // PhysicsConfig (16 bytes)
                bevy::render::render_resource::binding_types::storage_buffer::<i32>(false), // AtomicForcesX (4 bytes)
                bevy::render::render_resource::binding_types::storage_buffer::<i32>(false), // AtomicForcesY (4 bytes)
            ),
        );
        let physics_desc = bevy::render::render_resource::BindGroupLayoutDescriptor {
            label: "physics_bind_group_layout".into(),
            entries: entries.to_vec(),
        };

        let physics_bind_group_layout = render_device
            .create_bind_group_layout(Some("physics_bind_group_layout"), &physics_desc.entries);

        PhylonPhysicsPipeline {
            physics_bind_group_layout,
            physics_desc,
            shader: asset_server.load("shaders/compute/physics.wgsl"),
        }
    }
}

impl SpecializedComputePipeline for PhylonPhysicsPipeline {
    type Key = PhylonPipelineKey;

    fn specialize(&self, key: Self::Key) -> ComputePipelineDescriptor {
        let shader_defs = vec![];
        // TODO: Map to actual settings
        // shader_defs.push(ShaderDefVal::Bool("HYDRODYNAMIC_DRAG_ENABLED".into(), true));
        // shader_defs.push(ShaderDefVal::Bool("STERIC_HINDRANCE_ENABLED".into(), true));

        let entry_point = match key.pass_type {
            PassType::ComputeForces => "compute_forces",
            PassType::Integrate => "integrate",
            PassType::PbdProjection => "pbd_projection",
            PassType::ApplyPbd => "apply_pbd",
        };

        ComputePipelineDescriptor {
            label: Some("Phylon Physics Specialized Pipeline".into()),
            layout: vec![self.physics_desc.clone()],
            immediate_size: 0,
            shader: self.shader.clone(),
            shader_defs,
            entry_point: Some(entry_point.into()),
            zero_initialize_workgroup_memory: false,
        }
    }
}

#[derive(Resource)]
pub struct PhylonPipelineCache {
    pub compute_forces_id: CachedComputePipelineId,
    pub integrate_id: CachedComputePipelineId,
    pub pbd_projection_id: CachedComputePipelineId,
    pub apply_pbd_id: CachedComputePipelineId,
    pub brain_integrate_id: CachedComputePipelineId,
}

impl Default for PhylonPipelineCache {
    fn default() -> Self {
        Self {
            compute_forces_id: CachedComputePipelineId::INVALID,
            integrate_id: CachedComputePipelineId::INVALID,
            pbd_projection_id: CachedComputePipelineId::INVALID,
            apply_pbd_id: CachedComputePipelineId::INVALID,
            brain_integrate_id: CachedComputePipelineId::INVALID,
        }
    }
}

#[derive(Resource, Default)]
pub struct SimulationBindGroups {
    pub physics_bind_group: Option<BindGroup>,
    pub nodes_buffer: Option<bevy::render::render_resource::Buffer>,
    pub springs_buffer: Option<bevy::render::render_resource::Buffer>,
    pub config_buffer: Option<bevy::render::render_resource::Buffer>,
    pub atomic_x_buffer: Option<bevy::render::render_resource::Buffer>,
    pub atomic_y_buffer: Option<bevy::render::render_resource::Buffer>,

    pub readback_buffer: Option<bevy::render::render_resource::Buffer>,
    pub readback_state: std::sync::Arc<std::sync::atomic::AtomicU8>,

    pub node_count: u32,
    pub spring_count: u32,
    pub node_capacity: u32,
    pub spring_capacity: u32,
}

#[derive(Resource, Default)]
pub struct SdfRendererResource(pub Option<rendering::SdfSkinRenderer>);

#[derive(Resource, Default)]
pub struct DiffusionRendererResource(pub Option<gpu::diffusion_pipeline::DiffusionComputePipeline>);

#[derive(Resource, Default)]
pub struct SplatRendererResource(pub Option<rendering::SplatComputePipeline>);

#[derive(Resource, Default)]
pub struct FieldRendererResource(pub Option<rendering::FieldRenderer>);

#[derive(Resource, Default, Clone)]
pub struct ExtractedOverlayConfig {
    pub active_layer: Option<diffusion::FieldLayer>,
}

#[derive(Resource, Default)]
pub struct ExtractedDiffusionData {
    pub dt: f32,
    pub global_time: f32,
    pub emitters: Vec<gpu::diffusion_pipeline::GpuEmitter>,
    pub layers: [gpu::diffusion_pipeline::LayerConfig; 4],
}

#[derive(Resource, Default)]
pub struct ExtractedSplatData {
    pub splats: Vec<rendering::GpuSplat>,
}

fn extract_diffusion_data(
    mut commands: Commands,
    diffusion_config: Extract<Option<Res<diffusion::DiffusionConfig>>>,
    query_signals: Extract<Query<(&physics::ParticleNode, &diffusion::SignalEmitter)>>,
    query_emitters: Extract<Query<&diffusion::Emitter>>,
    query_chem: Extract<
        Query<(
            &physics::ParticleNode,
            &metabolism::ChemicalEconomy,
            &metabolism::Metabolism,
        )>,
    >,
    query_dead: Extract<
        Query<(&physics::ParticleNode, &metabolism::Metabolism), With<metabolism::Dead>>,
    >,
) {
    let mut gpu_emitters = Vec::new();
    let bounds_extents = 1500.0;
    let to_grid = |pos: common::Vec2, radius: f32| -> (f32, f32, f32) {
        let grid_x = (pos.x / bounds_extents) * 128.0 + 128.0;
        let grid_y = (-pos.y / bounds_extents) * 128.0 + 128.0;
        let grid_radius = (radius / bounds_extents) * 128.0;
        (grid_x, grid_y, grid_radius)
    };

    // Layer 0: Pheromones
    let pheromones_offset = gpu_emitters.len() as u32;
    for (node, signal) in query_signals.iter() {
        let (gx, gy, gr) = to_grid(node.position, signal.radius);
        gpu_emitters.push(gpu::diffusion_pipeline::GpuEmitter {
            grid_pos: [gx, gy],
            value: signal.value,
            grid_radius: gr,
        });
    }
    let pheromones_count = gpu_emitters.len() as u32 - pheromones_offset;

    // Layer 1: Energy
    let energy_offset = gpu_emitters.len() as u32;
    for emitter in query_emitters.iter() {
        if emitter.layer == diffusion::FieldLayer::Energy {
            let (gx, gy, gr) = to_grid(emitter.position, emitter.radius);
            gpu_emitters.push(gpu::diffusion_pipeline::GpuEmitter {
                grid_pos: [gx, gy],
                value: emitter.value,
                grid_radius: gr,
            });
        }
    }
    let energy_count = gpu_emitters.len() as u32 - energy_offset;

    // Layer 2: O2
    let o2_offset = gpu_emitters.len() as u32;
    for (node, _chem, meta) in query_chem.iter() {
        let (gx, gy, gr) = to_grid(node.position, 15.0);
        let net_o2 = if meta.is_plant { 1.5 } else { -1.0 };
        gpu_emitters.push(gpu::diffusion_pipeline::GpuEmitter {
            grid_pos: [gx, gy],
            value: net_o2 * meta.mass,
            grid_radius: gr,
        });
    }
    let o2_count = gpu_emitters.len() as u32 - o2_offset;

    // Layer 3: CO2
    let co2_offset = gpu_emitters.len() as u32;
    for (node, _chem, meta) in query_chem.iter() {
        let (gx, gy, gr) = to_grid(node.position, 15.0);
        let net_co2 = if meta.is_plant { -1.5 } else { 1.0 };
        gpu_emitters.push(gpu::diffusion_pipeline::GpuEmitter {
            grid_pos: [gx, gy],
            value: net_co2 * meta.mass,
            grid_radius: gr,
        });
    }
    for (node, meta) in query_dead.iter() {
        let (gx, gy, gr) = to_grid(node.position, 20.0);
        gpu_emitters.push(gpu::diffusion_pipeline::GpuEmitter {
            grid_pos: [gx, gy],
            value: 2.0 * meta.mass, // Corpses release a lot of CO2 as they decay
            grid_radius: gr,
        });
    }
    let co2_count = gpu_emitters.len() as u32 - co2_offset;

    let (diff_rate, dec_rate, global_time) = if let Some(config) = diffusion_config.as_ref() {
        (config.diffusion_rate, config.decay_rate, config.global_time)
    } else {
        (0.1, 0.005, 0.0)
    };

    let layers = [
        gpu::diffusion_pipeline::LayerConfig {
            diffusion_rate: diff_rate,
            decay_rate: dec_rate,
            emitter_count: pheromones_count,
            emitter_offset: pheromones_offset,
        },
        gpu::diffusion_pipeline::LayerConfig {
            diffusion_rate: 0.05,
            decay_rate: dec_rate * 0.5,
            emitter_count: energy_count,
            emitter_offset: energy_offset,
        },
        gpu::diffusion_pipeline::LayerConfig {
            diffusion_rate: 0.8,
            decay_rate: 0.005,
            emitter_count: o2_count,
            emitter_offset: o2_offset,
        },
        gpu::diffusion_pipeline::LayerConfig {
            diffusion_rate: 0.8,
            decay_rate: 0.005,
            emitter_count: co2_count,
            emitter_offset: co2_offset,
        },
    ];

    commands.insert_resource(ExtractedDiffusionData {
        dt: 0.016,
        global_time,
        emitters: gpu_emitters,
        layers,
    });
}

fn extract_splat_data(
    mut commands: Commands,
    manager: Extract<Option<Res<ecology::catastrophe::CatastropheManager>>>,
    config: Extract<Option<Res<ecology::catastrophe::CatastropheConfig>>>,
) {
    let mut splats = Vec::new();
    let bounds_extents = 1500.0;
    let to_grid = |pos: common::Vec2, radius: f32| -> (f32, f32, f32) {
        let grid_x = (pos.x / bounds_extents) * 128.0 + 128.0;
        let grid_y = (-pos.y / bounds_extents) * 128.0 + 128.0;
        let grid_radius = (radius / bounds_extents) * 128.0;
        (grid_x, grid_y, grid_radius)
    };

    if let (Some(manager), Some(config)) = (manager.as_ref(), config.as_ref()) {
        for hazard in &manager.hazards {
            let intensity = match hazard.state {
                ecology::catastrophe::HazardState::Impending { .. } => 0.5,
                ecology::catastrophe::HazardState::Active { .. } => 1.0,
            };
            let (gx, gy, gr) = to_grid(hazard.center, config.hazard_radius);
            splats.push(rendering::GpuSplat {
                grid_pos: [gx, gy],
                value: intensity,
                grid_radius: gr,
            });
        }
    }

    commands.insert_resource(ExtractedSplatData { splats });
}

fn extract_overlay_config(
    mut commands: Commands,
    active_overlay: Extract<Option<Res<crate::ActiveOverlay>>>,
) {
    let active_layer = active_overlay.as_ref().and_then(|r| r.0);
    commands.insert_resource(ExtractedOverlayConfig { active_layer });
}

// --- RENDER SYSTEMS ---

pub fn dispatch_physics_compute(
    pipeline_cache_ids: Res<PhylonPipelineCache>,
    bind_groups: Res<SimulationBindGroups>,
    pipeline_cache: Res<PipelineCache>,
    mut render_context: RenderContext,
    _sender: Res<PositionSender>,
) {
    if bind_groups.physics_bind_group.is_none() || bind_groups.readback_buffer.is_none() {
        return;
    }

    if let (Some(cf_pipe), Some(int_pipe), Some(pbd_proj_pipe), Some(apply_pbd_pipe)) = (
        pipeline_cache.get_compute_pipeline(pipeline_cache_ids.compute_forces_id),
        pipeline_cache.get_compute_pipeline(pipeline_cache_ids.integrate_id),
        pipeline_cache.get_compute_pipeline(pipeline_cache_ids.pbd_projection_id),
        pipeline_cache.get_compute_pipeline(pipeline_cache_ids.apply_pbd_id),
    ) {
        let command_encoder = render_context.command_encoder();

        // Zero-out atomic force buffers
        if bind_groups.node_count > 0 {
            command_encoder.clear_buffer(bind_groups.atomic_x_buffer.as_ref().unwrap(), 0, None);
            command_encoder.clear_buffer(bind_groups.atomic_y_buffer.as_ref().unwrap(), 0, None);
        }

        let mut compute_pass = command_encoder.begin_compute_pass(
            &bevy::render::render_resource::ComputePassDescriptor {
                label: Some("Phylon Physics Compute Pass"),
                timestamp_writes: None,
            },
        );

        let physics_bg = bind_groups.physics_bind_group.as_ref().unwrap();
        compute_pass.set_bind_group(0, physics_bg, &[]);

        let spring_wg = bind_groups.spring_count.div_ceil(64);
        let node_wg = bind_groups.node_count.div_ceil(64);

        // Pass 1: Compute Forces (Springs)
        if spring_wg > 0 {
            compute_pass.set_pipeline(cf_pipe);
            compute_pass.dispatch_workgroups(spring_wg, 1, 1);
        }

        // Pass 2: Integrate (Nodes)
        if node_wg > 0 {
            compute_pass.set_pipeline(int_pipe);
            compute_pass.dispatch_workgroups(node_wg, 1, 1);
        }

        // Pass 3: PBD Projection (Springs)
        if spring_wg > 0 {
            compute_pass.set_pipeline(pbd_proj_pipe);
            compute_pass.dispatch_workgroups(spring_wg, 1, 1);
        }

        // Pass 4: Apply PBD (Nodes)
        if node_wg > 0 {
            compute_pass.set_pipeline(apply_pbd_pipe);
            compute_pass.dispatch_workgroups(node_wg, 1, 1);
        }

        drop(compute_pass);

        // --- ASYNC READBACK ---
        let state = bind_groups.readback_state.load(Ordering::Acquire);
        if state == 0 && bind_groups.node_count > 0 {
            // IDLE: safely encode a copy command
            let nodes_buffer = bind_groups.nodes_buffer.as_ref().unwrap();
            let readback_buffer = bind_groups.readback_buffer.as_ref().unwrap();

            let buffer_size = (bind_groups.node_count as u64)
                * std::mem::size_of::<gpu::physics_pipeline::GpuParticleNode>() as u64;

            render_context.command_encoder().copy_buffer_to_buffer(
                nodes_buffer,
                0,
                readback_buffer,
                0,
                buffer_size,
            );

            bind_groups.readback_state.store(1, Ordering::Release);
        } else if state == 1 && bind_groups.node_count > 0 {
            // COPY QUEUED: safe to map_async
            let readback_buffer = bind_groups.readback_buffer.as_ref().unwrap().clone();
            let readback_buffer_clone = readback_buffer.clone();
            let state_arc = bind_groups.readback_state.clone();
            let tx = _sender.0.clone();

            readback_buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    if result.is_ok() {
                        let data = readback_buffer_clone.slice(..).get_mapped_range();
                        let bytes = data.to_vec();
                        drop(data);
                        readback_buffer_clone.unmap();
                        let _ = tx.send(bytes);
                    }
                    state_arc.store(0, Ordering::Release);
                });

            // Set state to 2 to prevent re-mapping while waiting for the callback
            bind_groups.readback_state.store(2, Ordering::Release);
        }
    }
}

pub fn dispatch_splat_render(
    mut sdf: ResMut<SdfRendererResource>,
    render_device: Res<bevy::render::renderer::RenderDevice>,
    render_queue: Res<bevy::render::renderer::RenderQueue>,
    camera_data: Res<ExtractedCameraData>,
    extracted_data: Option<Res<ExtractedPhysicsData>>,
    views: Query<&bevy::render::view::ViewTarget>,
    mut render_context: bevy::render::renderer::RenderContext,
) {
    let Some(data) = extracted_data else {
        return;
    };
    let Some(target) = views.iter().next() else {
        return;
    };

    let target_format = target.main_texture_format();

    // Recreate pipeline if MSAA changes
    if let Some(renderer) = &sdf.0 {
        if renderer.msaa_samples() != camera_data.msaa_samples {
            sdf.0 = None;
        }
    }

    if sdf.0.is_none() {
        let width = camera_data.screen_size[0] as u32;
        let height = camera_data.screen_size[1] as u32;
        let w = if width == 0 { 800 } else { width };
        let h = if height == 0 { 600 } else { height };
        sdf.0 = Some(rendering::SdfSkinRenderer::new(
            render_device.wgpu_device(),
            target_format,
            w,
            h,
            camera_data.msaa_samples,
        ));
    }

    let Some(renderer) = sdf.0.as_mut() else {
        return;
    };

    let mut bones = Vec::with_capacity(data.springs.len() + data.nodes.len());

    // Render individual nodes as spheres (zero-length capsules)
    for (i, node) in data.nodes.iter().enumerate() {
        bones.push(rendering::SdfBoneInstance {
            pos_a: [node.position[0], node.position[1]],
            pos_b: [node.position[0], node.position[1]],
            radius: 8.0, // TODO: Extract actual body thickness
            color: data.node_colors[i],
        });
    }

    // Render springs as capsules
    for spring in &data.springs {
        let node_a = &data.nodes[spring.node_a as usize];
        let node_b = &data.nodes[spring.node_b as usize];
        let color_a = data.node_colors[spring.node_a as usize];
        bones.push(rendering::SdfBoneInstance {
            pos_a: [node_a.position[0], node_a.position[1]],
            pos_b: [node_b.position[0], node_b.position[1]],
            radius: 8.0, // TODO: Extract actual body thickness if available
            color: color_a,
        });
    }

    let mut color_attachment = target.get_color_attachment();
    color_attachment.ops = wgpu::Operations {
        load: wgpu::LoadOp::Load,
        store: wgpu::StoreOp::Store,
    };

    renderer.render(
        render_device.wgpu_device(),
        &render_queue,
        render_context.command_encoder(),
        color_attachment,
        &bones,
        camera_data.screen_size,
        camera_data.position,
        camera_data.zoom,
        None,
    );
}

pub fn dispatch_diffusion_compute(
    mut diffusion_res: ResMut<DiffusionRendererResource>,
    render_device: Res<bevy::render::renderer::RenderDevice>,
    render_queue: Res<bevy::render::renderer::RenderQueue>,
    extracted_data: Option<Res<ExtractedDiffusionData>>,
    mut render_context: RenderContext,
    sender: Res<DiffusionDataSender>,
) {
    let Some(data) = extracted_data else {
        return;
    };

    if diffusion_res.0.is_none() {
        diffusion_res.0 = Some(gpu::diffusion_pipeline::DiffusionComputePipeline::new(
            render_device.wgpu_device(),
            256,
            256,
        ));
    }

    let Some(pipeline) = diffusion_res.0.as_mut() else {
        return;
    };

    let uniforms = gpu::diffusion_pipeline::DiffusionUniforms {
        dt: data.dt,
        _pad1: 0,
        _pad2: 0,
        _pad3: 0,
        layers: data.layers,
    };

    if let Some(field_data) = pipeline.try_read_field(render_device.wgpu_device()) {
        let _ = sender.0.send(field_data);
    }

    pipeline.step(
        render_device.wgpu_device(),
        &render_queue,
        render_context.command_encoder(),
        uniforms,
        &data.emitters,
        None,
    );
}

pub fn dispatch_splat_compute(
    mut splat_res: ResMut<SplatRendererResource>,
    render_device: Res<bevy::render::renderer::RenderDevice>,
    render_queue: Res<bevy::render::renderer::RenderQueue>,
    extracted_data: Option<Res<ExtractedSplatData>>,
    mut render_context: RenderContext,
) {
    let Some(data) = extracted_data else {
        return;
    };

    if splat_res.0.is_none() {
        splat_res.0 = Some(rendering::SplatComputePipeline::new(
            render_device.wgpu_device(),
            256,
            256,
        ));
    }

    let Some(pipeline) = splat_res.0.as_mut() else {
        return;
    };

    pipeline.step(
        render_device.wgpu_device(),
        &render_queue,
        render_context.command_encoder(),
        &data.splats,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn dispatch_field_render(
    mut field_res: ResMut<FieldRendererResource>,
    diffusion_res: Res<DiffusionRendererResource>,
    render_device: Res<bevy::render::renderer::RenderDevice>,
    render_queue: Res<bevy::render::renderer::RenderQueue>,
    camera_data: Res<ExtractedCameraData>,
    overlay_config: Option<Res<ExtractedOverlayConfig>>,
    mut render_context: RenderContext,
    views: Query<&bevy::render::view::ViewTarget>,
) {
    let Some(target) = views.iter().next() else {
        return;
    };

    // Recreate pipeline if MSAA changes
    if let Some(renderer) = &field_res.0 {
        if renderer.msaa_samples() != camera_data.msaa_samples {
            field_res.0 = None;
        }
    }

    if field_res.0.is_none() {
        let target_format = target.main_texture_format();
        field_res.0 = Some(rendering::FieldRenderer::new(
            render_device.wgpu_device(),
            target_format,
            camera_data.msaa_samples,
        ));
    }

    let Some(renderer) = field_res.0.as_mut() else {
        return;
    };

    let Some(diffusion_pipeline) = diffusion_res.0.as_ref() else {
        return;
    };

    // If an overlay is selected, update config and render
    if let Some(config) = overlay_config {
        if let Some(layer) = config.active_layer {
            let (colormap, min_val, max_val, layer_index) = match layer {
                diffusion::FieldLayer::Pheromones => (0, 0.0, 10.0, 0),
                diffusion::FieldLayer::Energy => (4, 0.0, 50.0, 1),
                diffusion::FieldLayer::O2 => (2, 0.0, 100.0, 2),
                diffusion::FieldLayer::CO2 => (3, 0.0, 100.0, 3),
            };

            renderer.update_config(
                render_queue.as_ref(),
                rendering::FieldConfig {
                    min_val,
                    max_val,
                    camera_pos: camera_data.position.into(),
                    camera_zoom: camera_data.zoom,
                    _pad0: 0,
                    screen_size: camera_data.screen_size,
                    colormap,
                    _pad: 0,
                    world_bounds: [1500.0, 1500.0],
                },
            );

            let mut color_attachment = target.get_color_attachment();
            color_attachment.ops = wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            };

            renderer.render(
                render_device.wgpu_device(),
                render_context.command_encoder(),
                color_attachment,
                diffusion_pipeline.current_layer_view(layer_index),
                None,
            );
        }
    }
}

pub fn prepare_simulation_bind_groups(
    _commands: Commands,
    pipeline: Res<PhylonPhysicsPipeline>,
    render_device: Res<RenderDevice>,
    _render_queue: Res<bevy::render::renderer::RenderQueue>,
    mut bind_groups: ResMut<SimulationBindGroups>,
    extracted_data: Option<Res<ExtractedPhysicsData>>,
) {
    let Some(data) = extracted_data else { return };

    bind_groups.node_count = data.nodes.len() as u32;
    bind_groups.spring_count = data.springs.len() as u32;

    if bind_groups.node_count == 0 {
        return;
    }

    if bind_groups.node_count > bind_groups.node_capacity
        || bind_groups.spring_count > bind_groups.spring_capacity
        || bind_groups.nodes_buffer.is_none()
    {
        // Reallocate with geometric growth
        bind_groups.node_capacity = bind_groups.node_count.next_power_of_two().max(128);
        bind_groups.spring_capacity = bind_groups.spring_count.next_power_of_two().max(128);

        let nodes_size = (bind_groups.node_capacity as usize)
            * std::mem::size_of::<gpu::physics_pipeline::GpuParticleNode>();
        let mut padded_nodes = vec![0u8; nodes_size];
        let positions_bytes = bytemuck::cast_slice(&data.nodes);
        padded_nodes[..positions_bytes.len()].copy_from_slice(positions_bytes);

        let springs_size = (bind_groups.spring_capacity as usize)
            * std::mem::size_of::<gpu::physics_pipeline::GpuPhysicsSpring>();
        let mut padded_springs = vec![0u8; springs_size];
        let springs_bytes = bytemuck::cast_slice(&data.springs);
        padded_springs[..springs_bytes.len()].copy_from_slice(springs_bytes);

        bind_groups.nodes_buffer = Some(render_device.create_buffer_with_data(
            &bevy::render::render_resource::BufferInitDescriptor {
                label: Some("nodes_buffer"),
                contents: &padded_nodes,
                usage: bevy::render::render_resource::BufferUsages::STORAGE
                    | bevy::render::render_resource::BufferUsages::COPY_SRC
                    | bevy::render::render_resource::BufferUsages::COPY_DST,
            },
        ));

        bind_groups.springs_buffer = Some(render_device.create_buffer_with_data(
            &bevy::render::render_resource::BufferInitDescriptor {
                label: Some("springs_buffer"),
                contents: &padded_springs,
                usage: bevy::render::render_resource::BufferUsages::STORAGE
                    | bevy::render::render_resource::BufferUsages::COPY_DST,
            },
        ));

        let config_data: [f32; 4] = [0.016, 0.0, 0.0, 0.0];
        bind_groups.config_buffer = Some(render_device.create_buffer_with_data(
            &bevy::render::render_resource::BufferInitDescriptor {
                label: Some("config_buffer"),
                contents: bytemuck::cast_slice(&config_data),
                usage: bevy::render::render_resource::BufferUsages::UNIFORM,
            },
        ));

        let atomic_size = (bind_groups.node_capacity as u64) * 4; // 4 bytes per i32
        bind_groups.atomic_x_buffer = Some(render_device.create_buffer(
            &bevy::render::render_resource::BufferDescriptor {
                label: Some("atomic_x_buffer"),
                size: atomic_size,
                usage: bevy::render::render_resource::BufferUsages::STORAGE
                    | bevy::render::render_resource::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        ));
        bind_groups.atomic_y_buffer = Some(render_device.create_buffer(
            &bevy::render::render_resource::BufferDescriptor {
                label: Some("atomic_y_buffer"),
                size: atomic_size,
                usage: bevy::render::render_resource::BufferUsages::STORAGE
                    | bevy::render::render_resource::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        ));

        let readback_size = (bind_groups.node_capacity as u64)
            * std::mem::size_of::<gpu::physics_pipeline::GpuParticleNode>() as u64;
        bind_groups.readback_buffer = Some(render_device.create_buffer(
            &bevy::render::render_resource::BufferDescriptor {
                label: Some("AsyncReadbackBuffer"),
                size: readback_size,
                usage: bevy::render::render_resource::BufferUsages::MAP_READ
                    | bevy::render::render_resource::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        ));

        bind_groups.physics_bind_group = Some(
            render_device.create_bind_group(
                Some("physics_bind_group"),
                &pipeline.physics_bind_group_layout,
                &[
                    bevy::render::render_resource::BindGroupEntry {
                        binding: 0,
                        resource: bind_groups
                            .nodes_buffer
                            .as_ref()
                            .unwrap()
                            .as_entire_binding(),
                    },
                    bevy::render::render_resource::BindGroupEntry {
                        binding: 1,
                        resource: bind_groups
                            .springs_buffer
                            .as_ref()
                            .unwrap()
                            .as_entire_binding(),
                    },
                    bevy::render::render_resource::BindGroupEntry {
                        binding: 2,
                        resource: bind_groups
                            .config_buffer
                            .as_ref()
                            .unwrap()
                            .as_entire_binding(),
                    },
                    bevy::render::render_resource::BindGroupEntry {
                        binding: 3,
                        resource: bind_groups
                            .atomic_x_buffer
                            .as_ref()
                            .unwrap()
                            .as_entire_binding(),
                    },
                    bevy::render::render_resource::BindGroupEntry {
                        binding: 4,
                        resource: bind_groups
                            .atomic_y_buffer
                            .as_ref()
                            .unwrap()
                            .as_entire_binding(),
                    },
                ],
            ),
        );
    } else {
        // If not reallocating, we still need to write the updated data into the buffers
        let positions_bytes = bytemuck::cast_slice(&data.nodes);
        let springs_bytes = bytemuck::cast_slice(&data.springs);

        if data.needs_sync && !positions_bytes.is_empty() {
            if let Some(nodes_buffer) = bind_groups.nodes_buffer.as_ref() {
                _render_queue.write_buffer(nodes_buffer, 0, positions_bytes);
            }
        }
        if !springs_bytes.is_empty() {
            if let Some(springs_buffer) = bind_groups.springs_buffer.as_ref() {
                _render_queue.write_buffer(springs_buffer, 0, springs_bytes);
            }
        }
    }
}

pub fn queue_physics_pipelines(
    mut pipeline_cache_ids: ResMut<PhylonPipelineCache>,
    pipeline: Res<PhylonPhysicsPipeline>,
    pipeline_cache: Res<PipelineCache>,
    mut compute_pipelines: ResMut<SpecializedComputePipelines<PhylonPhysicsPipeline>>,
) {
    let base_key = PhylonPipelineKey {
        pass_type: PassType::ComputeForces,
        drag_enabled: true,
    };

    pipeline_cache_ids.compute_forces_id = compute_pipelines.specialize(
        &pipeline_cache,
        &pipeline,
        PhylonPipelineKey {
            pass_type: PassType::ComputeForces,
            ..base_key
        },
    );
    pipeline_cache_ids.integrate_id = compute_pipelines.specialize(
        &pipeline_cache,
        &pipeline,
        PhylonPipelineKey {
            pass_type: PassType::Integrate,
            ..base_key
        },
    );
    pipeline_cache_ids.pbd_projection_id = compute_pipelines.specialize(
        &pipeline_cache,
        &pipeline,
        PhylonPipelineKey {
            pass_type: PassType::PbdProjection,
            ..base_key
        },
    );
    pipeline_cache_ids.apply_pbd_id = compute_pipelines.specialize(
        &pipeline_cache,
        &pipeline,
        PhylonPipelineKey {
            pass_type: PassType::ApplyPbd,
            ..base_key
        },
    );
}

// --- BRAIN INTEGRATION ---

#[derive(Resource, Default)]
pub struct ExtractedBrainData {
    pub nodes: Vec<brain::CtrnnNode>,
    pub synapses: Vec<brain::CtrnnSynapse>,
    pub entity_to_brain_offset: HashMap<Entity, usize>,
}

pub fn extract_brain_data(
    mut commands: Commands,
    query_brains: Extract<Query<(Entity, &brain::Brain)>>,
) {
    let mut gpu_nodes = Vec::new();
    let mut gpu_synapses = Vec::new();
    let mut entity_to_brain_offset = HashMap::new();

    for (entity, brain) in query_brains.iter() {
        let node_offset = gpu_nodes.len() as u32;
        entity_to_brain_offset.insert(entity, node_offset as usize);

        gpu_nodes.extend_from_slice(&brain.nodes);

        // Offset the synapse source/targets
        for mut syn in brain.synapses.clone() {
            syn.source += node_offset;
            syn.target += node_offset;
            gpu_synapses.push(syn);
        }
    }

    // Now fix the node.first_synapse offsets
    let mut current_syn_idx = 0;
    for node in &mut gpu_nodes {
        if node.synapse_count > 0 {
            node.first_synapse = current_syn_idx;
            current_syn_idx += node.synapse_count;
        }
    }

    commands.insert_resource(ExtractedBrainData {
        nodes: gpu_nodes,
        synapses: gpu_synapses,
        entity_to_brain_offset,
    });
}

#[derive(Resource, Default)]
pub struct BrainBindGroups {
    pub bind_group: Option<BindGroup>,
    pub nodes_buffer: Option<bevy::render::render_resource::Buffer>,
    pub synapses_buffer: Option<bevy::render::render_resource::Buffer>,
    pub config_buffer: Option<bevy::render::render_resource::Buffer>,

    pub readback_buffer: Option<bevy::render::render_resource::Buffer>,
    pub readback_state: std::sync::Arc<std::sync::atomic::AtomicU8>,

    pub node_count: u32,
    pub synapse_count: u32,
    pub node_capacity: u32,
    pub synapse_capacity: u32,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct PhylonBrainPipelineKey;

#[derive(Resource)]
pub struct PhylonBrainPipeline {
    pub bind_group_layout: bevy::render::render_resource::BindGroupLayout,
    pub desc: bevy::render::render_resource::BindGroupLayoutDescriptor,
    pub shader: Handle<Shader>,
}

impl FromWorld for PhylonBrainPipeline {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        let render_device = world.resource::<RenderDevice>();

        let entries = [
            bevy::render::render_resource::BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: bevy::render::render_resource::BindingType::Buffer {
                    ty: bevy::render::render_resource::BufferBindingType::Storage {
                        read_only: false,
                    },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            bevy::render::render_resource::BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                ty: bevy::render::render_resource::BindingType::Buffer {
                    ty: bevy::render::render_resource::BufferBindingType::Storage {
                        read_only: false,
                    },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            bevy::render::render_resource::BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::COMPUTE,
                ty: bevy::render::render_resource::BindingType::Buffer {
                    ty: bevy::render::render_resource::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ];
        let desc = bevy::render::render_resource::BindGroupLayoutDescriptor {
            label: "brain_bind_group_layout".into(),
            entries: entries.to_vec(),
        };

        let bind_group_layout =
            render_device.create_bind_group_layout(Some("brain_bind_group_layout"), &desc.entries);

        PhylonBrainPipeline {
            bind_group_layout,
            desc,
            shader: asset_server.load("shaders/compute/brain.wgsl"),
        }
    }
}

impl SpecializedComputePipeline for PhylonBrainPipeline {
    type Key = PhylonBrainPipelineKey;

    fn specialize(&self, _key: Self::Key) -> ComputePipelineDescriptor {
        ComputePipelineDescriptor {
            label: Some("Phylon Brain Pipeline".into()),
            layout: vec![self.desc.clone()],
            immediate_size: 0,
            shader: self.shader.clone(),
            shader_defs: vec![],
            entry_point: Some("integrate_nodes".into()),
            zero_initialize_workgroup_memory: false,
        }
    }
}

pub fn queue_brain_pipelines(
    mut pipeline_cache_ids: ResMut<PhylonPipelineCache>,
    pipeline: Res<PhylonBrainPipeline>,
    pipeline_cache: Res<PipelineCache>,
    mut compute_pipelines: ResMut<SpecializedComputePipelines<PhylonBrainPipeline>>,
) {
    pipeline_cache_ids.brain_integrate_id =
        compute_pipelines.specialize(&pipeline_cache, &pipeline, PhylonBrainPipelineKey);
}

pub fn prepare_brain_bind_groups(
    _commands: Commands,
    pipeline: Res<PhylonBrainPipeline>,
    render_device: Res<RenderDevice>,
    render_queue: Res<bevy::render::renderer::RenderQueue>,
    mut bind_groups: ResMut<BrainBindGroups>,
    extracted_data: Option<Res<ExtractedBrainData>>,
) {
    let Some(data) = extracted_data else { return };

    bind_groups.node_count = data.nodes.len() as u32;
    bind_groups.synapse_count = data.synapses.len() as u32;

    if bind_groups.node_count == 0 {
        return;
    }

    if bind_groups.node_count > bind_groups.node_capacity
        || bind_groups.synapse_count > bind_groups.synapse_capacity
        || bind_groups.nodes_buffer.is_none()
    {
        bind_groups.node_capacity = bind_groups.node_count.next_power_of_two().max(128);
        bind_groups.synapse_capacity = bind_groups.synapse_count.next_power_of_two().max(128);

        let nodes_size =
            (bind_groups.node_capacity as usize) * std::mem::size_of::<brain::CtrnnNode>();
        let mut padded_nodes = vec![0u8; nodes_size];
        let nodes_bytes = bytemuck::cast_slice(&data.nodes);
        padded_nodes[..nodes_bytes.len()].copy_from_slice(nodes_bytes);

        let syn_size =
            (bind_groups.synapse_capacity as usize) * std::mem::size_of::<brain::CtrnnSynapse>();
        let mut padded_syn = vec![0u8; syn_size];
        let syn_bytes = bytemuck::cast_slice(&data.synapses);
        padded_syn[..syn_bytes.len()].copy_from_slice(syn_bytes);

        bind_groups.nodes_buffer = Some(render_device.create_buffer_with_data(
            &bevy::render::render_resource::BufferInitDescriptor {
                label: Some("brain_nodes_buffer"),
                contents: &padded_nodes,
                usage: bevy::render::render_resource::BufferUsages::STORAGE
                    | bevy::render::render_resource::BufferUsages::COPY_SRC
                    | bevy::render::render_resource::BufferUsages::COPY_DST,
            },
        ));

        bind_groups.synapses_buffer = Some(render_device.create_buffer_with_data(
            &bevy::render::render_resource::BufferInitDescriptor {
                label: Some("brain_synapses_buffer"),
                contents: &padded_syn,
                usage: bevy::render::render_resource::BufferUsages::STORAGE
                    | bevy::render::render_resource::BufferUsages::COPY_DST,
            },
        ));

        let config_data: [f32; 4] = [0.016, 0.0, 0.0, 0.0];
        bind_groups.config_buffer = Some(render_device.create_buffer_with_data(
            &bevy::render::render_resource::BufferInitDescriptor {
                label: Some("brain_config_buffer"),
                contents: bytemuck::cast_slice(&config_data),
                usage: bevy::render::render_resource::BufferUsages::UNIFORM,
            },
        ));

        let readback_size =
            (bind_groups.node_capacity as u64) * std::mem::size_of::<brain::CtrnnNode>() as u64;
        bind_groups.readback_buffer = Some(render_device.create_buffer(
            &bevy::render::render_resource::BufferDescriptor {
                label: Some("BrainAsyncReadbackBuffer"),
                size: readback_size,
                usage: bevy::render::render_resource::BufferUsages::MAP_READ
                    | bevy::render::render_resource::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        ));

        bind_groups.bind_group = Some(
            render_device.create_bind_group(
                Some("brain_bind_group"),
                &pipeline.bind_group_layout,
                &[
                    bevy::render::render_resource::BindGroupEntry {
                        binding: 0,
                        resource: bind_groups
                            .nodes_buffer
                            .as_ref()
                            .unwrap()
                            .as_entire_binding(),
                    },
                    bevy::render::render_resource::BindGroupEntry {
                        binding: 1,
                        resource: bind_groups
                            .synapses_buffer
                            .as_ref()
                            .unwrap()
                            .as_entire_binding(),
                    },
                    bevy::render::render_resource::BindGroupEntry {
                        binding: 2,
                        resource: bind_groups
                            .config_buffer
                            .as_ref()
                            .unwrap()
                            .as_entire_binding(),
                    },
                ],
            ),
        );
    } else {
        let nodes_bytes = bytemuck::cast_slice(&data.nodes);
        let syn_bytes = bytemuck::cast_slice(&data.synapses);

        if !nodes_bytes.is_empty() {
            if let Some(nodes_buffer) = bind_groups.nodes_buffer.as_ref() {
                render_queue.write_buffer(nodes_buffer, 0, nodes_bytes);
            }
        }
        if !syn_bytes.is_empty() {
            if let Some(synapses_buffer) = bind_groups.synapses_buffer.as_ref() {
                render_queue.write_buffer(synapses_buffer, 0, syn_bytes);
            }
        }
    }
}

pub fn dispatch_brain_compute(
    pipeline_cache_ids: Res<PhylonPipelineCache>,
    bind_groups: Res<BrainBindGroups>,
    pipeline_cache: Res<PipelineCache>,
    mut render_context: RenderContext,
    sender: Res<BrainDataSender>,
) {
    if bind_groups.bind_group.is_none() || bind_groups.readback_buffer.is_none() {
        return;
    }

    if let Some(int_pipe) =
        pipeline_cache.get_compute_pipeline(pipeline_cache_ids.brain_integrate_id)
    {
        let command_encoder = render_context.command_encoder();

        let mut compute_pass = command_encoder.begin_compute_pass(
            &bevy::render::render_resource::ComputePassDescriptor {
                label: Some("Phylon Brain Compute Pass"),
                timestamp_writes: None,
            },
        );

        compute_pass.set_bind_group(0, bind_groups.bind_group.as_ref().unwrap(), &[]);
        let node_wg = bind_groups.node_count.div_ceil(64);

        if node_wg > 0 {
            compute_pass.set_pipeline(int_pipe);
            compute_pass.dispatch_workgroups(node_wg, 1, 1);
        }

        drop(compute_pass);

        // Async readback logic for brain nodes
        let state = bind_groups.readback_state.load(Ordering::Acquire);
        if state == 0 && bind_groups.node_count > 0 {
            let nodes_buffer = bind_groups.nodes_buffer.as_ref().unwrap();
            let readback_buffer = bind_groups.readback_buffer.as_ref().unwrap();
            let buffer_size =
                (bind_groups.node_count as u64) * std::mem::size_of::<brain::CtrnnNode>() as u64;

            render_context.command_encoder().copy_buffer_to_buffer(
                nodes_buffer,
                0,
                readback_buffer,
                0,
                buffer_size,
            );

            bind_groups.readback_state.store(1, Ordering::Release);
        } else if state == 1 && bind_groups.node_count > 0 {
            let readback_buffer = bind_groups.readback_buffer.as_ref().unwrap().clone();
            let readback_buffer_clone = readback_buffer.clone();
            let state_arc = bind_groups.readback_state.clone();
            let tx = sender.0.clone();

            readback_buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    if result.is_ok() {
                        let data = readback_buffer_clone.slice(..).get_mapped_range();
                        let bytes = data.to_vec();
                        drop(data);
                        readback_buffer_clone.unmap();
                        let _ = tx.send(bytes);
                    }
                    state_arc.store(0, Ordering::Release);
                });

            // Set state to 2 to prevent re-mapping while waiting for the callback
            bind_groups.readback_state.store(2, Ordering::Release);
        }
    }
}
