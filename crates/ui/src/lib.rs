//! # Phylon UI
//!
//! `egui`-based research interface: entity inspector, analytics dashboard,
//! experiment controls, replay timeline, and debug overlay toggles.
//!
//! The UI crate renders on top of the simulation frame using egui's wgpu
//! backend. It reads from the simulation state (via shared snapshots) and
//! publishes intervention events to the event bus.

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod shortcuts;
/// UI state types and enums.
pub mod types;
pub use types::{
    ActiveHeatmap, AppState, BottomTab, CameraBookmark, CanvasInteraction, HeatmapState,
    LineageView, MenuAction, ReplayBrowserSummary, SidebarTab, UiError,
};

pub mod layout;
/// Workbench UI state (dock tree, panel visibility, playback, toasts).
pub mod state;
pub use state::{
    EventLogFilter, PanelMode, PlaybackState, Toast, ToastSeverity, WorkbenchState, Workspace,
};

/// Reusable recent-items tracking (Phase 7, W0d) — see its own module doc
/// comment for the ordering/duplicate/cap/persistence policy.
pub mod recent_items;
pub use recent_items::{RecentCategory, RecentItemsService};

/// Per-panel UI plugins (sidebar, viewport, metrics, event log, menu, etc.).
pub mod plugins;

/// Shared design tokens — fonts, spacing, and global style.
pub mod theme;

/// UI helper utilities.
pub mod utils;

/// Shared, reusable UI primitives (kv_row, chart_legend_dot, empty/error
/// states) — see `docs/design/components.md` for the full catalog.
pub mod widgets;

/// Shared node-link graph canvas helpers (pan/zoom/hit-testing), used by
/// Neural Viewer and, as of Phase 3 M11, the GRN Viewer.
pub(crate) mod graph_canvas;

/// Shared `RegulatoryNetwork` display helpers, used by the GRN Viewer
/// (Phase 3 M11) and Evolution Debugger (Phase 3 M12).
pub(crate) mod regulatory_view;

/// Shared Development Timeline scrubber (Phase 3 M13), used by HOX
/// Visualizer and GRN Viewer.
pub(crate) mod timeline;

/// Immediate-mode rendering logic.
pub mod render;
pub use render::render_ui;
