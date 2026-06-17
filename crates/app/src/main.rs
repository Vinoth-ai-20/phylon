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
    frame_counter: u64,
    tracked_entity: Option<common::EntityId>,
}

impl PhylonApp {
    fn new(config: PhylonConfig) -> Self {
        let tick_rate = config.simulation.tick_rate;
        let mut world = PhylonWorld::new(config.simulation.world_chunk_size as f32);

        Self::spawn_starter_organisms(&mut world);

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
            frame_counter: 0,
            tracked_entity: None,
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let surface = self.surface.as_ref().unwrap();
        let device = self.device.as_ref().unwrap();
        let queue = self.queue.as_ref().unwrap();
        let config = self.surface_config.as_ref().unwrap();

        let output = surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        if let Some(renderer) = &mut self.renderer {
            let mut ui_flags = [0u32; 4];
            if let Some(ui) = &self.ui {
                ui_flags[0] = if ui.ui_state.show_species_colors {
                    1
                } else {
                    0
                };
                ui_flags[1] = if ui.ui_state.show_grid { 1 } else { 0 };
                ui_flags[2] = if ui.ui_state.show_sensor_cones { 1 } else { 0 };
                ui_flags[3] = if ui.ui_state.show_disease_highlight {
                    1
                } else {
                    0
                };
            }
            let (camera_pos, camera_zoom, vp_size) = if let Some(ui) = &self.ui {
                let vp = ui.ui_state.viewport_rect;
                let vp_size = vp.map(|r| [r.width(), r.height()]);
                (
                    glam::vec2(
                        ui.ui_state.camera.position[0],
                        ui.ui_state.camera.position[1],
                    ),
                    ui.ui_state.camera.zoom_level,
                    vp_size,
                )
            } else {
                (glam::Vec2::ZERO, 1.0, None)
            };

            renderer.prepare(
                device,
                queue,
                &mut self.world,
                config,
                camera_pos,
                camera_zoom,
                ui_flags,
                vp_size,
            );

            if let Some(field_pass) = &mut self.field_pass {
                if let Some(field) = &self.diffusion_field {
                    field_pass.prepare(device, queue, field, renderer.camera_buffer());
                }
            }
        }

        if let Some(food_pass) = &mut self.food_pass {
            food_pass.prepare(device, queue, &mut self.world);
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        if let (Some(ui), Some(trail), Some(post)) = (&self.ui, &self.trail_pass, &self.post_pass) {
            let trail_uniforms = [ui.ui_state.trail_decay, 0.0, 0.0, 0.0];
            queue.write_buffer(
                &trail.uniforms_buffer,
                0,
                bytemuck::cast_slice(&trail_uniforms),
            );

            let post_uniforms = [
                ui.ui_state.bloom_threshold,
                ui.ui_state.bloom_intensity,
                0.0,
                0.0,
            ];
            queue.write_buffer(
                &post.uniforms_buffer,
                0,
                bytemuck::cast_slice(&post_uniforms),
            );
        }

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

        let scissor_rect = self.ui.as_ref().and_then(|ui| ui.ui_state.viewport_rect);

        {
            let mut scene_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Field Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: scene_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.02745,
                            g: 0.02745,
                            b: 0.2196,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let Some(rect) = scissor_rect {
                let scale = self
                    .window
                    .as_ref()
                    .map(|w| w.scale_factor() as f32)
                    .unwrap_or(1.0);

                // set viewport
                scene_pass.set_viewport(
                    rect.min.x * scale,
                    rect.min.y * scale,
                    rect.width() * scale,
                    rect.height() * scale,
                    0.0,
                    1.0,
                );

                // set scissor
                let x = (rect.min.x * scale) as u32;
                let y = (rect.min.y * scale) as u32;
                let width = (rect.width() * scale) as u32;
                let height = (rect.height() * scale) as u32;

                let surface_width = config.width;
                let surface_height = config.height;
                let x = x.min(surface_width.saturating_sub(1));
                let y = y.min(surface_height.saturating_sub(1));
                let width = width.min(surface_width - x);
                let height = height.min(surface_height - y);

                if width > 0 && height > 0 {
                    scene_pass.set_scissor_rect(x, y, width, height);
                }
            }

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
                color_attachments.push(Some(wgpu::RenderPassColorAttachment {
                    view: &trail.trail_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                }));
            }

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Organism Render Pass"),
                color_attachments: &color_attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let Some(rect) = scissor_rect {
                let scale = self
                    .window
                    .as_ref()
                    .map(|w| w.scale_factor() as f32)
                    .unwrap_or(1.0);

                // set viewport
                render_pass.set_viewport(
                    rect.min.x * scale,
                    rect.min.y * scale,
                    rect.width() * scale,
                    rect.height() * scale,
                    0.0,
                    1.0,
                );

                // set scissor
                let x = (rect.min.x * scale) as u32;
                let y = (rect.min.y * scale) as u32;
                let width = (rect.width() * scale) as u32;
                let height = (rect.height() * scale) as u32;

                let surface_width = config.width;
                let surface_height = config.height;
                let x = x.min(surface_width.saturating_sub(1));
                let y = y.min(surface_height.saturating_sub(1));
                let width = width.min(surface_width - x);
                let height = height.min(surface_height - y);

                if width > 0 && height > 0 {
                    render_pass.set_scissor_rect(x, y, width, height);
                }
            }

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
            let mut scissor = None;
            let mut viewport = None;
            if let Some(rect) = scissor_rect {
                let scale = self
                    .window
                    .as_ref()
                    .map(|w| w.scale_factor() as f32)
                    .unwrap_or(1.0);
                let x = (rect.min.x * scale) as u32;
                let y = (rect.min.y * scale) as u32;
                let width = (rect.width() * scale) as u32;
                let height = (rect.height() * scale) as u32;

                let surface_width = config.width;
                let surface_height = config.height;
                let x = x.min(surface_width.saturating_sub(1));
                let y = y.min(surface_height.saturating_sub(1));
                let width = width.min(surface_width - x);
                let height = height.min(surface_height - y);

                if width > 0 && height > 0 {
                    scissor = Some((x, y, width, height));
                    viewport = Some((
                        rect.min.x * scale,
                        rect.min.y * scale,
                        rect.width() * scale,
                        rect.height() * scale,
                    ));
                }
            }
            post_pass.render(&mut encoder, &view, scissor, viewport);
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
                    &mut self.world,
                );
            }
        }

        queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    fn spawn_starter_organisms(world: &mut PhylonWorld) {
        let mut rng = rand::thread_rng();
        let spawn_range = 400.0;
        for _ in 0..100 {
            let diet = match rng.gen_range(0..3) {
                0 => genetics::Diet::Herbivore,
                1 => genetics::Diet::Carnivore,
                _ => genetics::Diet::Omnivore,
            };
            let mut genome = genetics::Genome::default_for_diet(diet);

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
                            PhysicalKey::Code(KeyCode::Period) => {
                                if let Some(tx) = &ui.ui_state.app_tx {
                                    let _ = tx.send(crate::commands::AppCommand::StepOneTick);
                                }
                                skip_egui = true;
                            }
                            PhysicalKey::Code(KeyCode::Digit1) => {
                                ui.ui_state.simulation_speed = 1.0;
                                skip_egui = true;
                            }
                            PhysicalKey::Code(KeyCode::Digit2) => {
                                ui.ui_state.simulation_speed = 2.0;
                                skip_egui = true;
                            }
                            PhysicalKey::Code(KeyCode::Digit3) => {
                                ui.ui_state.simulation_speed = 5.0;
                                skip_egui = true;
                            }
                            PhysicalKey::Code(KeyCode::Digit4) => {
                                ui.ui_state.simulation_speed = 10.0;
                                skip_egui = true;
                            }
                            PhysicalKey::Code(KeyCode::KeyF) => {
                                if let Some(&first) = ui.ui_state.selected_entities.first() {
                                    if let Some(tx) = &ui.ui_state.app_tx {
                                        let _ = tx
                                            .send(crate::commands::AppCommand::TrackEntity(first));
                                    }
                                }
                                skip_egui = true;
                            }
                            PhysicalKey::Code(KeyCode::Home) => {
                                if let Some(tx) = &ui.ui_state.app_tx {
                                    let _ = tx.send(crate::commands::AppCommand::ResetCamera);
                                }
                                skip_egui = true;
                            }
                            PhysicalKey::Code(KeyCode::ArrowUp)
                            | PhysicalKey::Code(KeyCode::KeyW) => {
                                ui.ui_state.camera.pan([0.0, 50.0]); // Pan Up
                                skip_egui = true;
                            }
                            PhysicalKey::Code(KeyCode::ArrowDown)
                            | PhysicalKey::Code(KeyCode::KeyS) => {
                                ui.ui_state.camera.pan([0.0, -50.0]); // Pan Down
                                skip_egui = true;
                            }
                            PhysicalKey::Code(KeyCode::ArrowLeft)
                            | PhysicalKey::Code(KeyCode::KeyA) => {
                                ui.ui_state.camera.pan([-50.0, 0.0]); // Pan Left
                                skip_egui = true;
                            }
                            PhysicalKey::Code(KeyCode::ArrowRight)
                            | PhysicalKey::Code(KeyCode::KeyD) => {
                                ui.ui_state.camera.pan([50.0, 0.0]); // Pan Right
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
                            PhysicalKey::Code(KeyCode::Equal)
                            | PhysicalKey::Code(KeyCode::NumpadAdd) => {
                                if let Some(vp) = ui.ui_state.viewport_rect {
                                    let center = [vp.center().x, vp.center().y];
                                    ui.ui_state.camera.zoom_toward(center, -1.0, vp);
                                    // Delta -1.0 to increase scale
                                }
                                skip_egui = true;
                            }
                            PhysicalKey::Code(KeyCode::Minus)
                            | PhysicalKey::Code(KeyCode::NumpadSubtract) => {
                                if let Some(vp) = ui.ui_state.viewport_rect {
                                    let center = [vp.center().x, vp.center().y];
                                    ui.ui_state.camera.zoom_toward(center, 1.0, vp);
                                    // Delta 1.0 to decrease scale
                                }
                                skip_egui = true;
                            }
                            PhysicalKey::Code(KeyCode::Digit0)
                            | PhysicalKey::Code(KeyCode::Numpad0) => {
                                ui.ui_state.camera.position = [0.0, 0.0];
                                ui.ui_state.camera.zoom_level = 1.0;
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
                WindowEvent::MouseWheel { delta: _, .. } => {
                    // TODO: handle zoom
                }
                WindowEvent::CursorMoved { position, .. } => {
                    if let Some(ui) = &mut self.ui {
                        ui.ui_state.last_mouse_pos = Some([position.x as f32, position.y as f32]);
                    }
                }
                WindowEvent::MouseInput { .. } => {
                    if !consumed {
                        // TODO: Handle mouse click events for simulation
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
                                    Self::spawn_starter_organisms(&mut self.world);
                                    info!("World reset");
                                }
                                crate::commands::AppCommand::LoadSnapshot(path) => {
                                    match storage::snapshot::load_world(&mut self.world, &path) {
                                        Ok(_) => {
                                            info!("Snapshot loaded from {:?}", path);
                                            if let Some(ui) = &mut self.ui {
                                                ui.ui_state.last_snapshot_path = Some(path.clone());
                                                ui.ui_state.unsaved_changes = false;
                                            }
                                        }
                                        Err(e) => error!("Failed to load snapshot: {}", e),
                                    }
                                }
                                crate::commands::AppCommand::SaveSnapshot(path) => {
                                    match storage::snapshot::save_world(&self.world, &path) {
                                        Ok(_) => {
                                            info!("Snapshot saved to {:?}", path);
                                            if let Some(ui) = &mut self.ui {
                                                ui.ui_state.last_snapshot_path = Some(path.clone());
                                                ui.ui_state.unsaved_changes = false;
                                            }
                                        }
                                        Err(e) => error!("Failed to save snapshot: {}", e),
                                    }
                                }

                                crate::commands::AppCommand::TrackEntity(id) => {
                                    info!("Tracking entity {:?}", id);
                                    self.tracked_entity = Some(id);
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
                                    if let Some(ui) = &mut self.ui {
                                        ui.ui_state.selected_entities.clear();
                                        for (entity, genome) in
                                            self.world.ecs.query::<&genetics::Genome>().iter()
                                        {
                                            let matches = match genome.diet {
                                                genetics::Diet::Herbivore => filter.herbivore,
                                                genetics::Diet::Carnivore => filter.carnivore,
                                                genetics::Diet::Omnivore => filter.scavenger,
                                            };
                                            if matches {
                                                ui.ui_state.selected_entities.push(
                                                    common::EntityId(entity.to_bits().into()),
                                                );
                                            }
                                        }
                                    }
                                }
                                crate::commands::AppCommand::SelectBySpecies(ids) => {
                                    if let Some(ui) = &mut self.ui {
                                        ui.ui_state.selected_entities.clear();
                                        for (entity, species) in
                                            self.world.ecs.query::<&organisms::SpeciesId>().iter()
                                        {
                                            if ids.iter().any(|s| s.0 == species.0) {
                                                ui.ui_state.selected_entities.push(
                                                    common::EntityId(entity.to_bits().into()),
                                                );
                                            }
                                        }
                                    }
                                }
                                crate::commands::AppCommand::InvertSelection => {
                                    if let Some(ui) = &mut self.ui {
                                        let current: std::collections::HashSet<u64> = ui
                                            .ui_state
                                            .selected_entities
                                            .iter()
                                            .map(|e| e.0)
                                            .collect();
                                        ui.ui_state.selected_entities.clear();
                                        for (entity, _) in
                                            self.world.ecs.query::<&organisms::Organism>().iter()
                                        {
                                            let id: u64 = entity.to_bits().into();
                                            if !current.contains(&id) {
                                                ui.ui_state
                                                    .selected_entities
                                                    .push(common::EntityId(id));
                                            }
                                        }
                                    }
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
                                    if let Some(ui) = &mut self.ui {
                                        let mut species_counts = std::collections::HashMap::new();
                                        for (_, species) in
                                            self.world.ecs.query::<&organisms::SpeciesId>().iter()
                                        {
                                            *species_counts.entry(species.0).or_insert(0) += 1;
                                        }
                                        let mut list: Vec<_> = species_counts.into_iter().collect();
                                        list.sort_by_key(|(id, _)| *id);
                                        ui.ui_state.species_list = list;
                                        ui.ui_state.active_modal =
                                            Some(ui::modal::UiModal::FilterBySpecies {
                                                selected: std::collections::HashSet::new(),
                                            });
                                    }
                                }

                                crate::commands::AppCommand::SeekReplayToTick(target_tick) => {
                                    info!("Seeking to tick {}", target_tick);
                                    while self.scheduler.current_tick.0 < target_tick {
                                        self.scheduler.tick(&mut self.world);
                                    }
                                }
                                crate::commands::AppCommand::SeekToPreviousSpeciationEvent => {
                                    info!("Seeking to previous speciation");
                                    if let Some(db) = &self.db_writer {
                                        let current = self.scheduler.current_tick.0 as i64;
                                        if let Ok(target) = db.get_conn().query_row(
                                            "SELECT tick FROM metrics WHERE population > (SELECT population FROM metrics m2 WHERE m2.tick = metrics.tick - 1) AND tick < ?1 ORDER BY tick DESC LIMIT 1",
                                            [current],
                                            |row| row.get::<_, i64>(0),
                                        ) {
                                            info!("Found previous speciation event at tick {}", target);
                                            // Replay/jump is only forward, so if target < current, we can't jump backwards directly without reloading snapshot
                                            // Since we only implement forward jump right now:
                                            error!("Cannot seek backwards to {} without snapshot reload", target);
                                        }
                                    }
                                }
                                crate::commands::AppCommand::SeekToNextSpeciationEvent => {
                                    info!("Seeking to next speciation");
                                    if let Some(db) = &self.db_writer {
                                        let current = self.scheduler.current_tick.0 as i64;
                                        if let Ok(target) = db.get_conn().query_row(
                                            "SELECT tick FROM metrics WHERE population > (SELECT population FROM metrics m2 WHERE m2.tick = metrics.tick - 1) AND tick > ?1 ORDER BY tick ASC LIMIT 1",
                                            [current],
                                            |row| row.get::<_, i64>(0),
                                        ) {
                                            info!("Found next speciation event at tick {}", target);
                                            while self.scheduler.current_tick.0 < target as u64 {
                                                self.scheduler.tick(&mut self.world);
                                            }
                                        }
                                    }
                                }
                                crate::commands::AppCommand::RunExperiment(_exp) => {
                                    info!("Running experiment...");
                                }
                                crate::commands::AppCommand::StageExperiment(exp) => {
                                    if let Some(ui) = &mut self.ui {
                                        ui.ui_state.active_experiment = Some(exp);
                                        ui.ui_state.active_modal =
                                            Some(ui::modal::UiModal::ExperimentReady);
                                    }
                                }
                                crate::commands::AppCommand::StopExperiment => {
                                    info!("Stopping experiment");
                                }
                                crate::commands::AppCommand::RunScript(path) => {
                                    self.script_path = path.to_string_lossy().to_string();
                                    self.load_script = true;
                                    if let Some(ui) = &mut self.ui {
                                        ui.ui_state.god_mode_action_stack.push(
                                            ui::state::GodModeAction::ScriptRun {
                                                script_path: self.script_path.clone(),
                                                affected_entity_ids: Vec::new(),
                                            },
                                        );
                                        ui.ui_state.panels.script_console = true;
                                    }
                                }
                                crate::commands::AppCommand::RunScriptLine(line) => {
                                    if let Some(ui) = &mut self.ui {
                                        ui.ui_state
                                            .script_console_log
                                            .push_str(&format!("> {}\n", line));
                                        ui.ui_state.script_console_log.push_str("Executed.\n");
                                        ui.ui_state.god_mode_action_stack.push(
                                            ui::state::GodModeAction::ScriptRun {
                                                script_path: line,
                                                affected_entity_ids: Vec::new(),
                                            },
                                        );
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
                                crate::commands::AppCommand::ExportLineageTree(path) => {
                                    #[derive(serde::Serialize)]
                                    struct Node {
                                        entity_id: u64,
                                        generation: u32,
                                        genome: genetics::Genome,
                                    }
                                    let mut nodes = Vec::new();
                                    for (e, (gen, genome)) in self
                                        .world
                                        .ecs
                                        .query::<(&organisms::Generation, &genetics::Genome)>()
                                        .iter()
                                    {
                                        nodes.push(Node {
                                            entity_id: e.to_bits().into(),
                                            generation: gen.0,
                                            genome: genome.clone(),
                                        });
                                    }
                                    let json = serde_json::to_string_pretty(&nodes)
                                        .unwrap_or_else(|_| "[]".to_string());
                                    if let Ok(mut w) = std::fs::File::create(&path) {
                                        use std::io::Write;
                                        let _ = write!(w, "{}", json);
                                        info!("Lineage tree exported to {:?}", path);
                                    }
                                }
                                crate::commands::AppCommand::UndoGodMode(action) => {
                                    info!("Undoing god mode action: {:?}", action);
                                }
                                crate::commands::AppCommand::RedoGodMode(action) => {
                                    info!("Redoing god mode action: {:?}", action);
                                }
                                crate::commands::AppCommand::StepOneTick => {
                                    if self
                                        .ui
                                        .as_ref()
                                        .map(|ui| ui.ui_state.is_paused)
                                        .unwrap_or(false)
                                    {
                                        self.scheduler.tick_loop(&mut self.world);
                                    }
                                }
                                crate::commands::AppCommand::ZoomIn => {
                                    if let Some(ui) = &mut self.ui {
                                        if let Some(vp) = ui.ui_state.viewport_rect {
                                            let center = [vp.center().x, vp.center().y];
                                            ui.ui_state.camera.zoom_toward(center, -1.0, vp);
                                        }
                                    }
                                }
                                crate::commands::AppCommand::ZoomOut => {
                                    if let Some(ui) = &mut self.ui {
                                        if let Some(vp) = ui.ui_state.viewport_rect {
                                            let center = [vp.center().x, vp.center().y];
                                            ui.ui_state.camera.zoom_toward(center, 1.0, vp);
                                        }
                                    }
                                }
                                crate::commands::AppCommand::WheelZoom {
                                    mouse_position,
                                    delta,
                                } => {
                                    if let Some(ui) = &mut self.ui {
                                        if let Some(vp) = ui.ui_state.viewport_rect {
                                            // Convert mouse wheel delta to zoom steps (delta usually +/- 1.0)
                                            let steps = if delta > 0.0 { -1.0 } else { 1.0 };
                                            ui.ui_state.camera.zoom_toward(
                                                mouse_position,
                                                steps,
                                                vp,
                                            );
                                        }
                                    }
                                }
                                crate::commands::AppCommand::PanCamera(delta) => {
                                    if let Some(ui) = &mut self.ui {
                                        ui.ui_state.camera.pan(delta);
                                        self.tracked_entity = None; // Stop tracking on manual pan
                                    }
                                }
                                crate::commands::AppCommand::ClickWorld(pos) => {
                                    if let Some(ui) = &mut self.ui {
                                        if let Some(vp) = ui.ui_state.viewport_rect {
                                            let world_pos =
                                                ui.ui_state.camera.screen_to_world(pos, vp);
                                            // zoom-aware threshold: say 10 pixels screen distance max
                                            let threshold_world_sq =
                                                (10.0 / ui.ui_state.camera.zoom_level).powi(2);

                                            let mut closest = None;
                                            let mut min_dist_sq = f32::MAX;
                                            for (entity, p) in
                                                self.world.ecs.query::<&physics::Position>().iter()
                                            {
                                                let dist_sq = (p.0.x - world_pos[0]).powi(2)
                                                    + (p.0.y - world_pos[1]).powi(2);
                                                if dist_sq < min_dist_sq
                                                    && dist_sq < threshold_world_sq
                                                {
                                                    min_dist_sq = dist_sq;
                                                    closest = Some(entity);
                                                }
                                            }

                                            ui.ui_state.selected_entities.clear();
                                            if let Some(e) = closest {
                                                ui.ui_state
                                                    .selected_entities
                                                    .push(common::EntityId(e.to_bits().into()));
                                            }
                                        }
                                    }
                                }
                                crate::commands::AppCommand::DoubleClickWorld(pos) => {
                                    if let Some(ui) = &mut self.ui {
                                        if let Some(vp) = ui.ui_state.viewport_rect {
                                            let world_pos =
                                                ui.ui_state.camera.screen_to_world(pos, vp);
                                            let threshold_world_sq =
                                                (15.0 / ui.ui_state.camera.zoom_level).powi(2);

                                            let mut closest = None;
                                            let mut min_dist_sq = f32::MAX;
                                            for (entity, p) in
                                                self.world.ecs.query::<&physics::Position>().iter()
                                            {
                                                let dist_sq = (p.0.x - world_pos[0]).powi(2)
                                                    + (p.0.y - world_pos[1]).powi(2);
                                                if dist_sq < min_dist_sq
                                                    && dist_sq < threshold_world_sq
                                                {
                                                    min_dist_sq = dist_sq;
                                                    closest = Some(entity);
                                                }
                                            }

                                            if let Some(e) = closest {
                                                let id = common::EntityId(e.to_bits().into());
                                                ui.ui_state.selected_entities.clear();
                                                ui.ui_state.selected_entities.push(id);
                                                self.tracked_entity = Some(id);
                                            }
                                        }
                                    }
                                }
                                crate::commands::AppCommand::ResetCamera => {
                                    self.tracked_entity = None;

                                    // Calculate center of mass of organisms
                                    let mut sum_x = 0.0;
                                    let mut sum_y = 0.0;
                                    let mut count = 0;
                                    for (_, pos) in self
                                        .world
                                        .ecs
                                        .query::<&physics::Position>()
                                        .with::<&organisms::Organism>()
                                        .iter()
                                    {
                                        sum_x += pos.0.x;
                                        sum_y += pos.0.y;
                                        count += 1;
                                    }

                                    let target_pos = if count > 0 {
                                        [sum_x / count as f32, sum_y / count as f32]
                                    } else {
                                        [0.0, 0.0]
                                    };

                                    if let Some(ui) = &mut self.ui {
                                        ui.ui_state.camera.target_position = target_pos;
                                        ui.ui_state.camera.target_zoom = 1.0;
                                    }
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

                    let speed = self
                        .ui
                        .as_ref()
                        .map(|ui| ui.ui_state.simulation_speed)
                        .unwrap_or(1.0);

                    if !is_paused {
                        let ticks_this_frame = if speed <= 0.25 {
                            self.frame_counter += 1;
                            if self.frame_counter.is_multiple_of(4) {
                                1
                            } else {
                                0
                            }
                        } else if speed <= 1.0 {
                            1
                        } else if speed <= 2.0 {
                            2
                        } else if speed <= 5.0 {
                            5
                        } else if speed <= 10.0 {
                            10
                        } else {
                            usize::MAX
                        };

                        if ticks_this_frame == usize::MAX {
                            let frame_budget = std::time::Duration::from_millis(16);
                            let start = std::time::Instant::now();
                            while start.elapsed() < frame_budget {
                                self.scheduler.tick_loop(&mut self.world);
                            }
                        } else {
                            for _ in 0..ticks_this_frame {
                                self.scheduler.tick_loop(&mut self.world);
                            }
                        }
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

                    if let Some(id) = self.tracked_entity {
                        if let Some(e) = hecs::Entity::from_bits(id.0) {
                            if let Ok(pos) = self.world.ecs.query_one_mut::<&physics::Position>(e) {
                                if let Some(ui) = &mut self.ui {
                                    ui.ui_state.camera.target_position = [pos.0.x, pos.0.y];
                                }
                            } else {
                                // Entity not found (dead?), stop tracking
                                self.tracked_entity = None;
                            }
                        } else {
                            self.tracked_entity = None;
                        }
                    }

                    if let Some(ui) = &mut self.ui {
                        // Normally use real dt, assuming 60hz for now
                        ui.ui_state.camera.update(1.0 / 60.0);
                    }

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
