use crate::modal::UiModal;
use common::EntityId;
use std::sync::{atomic::AtomicBool, Arc};

#[derive(Clone, Debug)]
pub struct GodModeAction {
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct CameraState {
    pub zoom: f32,
    pub offset: [f32; 2],
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            offset: [0.0, 0.0],
        }
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
    pub simulation_speed: f32,
    pub is_paused: bool,
    pub show_field_overlay: bool,
    pub show_trails: bool,
    pub show_species_colors: bool,
    pub show_grid: bool,
    pub show_sensor_cones: bool,
    pub show_disease_highlight: bool,
    pub panels: PanelVisibility,
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
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            simulation_speed: 1.0,
            is_paused: false,
            show_field_overlay: true,
            show_trails: true,
            show_species_colors: false,
            show_grid: false,
            show_sensor_cones: false,
            show_disease_highlight: true,
            panels: PanelVisibility::default(),
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
        }
    }
}
