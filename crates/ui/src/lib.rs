use analytics::SimulationStats;
use common::Tick;
use egui::Context;
use egui_wgpu::{Renderer, ScreenDescriptor};
use egui_winit::State;
use wgpu::{CommandEncoder, Device, Queue, TextureFormat, TextureView};
use winit::event::WindowEvent;
use winit::window::Window;

pub struct EguiContext {
    pub context: Context,
    pub state: State,
    pub renderer: Renderer,
    pub show_profiler: bool,
}

impl EguiContext {
    pub fn new(device: &Device, format: TextureFormat, window: &Window) -> Self {
        let context = Context::default();
        let id = context.viewport_id();

        // Optional: configure puffin to only record if we want, but usually it records globally.
        puffin::set_scopes_on(true);

        let state = State::new(context.clone(), id, window, None, None, None);
        let renderer = Renderer::new(device, format, None, 1, false);

        Self {
            context,
            state,
            renderer,
            show_profiler: false,
        }
    }

    /// Returns `true` if the event was consumed by egui (e.g. mouse click on UI).
    pub fn handle_event(&mut self, window: &Window, event: &WindowEvent) -> bool {
        let response = self.state.on_window_event(window, event);

        // Toggle profiler on F3
        if let WindowEvent::KeyboardInput {
            event: kb_event, ..
        } = event
        {
            if kb_event.state == winit::event::ElementState::Pressed
                && kb_event.physical_key == winit::keyboard::KeyCode::F3
            {
                self.show_profiler = !self.show_profiler;
            }
        }

        response.consumed
    }

    /// Renders the egui UI overlaid on the application window.
    ///
    /// This handles passing input to egui, building the UI layout (e.g. Analytics window),
    /// and executing the required wgpu render passes to draw the UI into the given texture view.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        view: &TextureView,
        window: &Window,
        stats: &SimulationStats,
        tick: Tick,
        script_path: &mut String,
        load_script: &mut bool,
    ) {
        let raw_input = self.state.take_egui_input(window);

        self.context.begin_pass(raw_input);

        // --- UI Construction ---

        // Analytics Window
        egui::Window::new("Analytics").show(&self.context, |ui| {
            ui.label(format!("Tick: {}", tick.0));
            ui.label(format!("Population: {}", stats.current_population));
            ui.separator();
            ui.label("Deaths by Cause:");
            ui.label(format!("- Starvation: {}", stats.deaths_by_starvation));
            ui.label(format!("- Predation: {}", stats.deaths_by_predation));
            ui.label(format!("- Old Age: {}", stats.deaths_by_age));

            ui.separator();
            ui.label("Population History");

            let points: egui_plot::PlotPoints = stats
                .population_history
                .iter()
                .map(|(t, p)| [*t, *p])
                .collect();

            let line = egui_plot::Line::new(points);
            egui_plot::Plot::new("population_plot")
                .view_aspect(2.0)
                .show(ui, |plot_ui| plot_ui.line(line));
        });

        // Research & Plugins Window
        egui::Window::new("Research & Plugins").show(&self.context, |ui| {
            ui.horizontal(|ui| {
                ui.label("Script:");
                ui.text_edit_singleline(script_path);
            });
            if ui.button("Load & Run").clicked() {
                *load_script = true;
            }
        });

        // Profiler removed due to version incompatibility with egui 0.29
        // We still support F3 to toggle puffin scope recording logic if desired,
        // though puffin normally records anyway.

        // --- Render UI ---
        let full_output = self.context.end_pass();

        self.state
            .handle_platform_output(window, full_output.platform_output);

        let tris = self
            .context
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }

        let size = window.inner_size();
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [size.width, size.height],
            pixels_per_point: window.scale_factor() as f32,
        };

        self.renderer
            .update_buffers(device, queue, encoder, &tris, &screen_descriptor);

        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Egui Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.renderer.render(
                &mut render_pass.forget_lifetime(),
                &tris,
                &screen_descriptor,
            );
        }

        for x in &full_output.textures_delta.free {
            self.renderer.free_texture(x);
        }
    }
}
