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

use std::sync::Arc;

use anyhow::{Context, Result};
use tracing::info;
use winit::window::Window;

use config::PhylonConfig;
use scheduler::SimulationScheduler;

pub struct GpuSurface {
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) query_set: Option<wgpu::QuerySet>,
    pub(crate) resolve_buffer: Option<wgpu::Buffer>,
    pub(crate) readback_buffer: Option<wgpu::Buffer>,
}

pub struct PhylonApp {
    /// Simulation configuration loaded at startup.
    pub(crate) sim_config: PhylonConfig,

    /// The scheduler that drives all simulation ticks.
    #[allow(dead_code)]
    pub(crate) scheduler: SimulationScheduler,

    /// The core ECS world.
    pub(crate) world: world::World,

    /// GPU surface resources (created after window is ready).
    /// Must be declared before `window` so it drops first!
    pub(crate) gpu: Option<GpuSurface>,

    /// Compute pipeline for physics (forces, integration, PBD).
    pub(crate) physics_compute: Option<gpu::physics_pipeline::PhysicsComputePipeline>,

    /// Compute pipeline for the diffusion field.
    pub(crate) diffusion_compute: Option<gpu::diffusion_pipeline::DiffusionComputePipeline>,

    /// Compute pipeline for the CTRNN brain.
    pub(crate) brain_compute: Option<gpu::brain_pipeline::BrainComputePipeline>,

    /// Debug renderer for entities (grey quads / circles).
    pub(crate) debug_renderer: Option<rendering::DebugRenderer>,

    /// SDF organic skin renderer (accumulate-then-threshold).
    pub(crate) sdf_skin_renderer: Option<rendering::SdfSkinRenderer>,

    /// Renderer for the diffusion field.
    pub(crate) field_renderer: Option<rendering::FieldRenderer>,

    /// The main window (created on `Resumed`).
    pub(crate) window: Option<Arc<Window>>,

    /// Egui winit integration state
    pub(crate) egui_state: Option<egui_winit::State>,

    /// Egui wgpu renderer
    pub(crate) egui_renderer: Option<egui_wgpu::Renderer>,

    /// Camera2D position
    pub(crate) camera_pos: common::Vec2,
    /// Camera2D zoom (scale)
    pub(crate) camera_zoom: f32,

    /// Currently selected entity for inspection
    pub(crate) selected_entity: Option<bevy_ecs::entity::Entity>,

    /// Entity currently tracked by the camera
    pub(crate) tracked_entity: Option<bevy_ecs::entity::Entity>,

    /// Track keyboard modifiers
    pub(crate) modifiers: winit::keyboard::ModifiersState,

    /// Pending canvas click to be processed after the render pass
    pub(crate) pending_click: Option<common::Vec2>,

    /// Current hover position in physical pixels
    pub(crate) current_hover_pos: Option<common::Vec2>,

    /// The viewport dimensions of the simulation canvas (x, y, w, h) in physical pixels
    pub(crate) canvas_rect: Option<[u32; 4]>,

    /// When `true`, render raw physics quads; when `false`, render SDF skin.
    pub(crate) debug_structural: bool,
    /// Thickness of bone lines in structural view.
    pub(crate) bone_line_thickness: f32,
    /// Active tab in the sidebar
    pub(crate) active_tab: ui::SidebarTab,

    /// Maximum number of simulation ticks fired per frame.
    #[allow(dead_code)]
    pub(crate) max_ticks_per_frame: u32,

    /// Total simulation time in seconds.
    pub(crate) total_sim_time: f32,

    /// Multiplier for simulation speed (1.0 = normal, 0.5 = half speed, 2.0 = double).
    pub(crate) simulation_speed: f32,

    /// Accumulator for sub-frame simulation steps.
    pub(crate) accumulated_time: f32,

    /// High-level application state (Main Menu vs Simulation).
    pub(crate) app_state: ui::AppState,

    /// If true, the simulation is paused and no physics/biology ticks occur.
    pub(crate) is_paused: bool,

    /// If true, show the About dialog.
    pub(crate) show_about: bool,

    /// If true, show the Documentation window.
    pub(crate) show_docs: bool,

    /// If true, overlay vision cones on the simulation viewport.
    pub(crate) show_vision_cones: bool,
    /// Currently hovered entity from mouse pos
    pub(crate) hovered_entity: Option<bevy_ecs::entity::Entity>,
    /// Time when the user first clicked "Quit"
    pub(crate) quit_confirm_time: Option<f64>,
    /// Time when the user first clicked "Main Menu"
    pub(crate) main_menu_confirm_time: Option<f64>,
}

impl PhylonApp {
    pub(crate) fn new(sim_config: PhylonConfig) -> Self {
        let scheduler = SimulationScheduler::new(&sim_config);

        let mut world = world::World::new();

        // Add resources
        world.ecs.insert_resource(physics::PhysicsConfig {
            dt: 0.016,
            ..Default::default()
        }); // 60hz tick
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
            0,
            0,
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
            0,
            0,
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
            0,
            0,
        );

        // Organism 4: Omnivore — purple fish
        let omnivore_hox = genetics::HoxSequence::fish(4, 1, [0.8, 0.2, 0.8]);
        let omnivore_genome = genetics::Genome::new_hox_driven(
            genetics::GenomeId(4),
            common::EntityId(0),
            omnivore_hox,
        );
        organisms::spawn_organism(
            &mut world.ecs,
            &omnivore_genome,
            common::Vec2::new(-80.0, 0.0),
            ecology::Diet::Omnivore,
            ecology::EcologicalCategory::None,
            0,
            0,
        );

        // Organism 5: Decomposer — small grey worm
        let decomposer_hox = genetics::HoxSequence::worm(3, [0.4, 0.4, 0.4]);
        let decomposer_genome = genetics::Genome::new_hox_driven(
            genetics::GenomeId(5),
            common::EntityId(0),
            decomposer_hox,
        );
        organisms::spawn_organism(
            &mut world.ecs,
            &decomposer_genome,
            common::Vec2::new(80.0, 0.0),
            ecology::Diet::Decomposer,
            ecology::EcologicalCategory::None,
            0,
            0,
        );

        // Spawn a static food/nutrient emitter at the center
        world.ecs.spawn(diffusion::Emitter {
            position: common::Vec2::new(0.0, 0.0),
            value: 10.0,
            radius: 50.0,
        });

        // Spawn initial mineral pellets for Producers
        for i in 0..10 {
            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_micros() as f32;
            let px = ((t + i as f32 * 123.4) % 400.0) - 200.0;
            let py = ((t + i as f32 * 321.4) % 400.0) - 200.0;
            world.ecs.spawn(ecology::MineralPellet {
                position: common::Vec2::new(px, py),
                energy_value: 50.0,
            });
        }

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
            current_hover_pos: None,
            canvas_rect: None,
            max_ticks_per_frame: 10,
            total_sim_time: 0.0,
            simulation_speed: 1.0,
            accumulated_time: 0.0,
            app_state: ui::AppState::default(),
            is_paused: false,
            show_about: false,
            show_docs: false,
            show_vision_cones: false,
            hovered_entity: None,
            quit_confirm_time: None,
            main_menu_confirm_time: None,
        }
    }

    /// Initialises the wgpu instance, adapter, device, and surface for `window`.
    ///
    /// This is called once after the window is created in `Resumed`.
    pub(crate) fn init_gpu(&mut self, window: Arc<Window>) -> Result<()> {
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
        let mut has_timestamp_query = false;
        if adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY)
            && adapter
                .features()
                .contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS)
        {
            required_features |= wgpu::Features::TIMESTAMP_QUERY;
            required_features |= wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
            has_timestamp_query = true;
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

        let (query_set, resolve_buffer, readback_buffer) = if has_timestamp_query {
            let qs = device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("GpuTimestamps"),
                count: 4,
                ty: wgpu::QueryType::Timestamp,
            });
            let rb = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ResolveBuffer"),
                size: 8 * 4,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ReadbackBuffer"),
                size: 8 * 4,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            (Some(qs), Some(rb), Some(readback))
        } else {
            (None, None, None)
        };

        self.gpu = Some(GpuSurface {
            surface,
            device,
            queue,
            config: surface_config,
            query_set,
            resolve_buffer,
            readback_buffer,
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
    pub(crate) fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
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
    pub(crate) fn pick_entity(
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

        let mut food_query = self
            .world
            .ecs
            .query::<(bevy_ecs::entity::Entity, &ecology::FoodPellet)>();
        for (entity, pellet) in food_query.iter(&self.world.ecs) {
            let dist = (pellet.position - world_pos).length();
            if dist < best_dist {
                best_dist = dist;
                best = Some(entity);
            }
        }

        let mut mineral_query = self
            .world
            .ecs
            .query::<(bevy_ecs::entity::Entity, &ecology::MineralPellet)>();
        for (entity, mineral) in mineral_query.iter(&self.world.ecs) {
            let dist = (mineral.position - world_pos).length();
            if dist < best_dist {
                best_dist = dist;
                best = Some(entity);
            }
        }

        let mut corpse_query = self
            .world
            .ecs
            .query::<(bevy_ecs::entity::Entity, &ecology::Corpse)>();
        for (entity, corpse) in corpse_query.iter(&self.world.ecs) {
            let dist = (corpse.position - world_pos).length();
            if dist < best_dist {
                best_dist = dist;
                best = Some(entity);
            }
        }

        best
    }
}
