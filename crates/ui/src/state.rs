use crate::modal::UiModal;
use common::EntityId;
use std::sync::{atomic::AtomicBool, Arc};

#[derive(Clone, Debug)]
pub enum GodModeAction {
    ScriptRun {
        script_path: String,
        affected_entity_ids: Vec<EntityId>,
    },
    // Adding a generic string variant for Redo/Undo text display if needed, but sticking to prompt
}

#[derive(Clone, Debug)]
pub struct CameraState {
    pub position: [f32; 2],        // current world space centre
    pub zoom_level: f32,           // current zoom level (1.0 = default)
    pub target_position: [f32; 2], // for smooth interpolation
    pub target_zoom: f32,          // for smooth interpolation
    pub min_zoom: f32,             // 0.05
    pub max_zoom: f32,             // 50.0
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0],
            zoom_level: 1.0,
            target_position: [0.0, 0.0],
            target_zoom: 1.0,
            min_zoom: 0.05,
            max_zoom: 50.0,
        }
    }
}

impl CameraState {
    pub fn zoom_toward(&mut self, cursor_screen: [f32; 2], delta: f32, viewport: egui::Rect) {
        let world_before = self.screen_to_world(cursor_screen, viewport);

        let step = if self.target_zoom < 1.0 {
            0.05
        } else if self.target_zoom < 5.0 {
            0.25
        } else if self.target_zoom < 20.0 {
            1.0
        } else {
            2.0
        };

        // If zooming out, use the step of the target to prevent skipping ranges oddly,
        // but simply using current target_zoom is fine.
        self.target_zoom = (self.target_zoom + delta * step).clamp(self.min_zoom, self.max_zoom);

        // We calculate target position adjust assuming target_zoom was applied instantly
        // To keep the world point exactly under cursor, we must shift target_position.
        // It's a bit tricky to mix lerp with zoom_toward perfectly without drifting,
        // but approximating it:

        // world_after if we just changed zoom:
        let vw = viewport.width();
        let vh = viewport.height();
        let ndc_x = (cursor_screen[0] - viewport.min.x) / vw * 2.0 - 1.0;
        let ndc_y = (cursor_screen[1] - viewport.min.y) / vh * 2.0 - 1.0;
        let world_after = [
            self.target_position[0] + ndc_x / self.target_zoom * vw * 0.5,
            self.target_position[1] - ndc_y / self.target_zoom * vh * 0.5,
        ];

        self.target_position[0] += world_before[0] - world_after[0];
        self.target_position[1] += world_before[1] - world_after[1];
        self.clamp_bounds();
    }

    pub fn pan(&mut self, drag_delta_screen: [f32; 2]) {
        self.target_position[0] -= drag_delta_screen[0] / self.zoom_level;
        self.target_position[1] += drag_delta_screen[1] / self.zoom_level;
        self.clamp_bounds();
    }

    pub fn clamp_bounds(&mut self) {
        // Assume world bounds are around -5000.0 to 5000.0
        let limit = 5000.0;
        self.target_position[0] = self.target_position[0].clamp(-limit, limit);
        self.target_position[1] = self.target_position[1].clamp(-limit, limit);
    }

    pub fn update(&mut self, dt: f32) {
        let lerp_factor = (dt * 15.0).clamp(0.0, 1.0);
        self.position[0] += (self.target_position[0] - self.position[0]) * lerp_factor;
        self.position[1] += (self.target_position[1] - self.position[1]) * lerp_factor;
        self.zoom_level += (self.target_zoom - self.zoom_level) * lerp_factor;
    }

    pub fn screen_to_world(&self, screen: [f32; 2], viewport: egui::Rect) -> [f32; 2] {
        let vw = viewport.width();
        let vh = viewport.height();
        let ndc_x = (screen[0] - viewport.min.x) / vw * 2.0 - 1.0;
        let ndc_y = (screen[1] - viewport.min.y) / vh * 2.0 - 1.0;
        [
            self.position[0] + ndc_x / self.zoom_level * vw * 0.5,
            self.position[1] - ndc_y / self.zoom_level * vh * 0.5,
        ]
    }

    pub fn world_to_screen(&self, world: [f32; 2], viewport: egui::Rect) -> [f32; 2] {
        let vw = viewport.width();
        let vh = viewport.height();
        let ndc_x = (world[0] - self.position[0]) * self.zoom_level / (vw * 0.5);
        let ndc_y = (self.position[1] - world[1]) * self.zoom_level / (vh * 0.5);
        [
            (ndc_x + 1.0) * 0.5 * vw + viewport.min.x,
            (ndc_y + 1.0) * 0.5 * vh + viewport.min.y,
        ]
    }
}

pub struct LoadingTask {
    pub label: String,
    pub detail: String,
    pub progress: f32,
    pub can_cancel: bool,
    pub cancel_flag: Arc<AtomicBool>,
}

pub struct PanelVisibility {
    pub analytics: bool,
    pub entity_inspector: bool,
    pub genome_inspector: bool,
    pub brain_inspector: bool,
    pub research: bool,
    pub profiler: bool,
    pub script_console: bool,
    pub db_console: bool,
}

impl Default for PanelVisibility {
    fn default() -> Self {
        Self {
            analytics: true, // Shown by default originally
            entity_inspector: false,
            genome_inspector: false,
            brain_inspector: false,
            research: true, // Shown by default originally
            profiler: false,
            script_console: false,
            db_console: false,
        }
    }
}

pub struct UiState {
    pub trail_decay: f32,
    pub bloom_threshold: f32,
    pub bloom_intensity: f32,
    pub ui_scale: f32,
    pub simulation_speed: f32,
    pub is_paused: bool,
    pub show_field_overlay: bool,
    pub show_trails: bool,
    pub show_species_colors: bool,
    pub show_grid: bool,
    pub show_sensor_cones: bool,
    pub show_disease_highlight: bool,
    pub panels: PanelVisibility,
    pub active_left_tab: usize,
    pub is_left_collapsed: bool,
    pub active_right_tab: usize,
    pub is_right_collapsed: bool,
    pub active_context_menu: Option<(egui::Pos2, Option<common::EntityId>)>,
    pub is_search_active: bool,
    pub search_query: String,
    pub selected_entities: Vec<EntityId>,
    pub camera: CameraState,
    pub active_modal: Option<UiModal>,
    pub god_mode_action_stack: Vec<GodModeAction>,
    pub god_mode_redo_stack: Vec<GodModeAction>,
    pub unsaved_changes: bool,
    pub active_loading_task: Option<LoadingTask>,
    pub app_tx: Option<std::sync::mpsc::Sender<crate::commands::AppCommand>>,
    pub task_tx: Option<std::sync::mpsc::Sender<LoadingTask>>,
    pub last_snapshot_path: Option<std::path::PathBuf>,
    pub active_experiment: Option<research::Experiment>,
    pub script_console_log: String,
    pub db_query_results: Option<Result<Vec<Vec<String>>, String>>,
    pub db_query_input: String,
    pub species_list: Vec<(u32, usize)>,
    pub viewport_rect: Option<egui::Rect>,
    pub last_mouse_pos: Option<[f32; 2]>,
    pub system_logs: Vec<String>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            trail_decay: 0.95,
            bloom_threshold: 1.2,
            bloom_intensity: 0.5,
            ui_scale: 1.0,
            simulation_speed: 1.0,
            is_paused: false,
            show_field_overlay: true,
            show_trails: true,
            show_species_colors: false,
            show_grid: false,
            show_sensor_cones: false,
            show_disease_highlight: true,
            panels: PanelVisibility::default(),
            active_left_tab: 0,
            is_left_collapsed: false,
            active_right_tab: 0,
            is_right_collapsed: false,
            active_context_menu: None,
            is_search_active: false,
            search_query: String::new(),
            selected_entities: Vec::new(),
            camera: CameraState::default(),
            active_modal: None,
            god_mode_action_stack: Vec::new(),
            god_mode_redo_stack: Vec::new(),
            unsaved_changes: false,
            active_loading_task: None,
            app_tx: None,
            task_tx: None,
            last_snapshot_path: None,
            active_experiment: None,
            script_console_log: String::new(),
            db_query_results: None,
            db_query_input: String::new(),
            species_list: Vec::new(),
            viewport_rect: None,
            last_mouse_pos: None,
            system_logs: Vec::new(),
        }
    }
}
