//! Viewport plugin — transparent canvas with input capture, selection, and context menu.

use crate::types::*;
use crate::WorkbenchState;

/// Render the transparent viewport panel.
///
/// Captures mouse/scroll input for the 3D canvas and shows a context menu
/// on right-click that is entity-aware (different options when an organism
/// is under the cursor vs. empty space).
pub fn viewport_ui(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut WorkbenchState,
    canvas_interaction: &mut Option<CanvasInteraction>,
    actions: &mut Vec<MenuAction>,
) {
    // Keep the area transparent so wgpu shows through
    egui::Frame::none()
        .fill(egui::Color32::TRANSPARENT)
        .show(ui, |ui| {
            let rect = ui.available_rect_before_wrap();
            state.canvas_rect = Some([
                rect.min.x as u32,
                rect.min.y as u32,
                rect.width() as u32,
                rect.height() as u32,
            ]);

            let interact_response = ui.interact(
                rect,
                ui.id().with("viewport"),
                egui::Sense::click_and_drag(),
            );

            let hover_pos = ui.input(|i| i.pointer.hover_pos());
            let zoom_delta = ui.input(|i| i.zoom_delta());
            let camera = state.camera();

            // Cursor world-space position (Phase 2, M10) — `None` unless the
            // cursor is actually within this canvas's rect, so hovering a
            // different panel doesn't leave a stale/wrong readout. Phase 8,
            // ADR-P8-02: now goes through the one canonical
            // `Camera3d::screen_to_ray` unproject, intersected with the
            // `Z = 0` plane every organism/pellet still lives on, replacing
            // one of the 3 independently hand-derived screen↔world
            // transforms the Phase 8 audit found.
            state.cursor_world_pos = hover_pos.filter(|p| rect.contains(*p)).map(|p| {
                let ppp = ctx.pixels_per_point();
                let screen_pos =
                    common::Vec2::new((p.x - rect.min.x) * ppp, (p.y - rect.min.y) * ppp);
                let viewport_size = common::Vec2::new(rect.width() * ppp, rect.height() * ppp);
                let (origin, dir) = camera.screen_to_ray(screen_pos, viewport_size);
                crate::camera::ray_intersect_z0(origin, dir)
                    .unwrap_or_else(|| camera.position.truncate())
            });

            // Middle-button drag orbits/looks around (Orbit/Fly respectively
            // — see `app::render`'s interaction dispatch); left-button drag
            // continues to pan, unchanged from pre-Phase-8 behavior.
            let rotate_delta = if interact_response.dragged_by(egui::PointerButton::Middle) {
                interact_response.drag_delta()
            } else {
                egui::Vec2::ZERO
            };
            let pan_delta = if interact_response.dragged_by(egui::PointerButton::Primary) {
                interact_response.drag_delta()
            } else {
                egui::Vec2::ZERO
            };

            *canvas_interaction = Some(CanvasInteraction {
                rect: interact_response.rect,
                clicked: interact_response.clicked(),
                click_pos: interact_response.interact_pointer_pos(),
                hover_pos,
                drag_delta: pan_delta,
                rotate_delta,
                zoom_delta,
            });

            // ── Context Menu (Right Click) ────────────────────────────────
            interact_response.context_menu(|ui| {
                // Determine if the hover is over a selected organism
                let selected = state.selected_entity;
                let hovered = state.hovered_entity;

                // Use whichever entity is most relevant
                let target = hovered.or(selected);

                if let Some(entity) = target {
                    // Entity-specific actions
                    ui.label(
                        egui::RichText::new(format!("Entity {:?}", entity))
                            .small()
                            .color(crate::theme::DISABLED_FG),
                    );
                    ui.separator();

                    if ui
                        .button(format!("{} Inspect", egui_remixicon::icons::SEARCH_LINE))
                        .clicked()
                    {
                        // Phase 7, W0b: `MenuAction::SelectEntity`'s handler
                        // now opens the Inspector/sidebar itself (via
                        // `WorkbenchState::select`), so this button no
                        // longer needs its own copy of that logic — it was
                        // a second, slightly different implementation of
                        // the same "select and inspect" behavior plain
                        // viewport clicks lacked (see W0a's finding #1).
                        actions.push(MenuAction::SelectEntity(entity));
                        ui.close_menu();
                    }
                    if ui
                        .button(format!(
                            "{} Track / Follow",
                            egui_remixicon::icons::FOCUS_LINE
                        ))
                        .clicked()
                    {
                        actions.push(MenuAction::TrackEntity(entity));
                        ui.close_menu();
                    }
                    if ui
                        .button(format!(
                            "{} Export Genome…",
                            egui_remixicon::icons::DOWNLOAD_LINE
                        ))
                        .clicked()
                    {
                        actions.push(MenuAction::SelectEntity(entity));
                        actions.push(MenuAction::ExportGenome);
                        ui.close_menu();
                    }
                    if ui
                        .button(format!("{} Copy ID", egui_remixicon::icons::CLIPBOARD_LINE))
                        .clicked()
                    {
                        actions.push(MenuAction::CopyEntityId(entity));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add(egui::Button::new(
                            egui::RichText::new(format!(
                                "{} Kill Entity",
                                egui_remixicon::icons::DELETE_BIN_LINE
                            ))
                            .color(crate::theme::DANGER),
                        ))
                        .clicked()
                    {
                        actions.push(MenuAction::KillEntity(entity));
                        ui.close_menu();
                    }

                    ui.separator();
                }

                // Empty-space / general actions
                ui.menu_button(
                    format!("{} Spawn…", egui_remixicon::icons::ADD_CIRCLE_LINE),
                    |ui| {
                        if ui.button("Proto-Fish").clicked() {
                            actions.push(MenuAction::SpawnProtoFish);
                            ui.close_menu();
                        }
                        for preset in organisms::sandbox::PresetDefinition::standard_presets() {
                            if ui.button(&preset.name).clicked() {
                                actions.push(MenuAction::SpawnPreset(preset.name));
                                ui.close_menu();
                            }
                        }
                    },
                );

                if ui
                    .button(format!("{} Spawn Hazard", egui_remixicon::icons::FIRE_LINE))
                    .clicked()
                {
                    actions.push(MenuAction::SpawnManualHazard);
                    ui.close_menu();
                }

                ui.separator();
                if ui
                    .button(format!("{} Reset Camera", egui_remixicon::icons::HOME_LINE))
                    .clicked()
                {
                    actions.push(MenuAction::CameraHome);
                    ui.close_menu();
                }
                if ui
                    .button(format!(
                        "{} Clear Selection",
                        egui_remixicon::icons::CLOSE_CIRCLE_LINE
                    ))
                    .clicked()
                {
                    actions.push(MenuAction::Deselect);
                    ui.close_menu();
                }
            });

            // Double-click to focus the entity under the cursor (falling
            // back to whatever's already selected) — Phase 7, W0b: this
            // used to set `tracked_entity` directly, silently turning a
            // "look at this once" gesture into permanent camera-follow.
            // Now it's a one-shot snap only (`MenuAction::FocusSelection`,
            // which already existed for the menu-triggered case — see its
            // own doc comment), matching the milestone's explicit
            // requirement that double-click focuses once and never enables
            // persistent tracking. Follow remains a separate, always-
            // explicit action (toolbar, Inspector, or context menu).
            if interact_response.double_clicked() {
                if let Some(entity) = state.hovered_entity.or(state.selected_entity) {
                    state.select(entity);
                    state.spectator_mode = false;
                    actions.push(MenuAction::FocusSelection);
                }
            }

            let ppp = ctx.pixels_per_point();
            let viewport_size_px = common::Vec2::new(rect.width() * ppp, rect.height() * ppp);
            let to_local_px = |p: egui::Pos2| {
                common::Vec2::new((p.x - rect.min.x) * ppp, (p.y - rect.min.y) * ppp)
            };
            // Real unproject (ADR-P8-02) — the second of the 3 duplicated
            // screen↔world transforms the Phase 8 audit found, now going
            // through `Camera3d::screen_to_ray` + the shared Z=0 plane
            // intersection instead of its own hand-derived ortho inverse.
            let to_world = |p: egui::Pos2| {
                let (origin, dir) = camera.screen_to_ray(to_local_px(p), viewport_size_px);
                crate::camera::ray_intersect_z0(origin, dir)
                    .unwrap_or_else(|| camera.position.truncate())
            };
            // World→screen projection (the inverse direction) — now a real
            // `Camera3d::world_to_screen` projection (Phase 8, Epic 8.4;
            // previously a flat approximation fed by the `camera_pos_2d`/
            // `camera_zoom_2d` bridge, before that method existed), local to
            // this viewport's rect rather than the raw screen.
            let to_screen = |p: common::Vec2| {
                camera
                    .world_to_screen(p.extend(0.0), viewport_size_px)
                    .map(|s| egui::pos2(s.x / ppp + rect.min.x, s.y / ppp + rect.min.y))
                    .unwrap_or(rect.center())
            };

            // Box-select (Phase 2, M8) / Lasso-select (Phase 8, Epic 8.4) /
            // Measure (Phase 2, M11) share one click-drag gesture, branching
            // on `state.marquee_mode` (toggled from the toolbar) — the drag
            // start is tracked explicitly in `state` (set on
            // `drag_started_by`, cleared on `drag_stopped_by`) rather than
            // relying on `interact_pointer_pos()` staying valid past the
            // exact frame the drag ends.
            if interact_response.drag_started_by(egui::PointerButton::Primary) {
                state.marquee_drag_start = interact_response.interact_pointer_pos();
                state.lasso_points.clear();
                if let Some(p) = state.marquee_drag_start {
                    state.lasso_points.push(p);
                }
            }
            if interact_response.dragged_by(egui::PointerButton::Primary) {
                if let (Some(start), Some(current)) = (state.marquee_drag_start, hover_pos) {
                    if start != current {
                        match state.marquee_mode {
                            crate::MarqueeMode::Measure => {
                                let distance = (to_world(start) - to_world(current)).length();
                                ui.painter().line_segment(
                                    [start, current],
                                    egui::Stroke::new(2.0_f32, crate::theme::ACCENT),
                                );
                                ui.painter().text(
                                    current,
                                    egui::Align2::LEFT_TOP,
                                    format!("{distance:.1} units"),
                                    egui::FontId::monospace(crate::theme::SIZE_SMALL),
                                    egui::Color32::WHITE,
                                );
                            }
                            crate::MarqueeMode::Select => {
                                let sel_rect = egui::Rect::from_two_pos(start, current);
                                ui.painter().rect(
                                    sel_rect,
                                    0.0,
                                    egui::Color32::from_white_alpha(20),
                                    egui::Stroke::new(
                                        1.0_f32,
                                        egui::Color32::from_white_alpha(180),
                                    ),
                                );
                            }
                            crate::MarqueeMode::Lasso => {
                                // Only append when the cursor has moved a
                                // few pixels since the last vertex, so a
                                // slow drag doesn't flood the polygon with
                                // near-duplicate points.
                                let should_append = state
                                    .lasso_points
                                    .last()
                                    .is_none_or(|&last| (last - current).length() > 3.0);
                                if should_append {
                                    state.lasso_points.push(current);
                                }
                                if state.lasso_points.len() >= 2 {
                                    ui.painter().add(egui::Shape::closed_line(
                                        state.lasso_points.clone(),
                                        egui::Stroke::new(
                                            1.5_f32,
                                            egui::Color32::from_white_alpha(200),
                                        ),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            if interact_response.drag_stopped_by(egui::PointerButton::Primary) {
                if let (Some(start), Some(current)) = (state.marquee_drag_start, hover_pos) {
                    if (start - current).length() > 4.0 {
                        match state.marquee_mode {
                            crate::MarqueeMode::Measure => {
                                let world_a = to_world(start);
                                let world_b = to_world(current);
                                let distance = (world_a - world_b).length();
                                state.measure_result = Some((world_a, world_b, distance));
                            }
                            crate::MarqueeMode::Select => {
                                let a = to_local_px(start);
                                let b = to_local_px(current);
                                actions.push(MenuAction::SelectInRect {
                                    screen_min: common::Vec2::new(a.x.min(b.x), a.y.min(b.y)),
                                    screen_max: common::Vec2::new(a.x.max(b.x), a.y.max(b.y)),
                                    viewport_size: viewport_size_px,
                                });
                            }
                            crate::MarqueeMode::Lasso => {
                                if state.lasso_points.len() >= 3 {
                                    let points = state
                                        .lasso_points
                                        .iter()
                                        .map(|&p| to_local_px(p))
                                        .collect();
                                    actions.push(MenuAction::SelectInLasso {
                                        points,
                                        viewport_size: viewport_size_px,
                                    });
                                }
                            }
                        }
                    }
                }
                state.marquee_drag_start = None;
                state.lasso_points.clear();
            }

            // Persist the last completed measurement across frames (not
            // just while dragging) until the next one replaces it.
            if let Some((start, end, distance)) = state.measure_result {
                let (screen_start, screen_end) = (to_screen(start), to_screen(end));
                ui.painter().line_segment(
                    [screen_start, screen_end],
                    egui::Stroke::new(2.0_f32, crate::theme::ACCENT),
                );
                ui.painter().text(
                    screen_end,
                    egui::Align2::LEFT_TOP,
                    format!("{distance:.1} units"),
                    egui::FontId::monospace(crate::theme::SIZE_SMALL),
                    egui::Color32::WHITE,
                );
            }
        });
}
