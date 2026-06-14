use analytics::SimulationStats;
use common::Tick;
use egui::Context;
use egui_wgpu::{Renderer, ScreenDescriptor};
use egui_winit::State;
use wgpu::{CommandEncoder, Device, Queue, TextureFormat, TextureView};
use winit::event::WindowEvent;
use winit::window::Window;

pub mod commands;
pub mod layout;
pub mod menu;
pub mod modal;
pub mod overlay;
pub mod panels;
pub mod state;

use state::UiState;

pub struct EguiContext {
    pub context: Context,
    pub state: State,
    pub renderer: Renderer,
    pub ui_state: UiState,
    pub tree: egui_tiles::Tree<layout::Pane>,
}

impl EguiContext {
    pub fn new(device: &Device, format: TextureFormat, window: &Window) -> Self {
        let context = Context::default();
        let id = context.viewport_id();

        // Optional: configure puffin to only record if we want, but usually it records globally.
        puffin::set_scopes_on(true);

        let state = State::new(context.clone(), id, window, None, None, None);
        let renderer = Renderer::new(device, format, None, 1, false);

        // Dark cinematic theme
        let mut style = (*context.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.window_fill = egui::Color32::from_rgb(20, 20, 28);
        style.visuals.panel_fill = egui::Color32::from_rgb(15, 15, 20);
        style.visuals.override_text_color = Some(egui::Color32::from_rgb(220, 220, 230));
        style.visuals.window_stroke = egui::Stroke::new(0.5, egui::Color32::from_rgb(50, 50, 70));

        // Ensure panels and windows have header backgrounds as requested
        // (Panel headers will be handled by Frame in the individual panel rendering)

        context.set_style(style);

        let mut tiles = egui_tiles::Tiles::default();
        let left_pane = tiles.insert_pane(layout::Pane::Analytics);
        let centre = tiles.insert_pane(layout::Pane::SimulationViewport);
        let right_pane = tiles.insert_pane(layout::Pane::BrainAndGenome);

        let top_row = tiles.insert_horizontal_tile(vec![left_pane, centre, right_pane]);

        let bottom_left = tiles.insert_pane(layout::Pane::Timeline);
        let bottom_right = tiles.insert_pane(layout::Pane::SystemLogs);
        let bottom_row = tiles.insert_horizontal_tile(vec![bottom_left, bottom_right]);

        let root = tiles.insert_vertical_tile(vec![top_row, bottom_row]);
        let tree = egui_tiles::Tree::new("phylon_tree", root, tiles);

        Self {
            context,
            state,
            renderer,
            ui_state: UiState::default(),
            tree,
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
                self.ui_state.panels.profiler = !self.ui_state.panels.profiler;
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

        crate::menu::render_menu_bar(&self.context, &mut self.ui_state, stats);

        // Removed the tile toggling logic for now since we have a fixed layout.
        // We can re-implement visibility toggling if needed, but the prompt
        // implies a docked layout with close buttons inside the panes.

        let mut behavior = layout::TreeBehavior {
            ui_state: &mut self.ui_state,
            stats,
            tick,
            script_path,
            load_script,
        };

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::TRANSPARENT))
            .show(&self.context, |ui| {
                self.tree.ui(&mut behavior, ui);
            });

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
