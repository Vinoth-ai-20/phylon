//! The main application binary for Phylon.

use anyhow::Result;
use common::Vec2;
use phylon_config::PhylonConfig;
use physics::{Acceleration, Mass, Position, Radius, Velocity};
use rand::Rng;
use rendering::{DebugRenderer, FieldRenderer};
use scheduler::SimulationScheduler;
use std::path::Path;
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};
use world::PhylonWorld;
use diffusion::DiffusionField;
use gpu::compute::DiffusionPipeline;

struct PhylonApp {
    config: PhylonConfig,
    scheduler: SimulationScheduler,
    world: PhylonWorld,
    renderer: Option<DebugRenderer>,
    field_renderer: Option<FieldRenderer>,
    diffusion_pipeline: Option<DiffusionPipeline>,
    diffusion_field: Option<DiffusionField>,
    window: Option<std::sync::Arc<Window>>,
    surface: Option<wgpu::Surface<'static>>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    instance: wgpu::Instance,
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
            let num_weights = brain::BRAIN_WEIGHTS_COUNT;
            genome.brain_weights = (0..num_weights)
                .map(|_| rng.gen_range(-1.0..1.0))
                .collect();

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
            config,
            scheduler: SimulationScheduler::new(tick_rate),
            world,
            renderer: None,
            field_renderer: None,
            diffusion_pipeline: None,
            diffusion_field: None,
            window: None,
            surface: None,
            device: None,
            queue: None,
            surface_config: None,
            instance: wgpu::Instance::default(),
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
        
        if let Some(field_renderer) = &mut self.field_renderer {
            if let (Some(field), Some(renderer)) = (&self.diffusion_field, &self.renderer) {
                field_renderer.prepare(device, queue, field, renderer.camera_buffer());
            }
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // Compute Pass for Diffusion
        if let (Some(pipeline), Some(field)) = (&self.diffusion_pipeline, &mut self.diffusion_field) {
            field.dispatch(&mut encoder, pipeline);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
            if let (Some(field_renderer), Some(field)) = (&self.field_renderer, &self.diffusion_field) {
                field_renderer.render(&mut render_pass, field);
            }

            // Render Entities on top
            if let Some(renderer) = &self.renderer {
                renderer.render(&mut render_pass);
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
                .with_title("Phylon - Research-Grade Artificial Life Laboratory");

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

            let renderer = DebugRenderer::new(&device, &surface_config);
            let field_renderer = FieldRenderer::new(&device, &surface_config, renderer.camera_bind_group_layout());
            
            let diffusion_pipeline = DiffusionPipeline::new(&device);
            let diffusion_field = DiffusionField::new(&device, &diffusion_pipeline, 256, 256, 0.2, 0.01);

            self.renderer = Some(renderer);
            self.field_renderer = Some(field_renderer);
            self.diffusion_pipeline = Some(diffusion_pipeline);
            self.diffusion_field = Some(diffusion_field);

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
            match event {
                WindowEvent::CloseRequested => {
                    event_loop.exit();
                }
                WindowEvent::Resized(physical_size) => {
                    if physical_size.width > 0 && physical_size.height > 0 {
                        if let (Some(surface), Some(device), Some(config)) =
                            (&self.surface, &self.device, &mut self.surface_config)
                        {
                            config.width = physical_size.width;
                            config.height = physical_size.height;
                            surface.configure(device, config);
                        }
                    }
                }
                WindowEvent::RedrawRequested => {
                    // Tick simulation
                    self.scheduler.tick_loop(&mut self.world);

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
