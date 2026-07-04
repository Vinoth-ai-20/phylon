//! Dialogs plugin — About, Documentation, and Keybinds floating windows.
//!
//! All dialogs are shown based on boolean flags in `WorkbenchState`. This module
//! consolidates them out of `render.rs` into a dedicated, testable location.

use crate::types::MenuAction;

/// Render all active dialogs. Call once per frame inside the egui pass.
pub fn show_dialogs(
    ctx: &egui::Context,
    state: &mut crate::WorkbenchState,
    _actions: &mut Vec<MenuAction>,
) {
    about_dialog(ctx, state);
    documentation_dialog(ctx, state);
    keybinds_dialog(ctx, state);
}

fn about_dialog(ctx: &egui::Context, state: &mut crate::WorkbenchState) {
    egui::Window::new("About Phylon")
        .open(&mut state.show_about)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading(
                    egui::RichText::new("PHYLON")
                        .size(36.0)
                        .strong()
                        .color(egui::Color32::from_rgb(100, 200, 255)),
                );
                ui.label(
                    egui::RichText::new("Artificial Life Simulation Engine")
                        .italics()
                        .color(egui::Color32::GRAY),
                );
                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);
                egui::Grid::new("about_grid").show(ui, |ui| {
                    ui.label("Version");
                    ui.label(egui::RichText::new("0.1.0").strong());
                    ui.end_row();
                    ui.label("Architecture");
                    ui.label("ECS + GPU Compute");
                    ui.end_row();
                    ui.label("Physics");
                    ui.label("Verlet Particle Nodes");
                    ui.end_row();
                    ui.label("Genetics");
                    ui.label("CPPN + Hox Sequences");
                    ui.end_row();
                    ui.label("Neural");
                    ui.label("CTRNN (Continuous-Time RNN)");
                    ui.end_row();
                    ui.label("Renderer");
                    ui.label("wgpu (WebGPU / Vulkan / DX12)");
                    ui.end_row();
                });
            });
        });
}

fn documentation_dialog(ctx: &egui::Context, state: &mut crate::WorkbenchState) {
    egui::Window::new("Documentation")
        .open(&mut state.show_docs)
        .resizable(true)
        .collapsible(true)
        .default_size(egui::vec2(500.0, 400.0))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Architecture Overview");
                ui.label(
                    "Phylon is a GPU-accelerated artificial life simulator. \
                    Organisms are soft-body creatures defined by a genome that \
                    encodes both their morphology (via a CPPN developmental \
                    program) and their neural controller (CTRNN).",
                );
                ui.add_space(8.0);

                ui.collapsing("Physics Engine", |ui| {
                    ui.label("• Verlet particle nodes connected by spring constraints");
                    ui.label("• Elastic muscles actuated by neural outputs");
                    ui.label("• Rigid bone springs for structural integrity");
                    ui.label("• GPU compute shader for parallel integration");
                });

                ui.collapsing("Genetics", |ui| {
                    ui.label("• Each organism has a Genome with two CPPNs:");
                    ui.label("  — Morph CPPN: encodes body plan and segment types");
                    ui.label("  — Brain CPPN: encodes neural topology");
                    ui.label("• Hox sequences define segment growth order");
                    ui.label("• Mutations perturb CPPN weights and connections");
                });

                ui.collapsing("Neural Control", |ui| {
                    ui.label("• CTRNN (Continuous-Time Recurrent Neural Network)");
                    ui.label("• Inputs: chemical sensors, vision, proprioception");
                    ui.label("• Outputs: muscle actuators, signal emission");
                    ui.label("• Integration done via GPU compute (Euler step)");
                });

                ui.collapsing("Ecology", |ui| {
                    ui.label("• Producers, Herbivores, Carnivores, Omnivores, Decomposers");
                    ui.label("• Chemical economy: ATP, Glucose, O2, CO2");
                    ui.label("• Food pellets, mineral pellets, corpse cycling");
                    ui.label("• Diffusion grid for pheromones and gas transport");
                });

                ui.collapsing("Workbench Usage", |ui| {
                    ui.label("• Click an organism to inspect it in the Inspector panel");
                    ui.label("• Double-click to track and follow an organism");
                    ui.label("• Right-click for the context menu (Kill, Export, Track)");
                    ui.label("• Use the Overlay selector to visualise chemical fields");
                    ui.label("• View → Vision Cones shows organism field of view");
                    ui.label("• Use Ctrl+S / Ctrl+O to save and restore simulation states");
                });
            });
        });
}

fn keybinds_dialog(ctx: &egui::Context, state: &mut crate::WorkbenchState) {
    egui::Window::new("Keyboard Shortcuts")
        .open(&mut state.show_keybinds)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                keybind_section(
                    ui,
                    "Simulation",
                    &[
                        ("Space", "Play / Pause"),
                        ("→", "Step Forward (one tick)"),
                        ("Ctrl+R", "Reset / Reseed Simulation"),
                    ],
                );
                keybind_section(
                    ui,
                    "File",
                    &[
                        ("Ctrl+S", "Save State"),
                        ("Ctrl+O", "Load State"),
                        ("Ctrl+P", "Spawn Proto-Fish"),
                    ],
                );
                keybind_section(
                    ui,
                    "Selection",
                    &[
                        ("Ctrl+A", "Select First Head Node"),
                        ("Escape", "Deselect All"),
                        ("X", "Delete Selected"),
                        ("C", "Duplicate Selected"),
                    ],
                );
                keybind_section(
                    ui,
                    "Camera",
                    &[
                        ("+ / =", "Zoom In"),
                        ("−", "Zoom Out"),
                        ("Home / 0", "Reset Camera"),
                        ("W A S D / Arrow Keys", "Pan Camera"),
                    ],
                );
                keybind_section(
                    ui,
                    "View",
                    &[
                        ("Ctrl+M", "Toggle Metrics"),
                        ("Ctrl+L", "Toggle Event Log"),
                        ("Ctrl+B", "Toggle Sidebar"),
                    ],
                );
                keybind_section(
                    ui,
                    "Editing",
                    &[
                        ("Z", "Undo"),
                        ("Y", "Redo"),
                        ("V", "Paste / Spawn from Clipboard"),
                        ("F", "Toggle Stationary"),
                        ("J", "Join Selection"),
                    ],
                );
            });
        });
}

fn keybind_section(ui: &mut egui::Ui, title: &str, binds: &[(&str, &str)]) {
    ui.collapsing(title, |ui| {
        egui::Grid::new(format!("kb_{}", title))
            .striped(true)
            .min_col_width(120.0)
            .show(ui, |ui| {
                for (key, action) in binds {
                    ui.label(
                        egui::RichText::new(*key)
                            .monospace()
                            .color(egui::Color32::from_rgb(200, 200, 100)),
                    );
                    ui.label(*action);
                    ui.end_row();
                }
            });
    });
}
