//! # GPU/Surface Bring-Up
//!
//! Everything here is about acquiring a `wgpu::Device`/`wgpu::Queue`, the
//! four GPU compute pipelines, and — windowed runs only — a swapchain
//! surface and the renderer/egui state that draws into it. Nothing in this
//! module touches ECS resources or simulation state; it is purely
//! infrastructure bring-up, kept separate from `app.rs`'s ECS/resource
//! wiring, entity picking, and starter-species seeding so each concern can
//! be read (and changed) independently.
//!
//! ## Purpose
//!
//! Phylon can run either windowed (interactive workbench, with rendering)
//! or headless (batch/scripted runs with no display). This module provides
//! one code path for each: [`PhylonApp::init_gpu`] (windowed) and
//! [`PhylonApp::init_gpu_headless`] (headless), sharing their common
//! adapter/device/pipeline construction through `request_gpu_core` so the
//! two modes cannot silently drift apart in how they build the GPU compute
//! pipelines.
//!
//! ## Architecture
//!
//! - `GpuContext` is the long-lived handle stored on `PhylonApp::gpu`:
//!   device, queue, and (windowed only) the surface, its configuration, and
//!   the optional GPU timestamp-query resources used for profiling.
//! - `GpuCore` is a private intermediate struct bundling everything
//!   `request_gpu_core` builds identically for both windowed and headless
//!   init (adapter, device, queue, the four compute pipelines, and the
//!   optional timestamp-query set/buffers). It exists purely to let one
//!   function serve both call sites; the two call sites differ only in a
//!   handful of parameters (surface compatibility, device label, base
//!   feature set, error messages).
//!
//! ## Lifecycle
//!
//! `init_gpu`/`init_gpu_headless` run once, after a window (or, headless,
//! immediately at startup) is available — see `app.rs`'s module doc for
//! where this fits in `PhylonApp`'s overall startup sequence. `resize` is
//! called on every window-resize event thereafter to reconfigure the
//! surface and renderer targets; there is no explicit teardown path, GPU
//! resources are released when `PhylonApp` (and the `GpuContext` it owns)
//! is dropped.
//!
//! ## Failure modes
//!
//! Both init functions return `Result` and surface failures (no suitable
//! adapter, device creation failure, surface creation failure) as
//! `anyhow::Error` with context describing which step failed; the caller
//! decides whether to fall back to headless mode or abort. GPU timestamp
//! queries are opportunistic — used only if the adapter reports both
//! `TIMESTAMP_QUERY` and `TIMESTAMP_QUERY_INSIDE_ENCODERS` support, with
//! `query_set`/`resolve_buffer`/`readback_buffer` left `None` otherwise (the
//! rest of the app treats their absence as "profiling unavailable," not an
//! error).
//!
//! ## Thread safety
//!
//! Adapter/device requests block the calling thread via `pollster::block_on`
//! rather than being awaited asynchronously — acceptable because GPU init
//! happens once, synchronously, during startup, not on a hot path.

use std::sync::Arc;

use anyhow::{Context, Result};
use tracing::info;
use winit::window::Window;

use crate::app::PhylonApp;

/// The retained `wgpu` graphics context: device, queue, and (windowed runs
/// only) the swapchain surface and GPU profiling resources.
///
/// ## Purpose
///
/// Running a simulation at organism counts in the thousands is not
/// practical on the CPU alone — `GpuContext` is what lets Phylon dispatch
/// its physics integration, reaction-diffusion fields, and CTRNN (the
/// organism brain model) forward passes as parallel GPU compute shaders,
/// and (windowed runs) render the resulting scene.
///
/// ## Lifecycle
///
/// Constructed once, during startup, by [`PhylonApp::init_gpu`] (windowed)
/// or [`PhylonApp::init_gpu_headless`] (headless) — see this module's
/// top-level doc comment. Kept alive for the duration of the application on
/// `PhylonApp::gpu` and passed by reference to the compute pipelines and
/// renderers each frame; there is no explicit shutdown, resources are freed
/// on drop.
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
/// and `init_gpu_headless` both construct identically. Kept as one shared
/// struct/function pair since the only real differences between the two
/// call sites are a handful of knobs (surface compatibility, device label,
/// base feature set, error messages), not the construction logic itself —
/// duplicating that logic per call site would risk the two modes silently
/// drifting apart (e.g. one gaining a feature flag the other doesn't get).
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
/// `TIMESTAMP_QUERY_INSIDE_ENCODERS` when the adapter supports both) and
/// builds the 4 compute pipelines plus the timestamp query-set/buffers if
/// timestamp queries are available. Shared by both `init_gpu` and
/// `init_gpu_headless` so the windowed and headless code paths cannot
/// silently diverge in how they build the GPU compute pipelines.
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

impl PhylonApp {
    pub(crate) fn init_gpu(&mut self, window: Arc<Window>) -> Result<()> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // SAFETY: `wgpu::Surface<'static>` borrows the window's raw
        // window/display handles for as long as the surface exists, so the
        // window must outlive the surface. `Arc<Window>` guarantees this:
        // the same `Arc` is stored both in the surface (via the clone
        // passed to `create_surface`) and on `PhylonApp::window`, so the
        // window cannot be dropped while `PhylonApp` (and therefore the
        // surface) is still alive.
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
}
