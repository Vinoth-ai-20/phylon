# PROMPT 2

**# Context: Architectural Migration to Bevy Engine**
I am migrating my custom Rust simulation project from a raw windowing/graphics loop to the **Bevy Engine (v0.19)**.

Currently, I manage the event loop, rendering, and UI manually using:

* `winit` (v0.30) for the OS window and event loop.
* `wgpu` (v22) for the custom graphics rendering pipeline.
* `egui` (v0.29), `egui_plot`, `egui_dock`, `egui-winit`, and `egui-wgpu` for the UI and its bridge to the renderer.

**# The Goal: Full Bevy Integration**
I want to completely strip out the manual `winit` event loop and `egui-wgpu` rendering bridge, and port the entire application into Bevy's ECS and Plugin architecture.

Please act as a Principal Rust/Bevy Engineer. I need an **Implementation Plan** broken into the following 4 Phases. Do not write all the code at once; provide the step-by-step plan and wait for my command to begin Phase 1.

**# Phase 1: Dependency Overhaul (`Cargo.toml`)**

* Outline which crates must be completely removed (e.g., `winit`, `egui-winit`, `egui-wgpu`).
* Outline the required Bevy dependencies to add, specifically `bevy` (v0.19) and `bevy_egui` (the official Bevy integration for egui). Note if `egui_plot` and `egui_dock` need version bumps to match `bevy_egui`'s re-exported egui version.

**# Phase 2: App Bootstrapping & Window Creation**

* Provide the new `main.rs` setup.
* Replace the old `winit::event_loop` with `App::new()`.
* Show how to configure `DefaultPlugins` to recreate my existing Window settings (title, resolution, vsync).
* Show how to add the `EguiPlugin`.

**# Phase 3: Migrating the UI to Bevy Systems**

* Explain how to transition my raw immediate-mode `egui` draw calls into Bevy systems.
* Provide an example Bevy system (e.g., `fn inspector_ui(mut contexts: EguiContexts)`) demonstrating how to extract the `egui::Context` and render a basic sidebar and plot.

**# Phase 4: The Custom WGPU Simulation Bridge**

* *Critical Architecture Check:* Since I was previously using raw `wgpu`, outline the best approach for integrating my custom compute/render shaders into Bevy. Should we use Bevy's `AsBindGroup` and custom Render Nodes, or migrate the simulation logic into standard Bevy ECS systems?

**# Your Task**
Please analyze these 4 Phases. Briefly explain the major "gotchas" I should look out for when moving from an immediate-mode event loop to Bevy's scheduled ECS framework. Then, wait for my command to begin coding Phase 1.

---

Are you planning to rewrite your custom `wgpu` simulation logic into standard Bevy ECS systems, or are you hoping to keep your custom compute shaders and just wrap them inside Bevy's render pipeline?
