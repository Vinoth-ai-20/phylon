//! Dialogs plugin — About, Documentation, Keybinds, and Onboarding Hints
//! floating windows.
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
    onboarding_hints_dialog(ctx, state);
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
                        .size(crate::theme::SIZE_DISPLAY * 1.6)
                        .strong()
                        .color(crate::theme::ACCENT),
                );
                ui.label(
                    egui::RichText::new("Artificial Life Simulation Engine")
                        .italics()
                        .color(crate::theme::DISABLED_FG),
                );
                ui.add_space(crate::theme::SPACE_MD);
                ui.separator();
                ui.add_space(crate::theme::SPACE_SM);
                egui::Grid::new("about_grid").striped(true).show(ui, |ui| {
                    crate::widgets::kv_row(ui, "Version", "0.1.0");
                    crate::widgets::kv_row(ui, "Architecture", "ECS + GPU Compute");
                    crate::widgets::kv_row(ui, "Physics", "Verlet Particle Nodes");
                    crate::widgets::kv_row(ui, "Genetics", "CPPN + Hox Sequences");
                    crate::widgets::kv_row(ui, "Neural", "CTRNN (Continuous-Time RNN)");
                    crate::widgets::kv_row(ui, "Renderer", "wgpu (WebGPU / Vulkan / DX12)");
                });
            });
        });
}

fn documentation_dialog(ctx: &egui::Context, state: &mut crate::WorkbenchState) {
    egui::Window::new("Documentation")
        .open(&mut state.show_docs)
        .resizable(true)
        .collapsible(true)
        .default_size(crate::theme::DIALOG_SIZE)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Architecture Overview");
                ui.label(
                    "Phylon is a GPU-accelerated artificial life simulator. \
                    Organisms are soft-body creatures defined by a genome that \
                    encodes both their morphology (via a CPPN developmental \
                    program) and their neural controller (CTRNN).",
                );
                ui.add_space(crate::theme::SPACE_SM);

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
                        ("↑ / ↓", "Speed Up / Slow Down"),
                    ],
                );
                keybind_section(
                    ui,
                    "File",
                    &[
                        ("Ctrl+S", "Save State"),
                        ("Ctrl+O", "Load State"),
                        ("Ctrl+Shift+I", "Import Genome"),
                        ("Ctrl+Shift+E", "Export Genome"),
                        ("Ctrl+Shift+S", "Take Screenshot"),
                        ("Ctrl+Shift+R", "Start / Stop Recording"),
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
                    ],
                );
                keybind_section(
                    ui,
                    "Camera",
                    &[
                        ("+ / =", "Zoom In"),
                        ("−", "Zoom Out"),
                        ("Home / 0 / Ctrl+R", "Reset Camera"),
                        ("W A S D / Arrow Keys", "Pan (Orbit) / Fly (Fly mode)"),
                        ("Middle-Drag", "Orbit / Look Around"),
                        ("Tab", "Toggle Orbit / Fly Camera"),
                        ("Double-Click", "Focus Selection"),
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
                keybind_section(ui, "Editing", &[("F", "Toggle Stationary")]);
            });
        });
}

/// First-run contextual hints (Phase 5, SX-9a) — deliberately *not* a full
/// tour (no multi-step wizard, no forced sequence): one dismissible dialog
/// pointing at the two things a first-time viewer has no way to discover on
/// their own — the viewport's population-wide state-legibility signals
/// (Epic 1) and the redesigned Inspector's progressive-disclosure sections
/// (Epic 6). Shown automatically once per session (see
/// `WorkbenchState::show_onboarding_hints`'s doc comment for exactly when),
/// re-openable afterward via Help → Welcome Tips.
fn onboarding_hints_dialog(ctx: &egui::Context, state: &mut crate::WorkbenchState) {
    if !state.show_onboarding_hints {
        return;
    }
    // A local `open`/`dismissed` pair, not `.open(&mut state.show_onboarding_hints)`
    // directly, since the "Got it" button below also needs to write
    // `state.show_onboarding_hints` from inside the same `.show()` closure —
    // borrowing it twice (once for `.open()`, once inside the closure) isn't
    // possible in one call chain.
    let mut open = true;
    let mut dismissed = false;
    egui::Window::new("Welcome to Phylon")
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.label(
                egui::RichText::new("A few things to look for before you dive in:")
                    .color(crate::theme::DISABLED_FG)
                    .italics(),
            );
            ui.add_space(crate::theme::SPACE_MD);

            ui.label(egui::RichText::new("In the viewport").strong());
            ui.add_space(crate::theme::SPACE_XS);
            onboarding_row(
                ui,
                egui_remixicon::icons::ARROW_UP_S_LINE,
                crate::theme::ACTIVITY_GLYPH,
                "A glyph above an organism shows what it's doing right now — hunting, fleeing, foraging, mating, or sleeping. No glyph means idle.",
            );
            onboarding_row(
                ui,
                egui_remixicon::icons::HEART_PULSE_LINE,
                crate::theme::GOOD,
                "An organism's outline brightness reflects its health — dimmer means closer to death.",
            );
            onboarding_row(
                ui,
                egui_remixicon::icons::VIRUS_LINE,
                crate::theme::WARN,
                "A tinted organism is infectious with disease.",
            );
            onboarding_row(
                ui,
                egui_remixicon::icons::SKULL_LINE,
                crate::theme::BAD,
                "Floating text marks births and deaths as they happen, including the specific cause of death.",
            );

            ui.add_space(crate::theme::SPACE_MD);
            ui.label(egui::RichText::new("In the Inspector").strong());
            ui.add_space(crate::theme::SPACE_XS);
            onboarding_row(
                ui,
                egui_remixicon::icons::SEARCH_LINE,
                crate::theme::ACCENT,
                "Click any organism to inspect it. Expand its Physiology, Circulation, Hormones, Immune Response, and Evolution / History sections for full detail — collapsed by default so the panel isn't a wall of numbers.",
            );

            ui.add_space(crate::theme::SPACE_MD);
            ui.separator();
            ui.add_space(crate::theme::SPACE_SM);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Reopen this any time via Help → Welcome Tips.")
                        .small()
                        .color(crate::theme::DISABLED_FG),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Got it").clicked() {
                        dismissed = true;
                    }
                });
            });
        });

    if !open || dismissed {
        state.show_onboarding_hints = false;
    }
}

/// One icon + colored swatch + explanation row for `onboarding_hints_dialog`
/// — not `widgets::kv_row` (that's a key/value pair, this is an
/// icon-led sentence), so a small local helper rather than a forced fit.
fn onboarding_row(ui: &mut egui::Ui, icon: &str, color: egui::Color32, text: &str) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(icon)
                .color(color)
                .size(crate::theme::ICON_MD),
        );
        ui.add_space(crate::theme::SPACE_XS);
        ui.label(text);
    });
    ui.add_space(crate::theme::SPACE_XS);
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
                            .color(crate::theme::ACCENT),
                    );
                    ui.label(*action);
                    ui.end_row();
                }
            });
    });
}
