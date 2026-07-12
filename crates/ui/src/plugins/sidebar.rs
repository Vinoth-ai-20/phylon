//! Sidebar plugin — navigation rail and content panels for each workspace tab.
//!
//! Two public functions:
//! - `activity_bar_ui()` — the narrow icon strip on the far left
//! - `sidebar_content_ui()` — the expandable content panel showing live data

use crate::types::*;

/// Activity bar (navigation rail, far left column) — the `labeled_icon_tab`
/// component from `docs/design/components.md`. Shows icon+label when
/// `state.activity_bar_expanded` (the default, fixing the audit's top
/// discoverability finding: an icon-only rail with only a hover tooltip),
/// or icon-only (the previous permanent behavior) when collapsed via the
/// pin toggle at the bottom of the rail.
#[allow(clippy::too_many_arguments)]
pub fn activity_bar_ui(
    _ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    _world: &mut world::World,
    actions: &mut Vec<MenuAction>,
) {
    let expanded = state.activity_bar_expanded;

    ui.add_space(crate::theme::SPACE_SM);
    ui.vertical(|ui| {
        for (icon, tab, tooltip) in NAV_TABS {
            // Tooltip is always present even when labeled — redundant
            // labeling costs nothing and helps anyone skimming quickly (see
            // docs/design/components.md's `labeled_icon_tab`).
            let response = if expanded {
                ui.add(egui::SelectableLabel::new(
                    state.active_tab == tab,
                    egui::RichText::new(format!("{icon}  {tooltip}")).size(crate::theme::ICON_MD),
                ))
                .on_hover_text(tooltip)
            } else {
                ui.vertical_centered(|ui| {
                    ui.selectable_label(
                        state.active_tab == tab,
                        egui::RichText::new(icon).size(crate::theme::ICON_LG),
                    )
                })
                .inner
                .on_hover_text(tooltip)
            };

            if response.clicked() {
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
            ui.add_space(crate::theme::SPACE_XS);
        }

        // Pin/collapse toggle — always icon-only regardless of `expanded`,
        // pinned to the bottom of the rail.
        ui.add_space(crate::theme::SPACE_MD);
        ui.separator();
        let (pin_icon, pin_tip) = if expanded {
            (
                egui_remixicon::icons::SIDEBAR_FOLD_LINE,
                "Collapse to icons only",
            )
        } else {
            (
                egui_remixicon::icons::SIDEBAR_UNFOLD_LINE,
                "Expand to icon + label",
            )
        };
        if ui
            .vertical_centered(|ui| {
                ui.selectable_label(
                    false,
                    egui::RichText::new(pin_icon).size(crate::theme::ICON_LG),
                )
            })
            .inner
            .on_hover_text(pin_tip)
            .clicked()
        {
            state.activity_bar_expanded = !expanded;
        }
    });
}

const NAV_TABS: [(&str, crate::SidebarTab, &str); 11] = [
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
        egui_remixicon::icons::TREE_LINE,
        crate::SidebarTab::Lineage,
        "Lineage",
    ),
    (
        egui_remixicon::icons::MICROSCOPE_LINE,
        crate::SidebarTab::HoxVisualizer,
        "HOX Visualizer",
    ),
    (
        egui_remixicon::icons::BUBBLE_CHART_LINE,
        crate::SidebarTab::GrnViewer,
        "GRN Viewer",
    ),
    (
        egui_remixicon::icons::CLOUD_LINE,
        crate::SidebarTab::Environment,
        "Environment",
    ),
    (
        egui_remixicon::icons::LINE_CHART_LINE,
        crate::SidebarTab::Analytics,
        "Snapshot",
    ),
    (
        egui_remixicon::icons::TOOLS_LINE,
        crate::SidebarTab::Sandbox,
        "Sandbox",
    ),
    (
        egui_remixicon::icons::EQUALIZER_LINE,
        crate::SidebarTab::Tuning,
        "Simulation Parameters",
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
            crate::SidebarTab::Lineage => lineage_panel(ui, state, world, actions),
            crate::SidebarTab::HoxVisualizer => {
                crate::plugins::hox_visualizer::hox_visualizer_ui(ui, state, world)
            }
            crate::SidebarTab::GrnViewer => {
                crate::plugins::grn_viewer::grn_viewer_ui(ui, state, world)
            }
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
                    .color(crate::theme::DISABLED_FG),
            );
            return;
        }
    };

    let mut genome_q = world.ecs.query::<&genetics::Genome>();
    if let Ok(genome) = genome_q.get(&world.ecs, entity) {
        egui::Grid::new("gen_panel").striped(true).show(ui, |ui| {
            crate::widgets::kv_row(ui, "Genome ID", &genome.id.0.to_string());
            crate::widgets::kv_row(ui, "Schema", &format!("v{}", genome.schema_version));
            crate::widgets::kv_row(ui, "Ploidy", &format!("{:?}", genome.ploidy));
            crate::widgets::kv_row(
                ui,
                "Brain nodes",
                &genome.brain_cppn.nodes.len().to_string(),
            );
            crate::widgets::kv_row(
                ui,
                "Brain edges",
                &genome.brain_cppn.connections.len().to_string(),
            );
            crate::widgets::kv_row(
                ui,
                "Morph nodes",
                &genome.morph_cppn.nodes.len().to_string(),
            );
            crate::widgets::kv_row(
                ui,
                "Morph edges",
                &genome.morph_cppn.connections.len().to_string(),
            );
            crate::widgets::kv_row(
                ui,
                "Regulatory nodes",
                &genome.regulatory_cppn.nodes.len().to_string(),
            );
            crate::widgets::kv_row(
                ui,
                "Regulatory edges",
                &genome.regulatory_cppn.connections.len().to_string(),
            );
        });
        ui.add_space(crate::theme::SPACE_SM);
        ui.label(
            egui::RichText::new(format!(
                "{} Brain Network graph moved to the Neural Viewer panel",
                egui_remixicon::icons::BRAIN_LINE
            ))
            .color(crate::theme::DISABLED_FG)
            .italics(),
        );

        ui.add_space(crate::theme::SPACE_SM);
        if ui.button("Export Genome…").clicked() {
            actions.push(MenuAction::ExportGenome);
        }
    } else {
        ui.label(
            egui::RichText::new("Genome not on this node. Select the head node.")
                .color(crate::theme::DISABLED_FG)
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
                crate::widgets::kv_row_colored(
                    ui,
                    "Producers",
                    &prod.to_string(),
                    crate::theme::chart_color(&ecology::Diet::Producer),
                );
                crate::widgets::kv_row_colored(
                    ui,
                    "Herbivores",
                    &herb.to_string(),
                    crate::theme::chart_color(&ecology::Diet::Herbivore),
                );
                crate::widgets::kv_row_colored(
                    ui,
                    "Carnivores",
                    &carn.to_string(),
                    crate::theme::chart_color(&ecology::Diet::Carnivore),
                );
                crate::widgets::kv_row_colored(
                    ui,
                    "Omnivores",
                    &omni.to_string(),
                    crate::theme::chart_color(&ecology::Diet::Omnivore),
                );
                crate::widgets::kv_row_colored(
                    ui,
                    "Decomposers",
                    &deco.to_string(),
                    crate::theme::chart_color(&ecology::Diet::Decomposer),
                );
                crate::widgets::kv_row_mono(ui, "TOTAL", &total_organisms.to_string());
            });
        });

    egui::CollapsingHeader::new(format!("{} Resources", egui_remixicon::icons::LEAF_LINE))
        .default_open(true)
        .show(ui, |ui| {
            egui::Grid::new("eco_res").striped(true).show(ui, |ui| {
                crate::widgets::kv_row_mono(ui, "Food Pellets", &food.to_string());
                crate::widgets::kv_row_mono(ui, "Minerals", &minerals.to_string());
                crate::widgets::kv_row_mono(ui, "Corpses", &corpses.to_string());
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

// ─── Lineage panel ───────────────────────────────────────────────────────────

/// Ancestry tree + species grouping over `evolution::LineageTracker`.
fn lineage_panel(
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
    actions: &mut Vec<MenuAction>,
) {
    // Clone the records out of the tracker (a cheap, bounded-size copy — one
    // entry per currently-alive organism) so the resource borrow ends here,
    // before the Diet/Entity query below needs `&mut world.ecs`.
    let mut records: Vec<evolution::LineageRecord> = {
        let Some(tracker) = world.ecs.get_resource::<evolution::LineageTracker>() else {
            crate::widgets::empty_state(ui, "Lineage tracking not yet available.");
            return;
        };
        tracker.active_records().cloned().collect()
    };

    if records.is_empty() {
        crate::widgets::empty_state(ui, "No living organisms to show ancestry for.");
        return;
    }
    records.sort_by_key(|r| (r.species.0, r.generation, r.entity.0));

    // One pass over live organisms builds both lookups this panel needs:
    // `EntityId -> Diet` (for color) and `EntityId -> Entity` (for
    // click-to-select) — safer than reconstructing an `Entity` handle from a
    // raw `EntityId` bit pattern, and matches the "snapshot once, don't
    // requery per row" pattern already used for adjacency maps elsewhere
    // (see `inspector.rs::render_body_plan`).
    let mut diet_by_id: std::collections::HashMap<common::EntityId, ecology::Diet> =
        std::collections::HashMap::new();
    let mut entity_by_id: std::collections::HashMap<common::EntityId, bevy_ecs::entity::Entity> =
        std::collections::HashMap::new();
    {
        let mut q = world
            .ecs
            .query::<(bevy_ecs::entity::Entity, Option<&ecology::Diet>)>();
        for (entity, diet) in q.iter(&world.ecs) {
            let id = common::EntityId(entity.to_bits());
            entity_by_id.insert(id, entity);
            if let Some(diet) = diet {
                diet_by_id.insert(id, diet.clone());
            }
        }
    }

    // Quick Organism Search — filters by a case-insensitive substring
    // match against the entity's debug form, diet, or species ID.
    // Applied to `records` directly, which means a matching child in the
    // Ancestry view can lose its (now-filtered-out) parent and render as a
    // root instead — an acceptable tradeoff for "find this organism fast,"
    // not a bug: full-tree context is secondary while actively searching.
    ui.horizontal(|ui| {
        ui.label(egui_remixicon::icons::SEARCH_LINE);
        ui.text_edit_singleline(&mut state.lineage_search);
        if !state.lineage_search.is_empty()
            && ui.small_button(egui_remixicon::icons::CLOSE_LINE).clicked()
        {
            state.lineage_search.clear();
        }
    });
    if !state.lineage_search.is_empty() {
        let needle = state.lineage_search.to_lowercase();
        records.retain(|r| {
            let entity_str = entity_by_id
                .get(&r.entity)
                .map(|e| format!("{e:?}"))
                .unwrap_or_default();
            let diet_str = diet_by_id
                .get(&r.entity)
                .map(|d| format!("{d:?}"))
                .unwrap_or_default();
            entity_str.to_lowercase().contains(&needle)
                || diet_str.to_lowercase().contains(&needle)
                || r.species.0.to_string().contains(&needle)
        });
        if records.is_empty() {
            crate::widgets::empty_state(ui, "No organisms match your search.");
            return;
        }
    }

    ui.horizontal(|ui| {
        ui.selectable_value(
            &mut state.lineage_view,
            crate::LineageView::Ancestry,
            "Ancestry",
        );
        ui.selectable_value(
            &mut state.lineage_view,
            crate::LineageView::Species,
            "Species",
        );
    });
    ui.separator();

    match state.lineage_view {
        crate::LineageView::Ancestry => {
            render_ancestry_tree(ui, state, actions, &records, &diet_by_id, &entity_by_id)
        }
        crate::LineageView::Species => {
            render_species_groups(ui, state, actions, &records, &diet_by_id, &entity_by_id)
        }
    }
}

/// One organism's display row: `Gen <n> — Entity(<idx>v<gen>) [Diet]`, or
/// without the `[Diet]` suffix if this entity has no `ecology::Diet` (should
/// not happen for a real organism, but a lineage record could theoretically
/// outlive its Diet component being queryable in the same frame).
fn lineage_row_label(
    record: &evolution::LineageRecord,
    diet_by_id: &std::collections::HashMap<common::EntityId, ecology::Diet>,
) -> String {
    match diet_by_id.get(&record.entity) {
        Some(diet) => format!("Gen {} — {:?}", record.generation, diet),
        None => format!("Gen {} — Entity", record.generation),
    }
}

fn lineage_row_color(
    record: &evolution::LineageRecord,
    diet_by_id: &std::collections::HashMap<common::EntityId, ecology::Diet>,
) -> egui::Color32 {
    diet_by_id
        .get(&record.entity)
        .map(crate::theme::chart_color)
        .unwrap_or(crate::theme::DISABLED_FG)
}

fn select_lineage_entity(
    actions: &mut Vec<MenuAction>,
    entity_by_id: &std::collections::HashMap<common::EntityId, bevy_ecs::entity::Entity>,
    id: common::EntityId,
) {
    if let Some(&entity) = entity_by_id.get(&id) {
        actions.push(MenuAction::SelectEntity(entity));
    }
}

fn render_ancestry_tree(
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    actions: &mut Vec<MenuAction>,
    records: &[evolution::LineageRecord],
    diet_by_id: &std::collections::HashMap<common::EntityId, ecology::Diet>,
    entity_by_id: &std::collections::HashMap<common::EntityId, bevy_ecs::entity::Entity>,
) {
    let by_entity: std::collections::HashMap<common::EntityId, &evolution::LineageRecord> =
        records.iter().map(|r| (r.entity, r)).collect();
    let mut children: std::collections::HashMap<common::EntityId, Vec<&evolution::LineageRecord>> =
        std::collections::HashMap::new();
    let mut roots: Vec<&evolution::LineageRecord> = Vec::new();
    for r in records {
        match r.parent_id {
            // A parent that's no longer alive (already died, or predates
            // this session's tracking) makes this organism a root too —
            // otherwise it would silently vanish from the tree entirely.
            Some(pid) if by_entity.contains_key(&pid) => {
                children.entry(pid).or_default().push(r);
            }
            _ => roots.push(r),
        }
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for root in &roots {
                draw_lineage_node(
                    ui,
                    state,
                    actions,
                    root,
                    &children,
                    diet_by_id,
                    entity_by_id,
                );
            }
        });
}

/// Recursively draws one ancestry node, same shape as
/// `utils::draw_segment_tree`: a leaf is a plain selectable row, a branch is
/// a default-open `CollapsingHeader` whose own header click *also* selects
/// the organism (not just its children's rows). Hovering either row shape
/// sets `state.panel_hover_entity`, which the viewport's highlight
/// rendering reads alongside its own cursor-picked hover.
fn draw_lineage_node(
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    actions: &mut Vec<MenuAction>,
    record: &evolution::LineageRecord,
    children: &std::collections::HashMap<common::EntityId, Vec<&evolution::LineageRecord>>,
    diet_by_id: &std::collections::HashMap<common::EntityId, ecology::Diet>,
    entity_by_id: &std::collections::HashMap<common::EntityId, bevy_ecs::entity::Entity>,
) {
    let label = egui::RichText::new(lineage_row_label(record, diet_by_id))
        .color(lineage_row_color(record, diet_by_id));
    let kids = children.get(&record.entity);

    if let Some(kids) = kids.filter(|k| !k.is_empty()) {
        let response = egui::CollapsingHeader::new(label)
            .id_salt(record.entity.0)
            .default_open(true)
            .show(ui, |ui| {
                for child in kids {
                    draw_lineage_node(
                        ui,
                        state,
                        actions,
                        child,
                        children,
                        diet_by_id,
                        entity_by_id,
                    );
                }
            });
        if response.header_response.hovered() {
            set_panel_hover(state, entity_by_id, record.entity);
        }
        if response.header_response.clicked() {
            select_lineage_entity(actions, entity_by_id, record.entity);
        }
    } else {
        let response = ui.selectable_label(false, label);
        if response.hovered() {
            set_panel_hover(state, entity_by_id, record.entity);
        }
        if response.clicked() {
            select_lineage_entity(actions, entity_by_id, record.entity);
        }
    }
}

/// Sets `state.panel_hover_entity` for a hovered lineage row, if its
/// `EntityId` still resolves to a live `Entity` this frame.
fn set_panel_hover(
    state: &mut crate::WorkbenchState,
    entity_by_id: &std::collections::HashMap<common::EntityId, bevy_ecs::entity::Entity>,
    id: common::EntityId,
) {
    if let Some(&entity) = entity_by_id.get(&id) {
        state.panel_hover_entity = Some(entity);
    }
}

fn render_species_groups(
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    actions: &mut Vec<MenuAction>,
    records: &[evolution::LineageRecord],
    diet_by_id: &std::collections::HashMap<common::EntityId, ecology::Diet>,
    entity_by_id: &std::collections::HashMap<common::EntityId, bevy_ecs::entity::Entity>,
) {
    let mut by_species: std::collections::BTreeMap<u64, Vec<&evolution::LineageRecord>> =
        std::collections::BTreeMap::new();
    for r in records {
        by_species.entry(r.species.0).or_default().push(r);
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (species_id, members) in &by_species {
                egui::CollapsingHeader::new(format!(
                    "Species #{} ({} members)",
                    species_id,
                    members.len()
                ))
                .id_salt(species_id)
                .default_open(false)
                .show(ui, |ui| {
                    for member in members {
                        let label = egui::RichText::new(lineage_row_label(member, diet_by_id))
                            .color(lineage_row_color(member, diet_by_id));
                        let response = ui.selectable_label(false, label);
                        if response.hovered() {
                            set_panel_hover(state, entity_by_id, member.entity);
                        }
                        if response.clicked() {
                            select_lineage_entity(actions, entity_by_id, member.entity);
                        }
                    }
                });
            }
        });
}

// ─── Environment panel ──────────────────────────────────────────────────────

fn environment_panel(ui: &mut egui::Ui, world: &mut world::World) {
    if let Some(atmo) = world.ecs.get_resource::<metabolism::GlobalAtmosphere>() {
        egui::CollapsingHeader::new(format!("{} Atmosphere", egui_remixicon::icons::CLOUD_LINE))
            .default_open(true)
            .show(ui, |ui| {
                egui::Grid::new("env_atmo").striped(true).show(ui, |ui| {
                    crate::widgets::kv_row_mono(
                        ui,
                        "Sunlight",
                        &format!("{:.1}%", atmo.sunlight * 100.0),
                    );
                    crate::widgets::kv_row_mono(ui, "O₂", &format!("{:.3}", atmo.o2));
                    crate::widgets::kv_row_mono(ui, "CO₂", &format!("{:.3}", atmo.co2));
                    crate::widgets::kv_row_mono(ui, "Temperature", &format!("{:.1}°C", atmo.temp));
                    crate::widgets::kv_row_mono(ui, "Day/Night Tick", &atmo.ticks.to_string());
                });
            });
    } else {
        ui.label(
            egui::RichText::new("GlobalAtmosphere resource not available.")
                .color(crate::theme::DISABLED_FG)
                .italics(),
        );
    }

    // EnvironmentManager not exposed to UI crate — world bounds shown in status bar.
}

// ─── Analytics panel ────────────────────────────────────────────────────────

fn analytics_panel(ui: &mut egui::Ui, world: &mut world::World) {
    if let Some(metrics) = world.ecs.get_resource::<analytics::MetricsState>() {
        egui::Grid::new("ana_grid").striped(true).show(ui, |ui| {
            crate::widgets::kv_row_mono(ui, "Sim Time", &format!("{:.1}s", metrics.sim_time));
            crate::widgets::kv_row_mono(ui, "FPS", &format!("{:.0}", metrics.smoothed_fps));
            crate::widgets::kv_row_mono(ui, "TPS", &format!("{:.0}", metrics.smoothed_tps));

            // Latest population counts from history
            let latest = |hist: &std::collections::VecDeque<[f64; 2]>| {
                hist.back().map(|p| p[1] as usize).unwrap_or(0)
            };
            crate::widgets::kv_row_colored(
                ui,
                "Producers",
                &latest(&metrics.producers_history).to_string(),
                crate::theme::chart_color(&ecology::Diet::Producer),
            );
            crate::widgets::kv_row_colored(
                ui,
                "Herbivores",
                &latest(&metrics.herbivores_history).to_string(),
                crate::theme::chart_color(&ecology::Diet::Herbivore),
            );
            crate::widgets::kv_row_colored(
                ui,
                "Carnivores",
                &latest(&metrics.carnivores_history).to_string(),
                crate::theme::chart_color(&ecology::Diet::Carnivore),
            );
            crate::widgets::kv_row_colored(
                ui,
                "Omnivores",
                &latest(&metrics.omnivores_history).to_string(),
                crate::theme::chart_color(&ecology::Diet::Omnivore),
            );
        });
    } else {
        ui.label(
            egui::RichText::new("MetricsState not available.")
                .color(crate::theme::DISABLED_FG)
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
    ui.label(egui::RichText::new("Spawn at camera position:").color(crate::theme::DISABLED_FG));
    ui.add_space(crate::theme::SPACE_XS);

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
            ui.checkbox(&mut state.debug_structural, "Wireframe View");
            ui.checkbox(&mut state.show_vision_cones, "Show Vision Cones");
            ui.add_space(crate::theme::SPACE_XS);
            ui.add(
                egui::Slider::new(&mut state.bone_line_thickness, 0.5..=5.0).text("Bone Thickness"),
            );
            ui.add(egui::Slider::new(&mut state.skin_thickness, 1.0..=10.0).text("Skin Thickness"));
            ui.add(egui::Slider::new(&mut state.node_radius, 2.0..=20.0).text("Node Radius"));
        });

    egui::CollapsingHeader::new(format!(
        "{} Clipping Plane",
        egui_remixicon::icons::SCISSORS_LINE
    ))
    .default_open(false)
    .show(ui, |ui| {
        // A horizontal world-space Z-plane the organism renderer clips
        // fragments against, letting the user slice into a dense
        // population to see inside it.
        ui.checkbox(&mut state.clip_plane.enabled, "Enabled");
        ui.add_enabled_ui(state.clip_plane.enabled, |ui| {
            ui.add(
                egui::Slider::new(&mut state.clip_plane.height, -20.0..=20.0).text("Height (Z)"),
            );
            ui.horizontal(|ui| {
                ui.label("Keep:");
                ui.selectable_value(&mut state.clip_plane.keep_above, true, "Above");
                ui.selectable_value(&mut state.clip_plane.keep_above, false, "Below");
            });
        });
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

    // A live colorblind preview would need a real color-transform
    // pipeline (a `palette`-crate-based trigger) and is out of scope for
    // this toggle — see `theme::apply_style`'s doc comment.
    egui::CollapsingHeader::new("Accessibility")
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut state.high_contrast, "High Contrast Mode");
            ui.horizontal(|ui| {
                ui.label("UI Scale");
                ui.add(egui::Slider::new(&mut state.ui_scale, 0.5..=2.0).text("×"));
            });
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
        crate::SidebarTab::Lineage => egui_remixicon::icons::TREE_LINE,
        crate::SidebarTab::HoxVisualizer => egui_remixicon::icons::MICROSCOPE_LINE,
        crate::SidebarTab::GrnViewer => egui_remixicon::icons::BUBBLE_CHART_LINE,
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
        crate::SidebarTab::Lineage => "Lineage",
        crate::SidebarTab::HoxVisualizer => "HOX Visualizer",
        crate::SidebarTab::GrnViewer => "GRN Viewer",
        crate::SidebarTab::Environment => "Environment",
        crate::SidebarTab::Analytics => "Snapshot",
        crate::SidebarTab::Sandbox => "Sandbox",
        crate::SidebarTab::Tuning => "Simulation Parameters",
        crate::SidebarTab::Settings => "Settings",
    }
}
