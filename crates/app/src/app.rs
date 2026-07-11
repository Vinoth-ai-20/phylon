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
//! 6. Run the event loop, calling `PhylonApp::update_simulation` each tick
//!    (the per-tick system order lives in `simulation::update_simulation`;
//!    see that module's doc comment — Phase 6, Epic A removed the
//!    `SimulationScheduler` this step previously constructed here, since it
//!    was never actually advanced by anything; the `scheduler` crate itself
//!    is untouched and remains a workspace member, just no longer a
//!    dependency of `app`) and presenting a cleared frame on each
//!    `RedrawRequested`.
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

/// The wgpu adapter/device/queue plus the 4 GPU compute pipelines and
/// optional timestamp-query resources — everything `init_gpu` (windowed)
/// and `init_gpu_headless` both construct identically. Extracted (Phase 7,
/// W5b) since the only real differences between the two call sites were a
/// handful of knobs (surface compatibility, device label, base feature
/// set, error messages), not the construction logic itself.
struct GpuCore {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    physics_compute: gpu::physics_pipeline::PhysicsComputePipeline,
    diffusion_compute: gpu::diffusion_pipeline::DiffusionComputePipeline,
    splat_compute: rendering::SplatComputePipeline,
    brain_compute: gpu::brain_pipeline::BrainComputePipeline,
    query_set: Option<wgpu::QuerySet>,
    resolve_buffer: Option<wgpu::Buffer>,
    readback_buffer: Option<wgpu::Buffer>,
}

/// Requests an adapter/device (opting into `TIMESTAMP_QUERY`/
/// `TIMESTAMP_QUERY_INSIDE_ENCODERS` when the adapter supports both, same
/// as before this extraction) and builds the 4 compute pipelines plus the
/// timestamp query-set/buffers if timestamp queries are available.
/// Verbatim extraction of the logic `init_gpu`/`init_gpu_headless`
/// previously duplicated — no behavior changed, only named and shared.
fn request_gpu_core(
    instance: &wgpu::Instance,
    compatible_surface: Option<&wgpu::Surface>,
    base_features: wgpu::Features,
    device_label: &'static str,
    adapter_error: &'static str,
    device_error: &'static str,
) -> Result<GpuCore> {
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface,
        force_fallback_adapter: false,
    }))
    .context(adapter_error)?;

    let mut required_features = base_features;
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
            label: Some(device_label),
            required_features,
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
        },
        None,
    ))
    .context(device_error)?;

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

    Ok(GpuCore {
        adapter,
        device,
        queue,
        physics_compute,
        diffusion_compute,
        splat_compute,
        brain_compute,
        query_set,
        resolve_buffer,
        readback_buffer,
    })
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
/// On every OS event loop iteration, it delegates control flow to the UI renderer, which
/// drives `simulation::update_simulation` directly (see this module's top doc comment).
/// Memory is passed over the PCIe bus to the GPU compute shaders
/// strictly through the `Option<GpuContext>` and mapped uniform buffers.
pub(crate) struct PhylonApp {
    /// Deserialised application/simulation config
    pub(crate) sim_config: PhylonConfig,

    /// Cross-session UI preferences (Phase 6, Epic J) — High Contrast Mode,
    /// UI scale, whether onboarding hints have ever been shown, (Phase 7,
    /// W0d) recent-items history, and (Phase 7, W3a) panel layout. See
    /// `crate::preferences`'s module doc comment for why this is separate
    /// from `sim_config`.
    pub(crate) preferences: crate::preferences::Preferences,

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

    /// Mesh-based capsule organism renderer (Phase 8, ADR-P8-03) — the
    /// replacement for `SdfSkinRenderer`.
    pub(crate) organism_renderer: Option<rendering::OrganismRenderer>,

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

    /// Set by `MenuAction::ExportChartPng` (Phase 5, SX-7c) — `(x, y, width,
    /// height)` in physical pixels. Same deferred-to-next-`render()` timing
    /// as `pending_screenshot`, just cropped to one Metrics chart's rect
    /// instead of the whole window.
    pub(crate) pending_chart_export: Option<(u32, u32, u32, u32)>,

    /// `Some` while a recording is in progress — accumulates captured frames
    /// until `MenuAction::ToggleRecording` stops it and encodes them to GIF.
    pub(crate) recording: Option<crate::capture::RecordingState>,

    /// This run's experiment identity (id, description, RNG seed) — built
    /// from `config::ResearchConfig::experiment_id`/`SimulationConfig::rng_seed`
    /// and persisted to `data/experiments/<id>/manifest.ron` at startup, so
    /// `research::ExperimentManifest` is no longer dead code (see that
    /// crate's doc comment for the history).
    pub(crate) experiment_manifest: research::ExperimentManifest,

    /// Every safe external intervention (see `storage::replay::ReplayAction`)
    /// applied this run, in tick order — always recording (cheap; these
    /// events are rare), so a `.phylon-replay` bundle is available to save
    /// at any point via `MenuAction::SaveState`'s replay counterpart.
    pub(crate) replay_log: storage::replay::ReplayLog,

    /// Phase 9, P9.1 (performance foundation): cross-frame scratch storage
    /// for `gather_world_render_instances`'s intermediate lookup tables —
    /// see `render::world_instances::RenderInstanceScratch`'s doc comment.
    pub(crate) render_scratch: crate::render::world_instances::RenderInstanceScratch,

    /// Phase 9, P9.1 (performance foundation): cross-tick scratch storage
    /// for `update_simulation`'s GPU node/spring buffer gathering — reused
    /// the same way `render_scratch` is (see that field's doc comment);
    /// `.clear()` at the top of each tick keeps the backing allocation
    /// instead of reallocating from empty every tick, which otherwise
    /// happens up to `max_ticks_per_frame` times per rendered frame.
    pub(crate) sim_scratch: crate::simulation::SimTickScratch,
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
        let preferences =
            crate::preferences::Preferences::load(&crate::preferences::preferences_path());

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
        // Phase 9, Goal 3 behavior-validation finding: `EcologyConfig`'s
        // default `max_organisms` (50) was never connected to
        // `SimulationConfig::target_organism_count` (default 1_000, the
        // value `seed_ecosystem` actually spawns towards) — since founders
        // are spawned directly (bypassing `reproduction_system`'s own
        // population-cap check), a founder population past 50 permanently
        // blocked *all* asexual and sexual reproduction from tick 1 onward
        // in every default-config run, measured directly via a real
        // headless run showing `births_since_start = 0` /
        // `reproductions_since_start = 0` across 2000 ticks. Wiring the
        // already-existing (previously unread) config field through fixes
        // the root cause without inventing a new one.
        world.ecs.insert_resource(ecology::EcologyConfig {
            max_organisms: sim_config.simulation.target_organism_count as usize,
            ..Default::default()
        });
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
        // Phase 4, P4-E1: the first real `events::PhylonEvent` producer/
        // consumer wiring — registered the same way as the two native bevy
        // events above (see `crates/app/src/simulation.rs`'s per-tick
        // `Events::update()` calls, extended for this one too).
        world
            .ecs
            .insert_resource(bevy_ecs::event::Events::<events::PhylonEvent>::default());
        world.ecs.insert_resource(events::TimedEffects::default());
        // Phase 5, SX-1a: reads `PHYLON_MOTION_DIAGNOSTIC` once at startup —
        // see `motion_diagnostic::MotionDiagnosticConfig`'s doc comment for
        // why this isn't re-checked per tick.
        world
            .ecs
            .insert_resource(crate::motion_diagnostic::MotionDiagnosticConfig::from_env());
        world
            .ecs
            .insert_resource(crate::motion_diagnostic::MotionDiagnosticState::default());
        // Phase 9, Goal 3: reads `PHYLON_BEHAVIOR_VALIDATION` once at
        // startup — see `behavior_validation::BehaviorValidationConfig`'s
        // doc comment for why this isn't re-checked per tick.
        world
            .ecs
            .insert_resource(crate::behavior_validation::BehaviorValidationConfig::from_env());
        world
            .ecs
            .insert_resource(crate::behavior_validation::BehaviorValidationState::default());
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
        world
            .ecs
            .insert_resource(organisms::FlockingConfig::default());
        world
            .ecs
            .insert_resource(organisms::PackHuntingConfig::default());
        world
            .ecs
            .insert_resource(organisms::BiofilmConfig::default());

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

        let experiment_manifest = research::ExperimentManifest::new(
            sim_config.research.experiment_id.clone(),
            format!("Phylon run: {}", sim_config.research.experiment_id),
            sim_config.simulation.rng_seed,
        );
        let manifest_path = std::path::Path::new("data/experiments")
            .join(&experiment_manifest.id)
            .join("manifest.ron");
        if let Err(e) = experiment_manifest.save_to_ron(&manifest_path) {
            tracing::warn!("failed to save experiment manifest: {e}");
        }

        let replay_log = storage::replay::ReplayLog::new(sim_config.simulation.rng_seed);

        let mut ui = ui::WorkbenchState::default();
        ui.high_contrast = preferences.high_contrast;
        ui.ui_scale = preferences.ui_scale;
        ui.recent_items = preferences.recent_items.clone();
        // Phase 7, W3a: restore the persisted panel layout in place of
        // `WorkbenchState::default()`'s own hardcoded starting tree, then
        // rebuild `dock_tree` from it — `rebuild_tree_from_modes` is the
        // sole authoritative tree builder (see `layout.rs`'s own doc
        // comment), so restoring layout is exactly "call it again with the
        // restored inputs," not a second, parallel tree-construction path.
        ui.panel_modes = preferences.panel_modes.clone();
        ui.layout_shares = preferences.layout_shares.clone();
        ui::layout::rebuild_tree_from_modes(&mut ui.dock_tree, &ui.panel_modes, &ui.layout_shares);
        // Phase 7, W3c: restore saved workspaces + which one was last
        // active. Purely metadata layered on top of the shape already
        // restored above — see `ui::workspace`'s module doc comment.
        ui.workspaces = preferences.workspaces.clone();

        Self {
            sim_config,
            preferences,
            world,
            gpu: None,
            physics_compute: None,
            diffusion_compute: None,
            splat_compute: None,
            brain_compute: None,
            debug_renderer: None,
            organism_renderer: None,
            field_renderer: None,
            window: None,
            egui_state: None,
            egui_renderer: None,
            ui,
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
            pending_chart_export: None,
            recording: None,
            experiment_manifest,
            replay_log,
            render_scratch: Default::default(),
            sim_scratch: Default::default(),
        }
    }

    /// Syncs the live `WorkbenchState` preference fields (`high_contrast`,
    /// `ui_scale` — toggled directly in `sidebar.rs`'s Settings tab, with no
    /// `MenuAction` round-trip to hook a save into) back into `preferences`
    /// and writes it to disk. Called at both real exit paths this app has
    /// (`MenuAction::Quit`, the window's `CloseRequested` event) — see
    /// `crate::preferences`'s module doc comment for why exit-time saving
    /// was chosen over saving on every individual toggle.
    pub(crate) fn save_preferences(&mut self) {
        self.preferences.high_contrast = self.ui.high_contrast;
        self.preferences.ui_scale = self.ui.ui_scale;
        // Phase 7, W0d: recent-items history persists across restarts the
        // same way high_contrast/ui_scale do.
        self.preferences.recent_items = self.ui.recent_items.clone();
        // Phase 7, W3a: panel layout persists the same way. `layout_shares`
        // is already kept current every frame by `ui::render`'s
        // `extract_shares` (reads the live tree's actual split ratios), so
        // this is just copying its current value, not computing anything.
        self.preferences.panel_modes = self.ui.panel_modes.clone();
        self.preferences.layout_shares = self.ui.layout_shares.clone();
        // Phase 7, W3c: saved workspaces + active-workspace identity
        // persist the same way.
        self.preferences.workspaces = self.ui.workspaces.clone();
        self.preferences
            .save(&crate::preferences::preferences_path());
    }

    /// The current simulation tick, derived from `total_sim_time` and the
    /// configured tick rate — the same computation `MenuAction::SpawnManualHazard`'s
    /// handler already used inline before this helper existed. Used to tag
    /// recorded `storage::replay::ReplayEvent`s with the tick they occurred
    /// at.
    pub(crate) fn current_tick(&self) -> u64 {
        let dt = self.world.ecs.resource::<common::TickRate>().dt();
        (self.total_sim_time / dt).round() as u64
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

        let GpuCore {
            adapter,
            device,
            queue,
            physics_compute,
            diffusion_compute,
            splat_compute,
            brain_compute,
            query_set,
            resolve_buffer,
            readback_buffer,
        } = request_gpu_core(
            &instance,
            Some(&surface),
            wgpu::Features::FLOAT32_FILTERABLE,
            "PhylonDevice",
            "no suitable GPU adapter found",
            "failed to create wgpu device",
        )?;

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
        let organism_renderer = rendering::OrganismRenderer::new(
            &device,
            surface_format,
            size.width.max(1),
            size.height.max(1),
        );
        let field_renderer = rendering::FieldRenderer::new(&device, surface_format);

        let egui_context = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        ui::theme::install_fonts(&mut fonts);
        egui_remixicon::add_to_fonts(&mut fonts);
        egui_context.set_fonts(fonts);
        ui::theme::apply_style(&egui_context, false);
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
        self.organism_renderer = Some(organism_renderer);
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

        let GpuCore {
            device,
            queue,
            physics_compute,
            diffusion_compute,
            splat_compute,
            brain_compute,
            query_set,
            resolve_buffer,
            readback_buffer,
            ..
        } = request_gpu_core(
            &instance,
            None,
            wgpu::Features::empty(),
            "PhylonDevice_Headless",
            "no suitable GPU adapter found for headless mode",
            "failed to create wgpu device for headless",
        )?;

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
        if let Some(organism_renderer) = self.organism_renderer.as_mut() {
            organism_renderer.resize(&gpu.device, new_size.width, new_size.height);
        }
    }

    /// Converts a physical-pixel screen coordinate to a world-space ray and
    /// finds the nearest entity it hits, by true ray-vs-capsule (Phase 8,
    /// Epic 8.4) intersection against the exact radius the renderer draws
    /// each entity at — replacing the previous flat technique (unproject to
    /// the `Z = 0` plane, then 2D nearest-point-within-a-fudge-radius),
    /// which was blind to camera tilt and to each entity's actual on-screen
    /// size. "Nearest" here means nearest *along the ray* (smallest hit
    /// `t`), i.e. depth-correct — the frontmost entity under the cursor,
    /// not merely the one whose point happens to be closest to the Z=0
    /// unprojection.
    ///
    /// Returns `None` if nothing is hit, or if GPU surface is not ready.
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
        let local_pos = common::Vec2::new(screen_pos.x - vx, screen_pos.y - vy);
        let viewport_size = common::Vec2::new(vw, vh);

        let camera = self.ui.camera();
        let (origin, dir) = camera.screen_to_ray(local_pos, viewport_size);
        let node_radius = self.ui.node_radius;

        let mut best: Option<bevy_ecs::entity::Entity> = None;
        let mut best_t = f32::INFINITY;

        // query() requires &mut World in bevy_ecs 0.14
        let mut query = self
            .world
            .ecs
            .query::<(bevy_ecs::entity::Entity, &physics::ParticleNode)>();
        for (entity, node) in query.iter(&self.world.ecs) {
            if let Some(t) =
                rendering::ray_capsule_hit(origin, dir, node.position, node.position, node_radius)
            {
                if t < best_t {
                    best_t = t;
                    best = Some(entity);
                }
            }
        }

        let mut food_query = self
            .world
            .ecs
            .query::<(bevy_ecs::entity::Entity, &ecology::FoodPellet)>();
        for (entity, pellet) in food_query.iter(&self.world.ecs) {
            if let Some(t) = rendering::ray_capsule_hit(
                origin,
                dir,
                pellet.position,
                pellet.position,
                crate::render::organism_visuals::FOOD_PELLET_RADIUS,
            ) {
                if t < best_t {
                    best_t = t;
                    best = Some(entity);
                }
            }
        }

        let mut mineral_query = self
            .world
            .ecs
            .query::<(bevy_ecs::entity::Entity, &ecology::MineralPellet)>();
        for (entity, mineral) in mineral_query.iter(&self.world.ecs) {
            if let Some(t) = rendering::ray_capsule_hit(
                origin,
                dir,
                mineral.position,
                mineral.position,
                crate::render::organism_visuals::MINERAL_PELLET_RADIUS,
            ) {
                if t < best_t {
                    best_t = t;
                    best = Some(entity);
                }
            }
        }

        let mut corpse_query = self
            .world
            .ecs
            .query::<(bevy_ecs::entity::Entity, &ecology::Corpse)>();
        for (entity, corpse) in corpse_query.iter(&self.world.ecs) {
            if let Some(t) = rendering::ray_capsule_hit(
                origin,
                dir,
                corpse.position,
                corpse.position,
                crate::render::organism_visuals::CORPSE_RADIUS,
            ) {
                if t < best_t {
                    best_t = t;
                    best = Some(entity);
                }
            }
        }

        best
    }
}

/// Per-seed tunable weights for [`seed_regulatory_cppn`]'s four independent
/// local-activation domains (one per [`genetics::RegulatoryGeneRole`] region)
/// plus its shared monotonic/periodic bases. A named struct rather than 7
/// positional `f32`s — with this many knobs, positional args at the call
/// site are an easy place to transpose two values silently.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RegulatorySeedWeights {
    /// The output node's own bias — the network's baseline "all regions
    /// off" level; every region weight below adds on top of this.
    pub output_bias: f32,
    /// Weight on the local-activation bump centered over the Hox gene
    /// region (indices 0-2 of 10 — see `REGULATORY_GENE_ROLES`).
    pub hox_weight: f32,
    /// Weight on the bump centered over the Differentiation region (3-4).
    pub differentiation_weight: f32,
    /// Weight on the bump centered over the Effector region (5-6) — the
    /// region SX-2a's first architecture starved (see this function's doc
    /// comment's "second problem" section).
    pub effector_weight: f32,
    /// Weight on the bump centered over the Pigment region (7-9).
    pub pigment_weight: f32,
    /// Weight on a coarse (~2-cycle) periodic basis across the full gene
    /// range, for broad repeated/alternating structure.
    pub sine_coarse_weight: f32,
    /// Weight on a fine (~5-cycle) periodic basis, for finer repeated
    /// structure than `sine_coarse_weight` alone can produce.
    pub sine_fine_weight: f32,
}

/// A seed regulatory CPPN with real combinatorial representational capacity
/// (Phase 5, SX-2a — see `PHASE5_SX_ROADMAP.md` §11's full architectural
/// analysis, ADR-P5-06 and ADR-P5-07).
///
/// **First problem this replaces (ADR-P5-06):** the very first seed was a
/// single `Linear` output node with one incoming connection — since
/// `RegulatoryNetwork::generate` derives every gene's bias and every
/// gene-pair's edge weight from a *linear* function of gene index, its
/// output was strictly monotonic in gene index. Since a 3-bit Hox code is
/// read off three specific, adjacent gene indices, a monotonic bias function
/// can only ever threshold to a non-decreasing or non-increasing bit
/// sequence (`000,001,011,111` or `000,100,110,111`) — six of the eight
/// possible `SegmentType` codes, including `Muscle` (`010`), were
/// **structurally unreachable**, for any choice of the old `(bias, weight)`
/// parameters. Measured directly (§11): the unmutated "mostly Muscle body"
/// seed decoded `Germinal` at 100% of positions, and even the real
/// spawn-time mutation regime never once produced a `Muscle` segment across
/// 30 independent trials.
///
/// **Second problem this replaces (ADR-P5-07):** the first fix added a
/// single `Sigmoid` + `Gaussian` + `Sine` basis trio, with the `Gaussian`
/// bump's *one* fixed center tuned to land on the Hox region (gene-index
/// fraction ≈0.1) so `Muscle` became reachable. That single bump was the
/// whole fix's local-activation budget — every other gene *role* (crucially
/// `Effector`, at index fraction ≈0.55) sat far outside the bump's reach and
/// collapsed to whatever the leftover Sigmoid+Sine terms gave, combined with
/// the strongly negative `output_bias` needed to suppress off-peak Hox bits.
/// Measured directly in a real headless run (§11): **363 of 364** sampled
/// non-Producer organisms had zero actuatable effector springs, even though
/// the isolated per-seed measurement (which mutates the *entire* CPPN,
/// relocating the bump over generations) showed 31.2% `Muscle` reachability.
/// The founding population never benefits from that drift — it uses the
/// seed unmutated, where the one bump structurally cannot reach `Effector`.
///
/// **The fix is modular, not another single retuned bump.** Gene *role* is
/// already fully determined by gene *position* under the current fixed
/// `REGULATORY_GENE_ROLES` table (Hox = 0-2, Differentiation = 3-4, Effector
/// = 5-6, Pigment = 7-9) — there's no missing input dimension, only
/// insufficient local-activation *capacity*. So this CPPN gives each region
/// its own independently-weighted `Gaussian` bump, centered at that region's
/// index-fraction midpoint, alongside the existing shared `Sigmoid`
/// (monotonic gradient) and *two* `Sine` bases at different frequencies
/// (coarse + fine periodic/repeated structure, rather than one). Every
/// region's bump can be independently strengthened, weakened, or inverted
/// (a negative weight is a local *repressor*, not just an activator) via its
/// own `RegulatorySeedWeights` field, without starving any other region —
/// this is what makes the fix "modular regulation, one evolvable genome"
/// rather than a minimal patch: tuning `effector_weight` can no longer come
/// at the expense of `hox_weight`, because they're separate connections.
///
/// This scope's four bumps match today's four fixed `RegulatoryGeneRole`
/// variants; a future role (organogenesis, physiology — explicitly listed as
/// future compatibility targets) would need one more region bump added here,
/// the same way this fix added four to the first version's one — not a
/// restructuring, since the pattern ("one independently-weighted local bump
/// per role region") generalizes directly.
///
/// All bases still combine at one `Linear` output node, so
/// `RegulatoryNetwork::generate`'s existing calling convention
/// (`evaluate(&[idx, idx])` for bias, `evaluate(&[i/total, j/total])` for
/// edge weight) is completely unchanged — this is a richer function being
/// queried the same way, not a change to how genes/edges are derived, and
/// nothing here reads `REGULATORY_GENE_ROLES` at runtime (the region centers
/// below are constants derived from that table by hand, not a live lookup) —
/// deliberately, so this stays a plain, cheap, deterministic `Cppn` rather
/// than a construction that depends on the table's exact contents at
/// call-time.
///
/// **Deliberately not tuned toward any specific `SegmentType`.** This
/// function has no `Muscle`-specific or `Fin`-specific logic anywhere — the
/// four region weights and two sine weights are swept per starter species
/// purely for *diversity* (see each call site's own comment for what was
/// empirically observed, not targeted), and the resulting network remains an
/// ordinary, evolvable `Cppn` — mutation's existing `mutate_add_node`/
/// `mutate_add_connection`/per-connection jitter operate on it exactly as
/// they would any other genome, with nothing special-cased for starter
/// organisms (ADR-P3-02).
pub(crate) fn seed_regulatory_cppn(w: RegulatorySeedWeights) -> genetics::Cppn {
    // Sigmoid basis: a smooth monotonic gradient, transitioning at the
    // midpoint of the gene-index range.
    const SIGMOID_INPUT_WEIGHT: f32 = 1.5;
    const SIGMOID_BIAS: f32 = -1.5;

    // Each region gets its own width: Hox must sharply discriminate 3
    // *adjacent* gene indices (0.1 apart) to produce a non-monotonic 3-bit
    // code, so it needs a narrow bump; Differentiation/Effector/Pigment each
    // cover 2-3 indices that should mostly move *together*, so a wider bump
    // (which was tried shared at width 4.0 for all four and measured to
    // collapse Hox discrimination — see §11's ADR-P5-07 entry) suits them
    // better. sum = bias + weight*pos + weight*pos = bias + 2*weight*center
    // at the peak, so bias = -2*weight*center places the peak at `center`.
    const HOX_WIDTH: f32 = 10.0;
    const DIFFERENTIATION_WIDTH: f32 = 6.0;
    const EFFECTOR_WIDTH: f32 = 4.0;
    const PIGMENT_WIDTH: f32 = 4.0;
    const HOX_CENTER: f32 = 0.1; // genes 0-2 of 10, midpoint index 1
    const DIFFERENTIATION_CENTER: f32 = 0.35; // genes 3-4, midpoint 3.5
    const EFFECTOR_CENTER: f32 = 0.55; // genes 5-6, midpoint 5.5
    const PIGMENT_CENTER: f32 = 0.8; // genes 7-9, midpoint 8
    const HOX_BIAS: f32 = -2.0 * HOX_WIDTH * HOX_CENTER;
    const DIFFERENTIATION_BIAS: f32 = -2.0 * DIFFERENTIATION_WIDTH * DIFFERENTIATION_CENTER;
    const EFFECTOR_BIAS: f32 = -2.0 * EFFECTOR_WIDTH * EFFECTOR_CENTER;
    const PIGMENT_BIAS: f32 = -2.0 * PIGMENT_WIDTH * PIGMENT_CENTER;

    // Two periodic bases at different frequencies, for repeated/alternating
    // structure at more than one spatial scale.
    const SINE_COARSE_INPUT_WEIGHT: f32 = 6.0; // ~1.9 cycles across [0, 1]
    const SINE_FINE_INPUT_WEIGHT: f32 = 15.0; // ~4.8 cycles across [0, 1]
    const SINE_BIAS: f32 = 0.0;

    genetics::Cppn {
        nodes: vec![
            // 0, 1: inputs (gene-index fractions).
            genetics::CppnNode {
                activation: brain::ActivationFn::Linear,
                bias: 0.0,
                layer: 0,
            },
            genetics::CppnNode {
                activation: brain::ActivationFn::Linear,
                bias: 0.0,
                layer: 0,
            },
            // 2-8: the seven hidden basis functions. `Cppn::evaluate`
            // collects only `layer == 1` nodes into its returned outputs vec
            // (and `RegulatoryNetwork::generate` reads just the first of
            // those) — these seven must stay off that list (`layer: 0`, the
            // same value used for raw inputs, but functionally just "not a
            // collected output" here; `evaluate`'s node-computation loop
            // itself is index-range-based, not layer-gated, so they are
            // still fully computed) so only node 9's combined value is ever
            // read. Getting this wrong (marking a basis node `layer: 1`) was
            // this milestone's first implementation's own first bug (§11) —
            // caught by directly inspecting `RegulatoryNetwork::generate`'s
            // output, not assumed fixed.
            genetics::CppnNode {
                activation: brain::ActivationFn::Sigmoid,
                bias: SIGMOID_BIAS,
                layer: 0,
            },
            genetics::CppnNode {
                activation: brain::ActivationFn::Gaussian,
                bias: HOX_BIAS,
                layer: 0,
            },
            genetics::CppnNode {
                activation: brain::ActivationFn::Gaussian,
                bias: DIFFERENTIATION_BIAS,
                layer: 0,
            },
            genetics::CppnNode {
                activation: brain::ActivationFn::Gaussian,
                bias: EFFECTOR_BIAS,
                layer: 0,
            },
            genetics::CppnNode {
                activation: brain::ActivationFn::Gaussian,
                bias: PIGMENT_BIAS,
                layer: 0,
            },
            genetics::CppnNode {
                activation: brain::ActivationFn::Sine,
                bias: SINE_BIAS,
                layer: 0,
            },
            genetics::CppnNode {
                activation: brain::ActivationFn::Sine,
                bias: SINE_BIAS,
                layer: 0,
            },
            // 9: output — linear combination of the seven bases. The only
            // `layer: 1` node, so it's the one `.first()` actually reads.
            genetics::CppnNode {
                activation: brain::ActivationFn::Linear,
                bias: w.output_bias,
                layer: 1,
            },
        ],
        connections: vec![
            // Inputs (0, 1) -> each of the 7 hidden bases (2-8).
            genetics::CppnConnection {
                source: 0,
                target: 2,
                weight: SIGMOID_INPUT_WEIGHT,
                enabled: true,
                innovation: 0,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 1,
                target: 2,
                weight: SIGMOID_INPUT_WEIGHT,
                enabled: true,
                innovation: 1,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 0,
                target: 3,
                weight: HOX_WIDTH,
                enabled: true,
                innovation: 2,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 1,
                target: 3,
                weight: HOX_WIDTH,
                enabled: true,
                innovation: 3,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 0,
                target: 4,
                weight: DIFFERENTIATION_WIDTH,
                enabled: true,
                innovation: 4,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 1,
                target: 4,
                weight: DIFFERENTIATION_WIDTH,
                enabled: true,
                innovation: 5,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 0,
                target: 5,
                weight: EFFECTOR_WIDTH,
                enabled: true,
                innovation: 6,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 1,
                target: 5,
                weight: EFFECTOR_WIDTH,
                enabled: true,
                innovation: 7,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 0,
                target: 6,
                weight: PIGMENT_WIDTH,
                enabled: true,
                innovation: 8,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 1,
                target: 6,
                weight: PIGMENT_WIDTH,
                enabled: true,
                innovation: 9,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 0,
                target: 7,
                weight: SINE_COARSE_INPUT_WEIGHT,
                enabled: true,
                innovation: 10,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 1,
                target: 7,
                weight: SINE_COARSE_INPUT_WEIGHT,
                enabled: true,
                innovation: 11,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 0,
                target: 8,
                weight: SINE_FINE_INPUT_WEIGHT,
                enabled: true,
                innovation: 12,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 1,
                target: 8,
                weight: SINE_FINE_INPUT_WEIGHT,
                enabled: true,
                innovation: 13,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            // Hidden bases (2-8) -> output (9), one per-seed evolvable weight
            // each (sigmoid stays fixed at 1.0 — it has no per-region
            // identity to tune independently).
            genetics::CppnConnection {
                source: 2,
                target: 9,
                weight: 1.0,
                enabled: true,
                innovation: 14,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 3,
                target: 9,
                weight: w.hox_weight,
                enabled: true,
                innovation: 15,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 4,
                target: 9,
                weight: w.differentiation_weight,
                enabled: true,
                innovation: 16,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 5,
                target: 9,
                weight: w.effector_weight,
                enabled: true,
                innovation: 17,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 6,
                target: 9,
                weight: w.pigment_weight,
                enabled: true,
                innovation: 18,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 7,
                target: 9,
                weight: w.sine_coarse_weight,
                enabled: true,
                innovation: 19,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 8,
                target: 9,
                weight: w.sine_fine_weight,
                enabled: true,
                innovation: 20,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
        ],
    }
}

/// The hand-built brain-wiring CPPN previously baked into `new_hox_driven`
/// (retired, Phase 3 M4) — unrelated to Hox/body-plan decoding, so carried
/// over unchanged as every seed genome's starting neural substrate.
pub(crate) fn seed_brain_cppn() -> genetics::Cppn {
    genetics::Cppn {
        nodes: vec![
            genetics::CppnNode {
                activation: brain::ActivationFn::Linear,
                bias: 0.0,
                layer: 0,
            }, // Input: Source Node Coord
            genetics::CppnNode {
                activation: brain::ActivationFn::Linear,
                bias: 0.0,
                layer: 0,
            }, // Input: Target Node Coord
            genetics::CppnNode {
                activation: brain::ActivationFn::Tanh,
                bias: 0.0,
                layer: 1,
            }, // Output: Connection Weight
            genetics::CppnNode {
                activation: brain::ActivationFn::Tanh,
                bias: 0.0,
                layer: 1,
            }, // Output: Bias
            genetics::CppnNode {
                activation: brain::ActivationFn::Linear,
                bias: 0.0,
                layer: 1,
            }, // Output: Time Constant
        ],
        connections: vec![
            genetics::CppnConnection {
                source: 0,
                target: 2,
                weight: 2.0,
                enabled: true,
                innovation: 1,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 1,
                target: 2,
                weight: -1.0,
                enabled: true,
                innovation: 2,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 1,
                target: 3,
                weight: 1.0,
                enabled: true,
                innovation: 3,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 1,
                target: 4,
                weight: 0.5,
                enabled: true,
                innovation: 4,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
        ],
    }
}

pub(crate) fn seed_ecosystem(
    world: &mut bevy_ecs::world::World,
    lineage_tracker: &mut evolution::LineageTracker,
    species_registry: &mut evolution::SpeciesRegistry,
    tracker: &mut genetics::GlobalInnovationTracker,
    rng: &mut impl rand::Rng,
) {
    // 1. Define Prototypes ("Seed Genomes" — Phase 3 M4, replacing the
    // retired `new_hox_driven`/`HoxSequence` template mechanism).
    //
    // Each seed is an ordinary hand-authored `Genome` — no special-cased
    // morphology generation (ADR-P3-02). Its body plan, branching, and
    // pigmentation all emerge from the same `develop_at_position` decode
    // pipeline every evolved organism goes through; `seed_regulatory_cppn`
    // just gives each species archetype a different starting point on that
    // decode (found by sweeping bias/weight and reading off the resulting
    // segment-type sequence, not hand-picked to match any specific shape).
    //
    // Colors are **not** set here — pigmentation is emergent (see
    // `RegulatoryGeneRole::Pigment`'s doc comment), so starter organisms no
    // longer necessarily render in their diet's canonical
    // `Diet::standard_color()`. This is an intentional consequence of
    // retiring genome-stored color, not an oversight.
    let brain_template = seed_brain_cppn();

    // Phase 5, SX-2a (ADR-P5-07): swept for *diversity* across the modular
    // region-bump basis — see `seed_regulatory_cppn`'s doc comment — not
    // hand-picked to hit any specific `SegmentType`. Measured diversity
    // across all six, including effector-activation rate, is recorded in
    // `PHASE5_SX_ROADMAP.md` §11.
    let worm_genome = genetics::Genome::seed(
        genetics::GenomeId(1),
        common::EntityId(0),
        brain_template.clone(),
        genetics::Cppn::new(),
        // Empirically found by random search over `RegulatorySeedWeights`
        // (20,000 draws), selecting for effector activity + Hox-type
        // diversity — not hand-picked to hit Muscle specifically. Unmutated
        // decode: [Germinal, Ganglion, Muscle, Muscle, Muscle, Muscle,
        // Ganglion, Ganglion, Germinal, Germinal], effector active 10/10.
        seed_regulatory_cppn(RegulatorySeedWeights {
            output_bias: -4.45,
            hox_weight: 8.97,
            differentiation_weight: 7.07,
            effector_weight: 3.12,
            pigment_weight: 1.22,
            sine_coarse_weight: 2.15,
            sine_fine_weight: 1.76,
        }),
    );

    let fish_genome = genetics::Genome::seed(
        genetics::GenomeId(2),
        common::EntityId(0),
        brain_template.clone(),
        genetics::Cppn::new(),
        // Phase 9, Goal 2 root-cause audit (re-tuned; see
        // `phase9_movement_root_cause_diagnostic` below): the previous
        // weights here decoded [Tail, Torso, Torso, Head, Torso, Torso,
        // Torso, Tail, Tail, Tail] — the "effector active 10/10" this
        // comment used to claim only ever measured raw
        // `actuation_amplitude != 0` at every position (every position
        // always produces *some* amplitude value, per
        // `develop_at_position`), never whether that position actually
        // decoded as `SegmentType::Muscle` (the only type that becomes an
        // `Elastic`, actuatable spring — see `develop.rs`/
        // `compile_segment`). The real bug this decode had: `Tail` at
        // position 0 (i.e. `growth_system`'s first real segment, position
        // 1, was still `Torso`, harmless) but every position had
        // `apoptosis = true` — DEF-002's germ-line-protection pruning fired
        // everywhere, so a real fish organism grew *zero* body past its
        // head, regardless of Hox/Muscle content. Re-tuned unmutated
        // decode (positions 1-9): [Head, Head, Muscle, Head, Head, Head,
        // Head, Head, Head], none apoptotic, 1 real actuatable Muscle
        // segment; ~65% of individuals retain >=1 actuatable effector after
        // `spawn_pop`'s 10-round mutation pass at the corrected 0.1 rate.
        seed_regulatory_cppn(RegulatorySeedWeights {
            output_bias: -6.3154593,
            hox_weight: 7.676084,
            differentiation_weight: 3.2809398,
            effector_weight: 6.233916,
            pigment_weight: 1.3872341,
            sine_coarse_weight: 0.5254265,
            sine_fine_weight: 2.490907,
        }),
    );

    let branchy_genome = genetics::Genome::seed(
        genetics::GenomeId(3),
        common::EntityId(0),
        brain_template.clone(),
        genetics::Cppn::new(),
        // Phase 9, Goal 2 root-cause audit (re-tuned; see
        // `phase9_movement_root_cause_diagnostic` below): the previous
        // weights here had the exact same DEF-002 apoptosis defect as
        // `fish_genome` above — every position except the two `Germinal`
        // ends had `apoptosis = true`, so a real branchy organism also grew
        // zero body past its head despite the Hox table showing 2 Muscle
        // positions. Re-tuned unmutated decode (positions 1-9): [Ganglion,
        // Germinal, Germinal, Germinal, Germinal, Germinal, Muscle, Muscle,
        // Muscle], none apoptotic, 3 real actuatable Muscle segments; ~51%
        // of individuals retain >=1 actuatable effector after `spawn_pop`'s
        // 10-round mutation pass at the corrected 0.1 rate.
        seed_regulatory_cppn(RegulatorySeedWeights {
            output_bias: -4.885546,
            hox_weight: 11.249819,
            differentiation_weight: 2.586886,
            effector_weight: 4.5433483,
            pigment_weight: 2.1518261,
            sine_coarse_weight: 2.6428568,
            sine_fine_weight: 1.3519208,
        }),
    );

    let omnivore_genome = genetics::Genome::seed(
        genetics::GenomeId(4),
        common::EntityId(0),
        brain_template.clone(),
        genetics::Cppn::new(),
        // Unmutated decode: [Muscle, Muscle, Germinal, Germinal, Germinal,
        // Ganglion, Muscle, Muscle, Muscle, Muscle], effector active 8/10.
        seed_regulatory_cppn(RegulatorySeedWeights {
            output_bias: -4.13,
            hox_weight: 8.84,
            differentiation_weight: 2.10,
            effector_weight: 2.96,
            pigment_weight: 2.22,
            sine_coarse_weight: 2.22,
            sine_fine_weight: 2.10,
        }),
    );

    let decomposer_genome = genetics::Genome::seed(
        genetics::GenomeId(5),
        common::EntityId(0),
        brain_template.clone(),
        genetics::Cppn::new(),
        // Unmutated decode: [Tail, Muscle, Muscle, Muscle, Muscle, Muscle,
        // Muscle, Muscle, Tail, Germinal], effector active 10/10.
        seed_regulatory_cppn(RegulatorySeedWeights {
            output_bias: -3.05,
            hox_weight: 6.90,
            differentiation_weight: 3.90,
            effector_weight: 0.69,
            pigment_weight: 0.40,
            sine_coarse_weight: 0.54,
            sine_fine_weight: 1.09,
        }),
    );

    let producer_genome = genetics::Genome::seed(
        genetics::GenomeId(6),
        common::EntityId(0),
        brain_template,
        genetics::Cppn::new(),
        // Producers stay a deliberately short, low-complexity seed (real
        // plants don't need a rich body plan or effector activity) — no
        // seed here is hardcoded to a specific segment outcome.
        seed_regulatory_cppn(RegulatorySeedWeights {
            output_bias: -3.0,
            hox_weight: 0.0,
            differentiation_weight: 0.0,
            effector_weight: 0.0,
            pigment_weight: 1.0,
            sine_coarse_weight: 0.0,
            sine_fine_weight: 0.0,
        }),
    );

    // 2. Helper to spawn a population
    let mut spawn_pop = |genome: &genetics::Genome, diet: ecology::Diet, count: usize| {
        let lineage_id = lineage_tracker.new_lineage_id();
        for _ in 0..count {
            let px = rng.gen_range(-1000.0..1000.0);
            let py = rng.gen_range(-1000.0..1000.0);

            // Give each individual a unique randomized brain if they are not
            // producers. `mutation_rate` here must stay in line with
            // `reproduction`'s own per-birth mutation calls (0.1-0.2 — see
            // `crates/reproduction/src/lib.rs`'s `child_genome.mutate(...)`
            // call sites), not a guaranteed full-strength pass: measured
            // directly (Phase 9 Goal 2 root-cause audit — see
            // `phase9_movement_root_cause_diagnostic` below), the previous
            // `mutate(1.0, ...)` x10 — an outer gate of 1.0 means every one
            // of the 10 rounds mutates at full strength, not 10 *chances* at
            // a milder mutation — collapsed the seed regulatory CPPNs'
            // actuatable-Muscle-segment rate from 100% down to ~11-23% for
            // 3 of 5 starter presets (worm/omnivore/decomposer), which is
            // this session's own real headless `PHYLON_MOTION_DIAGNOSTIC=1`
            // observation of 0 actuatable effectors across every sampled
            // organism, compounded further by per-generation reproduction
            // mutating the same already-degraded lineages again. At
            // `mutation_rate = 0.1` (matching reproduction's own asexual
            // rate), the same 10-round loop still gives every individual a
            // genuinely unique brain/body-plan while preserving a healthy
            // majority (~60-80%) actuatable-effector rate.
            let mut ind_genome = genome.clone();
            if diet != ecology::Diet::Producer {
                for _ in 0..10 {
                    ind_genome.mutate(0.1, rng, tracker);
                }
            }

            let species_id = species_registry.classify(&ind_genome);

            let e = organisms::spawn_organism(
                world,
                &ind_genome,
                common::Vec3::new(px, py, 0.0),
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
            position: common::Vec3::new(px, py, 0.0),
            energy_value: 50.0,
        });
    }

    // 6. Spawn Initial Food
    for _ in 0..300 {
        let px = rng.gen_range(-1000.0..1000.0);
        let py = rng.gen_range(-1000.0..1000.0);
        world.spawn(ecology::FoodPellet {
            position: common::Vec3::new(px, py, 0.0),
            energy_value: 50.0,
        });
    }
}

#[cfg(test)]
mod starter_species_locomotion_viability {
    //! Regression coverage for Phase 9 Goal 2's root-cause finding: every
    //! non-Producer starter species must actually be capable of
    //! muscle-driven locomotion, both unmutated and after `spawn_pop`'s
    //! founder-diversity mutation pass. Two independent, measured defects
    //! were found and fixed here (not guessed — see each call site's own
    //! comment in `seed_ecosystem` for the full measurement):
    //!
    //! 1. `spawn_pop` mutated every founder genome 10 times at
    //!    `mutation_rate = 1.0` (a guaranteed full-strength pass each
    //!    round) before ever spawning it — 100x more aggressive than
    //!    `reproduction`'s own per-birth convention (0.1-0.2, one call).
    //!    Measured effect: collapsed the actuatable-effector rate from
    //!    100% to single digits for otherwise-healthy presets. Fixed by
    //!    matching reproduction's own 0.1 rate.
    //! 2. `fish_genome`/`branchy_genome`'s regulatory seed weights caused
    //!    DEF-002's germ-line-protection apoptosis check to fire on nearly
    //!    every body position, pruning the entire body except the head —
    //!    these two starter species grew no muscle-bearing body at all,
    //!    independent of any mutation. Fixed by re-tuning their weights
    //!    (search anchored near the originals, gated on: apoptosis-survives
    //!    for >=4 positions AND >=1 real actuatable `Muscle` segment).
    use super::*;
    use rand::SeedableRng;

    fn preset(name: &str) -> RegulatorySeedWeights {
        match name {
            "worm" => RegulatorySeedWeights {
                output_bias: -4.45,
                hox_weight: 8.97,
                differentiation_weight: 7.07,
                effector_weight: 3.12,
                pigment_weight: 1.22,
                sine_coarse_weight: 2.15,
                sine_fine_weight: 1.76,
            },
            "fish" => RegulatorySeedWeights {
                output_bias: -6.3154593,
                hox_weight: 7.676084,
                differentiation_weight: 3.2809398,
                effector_weight: 6.233916,
                pigment_weight: 1.3872341,
                sine_coarse_weight: 0.5254265,
                sine_fine_weight: 2.490907,
            },
            "branchy" => RegulatorySeedWeights {
                output_bias: -4.885546,
                hox_weight: 11.249819,
                differentiation_weight: 2.586886,
                effector_weight: 4.5433483,
                pigment_weight: 2.1518261,
                sine_coarse_weight: 2.6428568,
                sine_fine_weight: 1.3519208,
            },
            "omnivore" => RegulatorySeedWeights {
                output_bias: -4.13,
                hox_weight: 8.84,
                differentiation_weight: 2.10,
                effector_weight: 2.96,
                pigment_weight: 2.22,
                sine_coarse_weight: 2.22,
                sine_fine_weight: 2.10,
            },
            "decomposer" => RegulatorySeedWeights {
                output_bias: -3.05,
                hox_weight: 6.90,
                differentiation_weight: 3.90,
                effector_weight: 0.69,
                pigment_weight: 0.40,
                sine_coarse_weight: 0.54,
                sine_fine_weight: 1.09,
            },
            _ => unreachable!(),
        }
    }

    /// Measures the fraction of organisms with >=1 actuatable (`Muscle`,
    /// nonzero amplitude) segment, for a given preset / mutation-round
    /// count / `total_segments`, mirroring `growth_system`'s real halting
    /// rule (position 0 is the pre-existing head; growth stops at the
    /// first decoded `Tail`; apoptotic positions are skipped).
    fn measure(
        preset_name: &str,
        mutate_rounds: usize,
        total_segments: usize,
        trials: u64,
    ) -> (usize, usize) {
        measure_with_rate(preset_name, mutate_rounds, 1.0, total_segments, trials)
    }

    fn measure_with_rate(
        preset_name: &str,
        mutate_rounds: usize,
        mutation_rate: f32,
        total_segments: usize,
        trials: u64,
    ) -> (usize, usize) {
        let mut tracker = genetics::GlobalInnovationTracker::default();
        let seed = seed_regulatory_cppn(preset(preset_name));

        let mut with_muscle = 0usize;
        let mut with_effector = 0usize;

        for trial in 0..trials {
            let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(trial);
            let mut genome = genetics::Genome::seed(
                genetics::GenomeId(trial),
                common::EntityId(0),
                genetics::Cppn::new(),
                genetics::Cppn::new(),
                seed.clone(),
            );
            for _ in 0..mutate_rounds {
                genome.mutate(mutation_rate, &mut rng, &mut tracker);
            }

            let mut any_effector = false;
            let mut any_muscle = false;
            for pos in 1..total_segments {
                let out = genetics::develop_at_position(
                    &genome.expressed_regulatory_cppn(),
                    pos,
                    total_segments,
                );
                if out.apoptosis {
                    continue;
                }
                if out.segment_type == genetics::SegmentType::Muscle {
                    any_muscle = true;
                    if out.actuation_amplitude.abs() > 0.01 {
                        any_effector = true;
                    }
                }
                if out.segment_type == genetics::SegmentType::Tail {
                    break;
                }
            }
            if any_muscle {
                with_muscle += 1;
            }
            if any_effector {
                with_effector += 1;
            }
        }

        (with_muscle, with_effector)
    }

    #[test]
    fn every_non_producer_preset_has_a_reachable_actuatable_effector_unmutated() {
        for preset_name in ["worm", "fish", "branchy", "omnivore", "decomposer"] {
            let (muscle, effector) = measure(preset_name, 0, 10, 1);
            assert_eq!(
                (muscle, effector),
                (1, 1),
                "preset {preset_name} must decode >=1 reachable, actuatable Muscle segment \
                 unmutated (DEF-002 apoptosis must not prune the entire body)"
            );
        }
    }

    #[test]
    fn founder_population_retains_a_healthy_effector_majority_after_spawn_pop_mutation() {
        // Mirrors `spawn_pop`'s exact mutation dosage (10 rounds at
        // `mutation_rate = 0.1`) — regression coverage for the measured
        // finding that `mutation_rate = 1.0` (the previous value) collapsed
        // this to single digits.
        let trials = 300u64;
        for preset_name in ["worm", "fish", "branchy", "omnivore", "decomposer"] {
            let (_, effector) =
                measure_with_rate(preset_name, 10, 0.1, organisms::MAX_SEGMENTS, trials);
            let rate = effector as f64 / trials as f64;
            assert!(
                rate > 0.3,
                "preset {preset_name}: post-mutation actuatable-effector rate {rate:.2} \
                 is too low (expected >0.3) at spawn_pop's real mutation dosage"
            );
        }
    }
}
