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

/// UI state types and enums.
pub mod types;
pub use types::{
    ActiveHeatmap, AppState, BottomTab, CanvasInteraction, HeatmapState, MenuAction, SidebarTab,
    UiError,
};

/// UI helper utilities.
pub mod utils;

/// Immediate-mode rendering logic.
pub mod render;
pub use render::render_ui;
