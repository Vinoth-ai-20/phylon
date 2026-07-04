use bevy_ecs::entity::Entity;
use egui_tiles::Tree;

/// Identifies the active workspace layout in the egui_tiles tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Workspace {
    /// Ecology view — organism populations and interactions.
    #[default]
    Ecology,
    /// Biology view — cellular physiology and metabolism.
    Biology,
    /// Evolution view — generational and mutation analytics.
    Evolution,
    /// Neural view — brain/CTRNN analysis.
    Neural,
    /// Genetics view — genome and CPPN graphs.
    Genetics,
    /// Rendering view — graphics and debug overlays.
    Rendering,
    /// Analytics view — time-series charts.
    Analytics,
    /// Performance view — framerate and ECS profiling.
    Performance,
    /// Debug view — raw ECS component inspection.
    Debug,
    /// Settings view — application configuration.
    Settings,
}

/// A transient notification message rendered as a toast card.
#[derive(Debug, Clone)]
pub struct Toast {
    /// The text to display.
    pub message: String,
    /// Determines color and icon.
    pub severity: ToastSeverity,
    /// Absolute time (from `state.time`) at which this toast should disappear.
    pub expires_at: f64,
}

/// Severity level for a toast notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastSeverity {
    /// Neutral information.
    Info,
    /// Operation completed successfully.
    Success,
    /// Non-blocking warning.
    Warning,
    /// Blocking error.
    Error,
}

/// Application playback state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackState {
    /// Simulation is paused; ticks do not advance.
    #[default]
    Paused,
    /// Simulation is running; ticks advance each frame.
    Playing,
}

/// Central state for the entire Workbench UI.
pub struct WorkbenchState {
    /// The active top-level workspace (Ecology, Biology, Neural, etc.).
    pub active_workspace: Workspace,
    /// The entity currently selected by the user, if any.
    pub selected_entity: Option<Entity>,
    /// The entity currently under the mouse cursor, if any.
    pub hovered_entity: Option<Entity>,
    /// Simulation speed multiplier (1.0 = real time).
    pub simulation_speed: f32,
    /// Whether the simulation is playing or paused.
    pub playback_state: PlaybackState,

    // Layout and docking
    /// The egui_tiles dock tree describing the current panel layout.
    pub dock_tree: Tree<String>,
    /// Whether the Sidebar panel is visible.
    pub sidebar_visible: bool,
    /// Whether the Inspector panel is visible.
    pub inspector_visible: bool,
    /// Whether the Metrics panel is visible.
    pub metrics_visible: bool,
    /// Whether the Event Log panel is visible.
    pub event_log_visible: bool,
    /// Whether the bottom status bar is visible.
    pub status_bar_visible: bool,
    /// Whether the top toolbar is visible.
    pub toolbar_visible: bool,
    /// Whether the navigation rail is visible.
    pub navigation_visible: bool,

    // Viewport
    /// World-space camera position.
    pub camera_pos: common::Vec2,
    /// Camera zoom factor (pixels per world unit).
    pub camera_zoom: f32,
    /// Whether the camera automatically follows the most interesting organism.
    pub spectator_mode: bool,
    /// Screen-space rect of the viewport canvas, if known this frame.
    pub canvas_rect: Option<[u32; 4]>,
    /// A world-space click that occurred this frame and has not yet been consumed.
    pub pending_click: Option<common::Vec2>,
    /// The current world-space mouse hover position, if hovering the canvas.
    pub current_hover_pos: Option<common::Vec2>,

    // Rendering parameters
    /// Whether to draw the raw structural (bone/spring) debug overlay.
    pub debug_structural: bool,
    /// Line thickness used when drawing structural bones.
    pub bone_line_thickness: f32,
    /// Thickness of the rendered organism skin/outline.
    pub skin_thickness: f32,
    /// Radius used when drawing structural nodes.
    pub node_radius: f32,

    // Input
    /// Current keyboard modifier state (ctrl/shift/alt/etc.).
    pub modifiers: egui::Modifiers,

    // Modals and Toasts (single source of truth)
    /// Names of currently open modal dialogs.
    pub open_dialogs: Vec<String>,
    /// The canonical notification queue. Use `push_toast()` to add entries.
    pub notifications: Vec<Toast>,
    /// Recently opened/saved file paths, most recent first.
    pub recent_files: Vec<String>,

    // Event log filter state
    /// Free-text search filter applied to the event log.
    pub event_log_search: String,
    /// Category filter applied to the event log.
    pub event_log_filter: EventLogFilter,
    /// Whether the event log auto-scrolls to the newest entry.
    pub event_log_auto_scroll: bool,

    // Tracked / selected
    /// The entity the camera is currently following, if any.
    pub tracked_entity: Option<Entity>,
    /// Whether the simulation is currently paused.
    pub is_paused: bool,
    /// Whether to show the About dialog.
    pub show_about: bool,
    /// Whether to show the Documentation dialog.
    pub show_docs: bool,
    /// Whether to show the Keybinds dialog.
    pub show_keybinds: bool,
    /// Whether to draw organism vision-cone overlays.
    pub show_vision_cones: bool,
    /// Whether to draw the world boundary outline (visual only — the
    /// simulation always hard-reflects organisms at the same bounds).
    pub show_world_boundary: bool,
    /// Whether a GIF recording is currently in progress. The actual frame
    /// buffer lives in `PhylonApp` (app crate) — this is just a lightweight
    /// mirror so the toolbar can show a recording indicator.
    pub recording_active: bool,
    /// Wall-clock time (`WorkbenchState::time`) the current recording
    /// started, for the toolbar's elapsed-time readout.
    pub recording_started_at: Option<f64>,
    /// Whether the left activity bar shows icon+label (discoverable, wider)
    /// or icon-only (compact, narrow — the previous permanent behavior).
    /// Defaults to expanded: an icon-only rail with only a hover tooltip was
    /// the audit's top discoverability finding for first-time users.
    pub activity_bar_expanded: bool,
    /// The active tab in the primary sidebar.
    pub active_tab: crate::SidebarTab,
    /// The active tab in the bottom panel.
    pub active_bottom_tab: crate::BottomTab,
    /// Time at which a quit confirmation was requested, for double-confirm UX.
    pub quit_confirm_time: Option<f64>,
    /// Time at which a return-to-main-menu confirmation was requested.
    pub main_menu_confirm_time: Option<f64>,
    /// Time at which spectator mode last switched its tracked entity.
    pub last_spectator_switch_time: f64,

    // Internal timing
    /// Current UI clock time, taken from `egui::InputState::time` each frame.
    pub time: f64,

    /// Keyboard shortcuts.
    pub shortcuts: crate::shortcuts::ShortcutManager,
    /// Visibility mode for each named panel (Docked / Floating / Closed).
    pub panel_modes: std::collections::HashMap<String, PanelMode>,
    /// Whether each floating panel is minimized (collapsed to title bar only).
    pub panel_minimized: std::collections::HashMap<String, bool>,
    /// Whether each floating panel was being dragged last frame, used to
    /// detect the drag-release transition for edge/corner snapping.
    pub floating_was_dragging: std::collections::HashMap<String, bool>,
    /// A one-shot corrected position to apply to a floating panel next frame
    /// after it snapped to a screen edge/corner on drag release.
    pub floating_snap_pos: std::collections::HashMap<String, egui::Pos2>,

    /// Neural Viewer's CTRNN (phenotype) graph pan/zoom — persisted across
    /// frames since the immediate-mode graph is otherwise stateless. A
    /// 40-hidden-node genome is unreadable at a fixed 1:1 scale; this is the
    /// Milestone 8 fix.
    pub neural_ctrnn_view: GraphViewState,
    /// Neural Viewer's CPPN (genotype) graph pan/zoom — separate from
    /// `neural_ctrnn_view` since the two graphs are independently sized and
    /// scrolled.
    pub neural_cppn_view: GraphViewState,

    /// Last-known split ratio for each named docking split, keyed by the
    /// child tile's label (`"Sidebar"`, `"MainColumn"`, `"Neural Viewer"`,
    /// `"Viewport"`, `"BottomTabs"` — see `layout::extract_shares`).
    /// Captured from the live tree every frame and fed back into
    /// `layout::rebuild_tree_from_modes` so a user's dragged split survives
    /// a dock/undock/reset-triggered rebuild instead of snapping back to the
    /// hardcoded default ratio every time.
    pub layout_shares: std::collections::HashMap<String, f32>,
}

/// Pan/zoom transform for one Neural Viewer graph canvas.
#[derive(Debug, Clone, Copy)]
pub struct GraphViewState {
    /// Scale factor applied to node positions/radii, anchored at `pan`.
    pub zoom: f32,
    /// Canvas-space offset added to every node position after scaling.
    pub pan: egui::Vec2,
}

impl Default for GraphViewState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
        }
    }
}

/// Filter level for the event log panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EventLogFilter {
    /// Show all events.
    #[default]
    All,
    /// Show only births.
    Births,
    /// Show only deaths.
    Deaths,
    /// Show only hazard events.
    Hazards,
    /// Show only user-triggered actions.
    UserActions,
}

/// Visibility mode for a named workspace panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanelMode {
    /// The panel is embedded in the egui_tiles docking tree.
    #[default]
    Docked,
    /// The panel is a free-floating `egui::Window` inside the OS window.
    Floating,
    /// The panel is hidden. Re-open it from the Windows menu.
    Closed,
}

/// The default visibility mode for every named panel: everything docked in
/// its canonical slot except "Neural Viewer", which starts closed. Shared by
/// `WorkbenchState::default()` and `layout::apply_default_layout()` so the
/// initial layout and a "Reset Layout" always agree.
pub fn default_panel_modes() -> std::collections::HashMap<String, PanelMode> {
    let mut m = std::collections::HashMap::new();
    for &name in crate::layout::ALL_PANEL_NAMES {
        let mode = if name == "Neural Viewer" || name == "Placeholder Panel" {
            PanelMode::Closed
        } else {
            PanelMode::Docked
        };
        m.insert(name.to_string(), mode);
    }
    m
}

impl Default for WorkbenchState {
    fn default() -> Self {
        let panel_modes = default_panel_modes();
        let dock_tree = {
            let mut tree = Tree::empty("workbench_tree");
            crate::layout::rebuild_tree_from_modes(
                &mut tree,
                &panel_modes,
                &std::collections::HashMap::new(),
            );
            tree
        };

        Self {
            active_workspace: Workspace::Ecology,
            selected_entity: None,
            hovered_entity: None,
            simulation_speed: 1.0,
            playback_state: PlaybackState::Paused,

            dock_tree,
            sidebar_visible: true,
            inspector_visible: true,
            metrics_visible: true,
            event_log_visible: true,
            status_bar_visible: true,
            toolbar_visible: true,
            navigation_visible: true,

            camera_pos: common::Vec2::ZERO,
            camera_zoom: 1.0,
            spectator_mode: false,
            canvas_rect: None,
            pending_click: None,
            current_hover_pos: None,

            debug_structural: false,
            bone_line_thickness: 1.0,
            skin_thickness: 3.0,
            node_radius: 5.0,

            modifiers: egui::Modifiers::default(),

            open_dialogs: Vec::new(),
            notifications: Vec::new(),
            recent_files: Vec::new(),

            event_log_search: String::new(),
            event_log_filter: EventLogFilter::All,
            event_log_auto_scroll: true,

            tracked_entity: None,
            is_paused: false,
            show_about: false,
            show_docs: false,
            show_keybinds: false,
            show_vision_cones: false,
            show_world_boundary: false,
            recording_active: false,
            recording_started_at: None,
            activity_bar_expanded: true,
            neural_ctrnn_view: GraphViewState::default(),
            neural_cppn_view: GraphViewState::default(),
            layout_shares: std::collections::HashMap::new(),
            active_tab: crate::SidebarTab::Inspector,
            active_bottom_tab: crate::BottomTab::Metrics,
            quit_confirm_time: None,
            main_menu_confirm_time: None,
            last_spectator_switch_time: 0.0,

            time: 0.0,

            shortcuts: crate::shortcuts::ShortcutManager::default(),
            panel_modes,
            panel_minimized: std::collections::HashMap::new(),
            floating_was_dragging: std::collections::HashMap::new(),
            floating_snap_pos: std::collections::HashMap::new(),
        }
    }
}

impl WorkbenchState {
    /// Push a transient notification. Displayed in the bottom-right toast area.
    pub fn push_toast(
        &mut self,
        message: impl Into<String>,
        severity: ToastSeverity,
        duration: f64,
    ) {
        self.notifications.push(Toast {
            message: message.into(),
            severity,
            expires_at: self.time + duration,
        });
    }

    /// Expire old toasts. Call once per frame.
    pub fn cleanup_toasts(&mut self) {
        let current_time = self.time;
        self.notifications.retain(|t| t.expires_at > current_time);
    }
}

/// Dispatched from the UI to the ECS Simulation.
#[derive(Debug, Clone)]
pub enum WorkbenchCommand {
    // Project / File
    /// Start a brand-new simulation, discarding the current one.
    NewSimulation,
    /// Open a previously saved simulation from disk.
    OpenSimulation,
    /// Save the current simulation to its existing project file.
    SaveProject,
    /// Save the current simulation to a new project file.
    SaveProjectAs,
    /// Import a genome file for the selected/spawned organism.
    ImportGenome,
    /// Export the selected organism's genome to disk.
    ExportGenome,
    /// Export the current CPPN/graph view to a file.
    ExportGraph,
    /// Capture a screenshot of the viewport.
    Screenshot,
    /// Start or stop recording the viewport.
    Recording,

    // Edit
    /// Undo the last action.
    Undo,
    /// Redo the last undone action.
    Redo,
    /// Copy the current selection.
    Copy,
    /// Paste the clipboard contents.
    Paste,
    /// Delete the currently selected entity.
    DeleteSelected,
    /// Duplicate the currently selected entity.
    DuplicateSelected,

    // Selection
    /// Select all entities.
    SelectAll,
    /// Select all entities belonging to a species.
    SelectSpecies,
    /// Select all producer organisms.
    SelectProducers,
    /// Select all herbivore organisms.
    SelectHerbivores,
    /// Select all carnivore organisms.
    SelectCarnivores,
    /// Select all omnivore organisms.
    SelectOmnivores,
    /// Invert the current selection.
    InvertSelection,
    /// Clear the current selection.
    ClearSelection,
    /// Pan/zoom the viewport to frame the current selection.
    FocusSelection,

    // Simulation
    /// Begin running the simulation.
    StartSimulation,
    /// Pause the running simulation.
    PauseSimulation,
    /// Stop the simulation entirely.
    StopSimulation,
    /// Restart the simulation from its initial state.
    RestartSimulation,
    /// Advance the simulation by exactly one tick.
    StepOneTick,
    /// Set the simulation speed multiplier.
    SetSimulationSpeed(f32),
    /// Set the random seed used for procedural generation.
    SetRandomSeed(u64),
    /// Reset the world to an empty state.
    ResetWorld,

    // Camera
    /// Reset the camera to its default position and zoom.
    ResetCamera,
    /// Center the camera on the world origin.
    CenterWorld,
    /// Make the camera follow the selected entity.
    FollowSelected,
    /// Toggle automatic spectator-mode camera following.
    ToggleSpectator,

    // Interaction
    /// Spawn a manually-triggered hazard at the cursor.
    SpawnManualHazard,
    /// Spawn a named preset organism at the cursor.
    SpawnPreset(String),
}
