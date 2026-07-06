/// UI plugin modules.
pub mod dialogs;
/// Event log panel — recent births, deaths, hazard events.
pub mod event_log;
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
