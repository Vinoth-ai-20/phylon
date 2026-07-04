use crate::types::*;

/// Renders the Metrics Dashboard (4-plot grid).
///
/// This is its own docked tile/tab (see `layout::rebuild_tree_from_modes`) —
/// it used to also carry an internal "Metrics Dashboard / Event Log" tab bar
/// and duplicate the Event Log content, which was redundant with the
/// separate "Event Log" tile (rendered by `event_log::event_log_ui`) and
/// made both "Metrics" and "Event Log" appear twice in the UI.
#[allow(clippy::too_many_arguments)]
pub fn metrics_ui(
    _ctx: &egui::Context,
    ui: &mut egui::Ui,
    _state: &mut crate::WorkbenchState,
    world: &mut world::World,
    _actions: &mut [MenuAction],
) {
    {
        {
            if let Some(metrics) = world.ecs.get_resource::<analytics::MetricsState>() {
                let to_pts =
                    |hist: &std::collections::VecDeque<[f64; 2]>| -> egui_plot::PlotPoints {
                        hist.iter().copied().collect()
                    };

                let prod_pts = to_pts(&metrics.producers_history);
                let herb_pts = to_pts(&metrics.herbivores_history);
                let carn_pts = to_pts(&metrics.carnivores_history);
                let omni_pts = to_pts(&metrics.omnivores_history);
                let deco_pts = to_pts(&metrics.decomposers_history);
                let food_pts = to_pts(&metrics.food_history);
                let min_pts = to_pts(&metrics.minerals_history);
                let corp_pts = to_pts(&metrics.corpses_history);

                let fps_pts = to_pts(&metrics.fps_history);
                let tps_pts = to_pts(&metrics.tps_history);
                let mem_pts = to_pts(&metrics.memory_history);
                let sun_pts = to_pts(&metrics.sunlight_history);
                let o2_pts = to_pts(&metrics.o2_history);
                let co2_pts = to_pts(&metrics.co2_history);
                let temp_pts = to_pts(&metrics.temp_history);

                // Split the available height between the 2 stacked plots per
                // column so they fill however much space the tile/window
                // gives us (previously a hardcoded 120.0 left dead space
                // below when the panel was resized or detached taller).
                let plot_height = ((ui.available_height() - 60.0) / 2.0).max(80.0);

                ui.columns(2, |cols| {
                    // Column 1
                    cols[0].vertical(|ui| {
                        ui.label("Demographics");
                        egui_plot::Plot::new("pop_plot")
                            .height(plot_height)
                            .legend(egui_plot::Legend::default())
                            .x_axis_formatter(|x, _range| format!("{:.1}s", x.value))
                            .show(ui, |plot_ui| {
                                plot_ui.line(
                                    egui_plot::Line::new(prod_pts)
                                        .name("Producers")
                                        .color(egui::Color32::from_rgb(100, 255, 100)),
                                );
                                plot_ui.line(
                                    egui_plot::Line::new(herb_pts)
                                        .name("Herbivores")
                                        .color(egui::Color32::from_rgb(200, 255, 150)),
                                );
                                plot_ui.line(
                                    egui_plot::Line::new(carn_pts)
                                        .name("Carnivores")
                                        .color(egui::Color32::from_rgb(255, 100, 100)),
                                );
                                plot_ui.line(
                                    egui_plot::Line::new(omni_pts)
                                        .name("Omnivores")
                                        .color(egui::Color32::from_rgb(255, 200, 100)),
                                );
                                plot_ui.line(
                                    egui_plot::Line::new(deco_pts)
                                        .name("Decomposers")
                                        .color(egui::Color32::from_rgb(200, 150, 200)),
                                );
                            });

                        ui.add_space(8.0);

                        ui.label("Performance (FPS, TPS, Mem)");
                        egui_plot::Plot::new("perf_plot")
                            .height(plot_height)
                            .legend(egui_plot::Legend::default())
                            .x_axis_formatter(|x, _range| format!("{:.1}s", x.value))
                            .show(ui, |plot_ui| {
                                plot_ui.line(
                                    egui_plot::Line::new(fps_pts)
                                        .name("FPS")
                                        .color(egui::Color32::WHITE),
                                );
                                plot_ui.line(
                                    egui_plot::Line::new(tps_pts)
                                        .name("TPS")
                                        .color(egui::Color32::LIGHT_GREEN),
                                );
                                plot_ui.line(
                                    egui_plot::Line::new(mem_pts)
                                        .name("Mem (MB)")
                                        .color(egui::Color32::LIGHT_RED),
                                );
                            });
                    });

                    // Column 2
                    cols[1].vertical(|ui| {
                        ui.label("Resources");
                        egui_plot::Plot::new("res_plot")
                            .height(plot_height)
                            .legend(egui_plot::Legend::default())
                            .x_axis_formatter(|x, _range| format!("{:.1}s", x.value))
                            .show(ui, |plot_ui| {
                                plot_ui.line(
                                    egui_plot::Line::new(food_pts)
                                        .name("Food")
                                        .color(egui::Color32::from_rgb(150, 255, 255)),
                                );
                                plot_ui.line(
                                    egui_plot::Line::new(min_pts)
                                        .name("Minerals")
                                        .color(egui::Color32::from_rgb(150, 150, 150)),
                                );
                                plot_ui.line(
                                    egui_plot::Line::new(corp_pts)
                                        .name("Corpses")
                                        .color(egui::Color32::from_rgb(200, 100, 100)),
                                );
                            });

                        ui.add_space(8.0);

                        ui.label("Environment");
                        egui_plot::Plot::new("env_plot")
                            .height(plot_height)
                            .legend(egui_plot::Legend::default())
                            .x_axis_formatter(|x, _range| format!("{:.1}s", x.value))
                            .show(ui, |plot_ui| {
                                plot_ui.line(
                                    egui_plot::Line::new(sun_pts)
                                        .name("Sunlight")
                                        .color(egui::Color32::YELLOW),
                                );
                                plot_ui.line(
                                    egui_plot::Line::new(o2_pts)
                                        .name("O2")
                                        .color(egui::Color32::LIGHT_BLUE),
                                );
                                plot_ui.line(
                                    egui_plot::Line::new(co2_pts)
                                        .name("CO2")
                                        .color(egui::Color32::GRAY),
                                );
                                plot_ui.line(
                                    egui_plot::Line::new(temp_pts)
                                        .name("Temp (°C)")
                                        .color(egui::Color32::from_rgb(255, 165, 0)),
                                );
                            });
                    });
                });
            } else {
                ui.label("Metrics not yet available.");
            }
        }
    }
}
