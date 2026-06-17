use crate::commands::AppCommand;
use crate::theme::{ACCENT_AMBER, ACCENT_GREEN, BG_CONTROL, TEXT_MUTED};
use common::Tick;
use egui::{Align, Color32, Frame, Layout, RichText, TopBottomPanel};

pub fn render_control_bar(ctx: &egui::Context, ui_state: &mut crate::state::UiState, tick: Tick) {
    let speed_value = ui_state.simulation_speed;
    let mut is_paused = ui_state.is_paused;
    let tracked_entity = ui_state.selected_entities.first().copied(); // Should match what's tracked

    TopBottomPanel::bottom("control_bar")
        .exact_height(48.0)
        .frame(
            Frame::none()
                .fill(Color32::from_rgb(12, 14, 18))
                .inner_margin(egui::Margin::symmetric(16.0, 8.0))
                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(25, 28, 38))), // Top border
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                // Left: Transport Controls (220px)
                ui.allocate_ui_with_layout(
                    egui::vec2(240.0, ui.available_height()),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        let play_pause_color = if is_paused {
                            ACCENT_GREEN
                        } else {
                            ACCENT_AMBER
                        };
                        let play_pause_icon = if is_paused {
                            egui_phosphor::regular::PLAY
                        } else {
                            egui_phosphor::regular::PAUSE
                        };

                        let pp_btn = egui::Button::new(
                            RichText::new(play_pause_icon)
                                .color(Color32::WHITE)
                                .size(16.0),
                        )
                        .fill(play_pause_color)
                        .min_size(egui::vec2(36.0, 32.0));

                        if ui.add(pp_btn).clicked() {
                            is_paused = !is_paused;
                            ui_state.is_paused = is_paused;
                        }

                        if ui
                            .add_enabled(
                                is_paused,
                                egui::Button::new(egui_phosphor::regular::SKIP_FORWARD)
                                    .min_size(egui::vec2(32.0, 32.0)),
                            )
                            .clicked()
                        {
                            if let Some(tx) = &ui_state.app_tx {
                                let _ = tx.send(AppCommand::StepOneTick);
                            }
                        }

                        if ui
                            .add(
                                egui::Button::new(egui_phosphor::regular::STOP)
                                    .min_size(egui::vec2(32.0, 32.0)),
                            )
                            .clicked()
                        {
                            if let Some(tx) = &ui_state.app_tx {
                                let _ = tx.send(AppCommand::ResetWorld);
                            }
                        }

                        if ui
                            .add(
                                egui::Button::new(egui_phosphor::regular::ARROWS_COUNTER_CLOCKWISE)
                                    .min_size(egui::vec2(32.0, 32.0)),
                            )
                            .clicked()
                        {
                            // Reset camera or something else? Prompt says Reset for stop, Step for ↩? Wait, ■ is stop, ↩ is reset.
                            if let Some(tx) = &ui_state.app_tx {
                                let _ = tx.send(AppCommand::ResetWorld);
                            }
                        }

                        ui.add_space(8.0);
                        ui.label(RichText::new("Step:").color(TEXT_MUTED).size(11.0));
                        // Editable tick input placeholder
                        let mut step_tick = tick.0.to_string();
                        let response = ui.add_sized(
                            egui::vec2(60.0, 24.0),
                            egui::TextEdit::singleline(&mut step_tick)
                                .font(egui::FontId::monospace(11.0))
                                .horizontal_align(egui::Align::Center),
                        );
                        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            // parse and jump logic if implemented
                        }
                    },
                );

                // Centre: Speed Control (280px)
                ui.with_layout(
                    Layout::centered_and_justified(egui::Direction::LeftToRight),
                    |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.label(RichText::new("Speed").color(TEXT_MUTED).size(11.0));
                            ui.add_space(8.0);

                            let speeds = [
                                (0.25, "0.25×"),
                                (0.5, "0.5×"),
                                (1.0, "1×"),
                                (2.0, "2×"),
                                (5.0, "5×"),
                                (10.0, "10×"),
                                (f32::MAX, "∞"),
                            ];

                            for (val, label) in speeds {
                                let is_active = (speed_value - val).abs() < f32::EPSILON
                                    || (val == f32::MAX && speed_value > 1000.0);

                                let (bg_color, text_color) = if is_active {
                                    (Color32::from_rgb(40, 80, 160), Color32::WHITE)
                                } else {
                                    (Color32::TRANSPARENT, Color32::from_rgb(100, 110, 130))
                                };

                                let btn = egui::Button::new(
                                    RichText::new(label).size(11.0).color(text_color),
                                )
                                .fill(bg_color)
                                .rounding(12.0)
                                .frame(true);

                                // Remove stroke for inactive
                                let mut style = ui.style().as_ref().clone();
                                if !is_active {
                                    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
                                }
                                ui.style_mut().visuals = style.visuals;

                                if ui.add(btn).clicked() {
                                    ui_state.simulation_speed = val;
                                }
                            }
                        });
                    },
                );

                // Right: Camera Controls (200px)
                // Use absolute positioning or right alignment
            });

            let right_rect = ui.max_rect();
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(right_rect), |right_ui| {
                right_ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(8.0);

                    let track_btn_text = if let Some(id) = tracked_entity {
                        format!("Track: #{}", id.0)
                    } else {
                        "Track: OFF".to_string()
                    };

                    let track_bg = if tracked_entity.is_some() {
                        Color32::from_rgb(40, 80, 160)
                    } else {
                        BG_CONTROL
                    };

                    if ui
                        .add(
                            egui::Button::new(RichText::new(track_btn_text).size(11.0))
                                .fill(track_bg)
                                .min_size(egui::vec2(80.0, 24.0)),
                        )
                        .clicked()
                    {
                        // toggle track logic
                    }

                    ui.add_space(8.0);

                    if ui
                        .add(
                            egui::Button::new(egui_phosphor::regular::MAGNIFYING_GLASS_MINUS)
                                .min_size(egui::vec2(28.0, 24.0)),
                        )
                        .clicked()
                    {
                        if let Some(tx) = &ui_state.app_tx {
                            let _ = tx.send(AppCommand::ZoomOut);
                        }
                    }
                    if ui
                        .add(
                            egui::Button::new(egui_phosphor::regular::MAGNIFYING_GLASS_PLUS)
                                .min_size(egui::vec2(28.0, 24.0)),
                        )
                        .clicked()
                    {
                        if let Some(tx) = &ui_state.app_tx {
                            let _ = tx.send(AppCommand::ZoomIn);
                        }
                    }
                    if ui
                        .add(
                            egui::Button::new(egui_phosphor::regular::CAMERA)
                                .min_size(egui::vec2(28.0, 24.0)),
                        )
                        .clicked()
                    {
                        if let Some(tx) = &ui_state.app_tx {
                            let _ = tx.send(AppCommand::ResetCamera);
                        }
                    }
                });
            });
        });
}
