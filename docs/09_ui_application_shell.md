# UI Application Shell Architecture

As of Phase 8, Phylon operates as a standalone desktop application with a robust, un-obstructive user interface shell. This shell wraps the entire WebGPU and winit simulation loop using `egui` and `egui-wgpu`.

## Core Philosophy

1. **Never obscure the simulation**: Phylon has no initial loading screen or splash menus. The simulation starts instantly with an active world, allowing immediate intervention.
2. **Non-blocking overlays**: Heavy operations like saving/loading SQLite snapshots or exporting CSVs are executed asynchronously. A small, centered loading overlay informs the user of progress while keeping the application responsive.
3. **Decoupled UI State**: The `UiState` singleton owns all configuration related to presentation. Menus merely flip flags in `UiState`, which are then parsed during the render/update cycles in `main.rs`.

## Architecture Split

The `crates/ui` module is partitioned logically:

- **`state.rs`**: Defines the `UiState` struct, which controls what gets rendered (e.g., `show_trails`, `show_field_overlay`), the simulation speed, pausing mechanisms, and the state of open inspector panels.
- **`menu.rs`**: Implements the persistent `egui::TopBottomPanel::top` menu. It handles the hierarchical menus for File, Edit, Simulation, View, Selection, Go, Run, Terminal, and Help.
- **`overlay.rs`**: Renders the non-blocking asynchronous task tracker. Using a dimming rect on the highest egui layer and a centered UI block, it displays ongoing multi-threaded operations gracefully without deadlocking the winit loop.
- **`modal.rs`**: Implements specialized confirmation and dialog boxes (e.g., `ConfirmQuit`, `FilterByDiet`).

## State Synchronization

In `app/src/main.rs`:

1. `winit` keyboard inputs are evaluated directly *before* `egui` consumes them. This allows global hotkeys (`F11` for fullscreen, `Space` for pause) to trigger instantly.
2. An `mpsc` receiver channel continuously drains background task status structs (`LoadingTask`) into `UiState::active_loading_task`, keeping the loading bar animated frame-by-frame.
3. `UiState::is_paused` conditionally wraps `scheduler.tick_loop(&mut world)`, meaning that a paused UI completely freezes the logic ticks while continuing to redraw the scene and UI.

## Adding Features

To add a new tool or panel:

1. Add a boolean flag to `PanelVisibility` in `state.rs`.
2. Map a toggle checkbox to it inside `menu.rs` (under the View menu).
3. Check the boolean state in `crates/ui/src/lib.rs` and render your `egui::Window` conditionally.
