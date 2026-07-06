//! # Phylon Application
//!
//! The main binary entry point for the Phylon simulation.
//!
//! ## Responsibilities
//!
//! 1. Parse CLI arguments and locate the config file.
//! 2. Initialise structured logging via `tracing_subscriber`.
//! 3. Load `PhylonConfig` from `data/default.ron` (falls back to defaults).
//! 4. Create a `winit` `EventLoop` and application window.
//! 5. Initialise a `wgpu` surface on the window.
//! 6. Create a `SimulationScheduler`.
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

/// # Hardware Graphics Context
///
/// ## 1. What Happens
/// The `GpuContext` holds the underlying device handles (`wgpu::Device`, `wgpu::Queue`)
/// and the swapchain (`wgpu::Surface`) required to interface with the physical GPU.
///
/// ## 2. Why It Happens
/// We cannot rely on a pure CPU simulation if we want to scale to 10,000 organisms.
/// We need low-level access to the GPU to dispatch massive parallel compute shaders
/// (for Physics and Diffusion) and to render the SDF organism skin.
///
/// ## 3. How It Happens
/// Initialized once during `PhylonApp` startup via `wgpu::Instance`. It is kept alive
/// for the duration of the application and passed by reference to the pipeline renderers
/// each frame.
pub struct GpuContext {
    pub(crate) surface: Option<wgpu::Surface<'static>>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: Option<wgpu::SurfaceConfiguration>,
    pub(crate) query_set: Option<wgpu::QuerySet>,
    pub(crate) resolve_buffer: Option<wgpu::Buffer>,
    pub(crate) readback_buffer: Option<wgpu::Buffer>,
}

// PhylonUIState was removed in favor of ui::WorkbenchState.

/// # Phylon Application Orchestrator
///
/// ## 1. What Happens
/// The `PhylonApp` struct is the Composition Root of the entire engine. It holds the
/// Bevy ECS `World`, the WGPU graphics context, the immediate-mode EGUI state, and the
/// bindings to the custom GPU Compute pipelines (Physics, Diffusion, and Neural Networks).
///
/// ## 2. Why It Happens
/// Architecturally, ALife engines require a strict boundary between discrete logic (biology,
/// genetics, metabolism) and continuous presentation (rendering, input). The `PhylonApp`
/// exists to safely bridge these domains without violating Rust's borrowing rules, managing
/// the lifetime of the GPU buffers alongside the ECS world data.
///
/// ## 3. How It Happens
/// During initialization, `PhylonApp` consumes the `PhylonConfig` to bootstrap the GPU device.
/// On every OS event loop iteration, it delegates control flow to either the UI renderer
/// or the `SimulationScheduler`. Memory is passed over the PCIe bus to the GPU compute shaders
/// strictly through the `Option<GpuContext>` and mapped uniform buffers.
pub(crate) struct PhylonApp {
    /// Deserialised application/simulation config
    pub(crate) sim_config: PhylonConfig,

    /// Drives the biological/physics simulation ticks
    #[allow(dead_code)]
    pub(crate) scheduler: SimulationScheduler,

    /// Central ECS World holding all entities and global resources
    pub(crate) world: world::World,

    /// Retained wgpu context (device, queue, optional surface)
    pub(crate) gpu: Option<GpuContext>,

    /// Compute pipeline for physics constraint resolution
    pub(crate) physics_compute: Option<gpu::physics_pipeline::PhysicsComputePipeline>,

    /// Compute pipeline for reaction-diffusion fields (pheromones)
    pub(crate) diffusion_compute: Option<gpu::diffusion_pipeline::DiffusionComputePipeline>,

    pub(crate) splat_compute: Option<rendering::SplatComputePipeline>,

    pub(crate) brain_compute: Option<gpu::brain_pipeline::BrainComputePipeline>,

    /// Rendering pipeline for structural/debug view
    pub(crate) debug_renderer: Option<rendering::DebugRenderer>,

    /// Rendering pipeline for organic SDF skin view
    pub(crate) sdf_skin_renderer: Option<rendering::SdfSkinRenderer>,

    /// Renderer for the diffusion field.
    pub(crate) field_renderer: Option<rendering::FieldRenderer>,

    /// The main window (created on `Resumed`).
    pub(crate) window: Option<Arc<Window>>,

    /// Egui winit integration state
    pub(crate) egui_state: Option<egui_winit::State>,

    /// Egui wgpu renderer
    pub(crate) egui_renderer: Option<egui_wgpu::Renderer>,

    /// The UI State bundle
    pub(crate) ui: ui::WorkbenchState,
    pub(crate) app_state: ui::AppState,

    /// Maximum number of simulation ticks fired per frame.
    #[allow(dead_code)]
    pub(crate) max_ticks_per_frame: u32,

    /// Total simulation time in seconds.
    pub(crate) total_sim_time: f32,

    /// Multiplier for simulation speed (1.0 = normal, 0.5 = half speed, 2.0 = double).
    pub(crate) simulation_speed: f32,

    /// Accumulator for sub-frame simulation steps.
    pub(crate) accumulated_time: f32,

    /// Wall-clock time of the previous `render()` call, used to compute the
    /// real elapsed time driving `accumulated_time` instead of a fixed
    /// per-redraw increment.
    pub(crate) last_frame_instant: std::time::Instant,

    /// Storage manager for snapshots and database logs
    #[allow(dead_code)]
    pub(crate) storage: storage::StorageManager,

    /// Channel for receiving background task results (like async save/load)
    pub(crate) task_rx: Option<std::sync::mpsc::Receiver<BackgroundTaskResult>>,

    /// Channel for sending background task results
    pub(crate) task_tx: Option<std::sync::mpsc::Sender<BackgroundTaskResult>>,

    /// Physics GPU readback dispatched last tick, resolved at the start of
    /// this tick (paired with the entities each returned node belongs to) —
    /// lets the GPU work for tick N overlap with tick N's CPU-side systems
    /// instead of stalling on it immediately after submission.
    pub(crate) pending_physics: Option<(
        gpu::physics_pipeline::PendingPhysicsReadback,
        Vec<bevy_ecs::entity::Entity>,
    )>,

    /// Brain (CTRNN) GPU readback dispatched last tick, resolved at the start
    /// of this tick (paired with the entity/start-node/length each integrated
    /// node range belongs to). Same overlap rationale as `pending_physics`.
    pub(crate) pending_brain: Option<(gpu::brain_pipeline::PendingBrainReadback, BrainOffsets)>,

    /// Set by `MenuAction::TakeScreenshot`; consumed at the start of the next
    /// `render()` call, right before `output.present()`, since that's the
    /// only place the live swapchain texture is available.
    pub(crate) pending_screenshot: bool,

    /// `Some` while a recording is in progress — accumulates captured frames
    /// until `MenuAction::ToggleRecording` stops it and encodes them to GIF.
    pub(crate) recording: Option<crate::capture::RecordingState>,
}

/// Per-entity `(start_node_index, node_count)` offsets into a batched
/// `GpuCtrnnNode` upload, used to scatter the resolved brain readback back
/// into each organism's `Brain` component.
pub(crate) type BrainOffsets = Vec<(bevy_ecs::entity::Entity, u32, usize)>;

pub(crate) enum BackgroundTaskResult {
    SaveComplete(Result<(), String>),
    LoadComplete(Result<storage::snapshot::SimulationSnapshot, String>),
}

impl PhylonApp {
    pub(crate) fn new(sim_config: PhylonConfig) -> Self {
        let scheduler = SimulationScheduler::new(&sim_config);

        let mut world = world::World::new();

        // The single source of truth for the fixed per-tick delta-time —
        // see common::TickRate's doc comment. Computed once, here, so the
        // physics config below and every other fixed-timestep call site
        // (simulation.rs, render.rs, status bar tick display) agree with
        // `config.simulation.tick_rate` and with each other.
        let tick_rate = common::TickRate::from_hz(sim_config.simulation.tick_rate);

        // Add resources
        world.ecs.insert_resource(physics::PhysicsConfig {
            dt: tick_rate.dt(),
            ..Default::default()
        });
        world.ecs.insert_resource(tick_rate);
        world
            .ecs
            .insert_resource(metabolism::GlobalAtmosphere::default());
        world
            .ecs
            .insert_resource(diffusion::DiffusionConfig::default());
        world
            .ecs
            .insert_resource(diffusion::CpuFieldState::default());
        world
            .ecs
            .insert_resource(diffusion::CpuHazardFieldState::default());
        world.ecs.insert_resource(ecology::EcologyConfig::default());
        world
            .ecs
            .insert_resource(ecology::ResourceSpatialGrids::new(50.0));
        world
            .ecs
            .insert_resource(ecology::catastrophe::CatastropheConfig::default());
        world
            .ecs
            .insert_resource(ecology::catastrophe::CatastropheManager::default());
        world
            .ecs
            .insert_resource(bevy_ecs::event::Events::<reproduction::BirthRequest>::default());
        world.ecs.insert_resource(
            bevy_ecs::event::Events::<ecology::catastrophe::HazardSpawned>::default(),
        );
        world.ecs.insert_resource(analytics::MetricsState::new());
        world.ecs.insert_resource(analytics::NarrationLog::new(100));
        world
            .ecs
            .insert_resource(ui::types::HeatmapState::default());
        world.ecs.insert_resource(behavior::BehaviorConfig {
            signal_energy_cost_per_unit: sim_config.simulation.signal_energy_cost_per_unit,
        });
        world
            .ecs
            .insert_resource(brain::PlasticityConfig::default());
        world.ecs.insert_resource(ecology::DiseaseConfig::default());
        world
            .ecs
            .insert_resource(ecology::FungalNetworkConfig::default());

        // The single seeded source of randomness for every stochastic system
        // (genetics mutation/crossover, spawn placement, mate selection, ...)
        // — see common::SimRng's doc comment for the determinism rationale.
        world
            .ecs
            .insert_resource(common::SimRng::from_seed(sim_config.simulation.rng_seed));

        let mut lineage_tracker = evolution::LineageTracker::new();
        let mut species_registry = evolution::SpeciesRegistry::default();

        let env_manager = environment::EnvironmentManager::new(
            sim_config.simulation.rng_seed,
            sim_config.simulation.toroidal_world,
            // Must match the hard physics/diffusion/render bounds (±1500,
            // physics.wgsl / simulation.rs / render.rs) so procedurally
            // generated resources never land outside the playable/rendered
            // area.
            1500.0, // World width for procedural generation
            1500.0, // World height
        );
        world.ecs.insert_resource(env_manager);

        let mut tracker = genetics::GlobalInnovationTracker::default();
        world
            .ecs
            .resource_scope::<common::SimRng, _>(|ecs, mut sim_rng| {
                seed_ecosystem(
                    ecs,
                    &mut lineage_tracker,
                    &mut species_registry,
                    &mut tracker,
                    &mut sim_rng.0,
                );
            });
        world.ecs.insert_resource(lineage_tracker);
        world.ecs.insert_resource(species_registry);
        world.ecs.insert_resource(tracker);

        let (task_tx, task_rx) = std::sync::mpsc::channel();
        let storage = storage::StorageManager::new();

        Self {
            sim_config,
            scheduler,
            world,
            gpu: None,
            physics_compute: None,
            diffusion_compute: None,
            splat_compute: None,
            brain_compute: None,
            debug_renderer: None,
            sdf_skin_renderer: None,
            field_renderer: None,
            window: None,
            egui_state: None,
            egui_renderer: None,
            ui: ui::WorkbenchState::default(),
            app_state: ui::AppState::default(),
            max_ticks_per_frame: 50,
            total_sim_time: 0.0,
            simulation_speed: 1.0,
            accumulated_time: 0.0,
            last_frame_instant: std::time::Instant::now(),
            storage,
            task_rx: Some(task_rx),
            task_tx: Some(task_tx),
            pending_physics: None,
            pending_brain: None,
            pending_screenshot: false,
            recording: None,
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
            // COPY_SRC (in addition to the required RENDER_ATTACHMENT) lets
            // the screenshot/recording capture (`crate::capture`) read the
            // presented frame back via `copy_texture_to_buffer` — without it
            // the swapchain texture only supports being rendered into, and
            // the copy is a validation-error panic (fatal by default in
            // wgpu 22, since it treats GPU errors as fatal panics).
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
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
        let splat_compute = rendering::SplatComputePipeline::new(&device, 256, 256);
        let brain_compute = gpu::brain_pipeline::BrainComputePipeline::new(&device);

        let egui_context = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        ui::theme::install_fonts(&mut fonts);
        egui_remixicon::add_to_fonts(&mut fonts);
        egui_context.set_fonts(fonts);
        ui::theme::apply_style(&egui_context);
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

        self.gpu = Some(GpuContext {
            surface: Some(surface),
            device,
            queue,
            config: Some(surface_config),
            query_set,
            resolve_buffer,
            readback_buffer,
        });
        self.debug_renderer = Some(debug_renderer);
        self.sdf_skin_renderer = Some(sdf_skin_renderer);
        self.field_renderer = Some(field_renderer);
        self.physics_compute = Some(physics_compute);
        self.diffusion_compute = Some(diffusion_compute);
        self.splat_compute = Some(splat_compute);
        self.brain_compute = Some(brain_compute);
        self.egui_state = Some(egui_state);
        self.egui_renderer = Some(egui_renderer);
        self.window = Some(window);

        info!("GPU surface initialised ({surface_format:?}, {present_mode:?})");
        Ok(())
    }

    /// Initialises the wgpu instance, adapter, and device for headless mode.
    /// No surface or rendering pipeline is created.
    pub(crate) fn init_gpu_headless(&mut self) -> Result<()> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .context("no suitable GPU adapter found for headless mode")?;

        let mut required_features = wgpu::Features::empty();
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
                label: Some("PhylonDevice_Headless"),
                required_features,
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .context("failed to create wgpu device for headless")?;

        let physics_compute = gpu::physics_pipeline::PhysicsComputePipeline::new(&device);
        let diffusion_compute =
            gpu::diffusion_pipeline::DiffusionComputePipeline::new(&device, 256, 256);
        let splat_compute = rendering::SplatComputePipeline::new(&device, 256, 256);
        let brain_compute = gpu::brain_pipeline::BrainComputePipeline::new(&device);

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

        self.gpu = Some(GpuContext {
            surface: None,
            device,
            queue,
            config: None,
            query_set,
            resolve_buffer,
            readback_buffer,
        });

        self.physics_compute = Some(physics_compute);
        self.diffusion_compute = Some(diffusion_compute);
        self.splat_compute = Some(splat_compute);
        self.brain_compute = Some(brain_compute);

        info!("GPU headless mode initialised");
        Ok(())
    }

    /// Reconfigures the surface after a window resize.
    pub(crate) fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        let Some(gpu) = self.gpu.as_mut() else { return };
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        if let Some(config) = &mut gpu.config {
            config.width = new_size.width;
            config.height = new_size.height;
            if let Some(surface) = &gpu.surface {
                surface.configure(&gpu.device, config);
            }
        }
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
        let canvas_rect = self
            .ui
            .canvas_rect
            .map(|r| [r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32])
            .unwrap_or([0.0, 0.0, gpu_w, gpu_h]);
        let [vx, vy, vw, vh] = canvas_rect;
        let local_x = screen_pos.x - vx;
        let local_y = screen_pos.y - vy;

        // NDC (Normalized Device Coordinates): [-1,1] × [-1,1]
        let ndc_x = (local_x / vw) * 2.0 - 1.0;
        let ndc_y = -((local_y / vh) * 2.0 - 1.0); // Y is flipped

        // World space: invert the orthographic projection
        let half_w = (vw / 2.0) / self.ui.camera_zoom;
        let half_h = (vh / 2.0) / self.ui.camera_zoom;
        let world_x = ndc_x * half_w + self.ui.camera_pos.x;
        let world_y = ndc_y * half_h + self.ui.camera_pos.y;
        let world_pos = common::Vec2::new(world_x, world_y);

        let pick_radius = 30.0 / self.ui.camera_zoom;

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

pub(crate) fn seed_ecosystem(
    world: &mut bevy_ecs::world::World,
    lineage_tracker: &mut evolution::LineageTracker,
    species_registry: &mut evolution::SpeciesRegistry,
    tracker: &mut genetics::GlobalInnovationTracker,
    rng: &mut impl rand::Rng,
) {
    // 1. Define Prototypes
    // Colors come from `Diet::standard_color()` — the single canonical
    // per-diet palette shared with the sandbox spawn tool, so an organism
    // looks the same regardless of how it was spawned.
    let worm_genome = genetics::Genome::new_hox_driven(
        genetics::GenomeId(1),
        common::EntityId(0),
        genetics::HoxSequence::worm(6, ecology::Diet::Herbivore.standard_color()),
    );

    let fish_genome = genetics::Genome::new_hox_driven(
        genetics::GenomeId(2),
        common::EntityId(0),
        genetics::HoxSequence::fish(5, 2, ecology::Diet::Carnivore.standard_color()),
    );

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
            ecology::Diet::Herbivore.standard_color(),
        ),
    );

    let omnivore_genome = genetics::Genome::new_hox_driven(
        genetics::GenomeId(4),
        common::EntityId(0),
        genetics::HoxSequence::fish(4, 1, ecology::Diet::Omnivore.standard_color()),
    );

    let decomposer_genome = genetics::Genome::new_hox_driven(
        genetics::GenomeId(5),
        common::EntityId(0),
        genetics::HoxSequence::worm(3, ecology::Diet::Decomposer.standard_color()),
    );

    let producer_genome = genetics::Genome::new_hox_driven(
        genetics::GenomeId(6),
        common::EntityId(0),
        genetics::HoxSequence::plant(ecology::Diet::Producer.standard_color()),
    );

    // 2. Helper to spawn a population
    let mut spawn_pop = |genome: &genetics::Genome, diet: ecology::Diet, count: usize| {
        let lineage_id = lineage_tracker.new_lineage_id();
        for _ in 0..count {
            let px = rng.gen_range(-1000.0..1000.0);
            let py = rng.gen_range(-1000.0..1000.0);

            // Give each individual a unique randomized brain if they are not producers
            let mut ind_genome = genome.clone();
            if diet != ecology::Diet::Producer {
                for _ in 0..10 {
                    ind_genome.mutate(1.0, rng, tracker);
                }
            }

            let species_id = species_registry.classify(&ind_genome);

            let e = organisms::spawn_organism(
                world,
                &ind_genome,
                common::Vec2::new(px, py),
                diet.clone(),
                ecology::EcologicalCategory::None,
                0,
                0,
                rng,
            );
            lineage_tracker.register_birth(
                common::EntityId(e.to_bits()),
                None,
                lineage_id,
                species_id,
                0,
                0,
            );
        }
    };

    // 3. Spawn Populations
    spawn_pop(&producer_genome, ecology::Diet::Producer, 260);
    spawn_pop(&worm_genome, ecology::Diet::Herbivore, 150);
    spawn_pop(&branchy_genome, ecology::Diet::Herbivore, 150);
    spawn_pop(&omnivore_genome, ecology::Diet::Omnivore, 40);
    spawn_pop(&decomposer_genome, ecology::Diet::Decomposer, 50);
    spawn_pop(&fish_genome, ecology::Diet::Carnivore, 20);

    // 4. Spawn Resource Hotspots
    for _ in 0..20 {
        let px = rng.gen_range(-1000.0..1000.0);
        let py = rng.gen_range(-1000.0..1000.0);
        world.spawn(diffusion::Emitter {
            position: common::Vec2::new(px, py),
            value: rng.gen_range(5.0..20.0),
            radius: rng.gen_range(50.0..150.0),
            layer: diffusion::FieldLayer::Energy,
        });
    }

    // 5. Spawn Initial Minerals
    for _ in 0..300 {
        let px = rng.gen_range(-1000.0..1000.0);
        let py = rng.gen_range(-1000.0..1000.0);
        world.spawn(ecology::MineralPellet {
            position: common::Vec2::new(px, py),
            energy_value: 50.0,
        });
    }

    // 6. Spawn Initial Food
    for _ in 0..300 {
        let px = rng.gen_range(-1000.0..1000.0);
        let py = rng.gen_range(-1000.0..1000.0);
        world.spawn(ecology::FoodPellet {
            position: common::Vec2::new(px, py),
            energy_value: 50.0,
        });
    }
}
