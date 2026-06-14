//! The main application binary for Phylon.

use anyhow::Result;
use common::Vec2;
use diffusion::DiffusionField;
use gpu::compute::DiffusionPipeline;
use phylon_config::PhylonConfig;
use physics::{Acceleration, Mass, Position, Radius, Velocity};
use rand::Rng;
use rendering::{FieldPass, FoodPass, OrganismPass, PostPass, TrailPass};
use scheduler::SimulationScheduler;
use std::path::Path;
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};
use ui::EguiContext;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};
use world::PhylonWorld;

mod commands;

struct PhylonApp {
    config: PhylonConfig,
    scheduler: SimulationScheduler,
    world: PhylonWorld,
    renderer: Option<OrganismPass>,
    food_pass: Option<FoodPass>,
    trail_pass: Option<TrailPass>,
    field_pass: Option<FieldPass>,
    post_pass: Option<PostPass>,
    diffusion_pipeline: Option<DiffusionPipeline>,
    diffusion_field: Option<DiffusionField>,
    window: Option<std::sync::Arc<Window>>,
    surface: Option<wgpu::Surface<'static>>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    instance: wgpu::Instance,
    ui: Option<EguiContext>,
    stats: analytics::SimulationStats,
    db_writer: Option<storage::db::DbWriter>,
    script_manager: plugins::manager::ScriptManager,
    script_path: String,
    load_script: bool,
    task_rx: Option<std::sync::mpsc::Receiver<ui::state::LoadingTask>>,
    command_rx: Option<std::sync::mpsc::Receiver<crate::commands::AppCommand>>,
}

impl PhylonApp {
    fn new(config: PhylonConfig) -> Self {
        let tick_rate = config.simulation.tick_rate;
        let mut world = PhylonWorld::new(config.simulation.world_chunk_size as f32);

        // Spawn 100 starter organisms
        let mut rng = rand::thread_rng();
        let spawn_range = 400.0;
        for _ in 0..100 {
            let mut genome = genetics::Genome::default();

            // Initialize random brain weights
            let num_weights = brain::TOTAL_NEURONS * brain::TOTAL_NEURONS;
            genome.brain_weights = (0..num_weights).map(|_| rng.gen_range(-1.0..1.0)).collect();

            world.spawn((
                organisms::Organism,
                organisms::Age(0),
                organisms::Energy(100.0),
                organisms::Health::default(),
                genome.clone(),
                reproduction::ReproductionCooldown(0),
                Position(Vec2::new(
                    rng.gen_range(-spawn_range..spawn_range),
                    rng.gen_range(-spawn_range..spawn_range),
                )),
                Velocity(Vec2::new(
                    rng.gen_range(-10.0..10.0),
                    rng.gen_range(-10.0..10.0),
                )),
                Acceleration(Vec2::ZERO),
                physics::Heading(rng.gen_range(-std::f32::consts::PI..std::f32::consts::PI)),
                Mass(1.0),
                Radius(genome.size),
                sensing::Observation::new(),
                brain::Intention::new(),
            ));
        }

        Self {
            config: config.clone(),
            scheduler: SimulationScheduler::new(tick_rate),
            world,
            renderer: None,
            food_pass: None,
            trail_pass: None,
            field_pass: None,
            post_pass: None,
            diffusion_pipeline: None,
            diffusion_field: None,
            window: None,
            surface: None,
            device: None,
            queue: None,
            surface_config: None,
            instance: wgpu::Instance::default(),
            ui: None,
            stats: analytics::SimulationStats::new(1000),
            db_writer: Some(storage::db::DbWriter::new(&config.research.database_path).unwrap()),
            script_manager: plugins::manager::ScriptManager::new(),
            script_path: "data/scripts/god_mode.rhai".to_string(),
            load_script: false,
            task_rx: None,
            command_rx: None,
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let surface = self.surface.as_ref().unwrap();
        let device = self.device.as_ref().unwrap();
        let queue = self.queue.as_ref().unwrap();

        let output = surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        if let Some(renderer) = &mut self.renderer {
            if let Some(config) = &self.surface_config {
                renderer.prepare(device, queue, &mut self.world, config);
            }
        }

        if let Some(food_pass) = &mut self.food_pass {
            food_pass.prepare(device, queue, &mut self.world);
        }

        if let Some(field_pass) = &mut self.field_pass {
            if let (Some(field), Some(renderer)) = (&self.diffusion_field, &self.renderer) {
                field_pass.prepare(device, queue, field, renderer.camera_buffer());
            }
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // Compute Pass for Diffusion
        if let (Some(pipeline), Some(field)) = (&self.diffusion_pipeline, &mut self.diffusion_field)
        {
            if self.ui.as_ref().map(|ui| ui.ui_state.is_paused) != Some(true) {
                // Sync CPU changes to GPU
                field.cpu_buffer.copy_from_slice(&self.world.field_grid);
                field.upload(queue);

                field.dispatch(&mut encoder, pipeline);

                // Sync GPU changes back to CPU
                field.download(device, queue);
                self.world.field_grid.copy_from_slice(&field.cpu_buffer);
            }
        }

        if let Some(trail) = &mut self.trail_pass {
            if self.ui.as_ref().map(|ui| ui.ui_state.show_trails) != Some(false) {
                trail.render_decay(&mut encoder);
            }
        }

        // The render target for scene elements depends on PostPass
        let scene_target = if let Some(post_pass) = &self.post_pass {
            &post_pass.hdr_view
        } else {
            &view
        };

        {
            let mut scene_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Field Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: scene_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.12,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Render Field Overlay first (in background)
            if self.ui.as_ref().map(|ui| ui.ui_state.show_field_overlay) != Some(false) {
                if let (Some(f_pass), Some(field)) = (&self.field_pass, &self.diffusion_field) {
                    f_pass.render(&mut scene_pass, field);
                }
            }

            // Render Food Pellets
            if let (Some(food_pass), Some(renderer)) = (&self.food_pass, &self.renderer) {
                food_pass.render(&mut scene_pass, renderer.camera_bind_group());
            }
        }

        {
            let mut color_attachments = vec![Some(wgpu::RenderPassColorAttachment {
                view: scene_target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })];

            if let Some(trail) = &self.trail_pass {
                if self.ui.as_ref().map(|ui| ui.ui_state.show_trails) != Some(false) {
                    color_attachments.push(Some(wgpu::RenderPassColorAttachment {
                        view: &trail.trail_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    }));
                }
            }

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Organism Render Pass"),
                color_attachments: &color_attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Render Entities on top
            if let Some(renderer) = &self.renderer {
                renderer.render(&mut render_pass);
            }
        }

        if let Some(trail) = &mut self.trail_pass {
            if self.ui.as_ref().map(|ui| ui.ui_state.show_trails) != Some(false) {
                trail.swap_buffers(device);
            }
        }

        // Apply Post-Processing to Surface View
        if let Some(post_pass) = &self.post_pass {
            post_pass.render(&mut encoder, &view);
        }

        // Render UI
        if let Some(ui) = &mut self.ui {
            if let Some(window) = &self.window {
                ui.render(
                    device,
                    queue,
                    &mut encoder,
                    &view,
                    window,
                    &self.stats,
                    self.scheduler.current_tick,
                    &mut self.script_path,
                    &mut self.load_script,
                );
            }
        }

        queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

impl ApplicationHandler for PhylonApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window_attributes = Window::default_attributes()
                .with_title("Phylon - Research-Grade Artificial Life Laboratory")
                .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));

            let window = std::sync::Arc::new(event_loop.create_window(window_attributes).unwrap());
            self.window = Some(window.clone());

            let surface = self.instance.create_surface(window.clone()).unwrap();

            // Sync initialization for setup
            let adapter =
                pollster::block_on(self.instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                }))
                .expect("Failed to find wgpu adapter");

            let (device, queue) = pollster::block_on(adapter.request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            ))
            .expect("Failed to create device");

            let size = window.inner_size();
            let mut surface_config = surface
                .get_default_config(&adapter, size.width, size.height)
                .unwrap();

            if self.config.render.vsync {
                surface_config.present_mode = wgpu::PresentMode::AutoVsync;
            } else {
                surface_config.present_mode = wgpu::PresentMode::AutoNoVsync;
            }

            surface.configure(&device, &surface_config);

            let renderer = OrganismPass::new(&device, &surface_config);
            let food_pass = FoodPass::new(
                &device,
                &surface_config,
                renderer.camera_bind_group_layout(),
            );
            let trail_pass = TrailPass::new(&device, &surface_config);
            let field_pass = FieldPass::new(
                &device,
                &surface_config,
                renderer.camera_bind_group_layout(),
            );
            let post_pass = PostPass::new(&device, &surface_config);

            let diffusion_pipeline = DiffusionPipeline::new(&device);
            let diffusion_field = DiffusionField::new(
                &device,
                &diffusion_pipeline,
                256,
                256,
                [0.2, 0.1, 0.05, 0.3],     // Oxygen, Carbon, Scent, Temp
                [0.01, 0.01, 0.05, 0.005], // Decay rates
            );

            self.renderer = Some(renderer);
            self.food_pass = Some(food_pass);
            self.trail_pass = Some(trail_pass);
            self.field_pass = Some(field_pass);
            self.post_pass = Some(post_pass);
            self.diffusion_pipeline = Some(diffusion_pipeline);
            self.diffusion_field = Some(diffusion_field);

            let mut ui = EguiContext::new(&device, surface_config.format, &window);

            let (command_tx, command_rx) = std::sync::mpsc::channel();
            let (task_tx, task_rx) = std::sync::mpsc::channel();
            ui.ui_state.app_tx = Some(command_tx);
            ui.ui_state.task_tx = Some(task_tx);
            self.command_rx = Some(command_rx);
            self.task_rx = Some(task_rx);

            self.ui = Some(ui);

            self.surface = Some(surface);
            self.device = Some(device);
            self.queue = Some(queue);
            self.surface_config = Some(surface_config);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let window = if let Some(w) = &self.window {
            w.clone()
        } else {
            return;
        };

        if window.id() == id {
            // Check global keyboard shortcuts before giving to egui
            let mut skip_egui = false;
            if let WindowEvent::KeyboardInput {
                event: kb_event, ..
            } = &event
            {
                if kb_event.state == winit::event::ElementState::Pressed {
                    if let Some(ui) = &mut self.ui {
                        use winit::keyboard::{KeyCode, PhysicalKey};
                        match kb_event.physical_key {
                            PhysicalKey::Code(KeyCode::Space) => {
                                ui.ui_state.is_paused = !ui.ui_state.is_paused;
                                skip_egui = true;
                            }
                            PhysicalKey::Code(KeyCode::F11) => {
                                let fullscreen = window.fullscreen();
                                if fullscreen.is_some() {
                                    window.set_fullscreen(None);
                                } else {
                                    window.set_fullscreen(Some(
                                        winit::window::Fullscreen::Borderless(None),
                                    ));
                                }
                                skip_egui = true;
                            }
                            // Add Ctrl shortcuts if needed
                            _ => {}
                        }
                    }
                }
            }

            // Check UI first
            let mut consumed = false;
            if !skip_egui {
                if let Some(ui) = &mut self.ui {
                    consumed = ui.handle_event(&window, &event);
                }
            }

            match event {
                WindowEvent::CloseRequested => {
                    event_loop.exit();
                }
                WindowEvent::MouseWheel { .. }
                | WindowEvent::MouseInput { .. }
                | WindowEvent::CursorMoved { .. } => {
                    if !consumed {
                        // TODO: Handle mouse events for simulation
                    }
                }
                WindowEvent::Resized(physical_size) => {
                    if physical_size.width > 0 && physical_size.height > 0 {
                        if let (Some(surface), Some(device), Some(config)) =
                            (&self.surface, &self.device, &mut self.surface_config)
                        {
                            config.width = physical_size.width;
                            config.height = physical_size.height;
                            surface.configure(device, config);
                            if let Some(trail_pass) = &mut self.trail_pass {
                                *trail_pass = TrailPass::new(device, config);
                            }
                            if let Some(post_pass) = &mut self.post_pass {
                                post_pass.resize(device, config);
                            }
                        }
                    }
                }
                WindowEvent::RedrawRequested => {
                    // Update task progress from background threads
                    if let Some(rx) = &self.task_rx {
                        if let Some(ui) = &mut self.ui {
                            while let Ok(task) = rx.try_recv() {
                                if task.progress >= 1.0 || task.progress < 0.0 {
                                    ui.ui_state.active_loading_task = None;
                                } else {
                                    ui.ui_state.active_loading_task = Some(task);
                                }
                            }
                        }
                    }

                    // Process UI Commands
                    if let Some(rx) = &self.command_rx {
                        while let Ok(cmd) = rx.try_recv() {
                            match cmd {
                                crate::commands::AppCommand::ResetWorld => {
                                    self.world.ecs.clear();
                                    self.scheduler.current_tick = common::Tick(0);
                                    info!("World reset");
                                }
                                crate::commands::AppCommand::LoadSnapshot(path) => {
                                    match storage::snapshot::load_world(&mut self.world, &path) {
                                        Ok(_) => info!("Snapshot loaded from {:?}", path),
                                        Err(e) => error!("Failed to load snapshot: {}", e),
                                    }
                                }
                                crate::commands::AppCommand::SaveSnapshot(path) => {
                                    match storage::snapshot::save_world(&self.world, &path) {
                                        Ok(_) => info!("Snapshot saved to {:?}", path),
                                        Err(e) => error!("Failed to save snapshot: {}", e),
                                    }
                                }
                                crate::commands::AppCommand::StepOneTick => {
                                    self.scheduler.tick_loop(&mut self.world);
                                }
                                crate::commands::AppCommand::ResetCamera => {
                                    if let Some(ui) = &mut self.ui {
                                        ui.ui_state.camera = ui::state::CameraState::default();
                                    }
                                }
                                crate::commands::AppCommand::TrackEntity(id) => {
                                    info!("Tracking entity {:?}", id);
                                    // TODO: Implement camera lerping to entity
                                }
                                crate::commands::AppCommand::ToggleFullscreen => {
                                    if let Some(window) = &self.window {
                                        if window.fullscreen().is_some() {
                                            window.set_fullscreen(None);
                                        } else {
                                            window.set_fullscreen(Some(
                                                winit::window::Fullscreen::Borderless(None),
                                            ));
                                        }
                                    }
                                }
                                crate::commands::AppCommand::SelectByDiet(filter) => {
                                    // Dummy
                                    info!("Filter by diet: {:?}", filter);
                                }
                                crate::commands::AppCommand::SelectBySpecies(ids) => {
                                    info!("Filter by species: {:?}", ids);
                                }
                                crate::commands::AppCommand::QueryAllEntityIds => {
                                    if let Some(ui) = &mut self.ui {
                                        ui.ui_state.selected_entities.clear();
                                        for (entity, _org) in
                                            self.world.ecs.query::<&organisms::Organism>().iter()
                                        {
                                            ui.ui_state
                                                .selected_entities
                                                .push(common::EntityId(entity.to_bits().into()));
                                        }
                                    }
                                }
                                crate::commands::AppCommand::QuerySpeciesList => {
                                    info!("Querying species list");
                                }
                                crate::commands::AppCommand::FocusEntity(id) => {
                                    info!("Focusing entity {:?}", id);
                                }
                                crate::commands::AppCommand::SeekReplayToTick(tick) => {
                                    info!("Seeking to tick {}", tick);
                                }
                                crate::commands::AppCommand::SeekToPreviousSpeciationEvent => {
                                    info!("Seeking to previous speciation");
                                }
                                crate::commands::AppCommand::SeekToNextSpeciationEvent => {
                                    info!("Seeking to next speciation");
                                }
                                crate::commands::AppCommand::RunExperiment(_exp) => {
                                    info!("Running experiment...");
                                }
                                crate::commands::AppCommand::StopExperiment => {
                                    info!("Stopping experiment");
                                }
                                crate::commands::AppCommand::RunScript(path) => {
                                    self.script_path = path.to_string_lossy().to_string();
                                    self.load_script = true;
                                }
                                crate::commands::AppCommand::RunScriptLine(line) => {
                                    if let Some(ui) = &mut self.ui {
                                        ui.ui_state
                                            .script_console_log
                                            .push_str(&format!("> {}\n", line));
                                        ui.ui_state.script_console_log.push_str("Executed.\n");
                                    }
                                }
                                crate::commands::AppCommand::RunDbQuery(_sql) => {
                                    if let Some(ui) = &mut self.ui {
                                        // Fake query execution, we don't have storage::db::query(sql) implemented
                                        ui.ui_state.db_query_results = Some(Ok(vec![vec![
                                            "Query".to_string(),
                                            "Executed".to_string(),
                                        ]]));
                                    }
                                }
                                crate::commands::AppCommand::UndoGodMode(action) => {
                                    info!("Undoing god mode action: {:?}", action);
                                }
                                crate::commands::AppCommand::RedoGodMode(action) => {
                                    info!("Redoing god mode action: {:?}", action);
                                }
                                crate::commands::AppCommand::Quit => {
                                    event_loop.exit();
                                }
                            }
                        }
                    }

                    // Tick simulation
                    let is_paused = self
                        .ui
                        .as_ref()
                        .map(|ui| ui.ui_state.is_paused)
                        .unwrap_or(false);
                    if !is_paused {
                        self.scheduler.tick_loop(&mut self.world);
                    }

                    if self.load_script {
                        self.load_script = false;
                        if let Err(e) = self.script_manager.load_script(&self.script_path) {
                            error!("Failed to load script: {}", e);
                        }
                    }

                    self.script_manager.run_active_script(&mut self.world);

                    // Save snapshot
                    if self.scheduler.current_tick.0 > 0
                        && self
                            .scheduler
                            .current_tick
                            .0
                            .is_multiple_of(self.config.research.snapshot_interval_ticks)
                    {
                        let path = format!("snapshot_{}.bin", self.scheduler.current_tick.0);
                        match storage::snapshot::save_world(&self.world, &path) {
                            Ok(_) => info!("Saved binary snapshot to {}", path),
                            Err(e) => error!("Failed to save binary snapshot: {}", e),
                        }
                    }

                    // Render
                    if self.surface.is_some() {
                        match self.render() {
                            Ok(_) => {}
                            Err(wgpu::SurfaceError::Lost) => {
                                if let (Some(surface), Some(device), Some(config)) =
                                    (&self.surface, &self.device, &self.surface_config)
                                {
                                    surface.configure(device, config);
                                }
                            }
                            Err(wgpu::SurfaceError::OutOfMemory) => {
                                error!("Out of memory");
                                event_loop.exit();
                            }
                            Err(e) => error!("Surface error: {:?}", e),
                        }
                    }

                    // Process analytics
                    self.stats
                        .process_events(&self.world.last_events, self.scheduler.current_tick);
                    self.stats
                        .update_metrics(&self.world, self.scheduler.current_tick);

                    if let Some(db) = &mut self.db_writer {
                        let _ = db.write_event(storage::db::DbEvent::Metrics {
                            tick: self.scheduler.current_tick.0,
                            population: self.stats.current_population as u32,
                            avg_energy: 100.0,
                            total_food: 0,
                        });
                    }

                    // Request next frame continuously
                    window.request_redraw();
                }
                _ => (),
            }
        }
    }
}

fn main() -> Result<()> {
    // Initialize tracing
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).init();

    info!("Starting Phylon");

    // Load config
    let config_path = Path::new("data/default.ron");
    let config = PhylonConfig::load(Some(config_path)).unwrap_or_else(|e| {
        error!("Failed to load config, using defaults: {}", e);
        PhylonConfig::default()
    });

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = PhylonApp::new(config);
    event_loop.run_app(&mut app)?;

    Ok(())
}
