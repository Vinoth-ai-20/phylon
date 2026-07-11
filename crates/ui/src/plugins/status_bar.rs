//! Status bar plugin — live simulation metrics strip at the bottom of the window.
//!
//! Organized into three zones per `docs/design/layout.md`: **Simulation**
//! (tick/fps/tps/playback/overlay/selection — what's happening right now),
//! **Population** (entity/diet/resource counts — what exists right now),
//! and **System** (engine/memory/render-mode/camera — operational detail
//! that matters less continuously, so it hides behind a hover tooltip
//! instead of taking up permanent strip width).

use crate::types::*;

/// Renders `text` as a tight-spaced (zero item-spacing) horizontal run of
/// labels, so a "prefix: number suffix" reads as one continuous label
/// instead of separate widgets with a full item-spacing gap between them.
/// Used to give every live-updating number tabular (`.monospace()`) digits
/// — see `docs/design/typography.md` — without also forcing the icon glyph
/// that usually precedes it into the Monospace font family, which doesn't
/// carry the Remix Icon glyphs and would render a tofu box instead.
fn tight_row(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    let saved = ui.spacing().item_spacing.x;
    ui.spacing_mut().item_spacing.x = 0.0;
    ui.horizontal(add_contents);
    ui.spacing_mut().item_spacing.x = saved;
}

fn mono(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(egui::RichText::new(text.into()).monospace());
}

/// A vertical rule between status-bar zones, more prominent than the
/// item-level `ui.separator()` used between fields inside a zone.
fn zone_separator(ui: &mut egui::Ui) {
    ui.add_space(crate::theme::SPACE_SM);
    ui.separator();
    ui.add_space(crate::theme::SPACE_SM);
}

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
        // ── Zone 1: Simulation ──────────────────────────────────────────────
        let (fps, tps, sim_time) = world
            .ecs
            .get_resource::<analytics::MetricsState>()
            .map(|m| (m.smoothed_fps, m.smoothed_tps, m.sim_time))
            .unwrap_or((0.0, 0.0, 0.0));
        let dt = world
            .ecs
            .get_resource::<common::TickRate>()
            .map(|r| r.dt() as f64)
            .unwrap_or(1.0 / 60.0);
        let tick_count = (sim_time / dt).round() as u64;

        crate::widgets::status_chip(ui, egui_remixicon::icons::TIMER_LINE, tick_count.to_string(), None);
        ui.separator();
        crate::widgets::status_chip(ui, egui_remixicon::icons::SPEED_LINE, format!("{:.0}", fps), None);
        ui.separator();
        crate::widgets::status_chip(ui, "TPS", format!("{:.0}", tps), None);
        ui.separator();

        let (pb_icon, pb_color) = if state.is_paused {
            (
                egui_remixicon::icons::PAUSE_CIRCLE_LINE,
                crate::theme::PLAYBACK_PAUSED,
            )
        } else {
            (
                egui_remixicon::icons::PLAY_CIRCLE_LINE,
                crate::theme::PLAYBACK_LIVE,
            )
        };
        crate::widgets::status_chip(
            ui,
            pb_icon,
            format!("{:.1}×", state.simulation_speed),
            Some(pb_color),
        );

        // Active overlay — only takes strip space when one is active.
        let overlay_name = world
            .ecs
            .get_resource::<HeatmapState>()
            .map(|h| crate::plugins::toolbar::heatmap_label(h.active))
            .unwrap_or("None");
        if overlay_name != "None" {
            ui.separator();
            crate::widgets::status_chip(
                ui,
                egui_remixicon::icons::MAP_LINE,
                overlay_name,
                Some(egui::Color32::LIGHT_BLUE),
            );
        }

        // Selected entity — only takes strip space when something is selected.
        if let Some(entity) = state.selected_entity {
            ui.separator();
            crate::widgets::status_chip(
                ui,
                egui_remixicon::icons::CURSOR_LINE,
                format!("{:?}", entity),
                Some(crate::theme::GOOD),
            );
        }

        // Cursor world-space position (Phase 2, M10) — only takes strip
        // space while the cursor is actually over the viewport.
        if let Some(pos) = state.cursor_world_pos {
            ui.separator();
            crate::widgets::status_chip(
                ui,
                egui_remixicon::icons::CROSSHAIR_LINE,
                format!("{:.0}, {:.0}", pos.x, pos.y),
                None,
            );
        }

        zone_separator(ui);

        // ── Zone 2: Population ───────────────────────────────────────────────
        let entity_count = world.ecs.entities().len();
        crate::widgets::status_chip(ui, egui_remixicon::icons::BUG_LINE, entity_count.to_string(), None);
        ui.separator();

        // Phase 9, P9.1 (performance foundation): these population-wide
        // counts were previously recomputed via 6 full-population ECS
        // queries every single frame, unconditionally — the status bar has
        // no visibility gate (unlike Metrics, which only pays this cost
        // while its panel is open), so this ran regardless of whether the
        // simulation was even paused. A status strip refreshing every
        // `COUNT_REFRESH_INTERVAL` frames (~0.25s at 60Hz) is visually
        // indistinguishable from every-frame updates for slowly-changing
        // population counts, while cutting this cost to a fraction of its
        // former total. Cached in a `thread_local!`, matching the existing
        // `SYS`/memory-probe pattern immediately below in this same file.
        const COUNT_REFRESH_INTERVAL: u32 = 15;
        struct CachedCounts {
            frame: u32,
            food: usize,
            mineral: usize,
            corpse: usize,
            prod: usize,
            herb: usize,
            carn: usize,
            omni: usize,
            deco: usize,
            hunting: usize,
            diseased: usize,
        }
        thread_local! {
            static COUNTS: std::cell::RefCell<CachedCounts> = const {
                std::cell::RefCell::new(CachedCounts {
                    frame: 0, food: 0, mineral: 0, corpse: 0,
                    prod: 0, herb: 0, carn: 0, omni: 0, deco: 0,
                    hunting: 0, diseased: 0,
                })
            };
        }
        let (
            food_count,
            mineral_count,
            corpse_count,
            prod_count,
            herb_count,
            carn_count,
            omni_count,
            deco_count,
            hunting_count,
            diseased_count,
        ) = COUNTS.with(|cell| {
            let mut c = cell.borrow_mut();
            c.frame = c.frame.wrapping_add(1);
            if c.frame % COUNT_REFRESH_INTERVAL == 0 {
                c.food = world.ecs.query::<&ecology::FoodPellet>().iter(&world.ecs).count();
                c.mineral = world.ecs.query::<&ecology::MineralPellet>().iter(&world.ecs).count();
                c.corpse = world.ecs.query::<&ecology::Corpse>().iter(&world.ecs).count();

                let (mut prod, mut herb, mut carn, mut omni, mut deco) = (0usize, 0usize, 0usize, 0usize, 0usize);
                for diet in world.ecs.query::<&ecology::Diet>().iter(&world.ecs) {
                    match diet {
                        ecology::Diet::Producer => prod += 1,
                        ecology::Diet::Herbivore => herb += 1,
                        ecology::Diet::Carnivore => carn += 1,
                        ecology::Diet::Omnivore => omni += 1,
                        ecology::Diet::Decomposer => deco += 1,
                    }
                }
                (c.prod, c.herb, c.carn, c.omni, c.deco) = (prod, herb, carn, omni, deco);

                c.hunting = world
                    .ecs
                    .query::<&behavior::BehaviorState>()
                    .iter(&world.ecs)
                    .filter(|s| **s == behavior::BehaviorState::Hunting)
                    .count();
                c.diseased = world
                    .ecs
                    .query::<&ecology::disease::Infection>()
                    .iter(&world.ecs)
                    .filter(|i| i.state == ecology::disease::InfectionState::Infectious)
                    .count();
            }
            (
                c.food, c.mineral, c.corpse, c.prod, c.herb, c.carn, c.omni, c.deco,
                c.hunting, c.diseased,
            )
        });

        tight_row(ui, |ui| {
            ui.label(format!("{} P:", egui_remixicon::icons::TEAM_LINE));
            mono(ui, prod_count.to_string());
            ui.label(" H:");
            mono(ui, herb_count.to_string());
            ui.label(" C:");
            mono(ui, carn_count.to_string());
            ui.label(" O:");
            mono(ui, omni_count.to_string());
            ui.label(" D:");
            mono(ui, deco_count.to_string());
        });
        ui.separator();
        tight_row(ui, |ui| {
            ui.label(format!("{} Food: ", egui_remixicon::icons::RESTAURANT_LINE));
            mono(ui, food_count.to_string());
            ui.label(format!("  {} Minerals: ", egui_remixicon::icons::COPPER_DIAMOND_LINE));
            mono(ui, mineral_count.to_string());
            ui.label(format!("  {} Corpses: ", egui_remixicon::icons::SKULL_LINE));
            mono(ui, corpse_count.to_string());
        });

        zone_separator(ui);

        // ── Zone 3: Behavior (Phase 5, SX-8c) ────────────────────────────────
        // Reuses the per-organism `BehaviorState`/`Infection` data SX-1b/1d
        // already added (population-wide behavior glyphs, disease tint) —
        // this is the same data aggregated into a status-bar count instead
        // of a second query mechanism. `hunting_count`/`diseased_count`
        // computed above, alongside the other Zone-2 counts (P9.1).
        tight_row(ui, |ui| {
            ui.label(format!(
                "{} Hunting: ",
                egui_remixicon::icons::CROSSHAIR_2_LINE
            ));
            mono(ui, hunting_count.to_string());
        });
        ui.separator();
        tight_row(ui, |ui| {
            ui.label(egui::RichText::new(format!(
                "{} Diseased: ",
                egui_remixicon::icons::VIRUS_LINE
            )).color(if diseased_count > 0 { crate::theme::WARN } else { ui.visuals().text_color() }));
            mono(ui, diseased_count.to_string());
        });

        // ── Zone 4: System (hover-reveal) ────────────────────────────────────
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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

            // A single compact "System" chip stands in for engine/memory/
            // render-mode/camera detail — full values reveal on hover
            // rather than occupying permanent strip width, per the
            // Interaction Design Standards (hover-reveal for low-priority
            // continuous detail).
            let render_mode = if state.debug_structural { "Wireframe" } else { "SDF Skin" };
            let camera_pos = state.camera_pos_2d();
            let hover_text = format!(
                "Engine Online\nMemory: {mem_mb} MB\nRender Mode: {render_mode}\nCamera: ({:.0}, {:.0}) × {:.1}",
                camera_pos.x, camera_pos.y, state.camera_zoom_2d()
            );
            ui.label(
                egui::RichText::new(format!("{} System", egui_remixicon::icons::SERVER_LINE))
                    .color(crate::theme::GOOD)
                    .size(crate::theme::SIZE_SMALL),
            )
            .on_hover_text(hover_text);
        });
    });
}
