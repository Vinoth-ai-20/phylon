//! Status bar plugin — live simulation metrics strip at the bottom of the window.

use crate::types::*;

/// Render the status bar strip.
#[allow(clippy::too_many_arguments)]
pub fn status_bar_ui(
    _ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
    _actions: &mut Vec<MenuAction>,
) {
    ui.horizontal(|ui| {
        // ── Simulation timing ─────────────────────────────────────────────
        let (fps, tps, sim_time) = world
            .ecs
            .get_resource::<analytics::MetricsState>()
            .map(|m| (m.smoothed_fps, m.smoothed_tps, m.sim_time))
            .unwrap_or((0.0, 0.0, 0.0));

        let tick_count = (sim_time / 0.016).round() as u64;
        ui.label(format!("{} Tick: {}", egui_remixicon::icons::TIMER_LINE, tick_count));
        ui.separator();
        ui.label(format!("{} FPS: {:.0}", egui_remixicon::icons::SPEED_LINE, fps));
        ui.separator();
        ui.label(format!("TPS: {:.0}", tps));
        ui.separator();

        // ── Entity counts ─────────────────────────────────────────────────
        let entity_count = world.ecs.entities().len();
        ui.label(format!("{} Entities: {}", egui_remixicon::icons::BUG_LINE, entity_count));
        ui.separator();

        // ── Render mode ───────────────────────────────────────────────────
        ui.label(if state.debug_structural {
            format!("{} Structural", egui_remixicon::icons::EYE_LINE)
        } else {
            format!("{} SDF Skin", egui_remixicon::icons::EYE_LINE)
        });
        ui.separator();

        // ── Organism diet counts ──────────────────────────────────────────
        let food_count = world.ecs.query::<&ecology::FoodPellet>().iter(&world.ecs).count();
        let mineral_count = world.ecs.query::<&ecology::MineralPellet>().iter(&world.ecs).count();
        let corpse_count = world.ecs.query::<&ecology::Corpse>().iter(&world.ecs).count();

        let mut prod_count = 0usize;
        let mut herb_count = 0usize;
        let mut carn_count = 0usize;
        let mut omni_count = 0usize;
        let mut deco_count = 0usize;
        for diet in world.ecs.query::<&ecology::Diet>().iter(&world.ecs) {
            match diet {
                ecology::Diet::Producer => prod_count += 1,
                ecology::Diet::Herbivore => herb_count += 1,
                ecology::Diet::Carnivore => carn_count += 1,
                ecology::Diet::Omnivore => omni_count += 1,
                ecology::Diet::Decomposer => deco_count += 1,
            }
        }

        ui.label(format!("{} P:{} H:{} C:{} O:{} D:{}", egui_remixicon::icons::TEAM_LINE, prod_count, herb_count, carn_count, omni_count, deco_count));
        ui.separator();
        ui.label(format!(
            "Food: {}  Minerals: {}  Corpses: {}",
            food_count, mineral_count, corpse_count
        ));

        // ── Right side ────────────────────────────────────────────────────
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Memory
            thread_local! {
                static SYS: std::cell::RefCell<sysinfo::System> = std::cell::RefCell::new(sysinfo::System::new());
            }
            let mem_mb = SYS.with(|sys_cell| {
                let mut sys = sys_cell.borrow_mut();
                if let Ok(pid) = sysinfo::get_current_pid() {
                    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
                    if let Some(process) = sys.process(pid) {
                        return process.memory() / 1024 / 1024;
                    }
                }
                0
            });
            ui.label(format!("Mem: {}MB", mem_mb));
            ui.separator();

            // Engine status
            ui.label(
                egui::RichText::new(format!("{} Engine Online", egui_remixicon::icons::SERVER_LINE))
                    .color(egui::Color32::GREEN),
            );
            ui.separator();

            // Active overlay
            let overlay_name = world
                .ecs
                .get_resource::<HeatmapState>()
                .map(|h| crate::plugins::toolbar::heatmap_label(h.active))
                .unwrap_or("None");
            if overlay_name != "None" {
                ui.label(
                    egui::RichText::new(format!("{} {}", egui_remixicon::icons::MAP_LINE, overlay_name))
                        .color(egui::Color32::LIGHT_BLUE),
                );
                ui.separator();
            }

            // Playback state
            let (pb_icon, pb_color) = if state.is_paused {
                (egui_remixicon::icons::PAUSE_CIRCLE_LINE, egui::Color32::from_rgb(255, 150, 50))
            } else {
                (egui_remixicon::icons::PLAY_CIRCLE_LINE, egui::Color32::LIGHT_GREEN)
            };
            ui.label(
                egui::RichText::new(format!("{} {:.1}×", pb_icon, state.simulation_speed))
                    .color(pb_color),
            );
            ui.separator();

            // Selected entity
            if let Some(entity) = state.selected_entity {
                ui.label(
                    egui::RichText::new(format!("{} {:?}", egui_remixicon::icons::CURSOR_LINE, entity))
                        .color(egui::Color32::LIGHT_GREEN)
                        .size(11.0),
                );
                ui.separator();
            }

            // Camera
            ui.label(
                egui::RichText::new(format!(
                    "Cam ({:.0}, {:.0}) ×{:.1}",
                    state.camera_pos.x, state.camera_pos.y, state.camera_zoom
                ))
                .color(egui::Color32::GRAY)
                .size(11.0),
            );
        });
    });
}
