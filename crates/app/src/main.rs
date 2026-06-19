//! # Phylon Application
//!
//! The main binary entry point for the Phylon simulation.
//!
//! ## Responsibilities
//!
//! 1. Parse CLI arguments and locate the config file.
//! 2. Initialise structured logging via `tracing_subscriber`.
//! 3. Load [`PhylonConfig`] from `data/default.ron` (falls back to defaults).
//! 4. Create a `winit` [`EventLoop`] and application window.
//! 5. Initialise a `wgpu` surface on the window.
//! 6. Create a [`SimulationScheduler`].
//! 7. Run the event loop — advancing the scheduler on each `AboutToWait` and
//!    presenting a cleared frame on each `RedrawRequested`.
//!
//! ## Architecture note
//!
//! The `app` crate is the **composition root** — the only crate permitted to
//! depend on everything. All other crates are decoupled from each other via
//! the dependency rules in `docs/02_crate_dependency_graph.md`.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use tracing::{error, info};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

use config::PhylonConfig;
use scheduler::SimulationScheduler;

// ────────────────────────────────────────────────────────────────────────────
// Application state
// ────────────────────────────────────────────────────────────────────────────

/// Lazily-initialised GPU surface resources, created once the window is ready.
struct GpuSurface {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
}

/// Top-level application state, owned by the winit event handler.
struct PhylonApp {
    /// Simulation configuration loaded at startup.
    sim_config: PhylonConfig,

    /// The scheduler that drives all simulation ticks.
    #[allow(dead_code)]
    scheduler: SimulationScheduler,

    /// The core ECS world.
    world: world::World,

    /// GPU surface resources (created after window is ready).
    /// Must be declared before `window` so it drops first!
    gpu: Option<GpuSurface>,

    /// Compute pipeline for physics (forces, integration, PBD).
    physics_compute: Option<gpu::physics_pipeline::PhysicsComputePipeline>,

    /// Compute pipeline for the diffusion field.
    diffusion_compute: Option<gpu::diffusion_pipeline::DiffusionComputePipeline>,

    /// Compute pipeline for the CTRNN brain.
    brain_compute: Option<gpu::brain_pipeline::BrainComputePipeline>,

    /// Debug renderer for entities (grey quads / circles).
    debug_renderer: Option<rendering::DebugRenderer>,

    /// SDF organic skin renderer (accumulate-then-threshold).
    sdf_skin_renderer: Option<rendering::SdfSkinRenderer>,

    /// Renderer for the diffusion field.
    field_renderer: Option<rendering::FieldRenderer>,

    /// The main window (created on `Resumed`).
    window: Option<Arc<Window>>,

    /// Egui winit integration state
    egui_state: Option<egui_winit::State>,

    /// Egui wgpu renderer
    egui_renderer: Option<egui_wgpu::Renderer>,

    /// Camera2D position
    camera_pos: common::Vec2,
    /// Camera2D zoom (scale)
    camera_zoom: f32,

    /// Currently selected entity for inspection
    selected_entity: Option<bevy_ecs::entity::Entity>,

    /// Entity currently tracked by the camera
    tracked_entity: Option<bevy_ecs::entity::Entity>,

    /// Track keyboard modifiers
    modifiers: winit::keyboard::ModifiersState,

    /// Pending canvas click to be processed after the render pass
    pending_click: Option<common::Vec2>,

    /// The viewport dimensions of the simulation canvas (x, y, w, h) in physical pixels
    canvas_rect: Option<[u32; 4]>,

    /// When `true`, render raw physics quads; when `false`, render SDF skin.
    debug_structural: bool,
    /// Thickness of bone lines in structural view.
    bone_line_thickness: f32,
    /// Active tab in the sidebar
    active_tab: ui::SidebarTab,

    /// Maximum number of simulation ticks fired per frame.
    #[allow(dead_code)]
    max_ticks_per_frame: u32,

    /// Total simulation time in seconds.
    total_sim_time: f32,

    /// Multiplier for simulation speed (1.0 = normal, 0.5 = half speed, 2.0 = double).
    simulation_speed: f32,

    /// Accumulator for sub-frame simulation steps.
    accumulated_time: f32,

    /// If true, the simulation is paused and no physics/biology ticks occur.
    is_paused: bool,

    /// If true, show the About dialog.
    show_about: bool,

    /// If true, show the Documentation window.
    show_docs: bool,

    /// If true, overlay vision cones on the simulation viewport.
    show_vision_cones: bool,
}

use bevy_ecs::prelude::*;

struct SpawnOrganismCommand {
    genome: genetics::Genome,
    position: common::Vec2,
    diet: ecology::Diet,
    category: ecology::EcologicalCategory,
}

impl bevy_ecs::world::Command for SpawnOrganismCommand {
    fn apply(self, world: &mut bevy_ecs::world::World) {
        organisms::spawn_organism(world, &self.genome, self.position, self.diet, self.category);
    }
}

pub fn process_births_system(
    mut commands: Commands,
    mut events: EventReader<reproduction::BirthRequest>,
) {
    for event in events.read() {
        commands.add(SpawnOrganismCommand {
            genome: event.genome.clone(),
            position: event.position,
            diet: event.diet.clone(),
            category: event.category.clone(),
        });
    }
}

impl PhylonApp {
    /// Creates a new application state from a loaded config.
    fn new(sim_config: PhylonConfig) -> Self {
        let scheduler = SimulationScheduler::new(&sim_config);

        let mut world = world::World::new();

        // Add resources
        world
            .ecs
            .insert_resource(physics::PhysicsConfig { dt: 0.016 }); // 60hz tick
        world
            .ecs
            .insert_resource(diffusion::DiffusionConfig::default());
        world
            .ecs
            .insert_resource(diffusion::CpuFieldState::default());
        world.ecs.insert_resource(ecology::EcologyConfig::default());
        world
            .ecs
            .insert_resource(bevy_ecs::event::Events::<reproduction::BirthRequest>::default());
        world.ecs.insert_resource(analytics::MetricsState::new());

        // ── Spawn three organisms with distinct Hox sequences ─────────────────
        // Organism 1: Simple worm — 6 muscle segments, no branching.
        let worm_hox = genetics::HoxSequence::worm(6, [0.85, 0.35, 0.35]);
        let worm_genome =
            genetics::Genome::new_hox_driven(genetics::GenomeId(1), common::EntityId(0), worm_hox);
        organisms::spawn_organism(
            &mut world.ecs,
            &worm_genome,
            common::Vec2::new(0.0, 80.0),
            ecology::Diet::Herbivore,
            ecology::EcologicalCategory::None,
        );

        // Organism 2: Fish — 5 segments, fin pair at segment index 2.
        let fish_hox = genetics::HoxSequence::fish(5, 2, [0.25, 0.60, 0.90]);
        let fish_genome =
            genetics::Genome::new_hox_driven(genetics::GenomeId(2), common::EntityId(0), fish_hox);
        organisms::spawn_organism(
            &mut world.ecs,
            &fish_genome,
            common::Vec2::new(0.0, 0.0),
            ecology::Diet::Carnivore,
            ecology::EcologicalCategory::None,
        );

        // Organism 3: Multi-fin — 8 segments, bilateral fins at positions 1 and 4.
        let branchy_genome = genetics::Genome::new_hox_driven(
            genetics::GenomeId(3),
            common::EntityId(0),
            genetics::HoxSequence::new(
                vec![
                    genetics::HoxGene::head(),
                    genetics::HoxGene::branching_torso(2.5, 0.0),
                    genetics::HoxGene::muscle(1.2, 0.0),
                    genetics::HoxGene::torso(),
                    genetics::HoxGene::branching_torso(2.5, std::f32::consts::PI * 0.5),
                    genetics::HoxGene::muscle(1.2, std::f32::consts::PI),
                    genetics::HoxGene::muscle(1.2, std::f32::consts::PI * 1.5),
                    genetics::HoxGene::tail(),
                ],
                [0.95, 0.75, 0.20],
            ),
        );
        organisms::spawn_organism(
            &mut world.ecs,
            &branchy_genome,
            common::Vec2::new(0.0, -90.0),
            ecology::Diet::Producer,
            ecology::EcologicalCategory::None,
        );

        // Spawn a static food/nutrient emitter at the center
        world.ecs.spawn(diffusion::Emitter {
            position: common::Vec2::new(0.0, 0.0),
            value: 10.0,
            radius: 50.0,
        });

        Self {
            sim_config,
            scheduler,
            world,
            gpu: None,
            physics_compute: None,
            diffusion_compute: None,
            brain_compute: None,
            debug_renderer: None,
            sdf_skin_renderer: None,
            field_renderer: None,
            window: None,
            egui_state: None,
            egui_renderer: None,
            camera_pos: common::Vec2::new(0.0, 0.0),
            camera_zoom: 1.0,
            selected_entity: None,
            tracked_entity: None,
            debug_structural: false,
            bone_line_thickness: 1.5,
            active_tab: ui::SidebarTab::Inspector,
            modifiers: winit::keyboard::ModifiersState::empty(),
            pending_click: None,
            canvas_rect: None,
            max_ticks_per_frame: 5,
            total_sim_time: 0.0,
            simulation_speed: 1.0,
            accumulated_time: 0.0,
            is_paused: false,
            show_about: false,
            show_docs: false,
            show_vision_cones: false,
        }
    }

    /// Initialises the wgpu instance, adapter, device, and surface for `window`.
    ///
    /// This is called once after the window is created in `Resumed`.
    fn init_gpu(&mut self, window: Arc<Window>) -> Result<()> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // SAFETY: The surface must not outlive the window. We wrap the window
        // in an Arc and keep it alive for the duration of the application.
        let surface = instance
            .create_surface(window.clone())
            .context("failed to create wgpu surface")?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .context("no suitable GPU adapter found")?;

        let mut required_features = wgpu::Features::FLOAT32_FILTERABLE;
        if adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            required_features |= wgpu::Features::TIMESTAMP_QUERY;
        }

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("PhylonDevice"),
                required_features,
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .context("failed to create wgpu device")?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let present_mode = if self.sim_config.render.vsync {
            wgpu::PresentMode::AutoVsync
        } else {
            wgpu::PresentMode::AutoNoVsync
        };

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let debug_renderer = rendering::DebugRenderer::new(&device, surface_format);
        let sdf_skin_renderer = rendering::SdfSkinRenderer::new(
            &device,
            surface_format,
            size.width.max(1),
            size.height.max(1),
        );
        let field_renderer = rendering::FieldRenderer::new(&device, surface_format);
        let physics_compute = gpu::physics_pipeline::PhysicsComputePipeline::new(&device);
        let diffusion_compute =
            gpu::diffusion_pipeline::DiffusionComputePipeline::new(&device, 256, 256);
        let brain_compute = gpu::brain_pipeline::BrainComputePipeline::new(&device);

        let egui_context = egui::Context::default();
        egui_context.options_mut(|o| {
            o.zoom_with_keyboard = false;
        });
        let scale_factor = window.scale_factor() as f32;
        let egui_state = egui_winit::State::new(
            egui_context,
            egui::ViewportId::ROOT,
            &window,
            Some(scale_factor),
            None,
            Some(2048),
        );
        let egui_renderer = egui_wgpu::Renderer::new(&device, surface_format, None, 1, false);

        self.gpu = Some(GpuSurface {
            surface,
            device,
            queue,
            config: surface_config,
        });
        self.debug_renderer = Some(debug_renderer);
        self.sdf_skin_renderer = Some(sdf_skin_renderer);
        self.field_renderer = Some(field_renderer);
        self.physics_compute = Some(physics_compute);
        self.diffusion_compute = Some(diffusion_compute);
        self.brain_compute = Some(brain_compute);
        self.egui_state = Some(egui_state);
        self.egui_renderer = Some(egui_renderer);
        self.window = Some(window);

        info!("GPU surface initialised ({surface_format:?}, {present_mode:?})");
        Ok(())
    }

    /// Reconfigures the surface after a window resize.
    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        let Some(gpu) = self.gpu.as_mut() else { return };
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        gpu.config.width = new_size.width;
        gpu.config.height = new_size.height;
        gpu.surface.configure(&gpu.device, &gpu.config);
        if let Some(sdf) = self.sdf_skin_renderer.as_mut() {
            sdf.resize(&gpu.device, new_size.width, new_size.height);
        }
    }

    /// Converts a physical-pixel screen coordinate to world space and finds the
    /// nearest `ParticleNode` within a pick radius.
    ///
    /// Returns `None` if no node is close enough, or if GPU surface is not ready.
    fn pick_entity(
        &mut self,
        screen_pos: common::Vec2,
        gpu_w: f32,
        gpu_h: f32,
    ) -> Option<bevy_ecs::entity::Entity> {
        let [vx, vy, vw, vh] = self
            .canvas_rect
            .unwrap_or([0, 0, gpu_w as u32, gpu_h as u32]);
        let local_x = screen_pos.x - vx as f32;
        let local_y = screen_pos.y - vy as f32;

        // NDC (Normalized Device Coordinates): [-1,1] × [-1,1]
        let ndc_x = (local_x / vw as f32) * 2.0 - 1.0;
        let ndc_y = -((local_y / vh as f32) * 2.0 - 1.0); // Y is flipped

        // World space: invert the orthographic projection
        let half_w = (vw as f32 / 2.0) / self.camera_zoom;
        let half_h = (vh as f32 / 2.0) / self.camera_zoom;
        let world_x = ndc_x * half_w + self.camera_pos.x;
        let world_y = ndc_y * half_h + self.camera_pos.y;
        let world_pos = common::Vec2::new(world_x, world_y);

        let pick_radius = 30.0 / self.camera_zoom;

        let mut best: Option<bevy_ecs::entity::Entity> = None;
        let mut best_dist = pick_radius;

        // query() requires &mut World in bevy_ecs 0.14
        let mut query = self
            .world
            .ecs
            .query::<(bevy_ecs::entity::Entity, &physics::ParticleNode)>();
        for (entity, node) in query.iter(&self.world.ecs) {
            let dist = (node.position - world_pos).length();
            if dist < best_dist {
                best_dist = dist;
                best = Some(entity);
            }
        }

        best
    }

    /// Advances the simulation and renders one frame.
    fn render(&mut self) -> Result<()> {
        let Some(gpu) = self.gpu.as_ref() else {
            return Ok(());
        };
        let Some(physics_compute) = self.physics_compute.as_ref() else {
            return Ok(());
        };

        const DT: f32 = 0.016; // Fixed 60 Hz timestep

        // 1. Camera Tracking
        if let Some(tracked) = self.tracked_entity {
            if let Ok(node) = self
                .world
                .ecs
                .query::<&physics::ParticleNode>()
                .get(&self.world.ecs, tracked)
            {
                // Smoothly follow the target
                self.camera_pos = self.camera_pos.lerp(node.position, 0.1);
            } else {
                // Entity no longer exists (e.g. died), drop tracking
                self.tracked_entity = None;
            }
        }

        if !self.is_paused {
            self.accumulated_time += self.simulation_speed;
        }

        let mut physics_duration_ms = 0.0;
        let mut diffusion_duration_ms = 0.0;

        let ticks_this_frame = self.accumulated_time.floor() as u32;
        let ticks_to_run = ticks_this_frame.min(self.max_ticks_per_frame);
        self.accumulated_time -= ticks_this_frame as f32;

        for _ in 0..ticks_to_run {
            self.total_sim_time += DT;

            // 2. Run Biology Systems (Sensing, Brain, Behavior)
            use bevy_ecs::system::RunSystemOnce;
            self.world.ecs.run_system_once(organisms::growth_system);
            self.world.ecs.run_system_once(sensing::sensing_system);

            // -- GPU CTRNN EVALUATION --
            let mut gpu_brain_nodes = Vec::new();
            let mut gpu_brain_synapses = Vec::new();
            let mut brain_offsets = Vec::new();

            let mut query = self.world.ecs.query::<(
                bevy_ecs::entity::Entity,
                &sensing::SensoryState,
                &mut brain::Brain,
            )>();
            for (entity, sensory, mut brain) in query.iter_mut(&mut self.world.ecs) {
                brain.set_inputs(&sensory.inputs);

                let start_node = gpu_brain_nodes.len() as u32;
                let start_syn = gpu_brain_synapses.len() as u32;

                for node in &brain.nodes {
                    gpu_brain_nodes.push(gpu::brain_pipeline::GpuCtrnnNode {
                        state: node.state,
                        time_constant: node.time_constant,
                        bias: node.bias,
                        activation: node.activation,
                        first_synapse: start_syn + node.first_synapse,
                        synapse_count: node.synapse_count,
                    });
                }

                for syn in &brain.synapses {
                    gpu_brain_synapses.push(gpu::brain_pipeline::GpuCtrnnSynapse {
                        source: start_node + syn.source,
                        target: start_node + syn.target,
                        weight: syn.weight,
                        _padding: 0,
                    });
                }

                brain_offsets.push((entity, start_node, brain.nodes.len()));
            }

            if let Some(gpu) = self.gpu.as_ref() {
                if let Some(brain_compute) = self.brain_compute.as_ref() {
                    brain_compute.compute_step(
                        &gpu.device,
                        &gpu.queue,
                        &mut gpu_brain_nodes,
                        &gpu_brain_synapses,
                        DT,
                    );
                }
            }

            // Readback integrated node state
            let mut query = self.world.ecs.query::<&mut brain::Brain>();
            for (entity, start_node, len) in brain_offsets {
                if let Ok(mut brain) = query.get_mut(&mut self.world.ecs, entity) {
                    for i in 0..len {
                        brain.nodes[i].state = gpu_brain_nodes[(start_node as usize) + i].state;
                    }
                }
            }

            self.world.ecs.run_system_once(behavior::behavior_system);

            // 2. Gather Nodes and build Entity -> Index map
            let mut entity_to_index = std::collections::HashMap::new();
            let mut gpu_nodes = Vec::new();
            let mut node_entities = Vec::new();

            let mut query_nodes = self
                .world
                .ecs
                .query::<(bevy_ecs::entity::Entity, &physics::ParticleNode)>();
            for (entity, node) in query_nodes.iter(&self.world.ecs) {
                entity_to_index.insert(entity, gpu_nodes.len() as u32);
                gpu_nodes.push(gpu::physics_pipeline::GpuParticleNode {
                    position: [node.position.x, node.position.y],
                    velocity: [node.velocity.x, node.velocity.y],
                    force: [node.force.x, node.force.y],
                    mass: node.mass,
                    _padding: 0,
                });
                node_entities.push(entity);
            }

            // 3. Gather Springs
            let mut query_springs = self.world.ecs.query::<&physics::Spring>();
            let mut gpu_springs = Vec::new();
            for spring in query_springs.iter(&self.world.ecs) {
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

            // 4. Compute GPU Physics
            let physics_start = std::time::Instant::now();
            let global_time = self
                .world
                .ecs
                .resource::<diffusion::DiffusionConfig>()
                .global_time;
            let updated_nodes = physics_compute.compute_step(
                &gpu.device,
                &gpu.queue,
                &gpu_nodes,
                &gpu_springs,
                0.016,
                global_time,
            );
            physics_duration_ms += physics_start.elapsed().as_secs_f64() * 1000.0;

            // 5. Update ECS Nodes
            for (i, entity) in node_entities.iter().enumerate() {
                if let Some(mut node) = self.world.ecs.get_mut::<physics::ParticleNode>(*entity) {
                    node.position.x = updated_nodes[i].position[0];
                    node.position.y = updated_nodes[i].position[1];
                    node.velocity.x = updated_nodes[i].velocity[0];
                    node.velocity.y = updated_nodes[i].velocity[1];
                    // Clear forces for next tick
                    node.force = common::Vec2::new(0.0, 0.0);
                }
            }

            // 6. Run remaining biological systems
            self.world.ecs.run_system_once(ecology::food_spawner_system);
            self.world.ecs.run_system_once(ecology::foraging_system);
            self.world
                .ecs
                .run_system_once(metabolism::metabolism_system);
            self.world
                .ecs
                .run_system_once(reproduction::reproduction_system);
            self.world.ecs.run_system_once(process_births_system);
            self.world.ecs.run_system_once(process_deaths_system);
            if let Some(mut events) = self
                .world
                .ecs
                .get_resource_mut::<bevy_ecs::event::Events<reproduction::BirthRequest>>()
            {
                events.update();
            }

            if let Some(diffusion_compute) = self.diffusion_compute.as_mut() {
                // 5. Gather diffusion emitters and run compute
                let (diff_rate, dec_rate) = {
                    let mut diffusion_config =
                        self.world.ecs.resource_mut::<diffusion::DiffusionConfig>();

                    // Diurnal modulation
                    diffusion_config.global_time += 0.016;
                    // Oscillate decay rate between 0.5x and 1.5x of base
                    let diurnal_mod = 1.0 + 0.5 * (diffusion_config.global_time * 0.1).sin();
                    diffusion_config.decay_rate = diffusion_config.base_decay_rate * diurnal_mod;

                    (diffusion_config.diffusion_rate, diffusion_config.decay_rate)
                };
                let mut query_emitters = self.world.ecs.query::<&diffusion::Emitter>();
                let mut gpu_emitters = Vec::new();

                let screen_w = gpu.config.width as f32;
                let screen_h = gpu.config.height as f32;

                for emitter in query_emitters.iter(&self.world.ecs) {
                    // Map world space to 256x256 grid space
                    let grid_x = (emitter.position.x / (screen_w * 0.5)) * 128.0 + 128.0;
                    let grid_y = (-emitter.position.y / (screen_h * 0.5)) * 128.0 + 128.0;
                    let grid_radius = (emitter.radius / (screen_w * 0.5)) * 128.0;

                    gpu_emitters.push(gpu::diffusion_pipeline::GpuEmitter {
                        grid_pos: [grid_x, grid_y],
                        value: emitter.value,
                        grid_radius,
                    });
                }

                let diffusion_start = std::time::Instant::now();
                diffusion_compute.step(
                    &gpu.device,
                    &gpu.queue,
                    gpu::diffusion_pipeline::DiffusionUniforms {
                        diffusion_rate: diff_rate,
                        decay_rate: dec_rate,
                        dt: 0.016, // fixed timestep
                        emitter_count: gpu_emitters.len() as u32,
                    },
                    &gpu_emitters,
                );

                if let Some(field_data) = diffusion_compute.try_read_field(&gpu.device) {
                    let mut cpu_field = self.world.ecs.resource_mut::<diffusion::CpuFieldState>();
                    cpu_field.data = field_data;
                }
                diffusion_duration_ms += diffusion_start.elapsed().as_secs_f64() * 1000.0;
            }
        }

        // Record analytics — read entity_count before mutably borrowing the resource
        let entity_count = self.world.ecs.entities().len() as usize;
        if let Some(mut metrics) = self.world.ecs.get_resource_mut::<analytics::MetricsState>() {
            let sim_dt = (ticks_to_run as f64) * f64::from(DT);
            let real_dt = f64::from(DT); // Fixed render step for now
            metrics.record_frame(entity_count, sim_dt, real_dt);

            if ticks_to_run > 0 {
                metrics.compute_profiles = vec![
                    analytics::PassTiming {
                        name: "Physics (Compute & PBD)".to_string(),
                        duration_ms: physics_duration_ms,
                    },
                    analytics::PassTiming {
                        name: "Diffusion Field".to_string(),
                        duration_ms: diffusion_duration_ms,
                    },
                ];
            }
        }

        // 6. Gather rendering instances
        let mut debug_instances = Vec::new();
        let mut sdf_bones = Vec::new();

        // Build node position lookup for bone endpoint resolution
        let mut node_positions: std::collections::HashMap<bevy_ecs::entity::Entity, [f32; 2]> =
            std::collections::HashMap::new();

        let mut query_nodes_render = self.world.ecs.query::<(
            bevy_ecs::entity::Entity,
            &physics::ParticleNode,
            Option<&ecology::EcologicalCategory>,
        )>();
        for (entity, node, category) in query_nodes_render.iter(&self.world.ecs) {
            node_positions.insert(entity, [node.position.x, node.position.y]);
            debug_instances.push(rendering::DebugInstance {
                pos_a: [node.position.x, node.position.y],
                pos_b: [node.position.x, node.position.y],
                color: match node.segment_type {
                    0 => [0.9, 0.3, 0.3, 1.0], // Head — red
                    2 => [0.3, 0.5, 1.0, 1.0], // Muscle — blue
                    3 => [0.6, 0.6, 0.3, 1.0], // Tail — yellow-green
                    4 => [0.3, 0.9, 0.9, 1.0], // Fin — cyan
                    _ => [0.5, 0.5, 0.5, 1.0], // Torso — grey
                },
                radius: if node.segment_type == 4 { 3.0 } else { 5.0 },
                segment_type: node.segment_type,
            });

            // Draw category ring around head
            if let Some(cat) = category {
                let ring_color = match cat {
                    ecology::EcologicalCategory::Keystone => Some([1.0, 0.84, 0.0, 1.0]), // Gold
                    ecology::EcologicalCategory::Indicator => Some([0.0, 1.0, 1.0, 1.0]), // Cyan
                    ecology::EcologicalCategory::Endemic => Some([0.0, 0.5, 0.5, 1.0]),   // Teal
                    ecology::EcologicalCategory::Invasive => Some([1.0, 0.0, 1.0, 1.0]),  // Magenta
                    _ => None,
                };
                if let Some(col) = ring_color {
                    // We render a slightly larger circle with the category color.
                    // To do an outline, we could rely on a new shader feature, or just render a bigger circle behind it.
                    // Since it pushes sequentially, we just push it before the main circle, wait, the loop is already pushing.
                    // We can just push it now; it's debug instances, order determines z-index (painters algorithm).
                    // Let's push it after, it might overlay, but wait! We can just make segment_type=99 for transparency or something.
                    // Actually, let's just push it.
                    debug_instances.push(rendering::DebugInstance {
                        pos_a: [node.position.x, node.position.y],
                        pos_b: [node.position.x, node.position.y],
                        color: [col[0], col[1], col[2], 0.3], // Semi-transparent
                        radius: 12.0,                         // Larger radius
                        segment_type: 99,
                    });
                }
            }
        }

        // Collect springs for SDF capsule rendering.
        // Rigid + Rotational: drawn as full-weight skin bones.
        // Elastic (muscle) + Passive (tail): drawn as thinner, slightly dimmer bones so
        // the spine of muscle-only organisms (e.g. the worm) is visible in skin mode.
        let mut query_springs_render = self
            .world
            .ecs
            .query::<(&physics::Spring, Option<&organisms::OrganismColor>)>();
        for (spring, opt_color) in query_springs_render.iter(&self.world.ecs) {
            // Skip springs that have no associated organism color (e.g. broken/detached).
            if spring.constraint_type == physics::ConstraintType::Passive && spring.is_fin == 0 {
                // Passive tail bones: thin and dimmed
                if let (Some(&pa), Some(&pb)) = (
                    node_positions.get(&spring.node_a),
                    node_positions.get(&spring.node_b),
                ) {
                    let color = opt_color
                        .map(|c| {
                            let c = c.0;
                            [c[0] * 0.6, c[1] * 0.6, c[2] * 0.6]
                        })
                        .unwrap_or([0.4, 0.4, 0.4]);
                    sdf_bones.push(rendering::SdfBoneInstance {
                        pos_a: pa,
                        pos_b: pb,
                        radius: 4.0,
                        color,
                    });
                    if self.debug_structural {
                        debug_instances.push(rendering::DebugInstance {
                            pos_a: pa,
                            pos_b: pb,
                            color: [color[0], color[1], color[2], 0.5],
                            radius: self.bone_line_thickness,
                            segment_type: 99,
                        });
                    }
                }
                continue;
            }

            if spring.constraint_type == physics::ConstraintType::Elastic {
                // Elastic muscle bones: medium weight
                if let (Some(&pa), Some(&pb)) = (
                    node_positions.get(&spring.node_a),
                    node_positions.get(&spring.node_b),
                ) {
                    let color = opt_color.map(|c| c.0).unwrap_or([0.5, 0.5, 0.8]);
                    sdf_bones.push(rendering::SdfBoneInstance {
                        pos_a: pa,
                        pos_b: pb,
                        radius: 6.0,
                        color,
                    });
                    if self.debug_structural {
                        debug_instances.push(rendering::DebugInstance {
                            pos_a: pa,
                            pos_b: pb,
                            color: [color[0], color[1], color[2], 0.6],
                            radius: self.bone_line_thickness,
                            segment_type: 99,
                        });
                    }
                }
                continue;
            }

            if spring.constraint_type != physics::ConstraintType::Rigid
                && spring.constraint_type != physics::ConstraintType::Rotational
            {
                continue;
            }
            if let (Some(&pa), Some(&pb)) = (
                node_positions.get(&spring.node_a),
                node_positions.get(&spring.node_b),
            ) {
                let color = opt_color.map(|c| c.0).unwrap_or([0.8, 0.4, 0.4]);
                let radius = if spring.is_fin == 1 { 5.0 } else { 8.0 };
                sdf_bones.push(rendering::SdfBoneInstance {
                    pos_a: pa,
                    pos_b: pb,
                    radius,
                    color,
                });

                // Also draw a line for Debug Structural View
                if self.debug_structural {
                    debug_instances.push(rendering::DebugInstance {
                        pos_a: pa,
                        pos_b: pb,
                        color: [color[0], color[1], color[2], 0.7],
                        radius: self.bone_line_thickness,
                        segment_type: 99,
                    });
                }
            }
        }

        // Render food pellets (always shown in debug view)
        let mut query_food = self.world.ecs.query::<&ecology::FoodPellet>();
        for food in query_food.iter(&self.world.ecs) {
            debug_instances.push(rendering::DebugInstance {
                pos_a: [food.position.x, food.position.y],
                pos_b: [food.position.x, food.position.y],
                color: [1.0, 0.8, 0.0, 1.0],
                radius: 2.5,
                segment_type: 0,
            });
        }

        // Render mineral pellets
        let mut query_mineral = self.world.ecs.query::<&ecology::MineralPellet>();
        for mineral in query_mineral.iter(&self.world.ecs) {
            debug_instances.push(rendering::DebugInstance {
                pos_a: [mineral.position.x, mineral.position.y],
                pos_b: [mineral.position.x, mineral.position.y],
                color: [1.0, 1.0, 1.0, 1.0], // Bright White
                radius: 2.0,
                segment_type: 0,
            });
        }

        // Render corpses
        let mut query_corpse = self.world.ecs.query::<&ecology::Corpse>();
        for corpse in query_corpse.iter(&self.world.ecs) {
            debug_instances.push(rendering::DebugInstance {
                pos_a: [corpse.position.x, corpse.position.y],
                pos_b: [corpse.position.x, corpse.position.y],
                color: [0.3, 0.3, 0.3, 1.0], // Dark Grey
                radius: 4.0,
                segment_type: 0,
            });
        }

        // Prepare render frame
        let output = match gpu.surface.get_current_texture() {
            Ok(tex) => tex,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                gpu.surface.configure(&gpu.device, &gpu.config);
                return Ok(());
            }
            Err(wgpu::SurfaceError::Timeout) => return Ok(()),
            Err(e) => return Err(anyhow::anyhow!("surface error: {e}")),
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut central_rect_px = None;

        let mut full_output = None;
        let mut egui_context = None;

        let mut interaction = ui::CanvasInteraction::default();
        let mut scale = 1.0;

        let mut ui_actions = Vec::new();

        if let (Some(egui_state), Some(window)) = (&mut self.egui_state, &self.window) {
            let raw_input = egui_state.take_egui_input(window);
            let ctx = egui_state.egui_ctx().clone();

            let output = ctx.run(raw_input, |ctx| {
                let (interact, acts) = ui::render_ui(
                    ctx,
                    &mut self.world,
                    self.camera_pos,
                    self.camera_zoom,
                    &mut self.selected_entity,
                    &mut self.tracked_entity,
                    &mut self.debug_structural,
                    &mut self.bone_line_thickness,
                    &mut self.active_tab,
                    &mut self.simulation_speed,
                    &mut self.is_paused,
                    &mut self.show_about,
                    &mut self.show_docs,
                    &mut self.show_vision_cones,
                );
                interaction = interact;
                ui_actions = acts;
            });

            scale = window.scale_factor() as f32;

            egui_state.handle_platform_output(window, output.platform_output.clone());

            let ui_rect = interaction.rect;

            let x = (ui_rect.min.x * scale).round() as u32;
            let y = (ui_rect.min.y * scale).round() as u32;
            let mut w = (ui_rect.width() * scale).round() as u32;
            let mut h = (ui_rect.height() * scale).round() as u32;

            if x + w > gpu.config.width {
                w = gpu.config.width.saturating_sub(x);
            }
            if y + h > gpu.config.height {
                h = gpu.config.height.saturating_sub(y);
            }

            if w > 0 && h > 0 {
                central_rect_px = Some([x, y, w, h]);
                self.canvas_rect = central_rect_px;
            }

            full_output = Some(output);
            egui_context = Some(ctx);
        }

        // Process native interactions from the transparent canvas
        if interaction.zoom_delta != 1.0 && interaction.zoom_delta > 0.0 {
            self.camera_zoom *= interaction.zoom_delta;
            self.camera_zoom = self.camera_zoom.clamp(0.1, 10.0);
        }

        if interaction.drag_delta.length_sq() > 0.0 {
            self.camera_pos.x -= (interaction.drag_delta.x * scale) / self.camera_zoom;
            self.camera_pos.y += (interaction.drag_delta.y * scale) / self.camera_zoom;
            // Only detach tracking if it's a genuine drag, not a trackpad micro-movement
            if interaction.drag_delta.length_sq() > 9.0 {
                self.tracked_entity = None;
            }
        }

        if interaction.clicked {
            if let Some(pos) = interaction.click_pos {
                self.pending_click = Some(common::Vec2::new(pos.x * scale, pos.y * scale));
            }
        }

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame"),
            });

        // Render the continuous diffusion field as the background (clearing the screen)
        if let (Some(field_renderer), Some(diffusion_compute)) = (
            self.field_renderer.as_ref(),
            self.diffusion_compute.as_ref(),
        ) {
            field_renderer.render(
                &gpu.device,
                &mut encoder,
                &view,
                diffusion_compute.current_texture_view(),
                central_rect_px,
            );
        }

        gpu.queue.submit(std::iter::once(encoder.finish()));

        let mut render_w = gpu.config.width as f32;
        let mut render_h = gpu.config.height as f32;
        if let Some([_, _, w, h]) = central_rect_px {
            render_w = w as f32;
            render_h = h as f32;
        }

        // ── Organism rendering — branch on debug_structural toggle ─────────
        if self.debug_structural {
            // Structural view: raw physics circles + coloured segment dots
            if let Some(debug_renderer) = self.debug_renderer.as_ref() {
                debug_renderer.render(
                    &gpu.device,
                    &gpu.queue,
                    &view,
                    &debug_instances,
                    [render_w, render_h],
                    self.camera_pos,
                    self.camera_zoom,
                    central_rect_px,
                );
            }
        } else {
            // Organic skin: accumulate-then-threshold SDF capsule skin
            if let Some(sdf) = self.sdf_skin_renderer.as_mut() {
                sdf.render(
                    &gpu.device,
                    &gpu.queue,
                    &view,
                    &sdf_bones,
                    [render_w, render_h],
                    self.camera_pos,
                    self.camera_zoom,
                    central_rect_px,
                );
            }
            // Always overlay food and food pellets as debug quads
            if let Some(debug_renderer) = self.debug_renderer.as_ref() {
                // Re-collect only food pellets for the overlay
                let food_only: Vec<rendering::DebugInstance> = debug_instances
                    .iter()
                    .filter(|i| i.color == [1.0, 0.8, 0.0, 1.0_f32])
                    .copied()
                    .collect();
                if !food_only.is_empty() {
                    debug_renderer.render(
                        &gpu.device,
                        &gpu.queue,
                        &view,
                        &food_only,
                        [render_w, render_h],
                        self.camera_pos,
                        self.camera_zoom,
                        central_rect_px,
                    );
                }
            }
        }

        if let (Some(egui_renderer), Some(window), Some(output), Some(ctx)) = (
            &mut self.egui_renderer,
            &self.window,
            full_output,
            egui_context,
        ) {
            let clipped_primitives = ctx.tessellate(output.shapes, window.scale_factor() as f32);
            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [gpu.config.width, gpu.config.height],
                pixels_per_point: window.scale_factor() as f32,
            };

            let mut egui_encoder =
                gpu.device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("egui_encoder"),
                    });

            for (id, image_delta) in &output.textures_delta.set {
                egui_renderer.update_texture(&gpu.device, &gpu.queue, *id, image_delta);
            }

            egui_renderer.update_buffers(
                &gpu.device,
                &gpu.queue,
                &mut egui_encoder,
                &clipped_primitives,
                &screen_descriptor,
            );

            {
                let mut render_pass = egui_encoder
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("egui_render_pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    })
                    .forget_lifetime();
                egui_renderer.render(&mut render_pass, &clipped_primitives, &screen_descriptor);
            }

            gpu.queue.submit(std::iter::once(egui_encoder.finish()));

            for id in &output.textures_delta.free {
                egui_renderer.free_texture(id);
            }
        }

        output.present();

        self.handle_menu_actions(ui_actions);

        Ok(())
    }

    fn handle_menu_actions(&mut self, actions: Vec<ui::MenuAction>) {
        for action in actions {
            match action {
                ui::MenuAction::SaveState => {
                    tracing::warn!("Save State not yet implemented fully.");
                }
                ui::MenuAction::LoadState => {
                    tracing::warn!("Load State not yet implemented fully.");
                }
                ui::MenuAction::Undo => {
                    tracing::warn!("Undo not yet implemented fully.");
                }
                ui::MenuAction::Redo => {
                    tracing::warn!("Redo not yet implemented fully.");
                }
                ui::MenuAction::StepForward => {
                    self.accumulated_time += 1.0;
                }
                ui::MenuAction::Reset => {
                    // Despawn all entities
                    let entities: Vec<_> = self.world.ecs.iter_entities().map(|e| e.id()).collect();
                    for entity in entities {
                        self.world.ecs.despawn(entity);
                    }

                    // Respawn defaults
                    let worm_hox = genetics::HoxSequence::worm(6, [0.85, 0.35, 0.35]);
                    let worm_genome = genetics::Genome::new_hox_driven(
                        genetics::GenomeId(1),
                        common::EntityId(0),
                        worm_hox,
                    );
                    organisms::spawn_organism(
                        &mut self.world.ecs,
                        &worm_genome,
                        common::Vec2::new(0.0, 80.0),
                        ecology::Diet::Herbivore,
                        ecology::EcologicalCategory::None,
                    );

                    let fish_hox = genetics::HoxSequence::fish(5, 2, [0.25, 0.60, 0.90]);
                    let fish_genome = genetics::Genome::new_hox_driven(
                        genetics::GenomeId(2),
                        common::EntityId(0),
                        fish_hox,
                    );
                    organisms::spawn_organism(
                        &mut self.world.ecs,
                        &fish_genome,
                        common::Vec2::new(0.0, 0.0),
                        ecology::Diet::Carnivore,
                        ecology::EcologicalCategory::None,
                    );
                }
                ui::MenuAction::SelectAll => {
                    // Just select the first head we find
                    let mut query = self
                        .world
                        .ecs
                        .query::<(bevy_ecs::entity::Entity, &physics::ParticleNode)>();
                    for (entity, node) in query.iter(&self.world.ecs) {
                        if node.segment_type == 0 {
                            // Head
                            self.selected_entity = Some(entity);
                            self.tracked_entity = Some(entity);
                            break;
                        }
                    }
                }
                ui::MenuAction::Deselect => {
                    self.selected_entity = None;
                    self.tracked_entity = None;
                }
                ui::MenuAction::SpawnProtoFish => {
                    let fish_hox = genetics::HoxSequence::fish(5, 2, [0.25, 0.60, 0.90]);
                    let fish_genome = genetics::Genome::new_hox_driven(
                        genetics::GenomeId(100),
                        common::EntityId(0),
                        fish_hox,
                    );
                    organisms::spawn_organism(
                        &mut self.world.ecs,
                        &fish_genome,
                        self.camera_pos,
                        ecology::Diet::Carnivore,
                        ecology::EcologicalCategory::None,
                    );
                }
                ui::MenuAction::ShowDocumentation => {
                    self.show_docs = true;
                }
                ui::MenuAction::ShowAbout => {
                    self.show_about = true;
                }
                ui::MenuAction::CameraZoomIn => {
                    self.camera_zoom *= 1.1;
                    self.camera_zoom = self.camera_zoom.clamp(0.1, 10.0);
                }
                ui::MenuAction::CameraZoomOut => {
                    self.camera_zoom /= 1.1;
                    self.camera_zoom = self.camera_zoom.clamp(0.1, 10.0);
                }
                ui::MenuAction::CameraHome => {
                    self.camera_pos = common::Vec2::new(0.0, 0.0);
                    self.camera_zoom = 1.0;
                    self.tracked_entity = None;
                }
            }
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// winit ApplicationHandler impl
// ────────────────────────────────────────────────────────────────────────────

impl ApplicationHandler for PhylonApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attrs = Window::default_attributes()
            .with_title(&self.sim_config.render.window_title)
            .with_inner_size(LogicalSize::new(
                self.sim_config.render.window_width,
                self.sim_config.render.window_height,
            ));

        let window = match event_loop.create_window(window_attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                error!("Failed to create window: {e}");
                event_loop.exit();
                return;
            }
        };

        if let Err(e) = self.init_gpu(window) {
            error!("Failed to initialise GPU: {e:#}");
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(egui_state) = &mut self.egui_state {
            if let Some(window) = &self.window {
                let _response = egui_state.on_window_event(window, &event);
                if _response.consumed {
                    // Only return early if egui consumed the event specifically (e.g. text input),
                    // since we now handle primary interactions inside the render loop via egui's output.
                    return;
                }
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                info!("Window close requested — exiting");
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                self.resize(new_size);
            }
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        physical_key,
                        state: winit::event::ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                use winit::keyboard::{KeyCode, PhysicalKey};
                let pan_speed = 10.0 / self.camera_zoom;
                match physical_key {
                    PhysicalKey::Code(KeyCode::KeyW) | PhysicalKey::Code(KeyCode::ArrowUp) => {
                        self.camera_pos.y += pan_speed;
                        self.tracked_entity = None;
                    }
                    PhysicalKey::Code(KeyCode::KeyS) | PhysicalKey::Code(KeyCode::ArrowDown) => {
                        self.camera_pos.y -= pan_speed;
                        self.tracked_entity = None;
                    }
                    PhysicalKey::Code(KeyCode::KeyA) | PhysicalKey::Code(KeyCode::ArrowLeft) => {
                        self.camera_pos.x -= pan_speed;
                        self.tracked_entity = None;
                    }
                    PhysicalKey::Code(KeyCode::KeyD) | PhysicalKey::Code(KeyCode::ArrowRight) => {
                        self.camera_pos.x += pan_speed;
                        self.tracked_entity = None;
                    }
                    // Zoom with + and -
                    PhysicalKey::Code(KeyCode::Equal) | PhysicalKey::Code(KeyCode::NumpadAdd) => {
                        self.camera_zoom *= 1.1;
                        self.camera_zoom = self.camera_zoom.clamp(0.1, 10.0);
                    }
                    PhysicalKey::Code(KeyCode::Minus)
                    | PhysicalKey::Code(KeyCode::NumpadSubtract) => {
                        self.camera_zoom /= 1.1;
                        self.camera_zoom = self.camera_zoom.clamp(0.1, 10.0);
                    }
                    _ => {}
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => {
                        if self.modifiers.control_key() {
                            // Zoom with Ctrl + Scroll
                            if y > 0.0 {
                                self.camera_zoom *= 1.1;
                            } else if y < 0.0 {
                                self.camera_zoom /= 1.1;
                            }
                        } else {
                            // Pan
                            self.camera_pos.x -= x * 20.0 / self.camera_zoom;
                            self.camera_pos.y += y * 20.0 / self.camera_zoom;
                        }
                    }
                    winit::event::MouseScrollDelta::PixelDelta(p) => {
                        if self.modifiers.control_key() {
                            // Zoom
                            let zoom_factor = 1.0 + (p.y as f32 * 0.01);
                            if zoom_factor > 0.0 {
                                self.camera_zoom *= zoom_factor;
                            }
                        } else {
                            // Touchpad two-finger swipe: pan
                            self.camera_pos.x -= p.x as f32 / self.camera_zoom;
                            self.camera_pos.y += p.y as f32 / self.camera_zoom;
                        }
                    }
                }
                self.camera_zoom = self.camera_zoom.clamp(0.1, 10.0);
            }

            WindowEvent::RedrawRequested => {
                if let Err(e) = self.render() {
                    error!("Render error: {e:#}");
                    event_loop.exit();
                }

                // Process pending clicks that require mutably borrowing self
                if let Some(click_pos) = self.pending_click.take() {
                    let dims = self
                        .gpu
                        .as_ref()
                        .map(|g| (g.config.width as f32, g.config.height as f32));
                    if let Some((gpu_w, gpu_h)) = dims {
                        let selected = self.pick_entity(click_pos, gpu_w, gpu_h);
                        self.selected_entity = selected;
                        self.tracked_entity = selected;
                    }
                }

                // Request the next frame immediately.
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Request a redraw every time the event loop is about to go idle
        // so the simulation keeps ticking even without user input.
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Entry point
// ────────────────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    // Initialise structured logging.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("info,wgpu_core=warn,wgpu_hal=warn")
            }),
        )
        .init();

    info!("Phylon v{} starting", env!("CARGO_PKG_VERSION"));

    // Load configuration.
    let config_path = Path::new("data/default.ron");
    let sim_config =
        PhylonConfig::load(Some(config_path)).context("failed to load configuration")?;
    info!(
        tick_rate = sim_config.simulation.tick_rate,
        rng_seed = sim_config.simulation.rng_seed,
        "Configuration loaded"
    );

    // Build and run the winit event loop.
    let event_loop = EventLoop::new().context("failed to create event loop")?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = PhylonApp::new(sim_config);
    event_loop.run_app(&mut app).context("event loop error")?;

    info!("Phylon shutdown complete");
    Ok(())
}

/// Traverses the physics spring network to completely remove organisms marked as Dead.
fn process_deaths_system(
    mut commands: bevy_ecs::prelude::Commands,
    dead_q: bevy_ecs::prelude::Query<
        (
            bevy_ecs::entity::Entity,
            &physics::ParticleNode,
            &metabolism::Energy,
        ),
        bevy_ecs::prelude::With<metabolism::Dead>,
    >,
    spring_q: bevy_ecs::prelude::Query<(bevy_ecs::entity::Entity, &physics::Spring)>,
) {
    if dead_q.is_empty() {
        return;
    }

    let mut adj: std::collections::HashMap<
        bevy_ecs::entity::Entity,
        Vec<(bevy_ecs::entity::Entity, bevy_ecs::entity::Entity)>,
    > = std::collections::HashMap::new();

    for (s_entity, spring) in spring_q.iter() {
        adj.entry(spring.node_a)
            .or_default()
            .push((spring.node_b, s_entity));
        adj.entry(spring.node_b)
            .or_default()
            .push((spring.node_a, s_entity));
    }

    let mut nodes_to_despawn = std::collections::HashSet::new();
    let mut springs_to_despawn = std::collections::HashSet::new();

    for (head, node, energy) in dead_q.iter() {
        if nodes_to_despawn.contains(&head) {
            continue;
        }

        // Spawn a corpse entity at the position of the dead organism
        commands.spawn(ecology::Corpse {
            position: node.position,
            energy_value: energy.max, // Corpse yields the organism's max potential energy
            decay_timer: 1800,        // About 30 seconds at 60 FPS
            max_decay: 1800,
        });

        let mut queue = std::collections::VecDeque::new();
        queue.push_back(head);
        nodes_to_despawn.insert(head);

        while let Some(curr) = queue.pop_front() {
            if let Some(neighbors) = adj.get(&curr) {
                for &(neighbor, s_entity) in neighbors {
                    springs_to_despawn.insert(s_entity);
                    if nodes_to_despawn.insert(neighbor) {
                        queue.push_back(neighbor);
                    }
                }
            }
        }
    }

    for n in nodes_to_despawn {
        commands.entity(n).despawn();
    }
    for s in springs_to_despawn {
        commands.entity(s).despawn();
    }
}
