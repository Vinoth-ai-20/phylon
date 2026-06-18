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

    /// Is the user currently dragging the camera?
    is_dragging: bool,
    /// Screen position where the last left-button press occurred.
    /// Used to distinguish a click (small delta) from a drag (large delta).
    click_start_pos: Option<common::Vec2>,
    /// Last recorded mouse position (physical pixels)
    last_mouse_pos: common::Vec2,

    /// When `true`, render raw physics quads; when `false`, render SDF skin.
    debug_structural: bool,

    /// Maximum number of simulation ticks fired per frame.
    #[allow(dead_code)]
    max_ticks_per_frame: u32,

    /// Total simulation time in seconds.
    total_sim_time: f32,
}

use bevy_ecs::prelude::*;

struct SpawnOrganismCommand {
    genome: genetics::Genome,
    position: common::Vec2,
}

impl bevy_ecs::world::Command for SpawnOrganismCommand {
    fn apply(self, world: &mut bevy_ecs::world::World) {
        organisms::spawn_organism(world, &self.genome, self.position);
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

        // Spawn the deterministic proto-fish for physics/rendering validation.
        // CPPN-driven organisms are still spawned via spawn_organism() during reproduction.
        organisms::spawn_proto_fish(&mut world.ecs, common::Vec2::new(0.0, 0.0));

        // Spawn a static food/nutrient emitter at the center
        world.ecs.spawn(diffusion::Emitter {
            position: common::Vec2::new(0.0, 0.0), // World center
            value: 50.0,
            radius: 20.0, // World radius
        });

        Self {
            sim_config,
            scheduler,
            world,
            gpu: None,
            physics_compute: None,
            diffusion_compute: None,
            debug_renderer: None,
            sdf_skin_renderer: None,
            field_renderer: None,
            window: None,
            egui_state: None,
            egui_renderer: None,
            camera_pos: common::Vec2::new(0.0, 0.0),
            camera_zoom: 1.0,
            selected_entity: None,
            is_dragging: false,
            click_start_pos: None,
            last_mouse_pos: common::Vec2::new(0.0, 0.0),
            debug_structural: false,
            max_ticks_per_frame: 4,
            total_sim_time: 0.0,
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

        let egui_context = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_context,
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
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
        // NDC (Normalized Device Coordinates): [-1,1] × [-1,1]
        let ndc_x = (screen_pos.x / gpu_w) * 2.0 - 1.0;
        let ndc_y = -((screen_pos.y / gpu_h) * 2.0 - 1.0); // Y is flipped

        // World space: invert the orthographic projection
        let half_w = (gpu_w / 2.0) / self.camera_zoom;
        let half_h = (gpu_h / 2.0) / self.camera_zoom;
        let world_x = ndc_x * half_w + self.camera_pos.x;
        let world_y = ndc_y * half_h + self.camera_pos.y;
        let world_pos = common::Vec2::new(world_x, world_y);

        let pick_radius = 12.0 / self.camera_zoom;

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

        // Advance time
        const DT: f32 = 0.016; // Fixed 60 Hz timestep
        self.total_sim_time += DT;

        // Record analytics — read entity_count before mutably borrowing the resource
        let entity_count = self.world.ecs.entities().len() as usize;
        if let Some(mut metrics) = self.world.ecs.get_resource_mut::<analytics::MetricsState>() {
            metrics.record_frame(entity_count, f64::from(DT));
        }

        // 1. Run Biology Systems (Sensing, Brain, Behavior)
        use bevy_ecs::system::RunSystemOnce;
        self.world.ecs.run_system_once(organisms::growth_system);
        self.world.ecs.run_system_once(sensing::sensing_system);
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
        let updated_nodes =
            physics_compute.compute_step(&gpu.device, &gpu.queue, &gpu_nodes, &gpu_springs, 0.016);

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
        if let Some(mut events) = self
            .world
            .ecs
            .get_resource_mut::<bevy_ecs::event::Events<reproduction::BirthRequest>>()
        {
            events.update();
        }

        let Some(diffusion_compute) = self.diffusion_compute.as_mut() else {
            return Ok(());
        };
        let Some(field_renderer) = self.field_renderer.as_ref() else {
            return Ok(());
        };

        // 5. Gather diffusion emitters and run compute
        let (diff_rate, dec_rate) = {
            let mut diffusion_config = self.world.ecs.resource_mut::<diffusion::DiffusionConfig>();

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

        if let Some(field_data) = diffusion_compute.try_read_field() {
            let mut cpu_field = self.world.ecs.resource_mut::<diffusion::CpuFieldState>();
            cpu_field.data = field_data;
        }

        // 6. Gather rendering instances
        let mut debug_instances = Vec::new();
        let mut sdf_bones = Vec::new();

        // Build node position lookup for bone endpoint resolution
        let mut node_positions: std::collections::HashMap<bevy_ecs::entity::Entity, [f32; 2]> =
            std::collections::HashMap::new();

        let mut query_nodes_render = self
            .world
            .ecs
            .query::<(bevy_ecs::entity::Entity, &physics::ParticleNode)>();
        for (entity, node) in query_nodes_render.iter(&self.world.ecs) {
            node_positions.insert(entity, [node.position.x, node.position.y]);
            debug_instances.push(rendering::DebugInstance {
                position: [node.position.x, node.position.y],
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
        }

        // Collect Rigid bones for SDF capsule rendering
        let mut query_springs_render = self.world.ecs.query::<&physics::Spring>();
        for spring in query_springs_render.iter(&self.world.ecs) {
            if spring.constraint_type != physics::ConstraintType::Rigid
                && spring.constraint_type != physics::ConstraintType::Rotational
            {
                continue;
            }
            if let (Some(&pa), Some(&pb)) = (
                node_positions.get(&spring.node_a),
                node_positions.get(&spring.node_b),
            ) {
                let radius = if spring.is_fin == 1 { 5.0 } else { 8.0 };
                sdf_bones.push(rendering::SdfBoneInstance {
                    pos_a: pa,
                    pos_b: pb,
                    radius,
                    color: [0.15, 0.72, 0.45],
                });
            }
        }

        // Render food pellets (always shown in debug view)
        let mut query_food = self.world.ecs.query::<&ecology::FoodPellet>();
        for food in query_food.iter(&self.world.ecs) {
            debug_instances.push(rendering::DebugInstance {
                position: [food.position.x, food.position.y],
                color: [1.0, 0.8, 0.0, 1.0],
                radius: 2.5,
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

        if let (Some(egui_state), Some(window)) = (&mut self.egui_state, &self.window) {
            let raw_input = egui_state.take_egui_input(window);
            let ctx = egui_state.egui_ctx().clone();

            let mut ui_rect = egui::Rect::NOTHING;
            let output = ctx.run(raw_input, |ctx| {
                ui_rect = ui::render_ui(
                    ctx,
                    &mut self.world,
                    self.camera_pos,
                    self.camera_zoom,
                    self.selected_entity,
                    &mut self.debug_structural,
                );
            });

            egui_state.handle_platform_output(window, output.platform_output.clone());

            let scale = window.scale_factor() as f32;
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
            }

            full_output = Some(output);
            egui_context = Some(ctx);
        }

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame"),
            });

        // Render the continuous diffusion field as the background (clearing the screen)
        field_renderer.render(
            &gpu.device,
            &mut encoder,
            &view,
            diffusion_compute.current_texture_view(),
            central_rect_px,
        );

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
                    [gpu.config.width as f32, gpu.config.height as f32],
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

        Ok(())
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
                let response = egui_state.on_window_event(window, &event);
                if response.consumed {
                    return; // event handled by egui
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
            WindowEvent::MouseInput { state, button, .. } => {
                if button == winit::event::MouseButton::Left {
                    match state {
                        winit::event::ElementState::Pressed => {
                            self.is_dragging = true;
                            self.click_start_pos = Some(self.last_mouse_pos);
                        }
                        winit::event::ElementState::Released => {
                            self.is_dragging = false;

                            // Distinguish click (≤5 px delta) from drag (>5 px)
                            if let Some(start) = self.click_start_pos.take() {
                                let delta = (self.last_mouse_pos - start).length();
                                if delta <= 5.0 {
                                    // Extract GPU dimensions first so we don't
                                    // hold a borrow on self when calling pick_entity
                                    let dims = self
                                        .gpu
                                        .as_ref()
                                        .map(|g| (g.config.width as f32, g.config.height as f32));
                                    if let Some((gpu_w, gpu_h)) = dims {
                                        let click_pos = self.last_mouse_pos;
                                        self.selected_entity =
                                            self.pick_entity(click_pos, gpu_w, gpu_h);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            WindowEvent::MouseWheel {
                delta: winit::event::MouseScrollDelta::LineDelta(_x, y),
                ..
            } => {
                self.camera_zoom *= 1.0 + (y * 0.1);
                self.camera_zoom = self.camera_zoom.clamp(0.1, 10.0);
            }
            WindowEvent::MouseWheel { .. } => {}
            WindowEvent::CursorMoved { position, .. } => {
                let current_pos = common::Vec2::new(position.x as f32, position.y as f32);
                if self.is_dragging {
                    let delta = current_pos - self.last_mouse_pos;
                    self.camera_pos.x -= delta.x / self.camera_zoom;
                    self.camera_pos.y += delta.y / self.camera_zoom;
                }
                self.last_mouse_pos = current_pos;
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.render() {
                    error!("Render error: {e:#}");
                    event_loop.exit();
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
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
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
