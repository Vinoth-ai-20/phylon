/// Command Palette overlay — fuzzy-searchable action list (Ctrl+Shift+P).
pub mod command_palette;
/// UI plugin modules.
pub mod dialogs;
/// Event log panel — recent births, deaths, hazard events.
pub mod event_log;
/// GRN Viewer panel — regulatory network graph, developmental-step time
/// playback, and mutation-vs-parent comparison (Phase 3, M11).
pub mod grn_viewer;
/// HOX Visualizer panel — per-position Hox code, segment identity, and
/// morphogen gradients for the selected organism (Phase 3, M10).
pub mod hox_visualizer;
/// Entity inspector panel — single organism and environment details.
pub mod inspector;
/// Top menu bar.
pub mod menu;
/// Metrics dashboard and event log bottom-panel tabs.
pub mod metrics;
/// Neural Viewer panel — CTRNN brain node-link graph for the selected organism.
pub mod neural_viewer;
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
