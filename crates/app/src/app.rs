//! # Phylon Application — Composition Root
//!
//! This module owns [`PhylonApp`], the top-level struct that ties together
//! the ECS simulation world, the `wgpu` GPU context, and the `egui`
//! immediate-mode UI into a single running program.
//!
//! ## Purpose
//!
//! `app` is the **composition root** of the workspace: the one crate allowed
//! to depend on every other crate (ECS world model, GPU compute pipelines,
//! rendering, UI, storage, and domain crates such as `genetics`, `ecology`,
//! `brain`). All other crates are decoupled from each other by the
//! dependency rules in `docs/02_crate_dependency_graph.md`; this crate is
//! where those independently-developed pieces are wired together into one
//! executable.
//!
//! ## Architecture
//!
//! `PhylonApp` bridges three otherwise-independent domains:
//!
//! - **ECS world state** (`world::World`, wrapping a `bevy_ecs::World`) —
//!   entities (organisms, food, hazards, ...) and the resources that drive
//!   per-tick simulation logic (genetics, metabolism, ecology, evolution).
//! - **GPU compute and rendering** (`GpuContext`, the physics/diffusion/brain
//!   compute pipelines, and the `rendering` crate's renderers) — offloads
//!   the parts of the simulation and its visualization that are
//!   embarrassingly parallel (particle-node physics integration, CTRNN —
//!   continuous-time recurrent neural network, the organism "brain" model —
//!   forward passes, reaction-diffusion fields) to the GPU.
//! - **egui UI** (`egui_winit::State`, `egui_wgpu::Renderer`, `ui::WorkbenchState`)
//!   — the workbench panels, menus, and overlays drawn on top of the 3D
//!   scene each frame.
//!
//! ## Data flow (per rendered frame)
//!
//! 1. `winit` delivers input/window events to `PhylonApp` (mouse, keyboard,
//!    resize, `RedrawRequested`).
//! 2. On `RedrawRequested`, elapsed wall-clock time (via `last_frame_instant`)
//!    is accumulated into `accumulated_time` and drained in fixed-size steps
//!    of `common::TickRate::dt()`, each step calling
//!    [`PhylonApp::update_simulation`] once — see `crate::simulation`'s
//!    module doc for the exact per-tick system order. `max_ticks_per_frame` bounds how
//!    many ticks a single slow frame can catch up by, so a debugger pause or
//!    a stalled GPU readback cannot cause an unbounded catch-up burst.
//! 3. GPU compute results dispatched on the *previous* tick (`pending_physics`,
//!    `pending_brain`) are resolved at the start of the *current* tick,
//!    letting GPU work for tick N overlap with tick N's CPU-side systems
//!    instead of stalling immediately after submission.
//! 4. After ticking, render data is gathered from the ECS world
//!    (`render::world_instances::gather_world_render_instances`) and
//!    submitted to the GPU renderers, followed by the egui pass drawing the
//!    workbench UI on top.
//! 5. The frame is presented (`output.present()`), after first servicing any
//!    pending screenshot/chart-export request, since the live swapchain
//!    texture is only available at that point.
//!
//! ## Lifecycle
//!
//! Window and GPU surface creation are deferred to the `winit` `Resumed`
//! event (required on some platforms, e.g. Android, and harmless elsewhere)
//! rather than done in [`PhylonApp::new`]. `new` only builds the ECS world,
//! registers resources, and seeds the initial organism population; `window`,
//! `gpu`, `egui_state`, and the compute/render pipelines all start as `None`
//! and are populated once a window exists. Shutdown (`MenuAction::Quit` or
//! the window's `CloseRequested` event) flushes preferences to disk via
//! [`PhylonApp::save_preferences`] before the process exits.
//!
//! ## Determinism
//!
//! `PhylonApp` owns the single seeded `common::SimRng` resource in `world`.
//! Every stochastic system (mutation, crossover, spawn placement, mate
//! selection, ...) must draw from this one RNG rather than constructing its
//! own, so that a given `PhylonConfig` (including its `rng_seed`) always
//! produces the same simulation trajectory. GPU compute does not
//! participate in this contract — the compute shaders are purely numerical
//! (physics integration, diffusion, CTRNN evaluation) and take no random
//! inputs.
//!
//! ## Thread safety
//!
//! `PhylonApp` and the ECS world it owns are single-threaded from the
//! simulation's point of view: `update_simulation` runs its systems on the
//! calling thread (bevy_ecs schedules are not used here — systems are called
//! directly in a fixed order). The one asynchronous boundary is
//! `task_tx`/`task_rx`, an `mpsc` channel used to hand background save/load
//! work off to a spawned thread without blocking the event loop; GPU
//! command submission itself is also inherently asynchronous (the driver
//! schedules submitted work), but `PhylonApp` treats `queue.submit` as
//! fire-and-forget and does not otherwise share GPU resources across
//! threads.
//!
//! ## Extension points
//!
//! - New per-tick systems: add them to
//!   [`PhylonApp::update_simulation`], not here.
//! - New GPU compute pipelines: follow the `Option<...ComputePipeline>`
//!   pattern already used by `physics_compute`/`diffusion_compute`/
//!   `splat_compute`/`brain_compute` — constructed once GPU init succeeds,
//!   `None` before then and on headless/failed-init runs.
//! - New persisted preferences: add the field to `preferences::Preferences`
//!   and mirror it into/out of `PhylonApp::ui` in `new`/`save_preferences`.
//!
//! ## Related modules
//!
//! - [`crate::simulation`] — the per-tick system order.
//! - [`crate::gpu_init`] — GPU device/surface bring-up, called once a window exists.
//! - [`crate::species_seed`] — initial organism population construction.
//! - `crate::preferences` — cross-session UI preference persistence.
//! - `crate::render` — per-frame render-instance gathering from ECS state.

use std::sync::Arc;

use winit::window::Window;

use config::PhylonConfig;

use crate::gpu_init::GpuContext;
use crate::species_seed::seed_ecosystem;

/// The Phylon application: owns the ECS world, the GPU context, and the UI
/// state, and drives the per-frame update/render loop.
///
/// ## Purpose
///
/// `PhylonApp` is the composition root's central struct — see this module's
/// top-level doc comment for the full architecture, data-flow, lifecycle,
/// determinism, and thread-safety discussion. In short: it exists to bridge
/// discrete simulation logic (genetics, metabolism, ecology — evaluated on
/// the ECS world) with continuous presentation (GPU rendering, egui UI)
/// without either domain needing to know about the other's internals.
///
/// ## Architecture
///
/// Fields fall into a few groups:
/// - Simulation state: `sim_config`, `world` (the ECS world and its
///   resources), `total_sim_time`, `accumulated_time`, `simulation_speed`,
///   `max_ticks_per_frame`.
/// - GPU context and compute/render pipelines: `gpu`, `physics_compute`,
///   `diffusion_compute`, `splat_compute`, `brain_compute`, `debug_renderer`,
///   `organism_renderer`, `field_renderer`. All `Option`-wrapped because
///   they are only constructed after a window exists (see "Lifecycle"
///   above) and may remain `None` if GPU init fails.
/// - Windowing/UI: `window`, `egui_state`, `egui_renderer`, `ui`, `app_state`.
/// - Cross-tick bookkeeping: `pending_physics`, `pending_brain` (deferred
///   GPU readbacks), `sim_scratch`, `render_scratch` (reused scratch
///   buffers, see their own doc comments), `replay_log`,
///   `experiment_manifest`.
/// - Deferred one-shot actions: `pending_screenshot`, `pending_chart_export`,
///   `recording`, `task_tx`/`task_rx` (background save/load results).
///
/// Ownership of GPU buffers and ECS world data never overlaps in a way that
/// would violate Rust's borrow rules: GPU readbacks are resolved into plain
/// CPU-side data (`PendingPhysicsReadback`, `PendingBrainReadback`) before
/// being scattered back into ECS components, rather than holding a live GPU
/// mapping alongside a `World` borrow.
pub(crate) struct PhylonApp {
    /// Deserialised application/simulation config
    pub(crate) sim_config: PhylonConfig,

    /// Cross-session UI preferences — high-contrast mode, UI scale, whether
    /// onboarding hints have ever been shown, recent-items history, and
    /// panel layout. See `crate::preferences`'s module doc comment for why
    /// this is kept separate from `sim_config`: it is user/workstation
    /// state, not simulation-affecting configuration, and is persisted and
    /// restored independently of any particular `PhylonConfig`.
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

    /// Mesh-based capsule organism renderer. Organisms are drawn as
    /// instanced capsule meshes (a cylinder-with-hemisphere-caps primitive)
    /// rather than via a signed-distance-field (SDF) raymarch, which is
    /// cheaper to shade at the resolution and organism counts this app
    /// targets and composes more easily with the rest of the mesh-based
    /// scene (debug renderer, field renderer).
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

    /// Wall-clock time of the previous `render()` call, used only to advance
    /// `ui::WorkbenchState::frame_animation` — deliberately a separate field
    /// from `last_frame_instant` rather than reusing it, since that one is
    /// consumed and overwritten by `advance_simulation_for_frame` (which
    /// runs after camera tracking), and camera-transition smoothness
    /// shouldn't become coupled to simulation-tick bookkeeping (e.g. a
    /// paused simulation should still animate smooth camera transitions).
    pub(crate) last_camera_animation_instant: std::time::Instant,

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

    /// Set by `MenuAction::ExportChartPng` — `(x, y, width, height)` in
    /// physical pixels. Same deferred-to-next-`render()` timing as
    /// `pending_screenshot`, just cropped to one Metrics chart's rect
    /// instead of the whole window.
    pub(crate) pending_chart_export: Option<(u32, u32, u32, u32)>,

    /// `Some` while a recording is in progress — accumulates captured frames
    /// until `MenuAction::ToggleRecording` stops it and encodes them to GIF.
    pub(crate) recording: Option<crate::capture::RecordingState>,

    /// This run's experiment identity (id, description, RNG seed) — built
    /// from `config::ResearchConfig::experiment_id`/`SimulationConfig::rng_seed`
    /// and persisted to `data/experiments/<id>/manifest.ron` at startup so
    /// that later reproducing or comparing a run doesn't depend on
    /// out-of-band notes about which config/seed produced it.
    pub(crate) experiment_manifest: research::ExperimentManifest,

    /// Every safe external intervention (see `storage::replay::ReplayAction`)
    /// applied this run, in tick order — always recording (cheap; these
    /// events are rare), so a `.phylon-replay` bundle is available to save
    /// at any point via `MenuAction::SaveState`'s replay counterpart.
    pub(crate) replay_log: storage::replay::ReplayLog,

    /// Cross-frame scratch storage for `gather_world_render_instances`'s
    /// intermediate lookup tables — see
    /// `render::world_instances::RenderInstanceScratch`'s doc comment for why
    /// this is kept as a reused allocation rather than built fresh each
    /// frame.
    pub(crate) render_scratch: crate::render::world_instances::RenderInstanceScratch,

    /// Cross-tick scratch storage for `update_simulation`'s GPU node/spring
    /// buffer gathering — reused the same way `render_scratch` is (see that
    /// field's doc comment); `.clear()` at the top of each tick keeps the
    /// backing allocation instead of reallocating from empty every tick,
    /// which otherwise happens up to `max_ticks_per_frame` times per
    /// rendered frame.
    pub(crate) sim_scratch: crate::simulation::SimTickScratch,

    /// Whether the hierarchical per-tick biology profiler (P9.1b) is active
    /// this run — decided once at startup, so every instrumented call site
    /// in `update_simulation` pays only a `bool` read when off. See
    /// `crate::biology_profiler`'s module doc comment.
    pub(crate) biology_profiler_config: crate::biology_profiler::BiologyProfilerConfig,

    /// This tick's shared population-count context for the biology
    /// profiler's "entities processed" figures, recomputed once per tick
    /// only when the profiler is enabled — see
    /// `simulation::update_simulation`'s own doc comment at its
    /// computation site for why this is one shared count, not a
    /// category-specific filtered query.
    pub(crate) biology_profiler_population: u64,

    /// Cross-tick accumulated timings for the biology profiler — see
    /// `crate::biology_profiler`'s module doc comment for why this is a
    /// plain field rather than an ECS resource.
    pub(crate) biology_profiler_state: crate::biology_profiler::BiologyProfilerState,
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
        // `EcologyConfig::max_organisms` must track `target_organism_count`
        // rather than use its own default: `seed_ecosystem` spawns founders
        // directly, bypassing `reproduction_system`'s population-cap check,
        // so if the cap were lower than the founder population, every
        // asexual and sexual reproduction attempt would be rejected from
        // tick 1 onward for the lifetime of the run — reproduction would be
        // permanently, silently disabled rather than merely throttled.
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
        // Registered the same way as the two native bevy_ecs events above —
        // see `simulation.rs`'s per-tick `Events::update()` calls, which
        // must be extended for every new `Events<T>` resource so its buffer
        // is drained on schedule instead of growing unbounded.
        world
            .ecs
            .insert_resource(bevy_ecs::event::Events::<events::PhylonEvent>::default());
        world.ecs.insert_resource(events::TimedEffects::default());
        // Reads `PHYLON_MOTION_DIAGNOSTIC` once at startup — see
        // `motion_diagnostic::MotionDiagnosticConfig`'s doc comment for why
        // this isn't re-checked per tick.
        world
            .ecs
            .insert_resource(crate::motion_diagnostic::MotionDiagnosticConfig::from_env());
        world
            .ecs
            .insert_resource(crate::motion_diagnostic::MotionDiagnosticState::default());
        // Reads `PHYLON_BEHAVIOR_VALIDATION` once at startup — see
        // `behavior_validation::BehaviorValidationConfig`'s doc comment for
        // why this isn't re-checked per tick.
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
        // Restore the persisted panel layout in place of
        // `WorkbenchState::default()`'s own hardcoded starting tree, then
        // rebuild `dock_tree` from it — `rebuild_tree_from_modes` is the
        // sole authoritative tree builder (see `layout.rs`'s own doc
        // comment), so restoring layout is exactly "call it again with the
        // restored inputs," not a second, parallel tree-construction path.
        ui.panel_modes = preferences.panel_modes.clone();
        ui.layout_shares = preferences.layout_shares.clone();
        ui::layout::rebuild_tree_from_modes(&mut ui.dock_tree, &ui.panel_modes, &ui.layout_shares);
        // Restore saved workspaces and which one was last active — purely
        // metadata layered on top of the shape already restored above, see
        // `ui::workspace`'s module doc comment.
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
            last_camera_animation_instant: std::time::Instant::now(),
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
            biology_profiler_config: crate::biology_profiler::BiologyProfilerConfig::from_env(),
            biology_profiler_population: 0,
            biology_profiler_state: Default::default(),
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
        // Recent-items history persists across restarts the same way
        // high_contrast/ui_scale do.
        self.preferences.recent_items = self.ui.recent_items.clone();
        // Panel layout persists the same way. `layout_shares` is already
        // kept current every frame by `ui::render`'s `extract_shares` (reads
        // the live tree's actual split ratios), so this is just copying its
        // current value, not computing anything.
        self.preferences.panel_modes = self.ui.panel_modes.clone();
        self.preferences.layout_shares = self.ui.layout_shares.clone();
        // Saved workspaces and active-workspace identity persist the same
        // way.
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

    /// Converts a physical-pixel screen coordinate to a world-space ray and
    /// finds the nearest entity it hits, by true ray-vs-capsule intersection
    /// against the exact radius the renderer draws each entity at. This is
    /// depth-correct and tilt-correct: it accounts for the camera's actual
    /// orientation and each entity's true on-screen size, unlike a flatter
    /// technique that unprojects to the `Z = 0` plane and finds the 2D
    /// nearest point within a fixed fudge radius (which misidentifies the
    /// picked entity whenever the camera is tilted or entities differ in
    /// apparent size). "Nearest" here means nearest *along the ray* (smallest
    /// hit `t`) — the frontmost entity under the cursor, not merely the one
    /// whose point happens to be closest to the Z=0 unprojection.
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
