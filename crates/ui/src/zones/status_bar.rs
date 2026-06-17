use crate::theme::{BG_VOID, TEXT_MUTED};
use analytics::SimulationStats;
use common::Tick;
use egui::{Align, Color32, Frame, Layout, RichText, TopBottomPanel};

pub fn render_status_bar(
    ctx: &egui::Context,
    ui_state: &mut crate::state::UiState,
    stats: &SimulationStats,
    tick: Tick,
) {
    TopBottomPanel::bottom("status_bar")
        .exact_height(24.0)
        .frame(
            Frame::none()
                .fill(BG_VOID)
                .inner_margin(egui::Margin::symmetric(8.0, 4.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Left: Timeline scrubber
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.set_min_width(ui.available_width() / 2.0);
                    let mut tick_value = tick.0 as f32;
                    let is_replay = ui_state.last_snapshot_path.is_some(); // Simplistic heuristic for now
                    ui.add_enabled_ui(is_replay, |ui| {
                        // Custom style for thin track
                        let mut style = ui.style().as_ref().clone();
                        style.spacing.slider_width = 200.0;
                        ui.style_mut().spacing = style.spacing;
                        let slider = egui::Slider::new(&mut tick_value, 0.0..=100000.0) // Placeholder max
                            .show_value(false);
                        ui.add(slider);
                        ui.label(
                            RichText::new(format!("Tick {} / ∞", tick.0))
                                .size(10.0)
                                .color(TEXT_MUTED),
                        );
                    });
                });

                // Right: System telemetry strip
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let fps = 60.0;
                    let tps = 60.0;
                    let gpu_ms = 3.2; // Placeholder
                    let cpu_ms = 8.1; // Placeholder
                    let organisms = stats.current_population;
                    let food = if let Some(last) = stats.history.back() { last.3 as usize } else { 0 };
                    let species = 1;
                    let deaths = stats.deaths_by_age + stats.deaths_by_predation + stats.deaths_by_starvation;

                    let text = format!(
                        "Tick/s: {:.0}  |  FPS: {:.0}  |  GPU: {:.1}ms  |  CPU: {:.1}ms    Organisms: {}  |  Food: {}  |  Species: {}  |  Deaths: {}",
                        tps, fps, gpu_ms, cpu_ms, organisms, food, species, deaths
                    );

                    let label = egui::Label::new(
                        RichText::new(text)
                            .family(egui::FontFamily::Monospace)
                            .size(10.0)
                            .color(Color32::from_rgb(90, 100, 115)),
                    ).sense(egui::Sense::click());

                    if ui.add(label).clicked() {
                        ui_state.panels.profiler = !ui_state.panels.profiler;
                    }
                });
            });
        });
}
