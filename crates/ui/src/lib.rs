use analytics::SimulationStats;
use common::Tick;
use egui::Context;
use egui_wgpu::{Renderer, ScreenDescriptor};
use egui_winit::State;
use wgpu::{CommandEncoder, Device, Queue, TextureFormat, TextureView};
use winit::event::WindowEvent;
use winit::window::Window;

pub mod commands;
pub mod components;
pub mod layout;
pub mod menu;
pub mod modal;
pub mod overlay;
pub mod overlays;
pub mod panels;
pub mod state;
pub mod theme;
pub mod zones;

use state::UiState;

pub struct EguiContext {
    pub context: Context,
    pub state: State,
    pub renderer: Renderer,
    pub ui_state: UiState,
}

impl EguiContext {
    pub fn new(device: &Device, format: TextureFormat, window: &Window) -> Self {
        let context = Context::default();
        let id = context.viewport_id();

        // Optional: configure puffin to only record if we want, but usually it records globally.
        puffin::set_scopes_on(true);

        // Setup Fonts for Phosphor Icons
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        context.set_fonts(fonts);

        let state = State::new(context.clone(), id, window, None, None, None);
        let renderer = Renderer::new(device, format, None, 1, false);

        crate::theme::apply_style(&context);

        Self {
            context,
            state,
            renderer,
            ui_state: UiState::default(),
        }
    }

    /// Returns `true` if the event was consumed by egui (e.g. mouse click on UI).
    pub fn handle_event(&mut self, window: &Window, event: &WindowEvent) -> bool {
        let response = self.state.on_window_event(window, event);

        if let WindowEvent::KeyboardInput {
            event: kb_event, ..
        } = event
        {
            if kb_event.state == winit::event::ElementState::Pressed && !response.consumed {
                match kb_event.physical_key {
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::F3) => {
                        self.ui_state.panels.profiler = !self.ui_state.panels.profiler;
                    }
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Space) => {
                        self.ui_state.is_paused = !self.ui_state.is_paused;
                    }
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyS) => {
                        if let Some(tx) = &self.ui_state.app_tx {
                            let _ = tx.send(crate::commands::AppCommand::StepOneTick);
                        }
                    }
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyF) => {
                        let modifiers = self.state.egui_ctx().input(|i| i.modifiers);
                        if modifiers.ctrl || modifiers.command {
                            self.ui_state.is_search_active = true;
                        } else if !self.ui_state.selected_entities.is_empty() {
                            // Focus logic
                        }
                    }
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Tab) => {
                        let currently_collapsed =
                            self.ui_state.is_left_collapsed && self.ui_state.is_right_collapsed;
                        self.ui_state.is_left_collapsed = !currently_collapsed;
                        self.ui_state.is_right_collapsed = !currently_collapsed;
                    }
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyM) => {
                        self.ui_state.show_field_overlay = !self.ui_state.show_field_overlay;
                    }
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyC) => {
                        self.ui_state.selected_entities.clear();
                    }
                    _ => {}
                }
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
        world: &mut world::PhylonWorld,
    ) {
        let raw_input = self.state.take_egui_input(window);

        self.context.begin_pass(raw_input);

        // --- UI Construction ---

        crate::zones::system_bar::render_system_bar(&self.context, &mut self.ui_state, stats, tick);

        crate::layout::render_dashboard(
            &self.context,
            &mut self.ui_state,
            world,
            stats,
            tick,
            script_path,
            load_script,
        );

        crate::overlay::render_loading_overlay(&self.context, &mut self.ui_state);
        crate::modal::render_modals(&self.context, &mut self.ui_state);

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
