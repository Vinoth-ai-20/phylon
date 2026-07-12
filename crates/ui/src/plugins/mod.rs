/// Circulation Viewer panel — per-segment `ChemicalEconomy` levels along the
/// Body Graph edges the transport pass relaxes.
pub mod circulation_viewer;
/// Command Palette overlay — fuzzy-searchable action list (Ctrl+Shift+P).
pub mod command_palette;
/// UI plugin modules.
pub mod dialogs;
/// Event log panel — recent births, deaths, hazard events.
pub mod event_log;
/// Evolution Debugger panel — cross-organism mutation diff and a
/// development-failure inspector.
pub mod evolution_debugger;
/// Viewport navigation gizmos — axis triad, navigation cube, world-origin
/// and camera-pivot indicators, selection bounding box, and a scientific
/// scene-info overlay.
pub mod gizmos;
/// Global Search overlay — fuzzy-searchable list of every currently-alive
/// organism, by diet or `{Idx, Gen}`.
pub mod global_search;
/// GRN Viewer panel — regulatory network graph, developmental-step time
/// playback, and mutation-vs-parent comparison.
pub mod grn_viewer;
/// Hormone Viewer panel — head `Neuromodulators` plus per-segment
/// `HormoneLevel`.
pub mod hormone_viewer;
/// HOX Visualizer panel — per-position Hox code, segment identity, and
/// morphogen gradients for the selected organism.
pub mod hox_visualizer;
/// Immune Viewer panel — organism-wide `Infection` plus per-segment
/// `SegmentInfection`/`SegmentImmunity`.
pub mod immune_viewer;
/// Entity inspector panel — single organism and environment details.
pub mod inspector;
/// Cell Lineage Viewer panel — ancestry plus the live, persistent Body
/// Graph.
pub mod lineage_viewer;
/// Top menu bar.
pub mod menu;
/// Metrics dashboard and event log bottom-panel tabs.
pub mod metrics;
/// Neural Viewer panel — CTRNN brain node-link graph for the selected organism.
pub mod neural_viewer;
/// Shared helper for the physiology-family panels (Physiology, Circulation,
/// Hormone, Immune Viewers) — resolving the selected organism's persistent
/// Body Graph.
pub mod organism_panel_common;
/// Physiology Viewer / Organ Inspector panel — per-segment `ChemicalEconomy`.
pub mod physiology_viewer;
/// Replay Browser panel — static inspection of a loaded `.phylon-replay` bundle.
pub mod replay_browser;
/// Research Dashboard panel — lists/compares experiment reports from `data/experiments/`.
pub mod research_dashboard;
/// Sidebar activity bar and per-workspace content panels.
pub mod sidebar;
/// Bottom status bar.
pub mod status_bar;
/// Top toolbar.
pub mod toolbar;
/// Central simulation viewport.
pub mod viewport;
/// Workspace Manager overlay — save/rename/duplicate/delete/export/import
/// user-defined panel layouts, reset a built-in preset back to its
/// canonical shape.
pub mod workspace_manager;
