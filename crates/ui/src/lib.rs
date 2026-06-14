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

        // Dark cinematic theme with cyan/amber accents
        let mut style = (*context.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.window_fill = egui::Color32::from_rgb(15, 15, 20);
        style.visuals.panel_fill = egui::Color32::from_rgb(15, 15, 20);
        style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(30, 30, 40);
        style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(40, 40, 50);
        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(60, 60, 70);
        style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(80, 80, 90);
        style.visuals.selection.bg_fill = egui::Color32::from_rgb(0, 150, 255); // Cyan accent
        style.visuals.selection.stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 255, 255));
        context.set_style(style);

        let mut tiles = egui_tiles::Tiles::default();
        let analytics = tiles.insert_pane(layout::Pane::Analytics);
        let research = tiles.insert_pane(layout::Pane::Research);
        let brain = tiles.insert_pane(layout::Pane::BrainInspector);

        // Setup initial docking layout
        let right_tabs = tiles.insert_vertical_tile(vec![analytics, research]);
        let root = tiles.insert_horizontal_tile(vec![brain, right_tabs]);

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

        // Render dockable tiles UI
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
