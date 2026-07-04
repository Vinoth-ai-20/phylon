//! Sidebar plugin — navigation rail and content panels for each workspace tab.
//!
//! Two public functions:
//! - `activity_bar_ui()` — the narrow icon strip on the far left
//! - `sidebar_content_ui()` — the expandable content panel showing live data

use crate::types::*;

/// Narrow activity bar (icon strip, far left column).
#[allow(clippy::too_many_arguments)]
pub fn activity_bar_ui(
    _ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    _world: &mut world::World,
    actions: &mut Vec<MenuAction>,
) {
    ui.add_space(8.0);
    ui.vertical_centered(|ui| {
        for (icon, tab, tooltip) in NAV_TABS {
            if ui
                .selectable_label(
                    state.active_tab == tab,
                    egui::RichText::new(icon).size(18.0),
                )
                .on_hover_text(tooltip)
                .clicked()
            {
                let mode = state
                    .panel_modes
                    .get("Sidebar")
                    .copied()
                    .unwrap_or(crate::state::PanelMode::Docked);

                if state.active_tab == tab && mode == crate::state::PanelMode::Docked {
                    // VS Code behavior: clicking the already-active tab's icon
                    // collapses the sidebar instead of doing nothing.
                    actions.push(MenuAction::ClosePanel("Sidebar".to_string()));
                } else {
                    state.active_tab = tab;
                    state.sidebar_visible = true;

                    // If the Sidebar panel is closed, reopen it by re-docking
                    // it into the tile tree.
                    if mode == crate::state::PanelMode::Closed {
                        actions.push(MenuAction::DockPanel("Sidebar".to_string()));
                    }
                }
            }
            ui.add_space(4.0);
        }
    });
}

const NAV_TABS: [(&str, crate::SidebarTab, &str); 8] = [
    (
        egui_remixicon::icons::SEARCH_LINE,
        crate::SidebarTab::Inspector,
        "Inspector",
    ),
    (
        egui_remixicon::icons::TEST_TUBE_LINE,
        crate::SidebarTab::Genetics,
        "Genetics",
    ),
    (
        egui_remixicon::icons::EARTH_LINE,
        crate::SidebarTab::Ecology,
        "Ecology",
    ),
    (
        egui_remixicon::icons::CLOUD_LINE,
        crate::SidebarTab::Environment,
        "Environment",
    ),
    (
        egui_remixicon::icons::LINE_CHART_LINE,
        crate::SidebarTab::Analytics,
        "Analytics",
    ),
    (
        egui_remixicon::icons::TOOLS_LINE,
        crate::SidebarTab::Sandbox,
        "Sandbox",
    ),
    (
        egui_remixicon::icons::EQUALIZER_LINE,
        crate::SidebarTab::Tuning,
        "Tuning",
    ),
    (
        egui_remixicon::icons::SETTINGS_LINE,
        crate::SidebarTab::Settings,
        "Settings",
    ),
];

/// Content panel for the active sidebar tab.
#[allow(clippy::too_many_arguments)]
pub fn sidebar_content_ui(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
    actions: &mut Vec<MenuAction>,
) {
    // No heading here — the active tab's icon/label is now shown in the
    // merged chrome bar (see `layout::panel_chrome`), so this content starts
    // straight into the scroll area instead of repeating the label below it.
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| match state.active_tab {
            crate::SidebarTab::Inspector => {
                crate::plugins::inspector::inspector_ui(ctx, ui, state, world, actions);
            }
            crate::SidebarTab::Genetics => genetics_panel(ui, state, world, actions),
            crate::SidebarTab::Ecology => ecology_panel(ui, world),
            crate::SidebarTab::Environment => environment_panel(ui, world),
            crate::SidebarTab::Analytics => analytics_panel(ui, world),
            crate::SidebarTab::Sandbox => sandbox_panel(ui, state, actions),
            crate::SidebarTab::Tuning => tuning_panel(ui, state, world),
            crate::SidebarTab::Settings => settings_panel(ui, state, actions),
        });
}

// ─── Genetics panel ─────────────────────────────────────────────────────────

fn genetics_panel(
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
    actions: &mut Vec<MenuAction>,
) {
    let entity = match state.selected_entity.or(state.tracked_entity) {
        Some(e) => e,
        None => {
            ui.label(
                egui::RichText::new("No organism selected.")
                    .italics()
                    .color(egui::Color32::GRAY),
            );
            return;
        }
    };

    let mut genome_q = world.ecs.query::<&genetics::Genome>();
    if let Ok(genome) = genome_q.get(&world.ecs, entity) {
        egui::Grid::new("gen_panel").striped(true).show(ui, |ui| {
            grid_row(ui, "Genome ID", &genome.id.0.to_string());
            grid_row(ui, "Schema", &format!("v{}", genome.schema_version));
            grid_row(ui, "Ploidy", &format!("{:?}", genome.ploidy));
            grid_row(
                ui,
                "Brain nodes",
                &genome.brain_cppn.nodes.len().to_string(),
            );
            grid_row(
                ui,
                "Brain edges",
                &genome.brain_cppn.connections.len().to_string(),
            );
            grid_row(
                ui,
                "Morph nodes",
                &genome.morph_cppn.nodes.len().to_string(),
            );
            grid_row(
                ui,
                "Morph edges",
                &genome.morph_cppn.connections.len().to_string(),
            );
            if let Some(hox) = &genome.hox {
                grid_row(ui, "Hox genes", &hox.genes.len().to_string());
            } else {
                grid_row(ui, "Hox genes", "None (CPPN-driven)");
            }
        });
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(format!(
                "{} Brain Network graph moved to the Neural Viewer panel",
                egui_remixicon::icons::BRAIN_LINE
            ))
            .color(egui::Color32::GRAY)
            .italics(),
        );

        ui.add_space(8.0);
        if ui.button("Export Genome…").clicked() {
            actions.push(MenuAction::ExportGenome);
        }
    } else {
        ui.label(
            egui::RichText::new("Genome not on this node. Select the head node.")
                .color(egui::Color32::GRAY)
                .italics(),
        );
    }
}

// ─── Ecology panel ──────────────────────────────────────────────────────────

fn ecology_panel(ui: &mut egui::Ui, world: &mut world::World) {
    let mut prod = 0usize;
    let mut herb = 0usize;
    let mut carn = 0usize;
    let mut omni = 0usize;
    let mut deco = 0usize;

    for diet in world.ecs.query::<&ecology::Diet>().iter(&world.ecs) {
        match diet {
            ecology::Diet::Producer => prod += 1,
            ecology::Diet::Herbivore => herb += 1,
            ecology::Diet::Carnivore => carn += 1,
            ecology::Diet::Omnivore => omni += 1,
            ecology::Diet::Decomposer => deco += 1,
        }
    }

    let food = world
        .ecs
        .query::<&ecology::FoodPellet>()
        .iter(&world.ecs)
        .count();
    let minerals = world
        .ecs
        .query::<&ecology::MineralPellet>()
        .iter(&world.ecs)
        .count();
    let corpses = world
        .ecs
        .query::<&ecology::Corpse>()
        .iter(&world.ecs)
        .count();
    let total_organisms = prod + herb + carn + omni + deco;

    egui::CollapsingHeader::new(format!("{} Population", egui_remixicon::icons::TEAM_LINE))
        .default_open(true)
        .show(ui, |ui| {
            egui::Grid::new("eco_pop").striped(true).show(ui, |ui| {
                grid_row_colored(
                    ui,
                    "Producers",
                    &prod.to_string(),
                    egui::Color32::from_rgb(100, 220, 100),
                );
                grid_row_colored(
                    ui,
                    "Herbivores",
                    &herb.to_string(),
                    egui::Color32::from_rgb(180, 255, 150),
                );
                grid_row_colored(
                    ui,
                    "Carnivores",
                    &carn.to_string(),
                    egui::Color32::from_rgb(255, 100, 100),
                );
                grid_row_colored(
                    ui,
                    "Omnivores",
                    &omni.to_string(),
                    egui::Color32::from_rgb(255, 200, 100),
                );
                grid_row_colored(
                    ui,
                    "Decomposers",
                    &deco.to_string(),
                    egui::Color32::from_rgb(180, 140, 200),
                );
                grid_row(ui, "TOTAL", &total_organisms.to_string());
            });
        });

    egui::CollapsingHeader::new(format!("{} Resources", egui_remixicon::icons::LEAF_LINE))
        .default_open(true)
        .show(ui, |ui| {
            egui::Grid::new("eco_res").striped(true).show(ui, |ui| {
                grid_row(ui, "Food Pellets", &food.to_string());
                grid_row(ui, "Minerals", &minerals.to_string());
                grid_row(ui, "Corpses", &corpses.to_string());
            });
        });

    // Predator/Prey ratio
    if herb + prod > 0 {
        let ratio = (carn + omni) as f32 / (herb + prod) as f32;
        egui::CollapsingHeader::new(format!("{} Ratios", egui_remixicon::icons::SCALES_LINE))
            .default_open(false)
            .show(ui, |ui| {
                ui.label(format!("Predator/Prey: {:.2}", ratio));
                let density = total_organisms as f32 / (2000.0 * 2000.0) * 1_000_000.0;
                ui.label(format!("Population density: {:.1}/km²", density));
            });
    }
}

// ─── Environment panel ──────────────────────────────────────────────────────

fn environment_panel(ui: &mut egui::Ui, world: &mut world::World) {
    if let Some(atmo) = world.ecs.get_resource::<metabolism::GlobalAtmosphere>() {
        egui::CollapsingHeader::new(format!("{} Atmosphere", egui_remixicon::icons::CLOUD_LINE))
            .default_open(true)
            .show(ui, |ui| {
                egui::Grid::new("env_atmo").striped(true).show(ui, |ui| {
                    grid_row(ui, "Sunlight", &format!("{:.1}%", atmo.sunlight * 100.0));
                    grid_row(ui, "O₂", &format!("{:.3}", atmo.o2));
                    grid_row(ui, "CO₂", &format!("{:.3}", atmo.co2));
                    grid_row(ui, "Temperature", &format!("{:.1}°C", atmo.temp));
                    grid_row(ui, "Day/Night Tick", &atmo.ticks.to_string());
                });
            });
    } else {
        ui.label(
            egui::RichText::new("GlobalAtmosphere resource not available.")
                .color(egui::Color32::GRAY)
                .italics(),
        );
    }

    // EnvironmentManager not exposed to UI crate — world bounds shown in status bar.
}

// ─── Analytics panel ────────────────────────────────────────────────────────

fn analytics_panel(ui: &mut egui::Ui, world: &mut world::World) {
    if let Some(metrics) = world.ecs.get_resource::<analytics::MetricsState>() {
        egui::Grid::new("ana_grid").striped(true).show(ui, |ui| {
            grid_row(ui, "Sim Time", &format!("{:.1}s", metrics.sim_time));
            grid_row(ui, "FPS", &format!("{:.0}", metrics.smoothed_fps));
            grid_row(ui, "TPS", &format!("{:.0}", metrics.smoothed_tps));

            // Latest population counts from history
            let latest = |hist: &std::collections::VecDeque<[f64; 2]>| {
                hist.back().map(|p| p[1] as usize).unwrap_or(0)
            };
            grid_row(
                ui,
                "Producers",
                &latest(&metrics.producers_history).to_string(),
            );
            grid_row(
                ui,
                "Herbivores",
                &latest(&metrics.herbivores_history).to_string(),
            );
            grid_row(
                ui,
                "Carnivores",
                &latest(&metrics.carnivores_history).to_string(),
            );
            grid_row(
                ui,
                "Omnivores",
                &latest(&metrics.omnivores_history).to_string(),
            );
        });
    } else {
        ui.label(
            egui::RichText::new("MetricsState not available.")
                .color(egui::Color32::GRAY)
                .italics(),
        );
    }
}

// ─── Sandbox panel ──────────────────────────────────────────────────────────

fn sandbox_panel(
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    actions: &mut Vec<MenuAction>,
) {
    ui.label(egui::RichText::new("Spawn at camera position:").color(egui::Color32::GRAY));
    ui.add_space(4.0);

    for preset in organisms::sandbox::PresetDefinition::standard_presets() {
        if ui.button(&preset.name).clicked() {
            actions.push(MenuAction::SpawnPreset(preset.name.clone()));
        }
    }

    ui.separator();
    ui.label("Direct Spawn:");
    if ui.button("Spawn Proto-Fish").clicked() {
        actions.push(MenuAction::SpawnProtoFish);
    }
    if ui.button("Spawn Manual Hazard").clicked() {
        actions.push(MenuAction::SpawnManualHazard);
    }

    ui.separator();
    ui.label("Selection:");
    ui.horizontal(|ui| {
        if ui.button("Select Producer").clicked() {
            actions.push(MenuAction::SelectByDiet(ecology::Diet::Producer));
        }
        if ui.button("Herbivore").clicked() {
            actions.push(MenuAction::SelectByDiet(ecology::Diet::Herbivore));
        }
    });
    ui.horizontal(|ui| {
        if ui.button("Carnivore").clicked() {
            actions.push(MenuAction::SelectByDiet(ecology::Diet::Carnivore));
        }
        if ui.button("Next Head").clicked() {
            actions.push(MenuAction::InvertSelection);
        }
    });

    let _ = state;
}

// ─── Tuning panel ───────────────────────────────────────────────────────────

fn tuning_panel(ui: &mut egui::Ui, state: &mut crate::WorkbenchState, world: &mut world::World) {
    egui::CollapsingHeader::new(format!("{} Rendering", egui_remixicon::icons::EYE_LINE))
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut state.debug_structural, "Debug Structural View");
            ui.checkbox(&mut state.show_vision_cones, "Show Vision Cones");
            ui.add_space(4.0);
            ui.add(
                egui::Slider::new(&mut state.bone_line_thickness, 0.5..=5.0).text("Bone Thickness"),
            );
            ui.add(egui::Slider::new(&mut state.skin_thickness, 1.0..=10.0).text("Skin Thickness"));
            ui.add(egui::Slider::new(&mut state.node_radius, 2.0..=20.0).text("Node Radius"));
        });

    egui::CollapsingHeader::new(format!(
        "{} Simulation",
        egui_remixicon::icons::SETTINGS_LINE
    ))
    .default_open(true)
    .show(ui, |ui| {
        ui.label("Speed multiplier:");
        ui.add(
            egui::Slider::new(&mut state.simulation_speed, 0.1..=10.0)
                .logarithmic(true)
                .text("×"),
        );
    });

    if let Some(mut atmo) = world.ecs.get_resource_mut::<metabolism::GlobalAtmosphere>() {
        egui::CollapsingHeader::new(format!("{} Atmosphere", egui_remixicon::icons::CLOUD_LINE))
            .default_open(false)
            .show(ui, |ui| {
                ui.add(egui::Slider::new(&mut atmo.sunlight, 0.0..=1.0).text("Sunlight"));
                ui.add(egui::Slider::new(&mut atmo.temp, -10.0..=50.0).text("Temp °C"));
            });
    }
}

// ─── Settings panel ─────────────────────────────────────────────────────────

fn settings_panel(
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    _actions: &mut Vec<MenuAction>,
) {
    egui::CollapsingHeader::new("Panel Visibility")
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut state.sidebar_visible, "Sidebar");
            ui.checkbox(&mut state.inspector_visible, "Inspector");
            ui.checkbox(&mut state.metrics_visible, "Metrics");
            ui.checkbox(&mut state.event_log_visible, "Event Log");
            ui.checkbox(&mut state.status_bar_visible, "Status Bar");
            ui.checkbox(&mut state.toolbar_visible, "Toolbar");
        });

    egui::CollapsingHeader::new("World")
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut state.show_world_boundary, "Show World Boundary");
        });

    egui::CollapsingHeader::new("About")
        .default_open(false)
        .show(ui, |ui| {
            if ui.button("Show About Dialog").clicked() {
                state.show_about = true;
            }
            if ui.button("Show Documentation").clicked() {
                state.show_docs = true;
            }
            if ui.button("Show Keybinds").clicked() {
                state.show_keybinds = true;
            }
        });
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Icon glyph for a sidebar tab, used both in the activity bar and the merged
/// panel chrome bar (see `layout::panel_chrome`).
pub fn tab_icon(tab: crate::SidebarTab) -> &'static str {
    match tab {
        crate::SidebarTab::Inspector => egui_remixicon::icons::SEARCH_LINE,
        crate::SidebarTab::Genetics => egui_remixicon::icons::TEST_TUBE_LINE,
        crate::SidebarTab::Ecology => egui_remixicon::icons::EARTH_LINE,
        crate::SidebarTab::Environment => egui_remixicon::icons::CLOUD_LINE,
        crate::SidebarTab::Analytics => egui_remixicon::icons::LINE_CHART_LINE,
        crate::SidebarTab::Sandbox => egui_remixicon::icons::TOOLS_LINE,
        crate::SidebarTab::Tuning => egui_remixicon::icons::EQUALIZER_LINE,
        crate::SidebarTab::Settings => egui_remixicon::icons::SETTINGS_LINE,
    }
}

/// Display label for a sidebar tab, used both in the activity bar tooltip and
/// the merged panel chrome bar (see `layout::panel_chrome`).
pub fn tab_label(tab: crate::SidebarTab) -> &'static str {
    match tab {
        crate::SidebarTab::Inspector => "Inspector",
        crate::SidebarTab::Genetics => "Genetics",
        crate::SidebarTab::Ecology => "Ecology",
        crate::SidebarTab::Environment => "Environment",
        crate::SidebarTab::Analytics => "Analytics",
        crate::SidebarTab::Sandbox => "Sandbox",
        crate::SidebarTab::Tuning => "Tuning",
        crate::SidebarTab::Settings => "Settings",
    }
}

fn grid_row(ui: &mut egui::Ui, key: &str, val: &str) {
    ui.label(
        egui::RichText::new(key)
            .color(egui::Color32::GRAY)
            .size(12.0),
    );
    ui.label(egui::RichText::new(val).strong().size(12.0));
    ui.end_row();
}

fn grid_row_colored(ui: &mut egui::Ui, key: &str, val: &str, color: egui::Color32) {
    ui.label(egui::RichText::new(key).color(color).size(12.0));
    ui.label(egui::RichText::new(val).color(color).strong().size(12.0));
    ui.end_row();
}

