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
    /// The entity currently selected by the user, if any — the "primary"
    /// selection. Every pre-Phase-2 call site keeps reading/writing this
    /// field exactly as before; it is unchanged by the addition of
    /// `secondary_selected` below (see `docs/design/layout.md`'s Phase 2
    /// UI Architecture note on the shared selection model).
    pub selected_entity: Option<Entity>,
    /// Additional entities selected alongside `selected_entity` (Phase 2,
    /// M7) — populated by multi-select interactions (marquee-select,
    /// ctrl+click) that didn't exist before this milestone. Empty for every
    /// ordinary single-click selection, so single-select behavior is
    /// unaffected. Use [`WorkbenchState::all_selected`] to iterate the full
    /// selection (primary + secondary) rather than reading this field
    /// directly.
    pub secondary_selected: std::collections::HashSet<Entity>,
    /// The entity currently under the mouse cursor, if any (viewport-picked
    /// — see `app::events.rs`'s per-frame `pick_entity` call, which
    /// overwrites this unconditionally, so it cannot double as a hover
    /// signal set by other panels).
    pub hovered_entity: Option<Entity>,
    /// An entity a non-viewport panel wants highlighted this frame (Phase
    /// 2, M9 — hover cross-highlight), e.g. hovering a row in the Lineage
    /// Explorer. Reset to `None` at the start of every `render_ui` call and
    /// set by whichever panel's row the cursor is over; combined with
    /// `hovered_entity` (via `.or()`) wherever the viewport decides what to
    /// highlight, so the two never need to be the same field.
    pub panel_hover_entity: Option<Entity>,
    /// The last few distinct entities `selected_entity` has pointed at, most
    /// recent first, capped at [`RECENT_SELECTIONS_CAPACITY`] (Phase 2, M13
    /// — "Recent Selections"). Updated once per frame by
    /// `render::track_recent_selections` diffing `selected_entity` against
    /// the previous frame — deliberately not updated at each of
    /// `selected_entity`'s ~20 existing write sites directly, so none of
    /// them needed to change for this feature to work.
    pub recent_selections: std::collections::VecDeque<Entity>,
    /// `selected_entity`'s value as of the end of the previous frame, used
    /// only by `render::track_recent_selections` to detect a change.
    pub(crate) previous_selected_entity: Option<Entity>,
    /// Screen-space anchor of an in-progress marquee-select drag in the
    /// viewport (Phase 2, M8), set once on `drag_started_by` and cleared on
    /// `drag_stopped_by` — tracked explicitly rather than relying on
    /// `Response::interact_pointer_pos()` remaining valid across the exact
    /// frame the drag ends.
    pub marquee_drag_start: Option<egui::Pos2>,
    /// The cursor's current world-space position while over the viewport,
    /// or `None` when the cursor is elsewhere (Phase 2, M10) — a baseline
    /// "scientific tool" affordance (Blender/RenderDoc/ParaView all show
    /// this) that was previously entirely absent; the status bar showed
    /// only the *camera's* position, never the cursor's.
    pub cursor_world_pos: Option<common::Vec2>,
    /// Whether the viewport's click-drag currently measures distance
    /// (Phase 2, M11) instead of marquee-selecting — mutually exclusive
    /// with M8's marquee-select on the same drag gesture, toggled from the
    /// toolbar.
    pub measure_mode: bool,
    /// The last completed measurement: `(start, end, distance)` in world
    /// units, persisted (not just shown during the drag) until the next
    /// measurement or a mode toggle clears it.
    pub measure_result: Option<(common::Vec2, common::Vec2, f32)>,
    /// Saved camera views (Phase 2, M12), most-recently-added last.
    /// Session-only by design — see `CameraBookmark`'s doc comment.
    pub bookmarks: Vec<crate::CameraBookmark>,
    /// Whether the Command Palette overlay is open (Phase 2, M15).
    pub show_command_palette: bool,
    /// The Command Palette's current search text.
    pub command_palette_query: String,
    /// `Some(prior panel_modes)` while Focus Mode (Phase 2, M16) is active,
    /// restored on exit — `None` means Focus Mode is off. A toggle, not a
    /// fourth `LayoutPreset`, since (unlike the 3 named presets, which are
    /// one-way resets) it needs to remember and restore whatever arrangement
    /// was active before it was turned on.
    pub focus_mode_previous: Option<std::collections::HashMap<String, PanelMode>>,
    /// Whether the viewport minimap overlay is shown (Phase 2, M17).
    pub show_minimap: bool,
    /// Whether Spotlight mode is active (Phase 5, SX-5b) — dims every
    /// organism except the selected entity, its connected body/colony
    /// (reusing the same BFS `render.rs`'s selection highlight already
    /// computes), and any other organism within its interaction radius.
    /// Deliberately **not** named "Focus Mode" — that name is already taken
    /// by `focus_mode_previous`/`layout::toggle_focus_mode` (Phase 2, M16),
    /// an unrelated panel-layout fullscreen-viewport toggle. Reusing the
    /// same name for a different concept would be a real, avoidable
    /// confusion; picked a distinct one instead.
    pub spotlight_mode: bool,
    /// Whether High Contrast Mode is active (Phase 2, M18 — Accessibility
    /// pass 2). Applied every frame via `theme::apply_style`.
    pub high_contrast: bool,
    /// Global UI scale factor (Phase 2, M18), applied via
    /// `egui::Context::set_zoom_factor` every frame — scales the whole
    /// interface (fonts, spacing, icons together), which egui's own zoom
    /// mechanism already handles correctly, rather than reimplementing a
    /// font-only scale that would leave layouts inconsistent at non-1.0
    /// values.
    pub ui_scale: f32,
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
    /// Recently opened/saved items, by category (Phase 7, W0d) — see
    /// `crate::recent_items`'s module doc comment for the ordering/
    /// duplicate/cap/persistence policy. Replaces the pre-W0d
    /// `recent_files: Vec<String>`, which nothing ever populated.
    pub recent_items: crate::RecentItemsService,

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
    /// Bounded recent-position history for whichever entity `tracked_entity`
    /// currently points at (Phase 5, SX-4c — Inspector's "Relationships/
    /// History" section). Reset whenever `tracked_entity` changes; sampled
    /// once per *simulation tick* (not per render frame) by
    /// `render::track_trajectory_history`, the same "diff once per frame,
    /// don't touch existing write sites" pattern `track_recent_selections`
    /// already established. This is UI-side derived history, not
    /// simulation state — it cannot be read live from `world::World` (which
    /// has no memory of past positions), so caching it here is a deliberate,
    /// narrow exception to this crate's usual "never cache simulation data"
    /// rule, the same exception `recent_selections` already is.
    pub trajectory_history: std::collections::VecDeque<common::Vec2>,
    /// Which entity `trajectory_history` belongs to, and the last simulation
    /// tick a sample was recorded for — both used only by
    /// `render::track_trajectory_history` to detect an entity change (reset)
    /// or a new tick (sample).
    pub(crate) trajectory_entity: Option<Entity>,
    pub(crate) trajectory_last_tick: Option<u64>,
    /// Which physiology layer's viewport overlay is active, if any (Phase 4,
    /// P4-V2) — see `crate::types::PhysiologyOverlayLayer`.
    pub physiology_overlay: Option<crate::types::PhysiologyOverlayLayer>,
    /// Whether the simulation is currently paused.
    pub is_paused: bool,
    /// Whether to show the About dialog.
    pub show_about: bool,
    /// Whether to show the Documentation dialog.
    pub show_docs: bool,
    /// Whether to show the Keybinds dialog.
    pub show_keybinds: bool,
    /// Whether to show the first-run onboarding hints dialog (Phase 5,
    /// SX-9a). Defaults `false` here (not at construction time — see below)
    /// and is set `true` by `MenuAction::StartSimulation`'s handler
    /// (`crates/app/src/events.rs`) the moment the user actually reaches
    /// the simulation view — `show_dialogs` also renders while
    /// `AppState::MainMenu` is active, and this dialog references the
    /// viewport, which doesn't exist yet there. Re-openable afterward via
    /// Help → Welcome Tips, same as About/Docs/Keybinds.
    ///
    /// Phase 6, Epic J: that handler now gates the `true` assignment on
    /// the `app` crate's `preferences::Preferences::onboarding_seen` (a
    /// separate, persisted `.ron` flag — this field alone was always
    /// session-scoped by construction and cannot itself remember across
    /// restarts), closing the "session-scoped only" limitation SX-9a
    /// originally disclosed.
    pub show_onboarding_hints: bool,
    /// Whether to draw organism vision-cone overlays.
    pub show_vision_cones: bool,
    /// Whether to draw organism name labels in the viewport (Phase 5,
    /// SX-5a) — opt-in and density-aware: even when enabled, only the
    /// selected/tracked organism plus the nearest `ORGANISM_LABEL_MAX_COUNT`
    /// others to the camera center are labeled (`render::render_organism_labels`),
    /// never the whole population — labeling every organism at typical
    /// scales (hundreds to thousands) would be unreadable clutter, not a
    /// signal, the exact failure mode this milestone's own name warns
    /// against.
    pub show_organism_labels: bool,
    /// Whether to draw the world boundary outline (visual only — the
    /// simulation always hard-reflects organisms at the same bounds).
    pub show_world_boundary: bool,
    /// Whether to draw the low-opacity world-space scale grid (see
    /// `render::render_scale_grid`). Defaults on (a permanent scale
    /// reference was the audit's finding), but toggleable since a research
    /// screenshot/recording may want a clean, grid-free viewport.
    pub show_scale_grid: bool,
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
    /// Which view the Lineage tab shows (Ancestry tree vs. Species groups).
    pub lineage_view: crate::LineageView,
    /// Quick Organism Search text (Phase 2, M13) — filters the Lineage tab's
    /// organism list by entity/species/diet substring match.
    pub lineage_search: String,
    /// The body position (index into `0..organisms::MAX_SEGMENTS`) currently
    /// expanded in the HOX Visualizer tab's detail view (Phase 3, M10).
    pub hox_visualizer_selected_index: Option<usize>,
    /// The Replay Browser panel's currently-loaded bundle summary, if any
    /// (`None` until the user opens one via `MenuAction::OpenReplayBundle`).
    pub replay_browser: Option<crate::ReplayBrowserSummary>,
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
    /// GRN Viewer's graph pan/zoom (Phase 3, M11).
    pub grn_view: GraphViewState,
    /// GRN Viewer's selected body position — which position's morphogen
    /// inputs feed the displayed `RegulatoryNetwork` (Phase 3, M11).
    pub grn_position: usize,
    /// GRN Viewer's selected developmental step (time playback scrubber,
    /// `0..=genetics::develop::DEVELOPMENT_STEPS`) — Phase 3, M11.
    pub grn_step: usize,
    /// Evolution Debugger's explicitly-picked comparison organism
    /// ("Organism B") — `None` means "use Organism A's lineage parent",
    /// the default (Phase 3, M12).
    pub evo_debugger_entity_b: Option<Entity>,
    /// Evolution Debugger's organism-picker search text (filters the live
    /// organism list by entity id substring) — Phase 3, M12.
    pub evo_debugger_search: String,
    /// Shared Development Timeline scrubber position (an index into the
    /// selected organism's actual grown-position sequence, not a raw body
    /// position) — shared between the HOX Visualizer and GRN Viewer tabs
    /// so scrubbing in one carries over to the other (Phase 3, M13).
    pub timeline_step: usize,

    /// Last-known split ratio for each named docking split, keyed by the
    /// child tile's label (`"Sidebar"`, `"MainColumn"`, `"Neural Viewer"`,
    /// `"Viewport"`, `"BottomTabs"` — see `layout::extract_shares`).
    /// Captured from the live tree every frame and fed back into
    /// `layout::rebuild_tree_from_modes` so a user's dragged split survives
    /// a dock/undock/reset-triggered rebuild instead of snapping back to the
    /// hardcoded default ratio every time.
    pub layout_shares: std::collections::HashMap<String, f32>,

    /// Metrics panel's per-series visibility toggles and running-mean
    /// overlays (Phase 5, SX-7b) — pure display preference, not simulation
    /// data, so it lives here rather than in `analytics::MetricsState`.
    pub metrics_options: MetricsSeriesOptions,
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

/// Metrics panel's per-series visibility toggles and running-mean overlays
/// (Phase 5, SX-7b), for the Demographics (5 diet series) and Diversity (4
/// index series) plots — the two the roadmap names as needing toggles/stats.
/// Every field defaults to visible/off so the plots look unchanged until a
/// user actually opens the new controls.
#[derive(Debug, Clone, Copy)]
pub struct MetricsSeriesOptions {
    /// Demographics plot: Producers/Herbivores/Carnivores/Omnivores/Decomposers.
    pub demographics_visible: [bool; 5],
    /// Diversity plot: Shannon/Simpson/Richness/Turnover.
    pub diversity_visible: [bool; 4],
    /// Overlay a running-mean line (see `metrics::RUNNING_MEAN_WINDOW`) atop
    /// each visible Demographics series.
    pub demographics_running_mean: bool,
    /// Same overlay, for the Diversity plot's visible series.
    pub diversity_running_mean: bool,
}

impl Default for MetricsSeriesOptions {
    fn default() -> Self {
        Self {
            demographics_visible: [true; 5],
            diversity_visible: [true; 4],
            demographics_running_mean: false,
            diversity_running_mean: false,
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
        let mode = if name == "Neural Viewer"
            || name == "Research Dashboard"
            || name == "Replay Browser"
            || name == "Evolution Debugger"
            || name == "Placeholder Panel"
        {
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
            secondary_selected: std::collections::HashSet::new(),
            hovered_entity: None,
            panel_hover_entity: None,
            recent_selections: std::collections::VecDeque::new(),
            previous_selected_entity: None,
            marquee_drag_start: None,
            cursor_world_pos: None,
            measure_mode: false,
            measure_result: None,
            bookmarks: Vec::new(),
            show_command_palette: false,
            command_palette_query: String::new(),
            focus_mode_previous: None,
            show_minimap: true,
            spotlight_mode: false,
            high_contrast: false,
            ui_scale: 1.0,
            simulation_speed: 1.0,
            playback_state: PlaybackState::Paused,

            dock_tree,
            sidebar_visible: true,
            inspector_visible: true,
            metrics_visible: true,
            event_log_visible: true,
            status_bar_visible: true,
            toolbar_visible: true,

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
            recent_items: crate::RecentItemsService::default(),

            event_log_search: String::new(),
            event_log_filter: EventLogFilter::All,
            event_log_auto_scroll: true,

            tracked_entity: None,
            trajectory_history: std::collections::VecDeque::new(),
            trajectory_entity: None,
            trajectory_last_tick: None,
            physiology_overlay: None,
            is_paused: false,
            show_about: false,
            show_docs: false,
            show_keybinds: false,
            show_onboarding_hints: false,
            show_vision_cones: false,
            show_organism_labels: false,
            show_world_boundary: false,
            show_scale_grid: false,
            recording_active: false,
            recording_started_at: None,
            activity_bar_expanded: true,
            neural_ctrnn_view: GraphViewState::default(),
            neural_cppn_view: GraphViewState::default(),
            grn_view: GraphViewState::default(),
            grn_position: 0,
            grn_step: genetics::develop::DEVELOPMENT_STEPS,
            evo_debugger_entity_b: None,
            evo_debugger_search: String::new(),
            timeline_step: 0,
            layout_shares: std::collections::HashMap::new(),
            metrics_options: MetricsSeriesOptions::default(),
            active_tab: crate::SidebarTab::Inspector,
            lineage_view: crate::LineageView::Ancestry,
            lineage_search: String::new(),
            hox_visualizer_selected_index: None,
            replay_browser: None,
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

/// Maximum number of entities kept in `WorkbenchState::recent_selections`.
pub const RECENT_SELECTIONS_CAPACITY: usize = 8;

/// Maximum number of position samples kept in
/// `WorkbenchState::trajectory_history` (Phase 5, SX-4c) — at one sample
/// per simulation tick and the default 60 Hz tick rate, this covers the
/// last 5 seconds of real-time movement, a "recent trajectory," not a
/// full-lifetime path (which would need unbounded memory for a
/// long-running organism).
pub const TRAJECTORY_HISTORY_CAPACITY: usize = 300;

/// Maximum number of non-selected/tracked organisms labeled at once by
/// `render::render_organism_labels` (Phase 5, SX-5a) — the "density-aware"
/// half of "opt-in, density-aware" labels: bounded regardless of total
/// population, so enabling labels at 1000+ organisms doesn't render 1000+
/// labels.
pub const ORGANISM_LABEL_MAX_COUNT: usize = 20;

impl WorkbenchState {
    /// Every currently-selected entity: the primary `selected_entity` plus
    /// every entity in `secondary_selected` (Phase 2, M7). New multi-select
    /// consumers (marquee-select, quick search's "select all matches")
    /// should iterate this rather than reading `selected_entity` alone.
    pub fn all_selected(&self) -> impl Iterator<Item = Entity> + '_ {
        self.selected_entity
            .into_iter()
            .chain(self.secondary_selected.iter().copied())
    }

    /// Whether `entity` is part of the current selection (primary or
    /// secondary).
    pub fn is_selected(&self, entity: Entity) -> bool {
        self.selected_entity == Some(entity) || self.secondary_selected.contains(&entity)
    }

    /// Clears both the primary and secondary selection, and stops
    /// following whatever was selected — Phase 7, W0b: there is nothing
    /// left to follow once the selection is cleared, and this is the
    /// counterpart to [`WorkbenchState::select`] for every caller that
    /// wants to fully reset selection state in one call rather than
    /// clearing `tracked_entity` separately (see that method's doc comment
    /// for why `tracked_entity` has its own dedicated setter otherwise).
    pub fn clear_selection(&mut self) {
        self.selected_entity = None;
        self.secondary_selected.clear();
        self.tracked_entity = None;
    }

    /// Replaces the entire selection with `entities` — the first becomes
    /// the new primary `selected_entity` (so single-entity readers like the
    /// Inspector keep working unchanged), the rest become
    /// `secondary_selected`. Used by marquee-select (M8); an empty iterator
    /// clears the selection. Phase 7, W0b: also opens the Inspector and
    /// reveals the sidebar when the selection is non-empty, matching
    /// [`WorkbenchState::select`]'s behavior — every selection entry point
    /// should produce the same visible result, not just single-entity ones.
    pub fn select_multiple(&mut self, entities: impl IntoIterator<Item = Entity>) {
        let mut iter = entities.into_iter();
        self.selected_entity = iter.next();
        self.secondary_selected = iter.collect();
        if self.selected_entity.is_some() {
            self.active_tab = crate::SidebarTab::Inspector;
            self.sidebar_visible = true;
        }
    }

    /// # The single selection pathway (Phase 7, W0b)
    ///
    /// Every selection source — viewport click, the context menu's
    /// "Inspect" action, recent-selection chips, the Evolution Debugger's
    /// failure list, and any future source (global search, the lineage
    /// explorer, 3D picking) — should call this rather than setting
    /// `selected_entity`/`active_tab`/`sidebar_visible` directly. Before
    /// this milestone, viewport left-click and the context menu's
    /// "Inspect" button independently implemented overlapping-but-
    /// different versions of "select and show the Inspector" (see
    /// `PHASE7_WORKBENCH_ROADMAP.md`'s W0a finding #1) — this method is
    /// the fix, not a parallel third implementation.
    ///
    /// Deliberately never touches `tracked_entity`: selecting something to
    /// look at it and telling the camera to permanently follow it are two
    /// different intents (W0a finding #2). Camera-follow is only ever set
    /// via [`WorkbenchState::set_follow`], called from an explicit Follow
    /// action (toolbar button, Inspector "Track" checkbox, or the context
    /// menu's "Track / Follow" item).
    ///
    /// TODO(Phase 8): once a real cross-crate event channel exists, this
    /// should emit a `SelectionChanged { old, new }` event instead of (or
    /// in addition to) mutating state directly, so consumers other than
    /// egui widgets (e.g. a future 3D viewport, or an out-of-process
    /// research tool) can react to selection changes without polling
    /// `WorkbenchState` every frame. Not implemented now — no event bus
    /// exists in this crate yet, and inventing one solely for this would
    /// be exactly the kind of premature architecture this project's own
    /// discipline avoids. Left as a note for whoever scopes that bus.
    pub fn select(&mut self, entity: Entity) {
        self.selected_entity = Some(entity);
        self.secondary_selected.clear();
        self.active_tab = crate::SidebarTab::Inspector;
        self.sidebar_visible = true;
    }

    /// The single camera-follow pathway (Phase 7, W0b) — the only method
    /// that should ever set `tracked_entity`. Independent of selection:
    /// following an entity does not require it to be `selected_entity`
    /// (the spectator-mode "most interesting organism" logic in
    /// `crate::render` follows an entity that was never explicitly
    /// selected at all), and selecting an entity does not start following
    /// it (see [`WorkbenchState::select`]'s doc comment).
    ///
    /// TODO(Phase 8): same note as `select` — a future `FollowChanged {
    /// old, new }` event would let camera-follow consumers (e.g. a 3D
    /// camera rig) react without polling. Not implemented now, same
    /// reasoning: no event bus exists yet to hang it on.
    pub fn set_follow(&mut self, entity: Option<Entity>) {
        self.tracked_entity = entity;
    }

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

// Phase 6, Epic J: `WorkbenchCommand` (a ~90-line, fully parallel catalog of
// UI-dispatchable commands — Undo/Redo/DuplicateSelected/FocusSelection/etc.)
// was removed from here. Confirmed via a workspace-wide search that nothing
// anywhere ever constructed, matched, or otherwise consumed it — a dead
// enum, not a partially-wired one. It appears to have been an early sketch
// superseded by `MenuAction` (`types.rs`) and the command palette's own
// `(&str, MenuAction)` list (`plugins/command_palette.rs`), left behind
// rather than deleted when that happened.
