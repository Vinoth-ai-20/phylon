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

    /// Compute pipeline for soft-body muscles.
    muscle_compute: Option<gpu::muscle::MuscleComputePipeline>,

    /// Compute pipeline for the diffusion field.
    diffusion_compute: Option<gpu::diffusion_pipeline::DiffusionComputePipeline>,

    /// Debug renderer for entities.
    debug_renderer: Option<rendering::DebugRenderer>,

    /// Renderer for the diffusion field.
    field_renderer: Option<rendering::FieldRenderer>,

    /// The main window (created on `Resumed`).
    window: Option<Arc<Window>>,

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

        // Spawn test soft body
        let genome = genetics::Genome::new(
            genetics::GenomeId(1),
            common::EntityId(0),
            vec![
                genetics::SegmentType::Head,
                genetics::SegmentType::Torso,
                genetics::SegmentType::Muscle,
                genetics::SegmentType::Muscle,
                genetics::SegmentType::Tail,
            ],
        );
        organisms::spawn_organism(&mut world.ecs, &genome, common::Vec2::new(0.0, 0.0));

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
            muscle_compute: None,
            diffusion_compute: None,
            debug_renderer: None,
            field_renderer: None,
            window: None,
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

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("PhylonDevice"),
                required_features: wgpu::Features::FLOAT32_FILTERABLE,
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
        let field_renderer = rendering::FieldRenderer::new(&device, surface_format);
        let muscle_compute = gpu::muscle::MuscleComputePipeline::new(&device);
        let diffusion_compute =
            gpu::diffusion_pipeline::DiffusionComputePipeline::new(&device, 256, 256);

        self.gpu = Some(GpuSurface {
            surface,
            device,
            queue,
            config: surface_config,
        });
        self.debug_renderer = Some(debug_renderer);
        self.field_renderer = Some(field_renderer);
        self.muscle_compute = Some(muscle_compute);
        self.diffusion_compute = Some(diffusion_compute);
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
    }

    /// Advances the simulation and renders one frame.
    fn render(&mut self) -> Result<()> {
        let Some(gpu) = self.gpu.as_ref() else {
            return Ok(());
        };
        let Some(muscle_compute) = self.muscle_compute.as_ref() else {
            return Ok(());
        };
        let Some(debug_renderer) = self.debug_renderer.as_ref() else {
            return Ok(());
        };

        // Advance time
        self.total_sim_time += 0.016; // Fixed step for now

        // 1. Gather springs
        let mut query_springs = self
            .world
            .ecs
            .query::<(bevy_ecs::entity::Entity, &physics::Spring)>();
        let mut gpu_springs = Vec::new();
        let mut spring_entities = Vec::new();
        for (entity, spring) in query_springs.iter(&self.world.ecs) {
            gpu_springs.push(gpu::muscle::GpuSpring {
                node_a: spring.node_a.to_bits() as u32,
                node_b: spring.node_b.to_bits() as u32,
                rest_length: spring.rest_length,
                base_length: spring.base_length,
                stiffness: spring.stiffness,
                damping: spring.damping,
                actuation_amplitude: spring.actuation_amplitude,
                actuation_phase: spring.actuation_phase,
            });
            spring_entities.push(entity);
        }

        // 2. Compute and readback
        let updated_springs = muscle_compute.compute_and_readback(
            &gpu.device,
            &gpu.queue,
            &gpu_springs,
            self.total_sim_time,
        );

        // 3. Update ECS Springs
        for (i, entity) in spring_entities.iter().enumerate() {
            if let Some(mut spring) = self.world.ecs.get_mut::<physics::Spring>(*entity) {
                spring.rest_length = updated_springs[i].rest_length;
            }
        }

        // 4. Run Physics and Biology Systems
        use bevy_ecs::system::RunSystemOnce;
        self.world.ecs.run_system_once(physics::spring_force_system);
        self.world
            .ecs
            .run_system_once(physics::physics_integration_system);
        self.world.ecs.run_system_once(ecology::food_spawner_system);
        self.world.ecs.run_system_once(ecology::foraging_system);
        self.world
            .ecs
            .run_system_once(metabolism::metabolism_system);
        self.world.ecs.run_system_once(organisms::growth_system);
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
        let mut instances = Vec::new();

        // Render soft body nodes
        let mut query_nodes = self.world.ecs.query::<&physics::ParticleNode>();
        for node in query_nodes.iter(&self.world.ecs) {
            instances.push(rendering::DebugInstance {
                position: [node.position.x, node.position.y],
                color: [0.2, 0.8, 0.4, 1.0], // Green
                radius: 4.0,                 // Fixed radius for nodes
            });
        }

        // Render food pellets
        let mut query_food = self.world.ecs.query::<&ecology::FoodPellet>();
        for food in query_food.iter(&self.world.ecs) {
            instances.push(rendering::DebugInstance {
                position: [food.position.x, food.position.y],
                color: [1.0, 0.8, 0.0, 1.0], // Gold/Yellow
                radius: 2.5,                 // Smaller radius for food
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
        );

        gpu.queue.submit(std::iter::once(encoder.finish()));

        // Render debug quads over the field background
        debug_renderer.render(
            &gpu.device,
            &gpu.queue,
            &view,
            &instances,
            [gpu.config.width as f32, gpu.config.height as f32],
        );

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
        match event {
            WindowEvent::CloseRequested => {
                info!("Window close requested — exiting");
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                self.resize(new_size);
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
