use crate::theme::{BG_DEEP, TEXT_MUTED};
use analytics::SimulationStats;
use common::Tick;
use egui::{Align, Frame, Layout, RichText, TopBottomPanel};

pub fn render_system_bar(
    ctx: &egui::Context,
    ui_state: &mut crate::state::UiState,
    stats: &SimulationStats,
    tick: Tick,
) {
    TopBottomPanel::top("system_bar")
        .exact_height(24.0)
        .frame(
            Frame::none()
                .fill(BG_DEEP)
                .inner_margin(egui::Margin::symmetric(8.0, 0.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                // Left Region: Application menu
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    crate::menu::render_compact_menu(ui, ui_state, stats);
                });

                // Centre Region: Simulation identity + live state
                ui.with_layout(
                    Layout::centered_and_justified(egui::Direction::LeftToRight),
                    |ui| {
                        let run_state_color = if ui_state.is_paused {
                            crate::theme::ACCENT_AMBER
                        } else {
                            crate::theme::ACCENT_GREEN
                        };

                        let run_state_text = if ui_state.is_paused {
                            "PAUSED"
                        } else {
                            "RUNNING"
                        };

                        let mut job = egui::text::LayoutJob::default();
                        job.append(
                            &format!("Phylon — World 1  ·  Tick {}  ·  ", tick.0),
                            0.0,
                            egui::TextFormat {
                                font_id: egui::FontId::monospace(11.0),
                                color: TEXT_MUTED,
                                ..Default::default()
                            },
                        );
                        job.append(
                            "● ",
                            0.0,
                            egui::TextFormat {
                                font_id: egui::FontId::monospace(11.0),
                                color: run_state_color,
                                ..Default::default()
                            },
                        );
                        job.append(
                            run_state_text,
                            0.0,
                            egui::TextFormat {
                                font_id: egui::FontId::monospace(11.0),
                                color: TEXT_MUTED,
                                ..Default::default()
                            },
                        );

                        ui.label(job);
                    },
                );

                // Right Region: System health strip
                // Will be rendered via absolute positioning to ensure it aligns right despite center layout taking up space
            });

            // Render Right Region over the same space
            let right_rect = ui.max_rect();
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(right_rect), |right_ui| {
                right_ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(8.0); // Right padding

                    let fps = 60.0;
                    let tps = 60.0;
                    let entity_count = stats.current_population;
                    let gpu_ms = 0.0; // Placeholder

                    let tps_color = if tps < 10.0 {
                        crate::theme::ACCENT_RED
                    } else if tps < 30.0 {
                        crate::theme::ACCENT_AMBER
                    } else {
                        TEXT_MUTED
                    };

                    let health_text = format!(
                        "⚡ {:.0}t/s   🖥 {:.0}fps   ◈ {:.1}ms   ⟁ {}",
                        tps, fps, gpu_ms, entity_count
                    );

                    if ui
                        .add(
                            egui::Label::new(
                                RichText::new(health_text).size(11.0).color(tps_color),
                            )
                            .sense(egui::Sense::click()),
                        )
                        .clicked()
                    {}
                });
            });
        });
}
