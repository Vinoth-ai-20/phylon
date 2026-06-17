pub fn render_dashboard(
    ctx: &egui::Context,
    ui_state: &mut crate::state::UiState,
    world: &mut world::PhylonWorld,
    stats: &analytics::SimulationStats,
    tick: common::Tick,
    script_path: &mut String,
    load_script: &mut bool,
) {
    crate::zones::left_panel::render_left_panel(ctx, ui_state, world, stats, tick);
    crate::zones::right_panel::render_right_panel(
        ctx,
        ui_state,
        world,
        tick,
        script_path,
        load_script,
    );

    crate::zones::status_bar::render_status_bar(ctx, ui_state, stats, tick);
    crate::zones::control_bar::render_control_bar(ctx, ui_state, tick);

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
            } else if response.secondary_clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    ui_state.active_context_menu =
                        Some((pos, ui_state.selected_entities.first().copied()));
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
        crate::overlays::camera_controls::render_camera_controls(ctx, ui_state, viewport_rect);
        crate::overlays::camera_controls::render_view_mode_selector(ctx, ui_state, viewport_rect);
        crate::overlays::camera_controls::render_selection_chip(ctx, ui_state, viewport_rect);
        crate::overlays::camera_controls::render_async_progress(ctx, ui_state, viewport_rect);
        crate::overlays::search_bar::render_search_bar(ctx, ui_state, viewport_rect);
    }

    crate::overlays::context_menu::render_context_menu(ctx, ui_state);
}
