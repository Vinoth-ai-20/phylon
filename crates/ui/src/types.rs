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
    /// Diffusion heatmaps and environmental overlays
    Environment,
    /// Global metrics and population charts
    Analytics,
    /// Entity Presets and Structure Generator
    Sandbox,
    /// Physics tuning and global parameters
    Tuning,
    /// Environmental data and cell info
    Ecology,
    /// Ancestry tree and species grouping over `evolution::LineageTracker`
    Lineage,
    /// Application Settings
    Settings,
}

/// Which view the Lineage tab's panel is currently showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineageView {
    /// Ancestry tree, rooted at organisms with no currently-alive parent.
    #[default]
    Ancestry,
    /// Organisms grouped by `evolution::SpeciesId`.
    Species,
}

/// A saved camera view (Phase 2, M12 — "Bookmarks"), session-only per the
/// Phase 2 roadmap's own risk note: there is no live tick-jumping in a
/// running simulation to meaningfully tie a bookmark to a specific tick
/// (replay's tick-seeking is a separate, non-interactive-UI mode — see
/// `ReplayBrowserSummary`'s doc comment), so a bookmark here is a saved
/// *camera position*, not a saved *moment in time*.
#[derive(Debug, Clone)]
pub struct CameraBookmark {
    /// User-facing label, e.g. "Predator cluster, south edge".
    pub label: String,
    /// World-space camera position at the time of saving.
    pub position: common::Vec2,
    /// Camera zoom at the time of saving.
    pub zoom: f32,
}

/// A lightweight, already-extracted summary of a loaded `.phylon-replay`
/// bundle for the Replay Browser panel (Phase 2, M6) — holds only what's
/// needed to browse its recorded interventions, not the full
/// `storage::replay::ReplayBundle` (which also carries a potentially large
/// `initial_snapshot`), so this is safe to keep in `WorkbenchState` unlike
/// live simulation data. Built in `app::events.rs` (the only place with a
/// `storage` dependency) and handed to the UI as plain data — `ui` itself
/// never depends on `storage`.
#[derive(Debug, Clone)]
pub struct ReplayBrowserSummary {
    /// The file path the bundle was loaded from, for display only.
    pub source_path: String,
    /// The RNG seed the recorded run started from.
    pub seed: u64,
    /// The tick of the last recorded event (0 if none were recorded).
    pub last_event_tick: u64,
    /// Every recorded event as `(tick, human-readable description)`, in
    /// chronological order.
    pub events: Vec<(u64, String)>,
}

/// The active tab in the bottom panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BottomTab {
    /// Metrics Dashboard (4-plot grid)
    #[default]
    Metrics,
    /// Event Log (recent births, deaths, hazard events)
    EventLog,
}

/// # Unified Viewport Interaction
///
/// ## 1. What Happens
/// `CanvasInteraction` captures all user input that occurred specifically within the
/// 3D/2D simulation view, avoiding UI panels.
///
/// ## 2. Why It Happens
/// If a user drags a slider in the inspector, they are clicking the mouse, but we don't
/// want the camera to pan. We must isolate input events that pass *through* the UI to
/// the underlying canvas.
///
/// ## 3. How It Happens
/// In `render_ui`, we allocate an `egui::CentralPanel`. We check `response.drag_delta()`
/// and `ui.input(|i| i.zoom_delta())` while ensuring the pointer is hovering the canvas rect.
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

/// # UI Command Dispatch
///
/// ## 1. What Happens
/// `MenuAction` is an enum of discrete commands (e.g., Save, Load, Spawn) returned by the UI.
///
/// ## 2. Why It Happens
/// `egui` requires mutable borrows to draw data, but executing a complex command (like
/// Reseeding the Ecosystem) requires a mutable borrow of the *entire* ECS World, which is
/// currently borrowed by the UI! `MenuAction` acts as a message queue to execute commands
/// *after* the UI finishes rendering.
///
/// ## 3. How It Happens
/// UI buttons push variants to a `Vec<MenuAction>`. The main loop receives this vector and
/// pattern-matches each action, safely applying structural changes to the ECS.
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
    /// Reseed the entire ecosystem
    ReseedEcosystem,
    /// Capture the current viewport as a PNG screenshot.
    TakeScreenshot,
    /// Start recording if not already recording, or stop and save (as an
    /// animated GIF) if a recording is in progress.
    ToggleRecording,
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
    /// Show keybinds.
    ShowKeybinds,
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

    // Viewport & Shortcut extensions
    /// Toggle between play and pause states.
    TogglePlayPause,
    /// Increase simulation speed by one step.
    SetSpeedUp,
    /// Decrease simulation speed by one step.
    SetSpeedDown,
    /// Toggle the Metrics panel visibility.
    ToggleMetrics,
    /// Toggle the Event Log panel visibility.
    ToggleLog,
    /// Toggle the Sidebar panel visibility.
    ToggleSidebar,
    /// Pan/zoom the viewport to the selected entity.
    FocusSelection,
    /// Open an import dialog for genome files.
    ImportGenome,
    /// Open an export dialog for the selected organism's genome.
    ExportGenome,
    /// Open a file dialog to load a `.phylon-replay` bundle for the Replay
    /// Browser panel (static inspection only — no live playback; replay
    /// execution is a separate headless mode, see `app::replay::run_replay`).
    OpenReplayBundle,
    /// Clear the Replay Browser's currently-loaded bundle summary.
    CloseReplayBundle,

    // Overlay — canonical command routed through HeatmapState
    /// Set the active simulation overlay (updates HeatmapState ECS resource).
    SetOverlay(ActiveHeatmap),
    /// Set the heatmap colormap variant (updates HeatmapState ECS resource).
    SetColormap(u32),

    // Entity interaction
    /// Kill (despawn) a specific entity.
    KillEntity(bevy_ecs::entity::Entity),
    /// Track (follow camera) a specific entity.
    TrackEntity(bevy_ecs::entity::Entity),
    /// Select a specific entity.
    SelectEntity(bevy_ecs::entity::Entity),
    /// Select every organism whose head node falls within a world-space
    /// rectangle (Phase 2, M8 — marquee-select). `min`/`max` are opposite
    /// corners in world space, not necessarily min-x/min-y vs. max-x/max-y
    /// at the call site, but normalized to true min/max before this is
    /// pushed.
    SelectInRect {
        /// World-space minimum corner (smaller x, smaller y).
        min: common::Vec2,
        /// World-space maximum corner (larger x, larger y).
        max: common::Vec2,
    },
    /// Copy an entity's ID to clipboard.
    CopyEntityId(bevy_ecs::entity::Entity),

    // Selection by diet
    /// Select first organism matching a given diet type.
    SelectByDiet(ecology::Diet),
    /// Invert the current selection.
    InvertSelection,
    /// Select the head node (`segment_type == 0`) of the organism identified
    /// by this `organism_id`.
    SelectHeadOf(u32),

    // Panel window management
    /// Move a named panel from Docked → Floating (pop it out of the tile tree).
    DetachPanel(String),
    /// Move a named panel from Floating/Closed → Docked (re-insert into tile tree).
    DockPanel(String),
    /// Move a named panel to Closed state (hidden; reopen via Windows menu).
    ClosePanel(String),
}

/// The currently active heatmap overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveHeatmap {
    /// No heatmap overlay.
    #[default]
    None,
    /// Glucose heatmap (splatted).
    Glucose,
    /// ATP heatmap (splatted).
    ATP,
    /// Pheromones heatmap (from diffusion grid).
    Pheromones,
    /// Energy Density heatmap (from diffusion grid).
    EnergyDensity,
    /// O2 concentration.
    O2,
    /// CO2 concentration.
    CO2,
}

/// ECS Resource storing the state of the heatmap UI and shader.
#[derive(bevy_ecs::prelude::Resource, Debug, Clone)]
pub struct HeatmapState {
    /// The currently selected heatmap.
    pub active: ActiveHeatmap,
    /// The global minimum value found in the active grid.
    pub min_val: f32,
    /// The global maximum value found in the active grid.
    pub max_val: f32,
    /// The index of the color mapping to use.
    pub colormap: u32,
}

impl Default for HeatmapState {
    fn default() -> Self {
        Self {
            active: ActiveHeatmap::None,
            min_val: 0.0,
            max_val: 1.0,
            colormap: 0,
        }
    }
}
