/// Circulation Viewer panel — per-segment `ChemicalEconomy` levels along the
/// Body Graph edges P4-F3's transport pass relaxes (Phase 4, P4-R2).
pub mod circulation_viewer;
/// Command Palette overlay — fuzzy-searchable action list (Ctrl+Shift+P).
pub mod command_palette;
/// UI plugin modules.
pub mod dialogs;
/// Event log panel — recent births, deaths, hazard events.
pub mod event_log;
/// Evolution Debugger panel — cross-organism mutation diff and a
/// development-failure inspector (Phase 3, M12).
pub mod evolution_debugger;
/// GRN Viewer panel — regulatory network graph, developmental-step time
/// playback, and mutation-vs-parent comparison (Phase 3, M11).
pub mod grn_viewer;
/// Hormone Viewer panel — head `Neuromodulators` plus per-segment
/// `HormoneLevel` (Phase 4, P4-R3).
pub mod hormone_viewer;
/// HOX Visualizer panel — per-position Hox code, segment identity, and
/// morphogen gradients for the selected organism (Phase 3, M10).
pub mod hox_visualizer;
/// Immune Viewer panel — organism-wide `Infection` plus per-segment
/// `SegmentInfection`/`SegmentImmunity` (Phase 4, P4-R4).
pub mod immune_viewer;
/// Entity inspector panel — single organism and environment details.
pub mod inspector;
/// Cell Lineage Viewer panel — ancestry plus the live, persistent Body
/// Graph (Phase 4, P4-R5).
pub mod lineage_viewer;
/// Top menu bar.
pub mod menu;
/// Metrics dashboard and event log bottom-panel tabs.
pub mod metrics;
/// Neural Viewer panel — CTRNN brain node-link graph for the selected organism.
pub mod neural_viewer;
/// Shared helper for the P4-R-tier physiology panels (Physiology,
/// Circulation, Hormone, Immune Viewers) — resolving the selected
/// organism's persistent Body Graph.
pub mod organism_panel_common;
/// Physiology Viewer / Organ Inspector panel — per-segment `ChemicalEconomy`
/// (Phase 4, P4-R1).
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
/// canonical shape (Phase 7, W3c).
pub mod workspace_manager;
