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

use winit::window::Window;

use config::PhylonConfig;

use crate::gpu_init::GpuContext;
use crate::species_seed::seed_ecosystem;

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

    /// Phase 9, P9.4: wall-clock time of the previous `render()` call, used
    /// only to advance `ui::WorkbenchState::frame_animation` — deliberately
    /// a separate field from `last_frame_instant` rather than reusing it,
    /// since that one is consumed and overwritten by
    /// `advance_simulation_for_frame` (which runs after camera tracking),
    /// and camera-transition smoothness shouldn't become coupled to
    /// simulation-tick bookkeeping.
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
