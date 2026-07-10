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

/// Which of the three mutually-exclusive gestures a left-button viewport
/// drag currently performs (Phase 8, Epic 8.4 — adds `Lasso` alongside the
/// pre-existing `Select`/`Measure` pair, replacing the previous
/// `measure_mode: bool` now that there are 3 states, not 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarqueeMode {
    /// Rectangular frustum-based box-select (Phase 2 M8; Phase 8 Epic 8.4
    /// upgraded this from a flat Z=0-plane rectangle to a real screen-space
    /// frustum test).
    #[default]
    Select,
    /// Freeform polygon lasso-select (Phase 8, Epic 8.4 — new).
    Lasso,
    /// Distance measurement (Phase 2, M11).
    Measure,
}

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
    /// Per-position Hox combinatorial code, decoded segment identity, and
    /// morphogen gradients for the selected organism (Phase 3, M10).
    HoxVisualizer,
    /// Regulatory network graph, developmental-step time playback, and
    /// mutation-vs-parent comparison for the selected organism (Phase 3, M11).
    GrnViewer,
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
    /// World-space camera eye position at the time of saving (Phase 8,
    /// ADR-P8-02 — widened from `Vec2` alongside `orientation` below;
    /// `zoom: f32` is dropped, superseded by `Camera3d`'s FOV/distance
    /// model).
    pub position: common::Vec3,
    /// World-space camera orientation at the time of saving.
    pub orientation: common::Quat,
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
    /// The screen-space delta for a left-button pan/drag gesture this frame.
    pub drag_delta: egui::Vec2,
    /// The screen-space delta for a middle-button orbit/look gesture this
    /// frame (Phase 8, ADR-P8-02) — drives `OrbitController::orbit` or
    /// `FlyController::look` depending on the active camera mode. Kept as a
    /// separate field from `drag_delta` (rather than overloading it) since
    /// the two buttons drive genuinely different camera operations that can
    /// both be in flight independently.
    pub rotate_delta: egui::Vec2,
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
            rotate_delta: egui::Vec2::ZERO,
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
    /// Load a simulation state from a specific, already-known path (Phase
    /// 7, W0d) — used by the "Open Recent" menu, which (unlike
    /// `LoadState`) must load the exact entry the user clicked rather than
    /// opening a fresh file picker. Handled gracefully if the path no
    /// longer exists (see `crates/ui/src/recent_items.rs`'s missing-file
    /// policy) — never a panic.
    LoadStateFromPath(String),
    /// Export a named saved workspace to a `.ron` file (Phase 7, W3c) — the
    /// only workspace-lifecycle operation that needs `app`-crate file I/O;
    /// every other lifecycle operation (save/rename/duplicate/delete/
    /// apply/reset) only touches `WorkbenchState` and is called directly,
    /// no `MenuAction` round-trip, matching `layout::apply_layout_preset`'s
    /// existing precedent.
    ExportWorkspace(String),
    /// Import a workspace from a `.ron` file (Phase 7, W3c). The imported
    /// layout is sanitized before being added as a saved workspace — see
    /// `ui::workspace::WorkspaceLayout::sanitized`'s doc comment.
    ImportWorkspace,
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
    /// Re-open the first-run onboarding hints dialog (Phase 5, SX-9a) —
    /// Help → Welcome Tips, same re-open pattern as `ShowAbout`/`ShowDocumentation`.
    ShowOnboardingHints,
    /// Zoom camera in.
    CameraZoomIn,
    /// Zoom camera out.
    CameraZoomOut,
    /// Reset camera view.
    CameraHome,
    /// Toggle between Orbit (default) and Fly camera modes (Phase 8,
    /// ADR-P8-02).
    ToggleCameraMode,
    /// Transition to Simulation State
    StartSimulation,
    /// Transition to Main Menu State
    GoToMainMenu,
    /// Quit the application.
    Quit,

    // Canvas Shortcuts
    /// Delete the selected entity.
    DeleteSelection,
    /// Toggle whether the selected entity is fixed in place.
    ToggleStationary,
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
    /// Open a save dialog and export the `lineages` SQLite table to CSV
    /// (Phase 2, M14).
    ExportLineagesCsv,
    /// Open a save dialog and export the `events` SQLite table to CSV
    /// (Phase 2, M14).
    ExportEventsCsv,
    /// Open a save dialog and export a fresh organism snapshot to CSV
    /// (Phase 2, M14).
    ExportOrganismsCsv,
    /// Open a save dialog and export `MetricsState` history to CSV
    /// (Phase 2, M14).
    ExportMetricsCsv,
    /// Open a save dialog and export `MetricsState` history to JSON
    /// (Phase 2, M14).
    ExportMetricsJson,
    /// Save one Metrics chart as a publication-quality PNG (Phase 5, SX-7c).
    /// The rect is the chart's screen area in *physical pixels* (already
    /// converted from egui's logical points via `ctx.pixels_per_point()` at
    /// the call site in `metrics.rs`), since the actual capture crops the
    /// swapchain texture read back in `crates/app/src/render.rs` — the same
    /// GPU readback `TakeScreenshot` already uses, just cropped to one
    /// chart's rect instead of the whole window. Deferred to next-frame
    /// capture the same way `TakeScreenshot` is (see `pending_chart_export`
    /// in `crates/app/src/app.rs`), so the rect must describe this frame's
    /// layout, not a stale one.
    ExportChartPng {
        /// Crop origin X, physical pixels.
        x: u32,
        /// Crop origin Y, physical pixels.
        y: u32,
        /// Crop width, physical pixels.
        width: u32,
        /// Crop height, physical pixels.
        height: u32,
    },
    /// Toggle the Command Palette overlay (Phase 2, M15).
    ToggleCommandPalette,
    /// Toggle the Global Search overlay (Phase 7, W6a).
    ToggleGlobalSearch,

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
    /// Select every organism whose head node projects into a screen-space
    /// rectangle (Phase 2, M8 — marquee-select; Phase 8 Epic 8.4 upgraded
    /// this from a flat Z=0-plane world-space rectangle to a real
    /// frustum-based test: each candidate's `Vec3` position is projected
    /// through the camera's own `view_proj` and tested against this
    /// rectangle in screen space, working correctly regardless of camera
    /// tilt or an entity's `Z`). `screen_min`/`screen_max` are viewport-
    /// local physical-pixel coordinates, normalized to true min/max before
    /// this is pushed; `viewport_size` is the viewport's pixel size at the
    /// time of the drag, needed to reconstruct the same projection.
    SelectInRect {
        /// Viewport-local minimum corner (smaller x, smaller y), in pixels.
        screen_min: common::Vec2,
        /// Viewport-local maximum corner (larger x, larger y), in pixels.
        screen_max: common::Vec2,
        /// The viewport's pixel size at the time of the drag.
        viewport_size: common::Vec2,
    },
    /// Select every organism whose head node projects inside a closed
    /// screen-space polygon (Phase 8, Epic 8.4 — lasso-select). `points`
    /// are viewport-local physical-pixel coordinates, in drag order (need
    /// not be explicitly closed); `viewport_size` is the viewport's pixel
    /// size at the time of the drag.
    SelectInLasso {
        /// Viewport-local polygon vertices, in pixels.
        points: Vec<common::Vec2>,
        /// The viewport's pixel size at the time of the drag.
        viewport_size: common::Vec2,
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

/// Which physiology layer's viewport overlay (Phase 4, P4-V2) is currently
/// active, if any — toggled from the corresponding P4-R1-R4 Viewer panel's
/// "Show on viewport" control; `None` shows nothing. See
/// `ui::render::render_physiology_overlay`'s doc comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysiologyOverlayLayer {
    /// Per-segment ATP level, from `metabolism::ChemicalEconomy` (P4-F2/F3).
    Circulation,
    /// Per-segment hormone channel intensity, from `brain::HormoneLevel` (P4-F4).
    Hormone,
    /// Per-segment infection severity, from `ecology::disease::SegmentInfection` (P4-F5).
    Immune,
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

/// UI-owned clipping-plane state (Phase 8, Epic 8.5, ADR-P8-05) — a
/// horizontal world-space `Z`-plane the organism renderer clips fragments
/// against, letting the user slice into a dense population to see inside
/// it. Plain UI state (like `HeatmapState`) rather than a `rendering` type,
/// since `ui` doesn't depend on `rendering` — `app`'s render loop converts
/// this into `rendering::ClipPlane` at the one call site that needs it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipPlaneState {
    /// Whether the clip test is active.
    pub enabled: bool,
    /// World-space `Z` height of the plane.
    pub height: f32,
    /// If `true`, geometry *above* `height` is kept; if `false`, geometry
    /// *below* it is kept.
    pub keep_above: bool,
}

impl Default for ClipPlaneState {
    fn default() -> Self {
        Self {
            enabled: false,
            height: 0.0,
            keep_above: true,
        }
    }
}
