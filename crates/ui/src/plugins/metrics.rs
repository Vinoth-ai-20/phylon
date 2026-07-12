use crate::types::*;

/// Window size (in samples) for the running-mean overlay — referenced by
/// `state::MetricsSeriesOptions`'s doc comment. 10 samples at the panel's
/// typical sampling rate smooths frame-to-frame demographic noise without
/// lagging far enough behind a real population swing to look misleading.
pub(crate) const RUNNING_MEAN_WINDOW: usize = 10;

/// Trailing running mean over `RUNNING_MEAN_WINDOW` samples, operating on
/// `[sim_time_s, value]` pairs. The X value is copied
/// from the *last* sample in each window (not re-averaged) so the overlay
/// lines up with the raw line it's smoothing rather than lagging on both
/// axes. A free function (not a closure inline in `metrics_ui`) so it has
/// its own unit tests below, independent of egui/`MetricsState`.
pub(crate) fn running_mean(pts: &[[f64; 2]]) -> Vec<[f64; 2]> {
    pts.iter()
        .enumerate()
        .map(|(i, _)| {
            let start = i.saturating_sub(RUNNING_MEAN_WINDOW - 1);
            let window = &pts[start..=i];
            let avg = window.iter().map(|p| p[1]).sum::<f64>() / window.len() as f64;
            [pts[i][0], avg]
        })
        .collect()
}

/// Renders the Metrics Dashboard (a 2-column, 3-row grid of 6 time-series
/// plots, plus one full-width snapshot bar chart below — Species
/// Distribution).
///
/// This is its own docked tile/tab (see `layout::rebuild_tree_from_modes`),
/// deliberately separate from the "Event Log" tile (rendered by
/// `event_log::event_log_ui`) rather than an internal tab bar duplicating
/// that content — each has its own dock slot, not two views of the same
/// data.
///
/// Includes hazard/predation/lineage markers (from `analytics::NarrationLog`)
/// on the Demographics time axis, per-series visibility toggles and a
/// running-mean overlay on Demographics/Diversity, and per-chart PNG export.
///
/// **Future Scope (explicitly out of scope for now — see
/// `docs/design/design_system.md`):** zoom/pan, time-range selection,
/// smoothing beyond the one running-mean window, multiple Y-axes, and saved
/// chart presets would make this a full scientific-analytics workspace
/// rather than a dashboard — tracked as a follow-on initiative, not
/// silently expanded here.
#[allow(clippy::too_many_arguments)]
pub fn metrics_ui(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
    actions: &mut Vec<MenuAction>,
) {
    let Some(metrics) = world.ecs.get_resource::<analytics::MetricsState>() else {
        crate::widgets::empty_state(ui, "Metrics not yet available.");
        return;
    };

    // Hazard/predation/lineage markers on the Demographics time axis —
    // `NarrationLog`'s own doc comment names exactly this use case
    // ("contextualizing why a population graph suddenly dropped"). Ticks are
    // converted to the same `sim_time` (seconds) unit the plots' X axis
    // already uses via `TickRate::dt()` — `MetricsState::sim_time` and
    // `GlobalAtmosphere::ticks` both advance once per simulation tick by the
    // same per-frame `ticks_to_run` (see `crates/app/src/render.rs`), so
    // `tick as f64 * dt` lands on the same time axis without needing new
    // plumbing.
    let dt = world
        .ecs
        .get_resource::<common::TickRate>()
        .map(|r| r.dt())
        .unwrap_or(0.0) as f64;
    let narrative_events: Vec<(f64, egui::Color32, String)> = world
        .ecs
        .get_resource::<analytics::NarrationLog>()
        .map(|log| {
            log.events
                .iter()
                .map(|e| {
                    let color = match e.event_type.as_str() {
                        "Hazard" => crate::theme::WARN,
                        "Predation" => crate::theme::BAD,
                        "Lineage" => crate::theme::GOOD,
                        _ => crate::theme::ACCENT,
                    };
                    (e.tick as f64 * dt, color, e.description.clone())
                })
                .collect()
        })
        .unwrap_or_default();

    ui.horizontal(|ui| {
        if ui
            .button(format!(
                "{} Export CSV",
                egui_remixicon::icons::DOWNLOAD_LINE
            ))
            .clicked()
        {
            actions.push(MenuAction::ExportMetricsCsv);
        }
        if ui
            .button(format!(
                "{} Export JSON",
                egui_remixicon::icons::DOWNLOAD_LINE
            ))
            .clicked()
        {
            actions.push(MenuAction::ExportMetricsJson);
        }
    });
    ui.add_space(crate::theme::SPACE_SM);

    let to_pts = |hist: &std::collections::VecDeque<[f64; 2]>| -> Vec<[f64; 2]> {
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

    let shannon_pts = to_pts(&metrics.shannon_history);
    let simpson_pts = to_pts(&metrics.simpson_history);
    let richness_pts = to_pts(&metrics.species_richness_history);
    let turnover_pts = to_pts(&metrics.species_turnover_history);
    let colony_diameter_pts = to_pts(&metrics.largest_colony_diameter_history);

    let producer_c = crate::theme::chart_color(&ecology::Diet::Producer);
    let herbivore_c = crate::theme::chart_color(&ecology::Diet::Herbivore);
    let carnivore_c = crate::theme::chart_color(&ecology::Diet::Carnivore);
    let omnivore_c = crate::theme::chart_color(&ecology::Diet::Omnivore);
    let decomposer_c = crate::theme::chart_color(&ecology::Diet::Decomposer);

    let fps_c = crate::theme::CHART_FPS;
    let tps_c = crate::theme::CHART_TPS;
    let mem_c = crate::theme::CHART_MEM;
    let food_c = crate::theme::CHART_FOOD;
    let minerals_c = crate::theme::CHART_MINERALS;
    let corpses_c = crate::theme::CHART_CORPSES;
    let sunlight_c = crate::theme::CHART_SUNLIGHT;
    let o2_c = crate::theme::CHART_O2;
    let co2_c = crate::theme::CHART_CO2;
    let temp_c = crate::theme::CHART_TEMP;

    let shannon_c = crate::theme::CHART_SHANNON;
    let simpson_c = crate::theme::CHART_SIMPSON;
    let richness_c = crate::theme::CHART_RICHNESS;
    let turnover_c = crate::theme::CHART_TURNOVER;
    let colony_diameter_c = crate::theme::CHART_COLONY_DIAMETER;

    // Split the available height between the 3 stacked plots per column so
    // they fill however much space the tile/window gives us (previously a
    // hardcoded 120.0 left dead space below when the panel was resized or
    // detached taller).
    let plot_height = ((ui.available_height() - 150.0) / 3.0).max(70.0);

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
                    demographics_toggle(
                        ui,
                        &mut state.metrics_options.demographics_visible[0],
                        producer_c,
                        "Producers",
                    );
                    demographics_toggle(
                        ui,
                        &mut state.metrics_options.demographics_visible[1],
                        herbivore_c,
                        "Herbivores",
                    );
                    demographics_toggle(
                        ui,
                        &mut state.metrics_options.demographics_visible[2],
                        carnivore_c,
                        "Carnivores",
                    );
                    demographics_toggle(
                        ui,
                        &mut state.metrics_options.demographics_visible[3],
                        omnivore_c,
                        "Omnivores",
                    );
                    demographics_toggle(
                        ui,
                        &mut state.metrics_options.demographics_visible[4],
                        decomposer_c,
                        "Decomposers",
                    );
                    ui.checkbox(
                        &mut state.metrics_options.demographics_running_mean,
                        "Running mean",
                    );
                });
                let demographics_visible = state.metrics_options.demographics_visible;
                let demographics_running_mean = state.metrics_options.demographics_running_mean;
                let pop_resp = egui_plot::Plot::new("pop_plot")
                    .height(plot_height)
                    .x_axis_formatter(|x, _range| format!("{:.1}s", x.value))
                    .y_axis_formatter(|y, _range| format!("{:.0}", y.value))
                    .y_axis_label("Population (count)")
                    .show(ui, |plot_ui| {
                        let series = [
                            (&prod_pts, "Producers", producer_c),
                            (&herb_pts, "Herbivores", herbivore_c),
                            (&carn_pts, "Carnivores", carnivore_c),
                            (&omni_pts, "Omnivores", omnivore_c),
                            (&deco_pts, "Decomposers", decomposer_c),
                        ];
                        for (i, (pts, name, color)) in series.iter().enumerate() {
                            if !demographics_visible[i] {
                                continue;
                            }
                            plot_ui.line(
                                egui_plot::Line::new((*pts).clone())
                                    .name(*name)
                                    .color(*color),
                            );
                            if demographics_running_mean {
                                plot_ui.line(
                                    egui_plot::Line::new(running_mean(pts))
                                        .name(format!("{name} (mean)"))
                                        .color(*color)
                                        .style(egui_plot::LineStyle::Dashed { length: 8.0 }),
                                );
                            }
                        }
                        // Hazard/predation/lineage markers.
                        for (x, color, description) in &narrative_events {
                            plot_ui.vline(
                                egui_plot::VLine::new(*x)
                                    .color(*color)
                                    .name(description.clone()),
                            );
                        }
                    });
                chart_png_export_button(ui, ctx, actions, pop_resp.response.rect);

                ui.add_space(crate::theme::SPACE_SM);

                ui.label(egui::RichText::new("Performance").strong());
                ui.horizontal_wrapped(|ui| {
                    crate::widgets::chart_legend_dot(ui, fps_c, "FPS");
                    crate::widgets::chart_legend_dot(ui, tps_c, "TPS");
                    crate::widgets::chart_legend_dot(ui, mem_c, "Mem (MB)");
                });
                let perf_resp = egui_plot::Plot::new("perf_plot")
                    .height(plot_height)
                    .x_axis_formatter(|x, _range| format!("{:.1}s", x.value))
                    .y_axis_label("Frames/Ticks per sec · MB")
                    .show(ui, |plot_ui| {
                        plot_ui.line(egui_plot::Line::new(fps_pts).name("FPS").color(fps_c));
                        plot_ui.line(egui_plot::Line::new(tps_pts).name("TPS").color(tps_c));
                        plot_ui.line(egui_plot::Line::new(mem_pts).name("Mem (MB)").color(mem_c));
                    });
                chart_png_export_button(ui, ctx, actions, perf_resp.response.rect);

                ui.add_space(crate::theme::SPACE_SM);

                ui.label(egui::RichText::new("Diversity").strong());
                ui.horizontal_wrapped(|ui| {
                    demographics_toggle(
                        ui,
                        &mut state.metrics_options.diversity_visible[0],
                        shannon_c,
                        "Shannon",
                    );
                    demographics_toggle(
                        ui,
                        &mut state.metrics_options.diversity_visible[1],
                        simpson_c,
                        "Simpson",
                    );
                    demographics_toggle(
                        ui,
                        &mut state.metrics_options.diversity_visible[2],
                        richness_c,
                        "Richness",
                    );
                    demographics_toggle(
                        ui,
                        &mut state.metrics_options.diversity_visible[3],
                        turnover_c,
                        "Turnover",
                    );
                    ui.checkbox(
                        &mut state.metrics_options.diversity_running_mean,
                        "Running mean",
                    );
                });
                let diversity_visible = state.metrics_options.diversity_visible;
                let diversity_running_mean = state.metrics_options.diversity_running_mean;
                let diversity_resp = egui_plot::Plot::new("diversity_plot")
                    .height(plot_height)
                    .x_axis_formatter(|x, _range| format!("{:.1}s", x.value))
                    .y_axis_label("Diversity index / count / fraction")
                    .show(ui, |plot_ui| {
                        let series = [
                            (&shannon_pts, "Shannon", shannon_c),
                            (&simpson_pts, "Simpson", simpson_c),
                            (&richness_pts, "Richness", richness_c),
                            (&turnover_pts, "Turnover", turnover_c),
                        ];
                        for (i, (pts, name, color)) in series.iter().enumerate() {
                            if !diversity_visible[i] {
                                continue;
                            }
                            plot_ui.line(
                                egui_plot::Line::new((*pts).clone())
                                    .name(*name)
                                    .color(*color),
                            );
                            if diversity_running_mean {
                                plot_ui.line(
                                    egui_plot::Line::new(running_mean(pts))
                                        .name(format!("{name} (mean)"))
                                        .color(*color)
                                        .style(egui_plot::LineStyle::Dashed { length: 8.0 }),
                                );
                            }
                        }
                    });
                chart_png_export_button(ui, ctx, actions, diversity_resp.response.rect);
            });

            // Column 2
            cols[1].vertical(|ui| {
                ui.label(egui::RichText::new("Resources").strong());
                ui.horizontal_wrapped(|ui| {
                    crate::widgets::chart_legend_dot(ui, food_c, "Food");
                    crate::widgets::chart_legend_dot(ui, minerals_c, "Minerals");
                    crate::widgets::chart_legend_dot(ui, corpses_c, "Corpses");
                });
                let res_resp = egui_plot::Plot::new("res_plot")
                    .height(plot_height)
                    .x_axis_formatter(|x, _range| format!("{:.1}s", x.value))
                    .y_axis_formatter(|y, _range| format!("{:.0}", y.value))
                    .y_axis_label("Resource count")
                    .show(ui, |plot_ui| {
                        plot_ui.line(egui_plot::Line::new(food_pts).name("Food").color(food_c));
                        plot_ui.line(
                            egui_plot::Line::new(min_pts)
                                .name("Minerals")
                                .color(minerals_c),
                        );
                        plot_ui.line(
                            egui_plot::Line::new(corp_pts)
                                .name("Corpses")
                                .color(corpses_c),
                        );
                    });
                chart_png_export_button(ui, ctx, actions, res_resp.response.rect);

                ui.add_space(crate::theme::SPACE_SM);

                ui.label(egui::RichText::new("Environment").strong());
                ui.horizontal_wrapped(|ui| {
                    crate::widgets::chart_legend_dot(ui, sunlight_c, "Sunlight");
                    crate::widgets::chart_legend_dot(ui, o2_c, "O2");
                    crate::widgets::chart_legend_dot(ui, co2_c, "CO2");
                    crate::widgets::chart_legend_dot(ui, temp_c, "Temp (°C)");
                });
                let env_resp = egui_plot::Plot::new("env_plot")
                    .height(plot_height)
                    .x_axis_formatter(|x, _range| format!("{:.1}s", x.value))
                    .y_axis_label("Sunlight/gas fraction · °C")
                    .show(ui, |plot_ui| {
                        plot_ui.line(
                            egui_plot::Line::new(sun_pts)
                                .name("Sunlight")
                                .color(sunlight_c),
                        );
                        plot_ui.line(egui_plot::Line::new(o2_pts).name("O2").color(o2_c));
                        plot_ui.line(egui_plot::Line::new(co2_pts).name("CO2").color(co2_c));
                        plot_ui.line(
                            egui_plot::Line::new(temp_pts)
                                .name("Temp (°C)")
                                .color(temp_c),
                        );
                    });
                chart_png_export_button(ui, ctx, actions, env_resp.response.rect);

                ui.add_space(crate::theme::SPACE_SM);

                ui.label(egui::RichText::new("Colony Connectivity").strong());
                ui.horizontal_wrapped(|ui| {
                    crate::widgets::chart_legend_dot(
                        ui,
                        colony_diameter_c,
                        "Largest colony diameter",
                    );
                    // `colony_size_distribution` is a point-in-time snapshot
                    // (one entry per connected component), not a time series
                    // like the line above, so it reads as a label rather than
                    // a plotted line — same treatment `age_distribution`/
                    // `generation_distribution` would need if charted.
                    ui.label(
                        egui::RichText::new(format!(
                            "{} colonies, largest {} organism(s)",
                            metrics.colony_size_distribution.len(),
                            metrics
                                .colony_size_distribution
                                .iter()
                                .copied()
                                .max()
                                .unwrap_or(0)
                        ))
                        .color(crate::theme::DISABLED_FG)
                        .small(),
                    );
                });
                let colony_resp = egui_plot::Plot::new("colony_plot")
                    .height(plot_height)
                    .x_axis_formatter(|x, _range| format!("{:.1}s", x.value))
                    .y_axis_formatter(|y, _range| format!("{:.0}", y.value))
                    .y_axis_label("Diameter (nodes)")
                    .show(ui, |plot_ui| {
                        plot_ui.line(
                            egui_plot::Line::new(colony_diameter_pts)
                                .name("Largest colony diameter")
                                .color(colony_diameter_c),
                        );
                    });
                chart_png_export_button(ui, ctx, actions, colony_resp.response.rect);
            });
        });

        // Population-by-species: `analytics_bridge_system` computes
        // per-species counts every sample to derive Shannon/Simpson/richness
        // (above); this reuses that same species-id/count pairing directly.
        // A snapshot distribution (like `colony_size_distribution` above),
        // not a time series — bar chart, not a line, since "which species
        // has how many members right now" is the question, not a trend.
        // Reuses
        // `CHART_RICHNESS` (already the "species" hue in the Diversity plot
        // above) rather than adding a new token for the same concept family.
        ui.add_space(crate::theme::SPACE_SM);
        ui.label(egui::RichText::new("Species Distribution").strong());
        if metrics.species_distribution.is_empty() {
            crate::widgets::empty_state(ui, "No species currently alive.");
        } else {
            let bars: Vec<egui_plot::Bar> = metrics
                .species_distribution
                .iter()
                .enumerate()
                .map(|(i, &(species_id, count))| {
                    egui_plot::Bar::new(i as f64, count as f64)
                        .name(format!("Species {species_id}"))
                        .fill(richness_c)
                })
                .collect();
            let species_resp = egui_plot::Plot::new("species_distribution_plot")
                .height(plot_height)
                .x_axis_formatter(|_x, _range| String::new())
                .y_axis_formatter(|y, _range| format!("{:.0}", y.value))
                .y_axis_label("Population (count)")
                .show(ui, |plot_ui| {
                    plot_ui.bar_chart(egui_plot::BarChart::new(bars).name("Species population"));
                });
            chart_png_export_button(ui, ctx, actions, species_resp.response.rect);
        }
    });
}

/// One legend entry doubling as a per-series visibility toggle —
/// `widgets::chart_legend_dot`'s colored-dot-plus-label composed
/// with a checkbox, rather than a second hand-rolled legend-row renderer.
/// Kept local to this file since Demographics/Diversity are the only two
/// plots the toggle applies to.
fn demographics_toggle(ui: &mut egui::Ui, visible: &mut bool, color: egui::Color32, label: &str) {
    ui.horizontal(|ui| {
        ui.checkbox(visible, "");
        let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 4.0, color);
        ui.add_space(2.0);
        ui.label(egui::RichText::new(label).small());
    });
}

/// Small button placed just below a chart, capturing that specific chart's
/// screen rect for `MenuAction::ExportChartPng`. The actual crop+encode
/// happens later, in `crates/app/src/render.rs`, against the
/// live swapchain texture — this only converts the rect from egui's logical
/// points to physical pixels (the unit the GPU readback works in) and
/// queues the action.
fn chart_png_export_button(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    actions: &mut Vec<MenuAction>,
    rect: egui::Rect,
) {
    if ui
        .small_button(format!("{} Export PNG", egui_remixicon::icons::IMAGE_LINE))
        .on_hover_text("Save this chart as a PNG (crops the next rendered frame)")
        .clicked()
    {
        let ppp = ctx.pixels_per_point();
        actions.push(MenuAction::ExportChartPng {
            x: (rect.min.x * ppp).max(0.0).round() as u32,
            y: (rect.min.y * ppp).max(0.0).round() as u32,
            width: (rect.width() * ppp).max(1.0).round() as u32,
            height: (rect.height() * ppp).max(1.0).round() as u32,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fewer than `RUNNING_MEAN_WINDOW` samples exist yet — every window
    /// must clamp to what's actually available (`start = 0`), not panic on
    /// an out-of-range slice.
    #[test]
    fn running_mean_clamps_window_before_full_history_exists() {
        let pts = vec![[0.0, 10.0], [1.0, 20.0]];
        let result = running_mean(&pts);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], [0.0, 10.0]); // window of 1: just itself
        assert_eq!(result[1], [1.0, 15.0]); // window of 2: (10+20)/2
    }

    /// Once history exceeds the window, each point averages only the
    /// trailing `RUNNING_MEAN_WINDOW` samples, not the whole history — a
    /// constant tail after a step change must fully reflect the new value
    /// once the window has slid past the step.
    #[test]
    fn running_mean_uses_only_the_trailing_window() {
        // 9 samples at 0.0, then a step to 100.0. RUNNING_MEAN_WINDOW is 10,
        // so the point at index 9 (the 10th sample) still averages in the
        // nine leading zeros; by index 18 the window has fully slid past
        // them and should read exactly 100.0.
        let mut pts: Vec<[f64; 2]> = (0..9).map(|i| [i as f64, 0.0]).collect();
        pts.extend((9..19).map(|i| [i as f64, 100.0]));

        let result = running_mean(&pts);
        assert_eq!(result[9][1], 10.0); // (9 zeros + 1*100) / 10
        assert_eq!(result[18][1], 100.0); // fully past the step
    }

    /// The overlay's X value must track the raw line's X for the same
    /// index (so the dashed mean line renders at the same time-axis
    /// position as what it's smoothing), not the mean of the X values.
    #[test]
    fn running_mean_preserves_the_sample_x_value() {
        let pts = vec![[5.0, 1.0], [7.0, 3.0]];
        let result = running_mean(&pts);
        assert_eq!(result[0][0], 5.0);
        assert_eq!(result[1][0], 7.0);
    }
}
