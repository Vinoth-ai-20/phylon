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
    ActiveHeatmap, AppState, BottomTab, CanvasInteraction, HeatmapState, MenuAction, SidebarTab,
    UiError,
};

pub mod layout;
/// Workbench UI state (dock tree, panel visibility, playback, toasts).
pub mod state;
pub use state::{
    EventLogFilter, PanelMode, PlaybackState, Toast, ToastSeverity, WorkbenchCommand,
    WorkbenchState, Workspace,
};

/// Per-panel UI plugins (sidebar, viewport, metrics, event log, menu, etc.).
pub mod plugins;

/// Shared design tokens — fonts, spacing, and global style.
pub mod theme;

/// UI helper utilities.
pub mod utils;

/// Shared, reusable UI primitives (kv_row, chart_legend_dot, empty/error
/// states) — see `docs/design/components.md` for the full catalog.
pub mod widgets;

/// Immediate-mode rendering logic.
pub mod render;
pub use render::render_ui;
