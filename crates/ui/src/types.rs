/// Errors from the UI subsystem.
#[derive(Debug, thiserror::Error)]
pub enum UiError {
    /// An egui widget encountered an invalid state.
    #[error("UI state error: {message}")]
    StateError {
        /// Description of the invalid state.
        message: String,
    },
}

impl common::PhylonError for UiError {}

/// The active tab in the primary sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidebarTab {
    /// Inspect single organisms and the physical environment
    #[default]
    Inspector,
    /// View neural networks and genotypes
    Genetics,
    /// Global metrics and population charts
    Analytics,
    /// Entity Presets and Structure Generator
    Sandbox,
    /// Physics tuning and global parameters
    Tuning,
    /// Environmental data and cell info
    Ecology,
    /// Application Settings
    Settings,
}

/// Contains the screen-space rect of the transparent canvas area and the
/// unified touch/mouse/trackpad gesture interactions performed on it.
#[derive(Debug, Clone, Copy)]
pub struct CanvasInteraction {
    /// The screen-space bounding rect of the central canvas panel.
    pub rect: egui::Rect,
    /// True if the user tapped/clicked on the canvas this frame.
    pub clicked: bool,
    /// The screen-space coordinates of the tap/click, if `clicked` is true.
    pub click_pos: Option<egui::Pos2>,
    /// The screen-space coordinates of the mouse hover, if any.
    pub hover_pos: Option<egui::Pos2>,
    /// The screen-space delta for a pan/drag gesture this frame.
    pub drag_delta: egui::Vec2,
    /// The scale factor for a pinch-to-zoom or scroll-zoom gesture this frame (1.0 = no change).
    pub zoom_delta: f32,
}

impl Default for CanvasInteraction {
    fn default() -> Self {
        Self {
            rect: egui::Rect::NOTHING,
            clicked: false,
            click_pos: None,
            hover_pos: None,
            drag_delta: egui::Vec2::ZERO,
            zoom_delta: 1.0,
        }
    }
}

/// The current high-level state of the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppState {
    /// The main title screen.
    #[default]
    MainMenu,
    /// The active simulation sandbox.
    Simulation,
}

/// Menu actions returned by the UI layer to the main app loop.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuAction {
    /// Save the simulation state to disk.
    SaveState,
    /// Load a simulation state from disk.
    LoadState,
    /// Undo the last action.
    Undo,
    /// Redo the last undone action.
    Redo,
    /// Advance the simulation by one tick while paused.
    StepForward,
    /// Reset the simulation to default organisms.
    Reset,
    /// Reseed the entire ecosystem
    ReseedEcosystem,
    /// Select all or cycle through organisms.
    SelectAll,
    /// Clear the current selection.
    Deselect,
    /// Spawn a preset by name
    SpawnPreset(String),
    /// Generate a procedural hex mesh
    GenerateHexMesh {
        /// Number of columns
        cols: usize,
        /// Number of rows
        rows: usize,
        /// Spacing between nodes
        spacing: f32,
        /// Spring stiffness
        stiffness: f32,
        /// Are the nodes anchored
        is_fixed: bool,
    },
    /// Spawn a new proto-fish under the camera.
    SpawnProtoFish,
    /// Show the Phylon documentation.
    ShowDocumentation,
    /// Show the About Phylon dialog.
    ShowAbout,
    /// Zoom camera in.
    CameraZoomIn,
    /// Zoom camera out.
    CameraZoomOut,
    /// Reset camera view.
    CameraHome,
    /// Transition to Simulation State
    StartSimulation,
    /// Transition to Main Menu State
    GoToMainMenu,
    /// Quit the application.
    Quit,

    // Canvas Shortcuts
    /// Delete the selected entity.
    DeleteSelection,
    /// Duplicate the selected entity.
    DuplicateSelection,
    /// Spawn/paste a new entity from the clipboard.
    SpawnPaste,
    /// Toggle whether the selected entity is fixed in place.
    ToggleStationary,
    /// Join/link the selected entity.
    JoinSelection,
    /// Enter drag mode for the selected entity.
    GrabSelection,
    /// Spawn a localized catastrophe hazard.
    SpawnManualHazard,
}
