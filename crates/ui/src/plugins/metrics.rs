use crate::types::*;

/// Renders the Metrics Dashboard (4-plot grid).
///
/// This is its own docked tile/tab (see `layout::rebuild_tree_from_modes`) —
/// it used to also carry an internal "Metrics Dashboard / Event Log" tab bar
/// and duplicate the Event Log content, which was redundant with the
/// separate "Event Log" tile (rendered by `event_log::event_log_ui`) and
/// made both "Metrics" and "Event Log" appear twice in the UI.
///
/// **Future Scope (explicitly out of scope here — see
/// `docs/design/design_system.md` / the roadmap's Milestone 7 note):**
/// zoom/pan, time-range selection, data export, running statistics,
/// smoothing, multiple Y-axes, per-series visibility toggles, and saved
/// chart presets would make this a full scientific-analytics workspace
/// rather than a dashboard — tracked as a follow-on initiative, not
/// silently expanded here.
#[allow(clippy::too_many_arguments)]
pub fn metrics_ui(
    _ctx: &egui::Context,
    ui: &mut egui::Ui,
    _state: &mut crate::WorkbenchState,
    world: &mut world::World,
    _actions: &mut [MenuAction],
) {
    let Some(metrics) = world.ecs.get_resource::<analytics::MetricsState>() else {
        crate::widgets::empty_state(ui, "Metrics not yet available.");
        return;
    };

    let to_pts = |hist: &std::collections::VecDeque<[f64; 2]>| -> egui_plot::PlotPoints {
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

    let producer_c = crate::theme::chart_color(&ecology::Diet::Producer);
    let herbivore_c = crate::theme::chart_color(&ecology::Diet::Herbivore);
    let carnivore_c = crate::theme::chart_color(&ecology::Diet::Carnivore);
    let omnivore_c = crate::theme::chart_color(&ecology::Diet::Omnivore);
    let decomposer_c = crate::theme::chart_color(&ecology::Diet::Decomposer);

    // Split the available height between the 2 stacked plots per column so
    // they fill however much space the tile/window gives us (previously a
    // hardcoded 120.0 left dead space below when the panel was resized or
    // detached taller).
    let plot_height = ((ui.available_height() - 100.0) / 2.0).max(80.0);

    // Plots use a tighter item spacing than the rest of the app:
    // `theme::apply_style`'s app-wide `item_spacing.y` is sized for
    // buttons/lists/panels, but a legend built from the same spacing token
    // would grow tall enough to overlap the plot's own axis labels/
    // gridlines. Scoped here rather than lowering the app-wide token
    // globally.
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;

        ui.columns(2, |cols| {
            // Column 1
            cols[0].vertical(|ui| {
                ui.label(egui::RichText::new("Demographics").strong());
                ui.horizontal_wrapped(|ui| {
                    crate::widgets::chart_legend_dot(ui, producer_c, "Producers");
                    crate::widgets::chart_legend_dot(ui, herbivore_c, "Herbivores");
                    crate::widgets::chart_legend_dot(ui, carnivore_c, "Carnivores");
                    crate::widgets::chart_legend_dot(ui, omnivore_c, "Omnivores");
                    crate::widgets::chart_legend_dot(ui, decomposer_c, "Decomposers");
                });
                egui_plot::Plot::new("pop_plot")
                    .height(plot_height)
                    .x_axis_formatter(|x, _range| format!("{:.1}s", x.value))
                    .y_axis_formatter(|y, _range| format!("{:.0}", y.value))
                    .y_axis_label("Population (count)")
                    .show(ui, |plot_ui| {
                        plot_ui.line(
                            egui_plot::Line::new(prod_pts)
                                .name("Producers")
                                .color(producer_c),
                        );
                        plot_ui.line(
                            egui_plot::Line::new(herb_pts)
                                .name("Herbivores")
                                .color(herbivore_c),
                        );
                        plot_ui.line(
                            egui_plot::Line::new(carn_pts)
                                .name("Carnivores")
                                .color(carnivore_c),
                        );
                        plot_ui.line(
                            egui_plot::Line::new(omni_pts)
                                .name("Omnivores")
                                .color(omnivore_c),
                        );
                        plot_ui.line(
                            egui_plot::Line::new(deco_pts)
                                .name("Decomposers")
                                .color(decomposer_c),
                        );
                    });

                ui.add_space(crate::theme::SPACE_SM);

                ui.label(egui::RichText::new("Performance").strong());
                ui.horizontal_wrapped(|ui| {
                    crate::widgets::chart_legend_dot(ui, egui::Color32::WHITE, "FPS");
                    crate::widgets::chart_legend_dot(ui, egui::Color32::LIGHT_GREEN, "TPS");
                    crate::widgets::chart_legend_dot(ui, egui::Color32::LIGHT_RED, "Mem (MB)");
                });
                egui_plot::Plot::new("perf_plot")
                    .height(plot_height)
                    .x_axis_formatter(|x, _range| format!("{:.1}s", x.value))
                    .y_axis_label("Frames/Ticks per sec · MB")
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
                ui.label(egui::RichText::new("Resources").strong());
                ui.horizontal_wrapped(|ui| {
                    crate::widgets::chart_legend_dot(
                        ui,
                        egui::Color32::from_rgb(150, 255, 255),
                        "Food",
                    );
                    crate::widgets::chart_legend_dot(
                        ui,
                        egui::Color32::from_rgb(150, 150, 150),
                        "Minerals",
                    );
                    crate::widgets::chart_legend_dot(
                        ui,
                        egui::Color32::from_rgb(200, 100, 100),
                        "Corpses",
                    );
                });
                egui_plot::Plot::new("res_plot")
                    .height(plot_height)
                    .x_axis_formatter(|x, _range| format!("{:.1}s", x.value))
                    .y_axis_formatter(|y, _range| format!("{:.0}", y.value))
                    .y_axis_label("Resource count")
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

                ui.add_space(crate::theme::SPACE_SM);

                ui.label(egui::RichText::new("Environment").strong());
                ui.horizontal_wrapped(|ui| {
                    crate::widgets::chart_legend_dot(ui, egui::Color32::YELLOW, "Sunlight");
                    crate::widgets::chart_legend_dot(ui, egui::Color32::LIGHT_BLUE, "O2");
                    crate::widgets::chart_legend_dot(ui, egui::Color32::GRAY, "CO2");
                    crate::widgets::chart_legend_dot(
                        ui,
                        egui::Color32::from_rgb(255, 165, 0),
                        "Temp (°C)",
                    );
                });
                egui_plot::Plot::new("env_plot")
                    .height(plot_height)
                    .x_axis_formatter(|x, _range| format!("{:.1}s", x.value))
                    .y_axis_label("Sunlight/gas fraction · °C")
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
    });
}
