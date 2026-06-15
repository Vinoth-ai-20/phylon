pub fn render_dashboard(
    ctx: &egui::Context,
    ui_state: &mut crate::state::UiState,
    world: &mut world::PhylonWorld,
    stats: &analytics::SimulationStats,
    tick: common::Tick,
    script_path: &mut String,
    load_script: &mut bool,
) {
    if ui_state.panels.analytics {
        egui::SidePanel::left("analytics_panel")
            .frame(
                egui::Frame::side_top_panel(&ctx.style()).fill(egui::Color32::from_rgb(12, 14, 20)),
            )
            .resizable(true)
            .default_width(350.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    crate::panels::analytics::render_analytics_dashboard(ui, stats, tick);
                });
            });
    }

    if ui_state.panels.entity_inspector
        || ui_state.panels.genome_inspector
        || ui_state.panels.brain_inspector
        || ui_state.panels.research
    {
        egui::SidePanel::right("inspector_panel")
            .frame(
                egui::Frame::side_top_panel(&ctx.style()).fill(egui::Color32::from_rgb(12, 14, 20)),
            )
            .resizable(true)
            .default_width(350.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if ui_state.panels.entity_inspector {
                        crate::panels::entity_inspector::render_entity_inspector(
                            ui,
                            &ui_state.selected_entities,
                            world,
                        );
                        ui.separator();
                    }
                    if ui_state.panels.brain_inspector {
                        crate::panels::brain_inspector::render_brain_inspector(
                            ui,
                            tick,
                            &ui_state.selected_entities,
                            world,
                        );
                        ui.separator();
                    }
                    if ui_state.panels.genome_inspector {
                        crate::panels::genome_inspector::render_genome_inspector(
                            ui,
                            &ui_state.selected_entities,
                            world,
                        );
                        ui.separator();
                    }
                    if ui_state.panels.research {
                        crate::panels::research::render_research(ui, script_path, load_script);
                    }
                });
            });
    }

    egui::TopBottomPanel::bottom("bottom_panel")
        .frame(egui::Frame::side_top_panel(&ctx.style()).fill(egui::Color32::from_rgb(12, 14, 20)))
        .resizable(true)
        .default_height(150.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width() * 0.5);
                    crate::panels::timeline::render_timeline(
                        ui,
                        tick,
                        &mut ui_state.simulation_speed,
                        &mut ui_state.is_paused,
                    );
                });
                ui.separator();
                ui.vertical(|ui| {
                    crate::panels::system_logs::render_system_logs(ui, &ui_state.system_logs);
                });
            });
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(egui::Color32::TRANSPARENT))
        .show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();
            ui_state.viewport_rect = Some(rect);

            // Capture interactions over the canvas
            let response = ui.interact(
                rect,
                ui.id().with("viewport_interact"),
                egui::Sense::click_and_drag(),
            );

            // Pan logic with a small drag threshold
            if response.dragged() {
                let delta = response.drag_delta();
                if delta.length_sq() > 4.0 {
                    if let Some(tx) = &ui_state.app_tx {
                        let _ = tx.send(crate::commands::AppCommand::PanCamera([delta.x, delta.y]));
                    }
                }
            }

            // Click / DoubleClick selection
            if response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    if let Some(tx) = &ui_state.app_tx {
                        let _ = tx.send(crate::commands::AppCommand::ClickWorld([pos.x, pos.y]));
                    }
                }
            } else if response.double_clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    if let Some(tx) = &ui_state.app_tx {
                        let _ = tx.send(crate::commands::AppCommand::DoubleClickWorld([
                            pos.x, pos.y,
                        ]));
                    }
                }
            }

            // Mouse wheel zoom over the canvas
            // If the user scrolls while hovering the canvas, egui intercepts it via input.
            let scroll = ui.input(|i| i.smooth_scroll_delta);
            if scroll.y != 0.0 && response.hovered() {
                if let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) {
                    if let Some(tx) = &ui_state.app_tx {
                        let _ = tx.send(crate::commands::AppCommand::WheelZoom {
                            mouse_position: [pointer.x, pointer.y],
                            delta: scroll.y,
                        });
                    }
                }
            }
        });

    if let Some(viewport_rect) = ui_state.viewport_rect {
        egui::Area::new(egui::Id::new("zoom_controls"))
            .fixed_pos(egui::pos2(
                viewport_rect.max.x - 110.0,
                viewport_rect.max.y - 44.0,
            ))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let frame = egui::Frame::none()
                        .fill(egui::Color32::from_rgba_unmultiplied(12, 14, 20, 200))
                        .rounding(egui::Rounding::same(4.0))
                        .inner_margin(egui::Margin::same(4.0));

                    frame.show(ui, |ui| {
                        if ui.button("−").clicked() {
                            if let Some(tx) = &ui_state.app_tx {
                                let _ = tx.send(crate::commands::AppCommand::ZoomOut);
                            }
                        }
                        ui.label(format!("{:.0}%", ui_state.camera.zoom_level * 100.0));
                        if ui.button("+").clicked() {
                            if let Some(tx) = &ui_state.app_tx {
                                let _ = tx.send(crate::commands::AppCommand::ZoomIn);
                            }
                        }
                        if ui.button("⌂").clicked() {
                            if let Some(tx) = &ui_state.app_tx {
                                let _ = tx.send(crate::commands::AppCommand::ResetCamera);
                            }
                        }
                    });
                });
            });
    }
}
