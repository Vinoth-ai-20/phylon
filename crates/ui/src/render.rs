//! # Phylon Research Interface — Render Entry Point
//!
//! `render_ui` is the single entry point for the egui layer. It arranges
//! all panels, dispatches all `MenuAction` events, and returns a
//! `CanvasInteraction` describing what the user did in the viewport.
//!
//! ## Panel Layout (from outermost to innermost)
//!
//! ```text
//! ┌─ Top: Menu Bar ─────────────────────────────────────────────────────────┐
//! ├─ Top: Toolbar ──────────────────────────────────────────────────────────┤
//! │ L:ActivityBar │ L:Sidebar │         Central (Viewport)       │          │
//! │               │           │                                  │          │
//! │               │           │                                  │          │
//! ├───────────────┴───────────┴──────────────────────────────────┴──────────┤
//! ├─ Bottom: Metrics / Event Log ───────────────────────────────────────────┤
//! └─ Bottom: Status Bar ────────────────────────────────────────────────────┘
//! Toast overlay: floating top-right cards, outside all panels
//! ```

use crate::types::*;

/// Main UI render entry point. Called every frame by the app.
#[allow(clippy::too_many_arguments)]
pub fn render_ui(
    ctx: &egui::Context,
    app_state: &mut AppState,
    world: &mut world::World,
    state: &mut crate::WorkbenchState,
) -> (CanvasInteraction, Vec<MenuAction>) {
    let mut actions = Vec::new();

    // ── Update internal clock ────────────────────────────────────────────────
    state.time = ctx.input(|i| i.time);
    state.cleanup_toasts();

    // ── Global keyboard shortcuts ────────────────────────────────────────────
    // `ShortcutManager::consume_all` (crate::shortcuts) is the single active
    // shortcut system — it used to be shadowed by a separate, hardcoded
    // `process_shortcuts` here that silently made several menu-advertised
    // shortcuts (Ctrl+M/L/B, speed up/down, Ctrl+Z/Y) dead, since egui's
    // `ShortcutManager` instance was only ever read for menu hint text, never
    // executed.
    state.shortcuts.consume_all(ctx, &mut actions);

    // ── Main Menu screen ─────────────────────────────────────────────────────
    if *app_state == AppState::MainMenu {
        render_main_menu(ctx, state, &mut actions);
        render_toasts(ctx, state);
        crate::plugins::dialogs::show_dialogs(ctx, state, &mut actions);
        return (CanvasInteraction::default(), actions);
    }

    // ── Dialogs (About, Docs, Keybinds) ─────────────────────────────────────
    crate::plugins::dialogs::show_dialogs(ctx, state, &mut actions);

    // ── Spectator mode logic ─────────────────────────────────────────────────
    tick_spectator(ctx, state, world);

    // ── Top: Menu Bar ────────────────────────────────────────────────────────
    egui::TopBottomPanel::top("top_menu_bar").show(ctx, |ui| {
        crate::plugins::menu::menu_ui(ctx, ui, state, world, &mut actions);
    });

    // ── Top: Toolbar (conditionally shown) ──────────────────────────────────
    if state.toolbar_visible {
        egui::TopBottomPanel::top("toolbar_panel")
            .exact_height(32.0)
            .show(ctx, |ui| {
                crate::plugins::toolbar::toolbar_ui(ctx, ui, state, world, &mut actions);
            });
    }

    // ── Bottom: Status Bar ──────────────────────────────────────────────────
    if state.status_bar_visible {
        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(24.0)
            .show(ctx, |ui| {
                crate::plugins::status_bar::status_bar_ui(ctx, ui, state, world, &mut actions);
            });
    }

    // ── Left: Activity Bar (icon+label rail, collapsible to icon-only) ──────
    // The activity bar switches the active_tab; the Inspector tile inside the
    // egui_tiles tree reads state.active_tab and renders the appropriate
    // content. Width depends on `activity_bar_expanded` — expanded (labeled)
    // is the default per the audit's discoverability finding; collapsed
    // (40px, icon-only) is the previous permanent behavior, still available
    // via the pin toggle at the bottom of the rail.
    let activity_bar_width = if state.activity_bar_expanded {
        140.0
    } else {
        40.0
    };
    egui::SidePanel::left("activity_bar")
        .exact_width(activity_bar_width)
        .resizable(false)
        .show(ctx, |ui| {
            crate::plugins::sidebar::activity_bar_ui(ctx, ui, state, world, &mut actions);
        });

    // ── Central Panel (egui_tiles viewport tree) ─────────────────────────────
    let interact_response = egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(egui::Color32::TRANSPARENT))
        .show(ctx, |ui| {
            let mut tree = std::mem::replace(&mut state.dock_tree, egui_tiles::Tree::empty("tmp"));

            let mut behavior = crate::layout::WorkbenchBehavior {
                state,
                world,
                commands: &mut actions,
                canvas_interaction: None,
            };

            tree.ui(&mut behavior, ui);
            let canvas_interaction = behavior.canvas_interaction;
            // behavior ends here naturally; no explicit drop needed

            // Capture this frame's live split ratios (a user may have just
            // dragged a divider) so the next dock/undock/reset-triggered
            // rebuild reproduces them instead of snapping back to the
            // hardcoded default — see `layout::extract_shares`.
            state.layout_shares = crate::layout::extract_shares(&tree);
            state.dock_tree = tree;

            canvas_interaction.unwrap_or_else(|| {
                let rect = ui.max_rect();
                CanvasInteraction {
                    rect,
                    clicked: false,
                    click_pos: None,
                    hover_pos: None,
                    drag_delta: egui::Vec2::ZERO,
                    zoom_delta: 1.0,
                }
            })
        })
        .inner;

    // ── Floating panels (Detached windows) ─────────────────────────────────
    let mut floating_canvas = None::<crate::types::CanvasInteraction>;
    crate::layout::render_floating_panels(ctx, state, world, &mut actions, &mut floating_canvas);
    // If the Viewport is floating, prefer its interaction over the fallback
    let interact_response = floating_canvas.unwrap_or(interact_response);

    // ── Vision cones overlay ────────────────────────────────────────────────
    if state.show_vision_cones {
        render_vision_cones(ctx, state, world, interact_response.rect);
    }

    // ── World-space scale grid (always on) ──────────────────────────────────
    // Low-opacity, so it reads as a subtle scale reference rather than
    // competing with the simulation — the audit's "no viewport scale
    // reference" finding. Unlike the boundary/vision-cone overlays, this one
    // is never gated behind a toggle: it's cheap (line count is bounded by
    // the visible world extent divided by the grid step, not by zoom level)
    // and useful at every zoom.
    render_scale_grid(ctx, state, interact_response.rect);

    // ── World boundary overlay ──────────────────────────────────────────────
    if state.show_world_boundary {
        render_world_boundary(ctx, state, interact_response.rect);
    }

    // ── Toast notifications overlay ─────────────────────────────────────────
    render_toasts(ctx, state);

    (interact_response, actions)
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn render_main_menu(
    ctx: &egui::Context,
    state: &mut crate::WorkbenchState,
    actions: &mut Vec<MenuAction>,
) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() / 4.0);
            ui.heading(
                egui::RichText::new("PHYLON")
                    .size(64.0)
                    .strong()
                    .color(egui::Color32::from_rgb(100, 200, 255)),
            );
            ui.label(
                egui::RichText::new("Artificial Life Simulation Engine")
                    .italics()
                    .color(crate::theme::DISABLED_FG),
            );
            ui.add_space(40.0);

            let btn_size = egui::vec2(200.0, 40.0);

            if ui
                .add_sized(
                    btn_size,
                    egui::Button::new(egui::RichText::new("New Simulation").size(20.0)),
                )
                .clicked()
            {
                actions.push(MenuAction::StartSimulation);
            }
            ui.add_space(10.0);

            if ui
                .add_sized(
                    btn_size,
                    egui::Button::new(egui::RichText::new("Load State…").size(20.0)),
                )
                .clicked()
            {
                actions.push(MenuAction::LoadState);
            }
            ui.add_space(10.0);

            if ui
                .add_sized(
                    btn_size,
                    egui::Button::new(egui::RichText::new("Settings").size(20.0)),
                )
                .clicked()
            {
                state.active_tab = SidebarTab::Settings;
                actions.push(MenuAction::StartSimulation);
            }
            ui.add_space(10.0);

            if ui
                .add_sized(
                    btn_size,
                    egui::Button::new(egui::RichText::new("About").size(20.0)),
                )
                .clicked()
            {
                state.show_about = true;
            }
            ui.add_space(10.0);

            if ui
                .add_sized(
                    btn_size,
                    egui::Button::new(
                        egui::RichText::new("Quit")
                            .size(20.0)
                            .color(egui::Color32::from_rgb(220, 100, 100)),
                    ),
                )
                .clicked()
            {
                actions.push(MenuAction::Quit);
            }
        });
    });
}

/// Render all active toast notifications as floating cards in the bottom-right.
fn render_toasts(ctx: &egui::Context, state: &crate::WorkbenchState) {
    // Show at most 5 toasts stacked upward from bottom-right
    let visible: Vec<_> = state.notifications.iter().rev().take(5).collect();

    let total = visible.len();
    for (idx, toast) in visible.into_iter().enumerate() {
        let offset_y = -(idx as f32) * 60.0 - 10.0;
        let (bg_color, border_color) = toast_colors(toast.severity);

        egui::Window::new(format!("__toast_{}", idx))
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, offset_y))
            .fixed_size(egui::vec2(280.0, 44.0))
            .frame(
                egui::Frame::none()
                    .fill(bg_color)
                    .stroke(egui::Stroke::new(1.5, border_color))
                    .rounding(egui::Rounding::same(crate::theme::RADIUS_STD))
                    .inner_margin(egui::Margin::symmetric(
                        crate::theme::SPACE_SM,
                        crate::theme::SPACE_XS,
                    )),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(toast_icon(toast.severity)).color(border_color));
                    ui.label(
                        egui::RichText::new(&toast.message)
                            .color(egui::Color32::WHITE)
                            .size(crate::theme::SIZE_BODY),
                    );
                });
            });

        let _ = total;
    }
}

/// (background, border) colors for a toast — routed through `theme::`'s
/// semantic `GOOD`/`WARN`/`BAD` tokens (and their `_SOFT` background tints)
/// instead of four independent hand-picked color pairs, so a toast and any
/// other semantically-colored surface (a validation error, a confirmation)
/// can never drift apart.
fn toast_colors(severity: crate::ToastSeverity) -> (egui::Color32, egui::Color32) {
    use crate::ToastSeverity::*;
    let opaque = |c: egui::Color32| egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 220);
    match severity {
        Info => (opaque(crate::theme::ACCENT_SOFT), crate::theme::ACCENT),
        Success => (opaque(crate::theme::GOOD_SOFT), crate::theme::GOOD),
        Warning => (opaque(crate::theme::WARN_SOFT), crate::theme::WARN),
        Error => (opaque(crate::theme::BAD_SOFT), crate::theme::BAD),
    }
}

fn toast_icon(severity: crate::ToastSeverity) -> &'static str {
    use crate::ToastSeverity::*;
    match severity {
        Info => egui_remixicon::icons::INFORMATION_LINE,
        Success => egui_remixicon::icons::CHECKBOX_CIRCLE_LINE,
        Warning => egui_remixicon::icons::ALERT_LINE,
        Error => egui_remixicon::icons::ERROR_WARNING_LINE,
    }
}

/// Spectator mode: automatically switch to the most interesting alive organism.
fn tick_spectator(
    ctx: &egui::Context,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
) {
    if !state.spectator_mode {
        return;
    }

    let current_time = ctx.input(|i| i.time);

    let is_tracked_dead = state
        .tracked_entity
        .is_none_or(|e| world.ecs.get_entity(e).is_none());

    if is_tracked_dead || current_time - state.last_spectator_switch_time > 15.0 {
        let mut best_entity = None;
        let mut highest_generation = 0u64;

        let mut query = world
            .ecs
            .query::<(bevy_ecs::entity::Entity, &organisms::OrganismColor)>();

        if let Some(tracker) = world.ecs.get_resource::<evolution::LineageTracker>() {
            for (entity, _) in query.iter(&world.ecs) {
                if let Some(record) = tracker.get_record(common::EntityId(entity.to_bits())) {
                    if record.generation >= highest_generation {
                        highest_generation = record.generation;
                        best_entity = Some(entity);
                    }
                } else if best_entity.is_none() {
                    best_entity = Some(entity);
                }
            }
        }

        if let Some(new_target) = best_entity {
            if state.tracked_entity != Some(new_target) {
                state.tracked_entity = Some(new_target);
                state.last_spectator_switch_time = current_time;
            }
        }
    }
}

/// Render vision cone overlays on the painter layer.
fn render_vision_cones(
    ctx: &egui::Context,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
    viewport_rect: egui::Rect,
) {
    let screen_center = viewport_rect.center();
    let ppp = ctx.pixels_per_point();

    let to_screen = |pos: common::Vec2| {
        egui::pos2(
            screen_center.x + (pos.x - state.camera_pos.x) * state.camera_zoom / ppp,
            screen_center.y - (pos.y - state.camera_pos.y) * state.camera_zoom / ppp,
        )
    };

    // Build selected component set for contextual filtering
    let selected_component: Option<std::collections::HashSet<bevy_ecs::entity::Entity>> =
        if let Some(entity) = state.selected_entity {
            let mut adj: std::collections::HashMap<
                bevy_ecs::entity::Entity,
                Vec<bevy_ecs::entity::Entity>,
            > = std::collections::HashMap::new();
            let mut query_springs = world.ecs.query::<&physics::Spring>();
            for spring in query_springs.iter(&world.ecs) {
                adj.entry(spring.node_a).or_default().push(spring.node_b);
                adj.entry(spring.node_b).or_default().push(spring.node_a);
            }

            let mut visited = std::collections::HashSet::new();
            let mut queue = std::collections::VecDeque::new();
            queue.push_back(entity);
            visited.insert(entity);
            while let Some(curr) = queue.pop_front() {
                if let Some(neighbors) = adj.get(&curr) {
                    for &n in neighbors {
                        if visited.insert(n) {
                            queue.push_back(n);
                        }
                    }
                }
            }
            Some(visited)
        } else {
            None
        };

    let mut painter = ctx.layer_painter(egui::LayerId::background());
    painter.set_clip_rect(viewport_rect);

    let mut query = world.ecs.query::<(
        bevy_ecs::entity::Entity,
        &physics::ParticleNode,
        &sensing::HeadVision,
    )>();

    for (ent, node, vision) in query.iter(&world.ecs) {
        if let Some(ref comp) = selected_component {
            if !comp.contains(&ent) {
                continue;
            }
        }

        let fwd = vision.last_forward;
        let head_radius = 12.0;
        let origin_pos = common::Vec2::new(
            node.position.x + fwd.x * head_radius,
            node.position.y + fwd.y * head_radius,
        );
        let origin = to_screen(origin_pos);
        let base_angle = fwd.y.atan2(fwd.x);
        let half_fov = vision.fov / 2.0;
        let segments = 16;

        let mut points = Vec::with_capacity(segments + 2);
        points.push(origin);
        for i in 0..=segments {
            let t = i as f32 / segments as f32;
            let angle = base_angle - half_fov + (vision.fov * t);
            let x = origin_pos.x + angle.cos() * vision.range;
            let y = origin_pos.y + angle.sin() * vision.range;
            points.push(to_screen(common::Vec2::new(x, y)));
        }

        painter.add(egui::Shape::closed_line(
            points,
            egui::Stroke::new(
                2.0,
                egui::Color32::from_rgba_premultiplied(0, 255, 255, 200),
            ),
        ));
    }
}

/// World half-extent in simulation units. Must match the hard physics/
/// diffusion/render bounds (`physics.wgsl`, `simulation.rs`, `render.rs`),
/// which are all ±1500.
const WORLD_HALF_EXTENT: f32 = 1500.0;

/// World-space spacing between scale-grid lines, in simulation units.
const SCALE_GRID_STEP: f32 = 100.0;

/// Draws a faint world-space grid across the visible viewport, plus a small
/// "N units" label in the corner, so a user always has a scale reference
/// under panning/zooming without needing to toggle anything on. Uses the
/// same world→screen transform as `render_world_boundary`/
/// `render_vision_cones`, but computes only the grid lines that intersect
/// the currently visible world extent (derived from `camera_pos`/
/// `camera_zoom`/`viewport_rect`) rather than a fixed line count, so cost
/// stays flat whether zoomed far in or far out.
fn render_scale_grid(
    ctx: &egui::Context,
    state: &crate::WorkbenchState,
    viewport_rect: egui::Rect,
) {
    if !viewport_rect.is_positive() {
        return;
    }
    let screen_center = viewport_rect.center();
    let ppp = ctx.pixels_per_point();
    let scale = state.camera_zoom / ppp;
    if scale <= 0.0 {
        return;
    }

    let to_screen = |x: f32, y: f32| {
        egui::pos2(
            screen_center.x + (x - state.camera_pos.x) * scale,
            screen_center.y - (y - state.camera_pos.y) * scale,
        )
    };

    // Visible world-space bounds of the viewport rect, plus one extra step
    // of margin so lines don't visibly pop in/out at the edges while panning.
    let half_w_world = (viewport_rect.width() / 2.0) / scale + SCALE_GRID_STEP;
    let half_h_world = (viewport_rect.height() / 2.0) / scale + SCALE_GRID_STEP;
    let min_x = ((state.camera_pos.x - half_w_world) / SCALE_GRID_STEP).floor() * SCALE_GRID_STEP;
    let max_x = ((state.camera_pos.x + half_w_world) / SCALE_GRID_STEP).ceil() * SCALE_GRID_STEP;
    let min_y = ((state.camera_pos.y - half_h_world) / SCALE_GRID_STEP).floor() * SCALE_GRID_STEP;
    let max_y = ((state.camera_pos.y + half_h_world) / SCALE_GRID_STEP).ceil() * SCALE_GRID_STEP;

    let mut painter = ctx.layer_painter(egui::LayerId::background());
    painter.set_clip_rect(viewport_rect);
    let stroke = egui::Stroke::new(
        1.0,
        egui::Color32::from_rgba_premultiplied(255, 255, 255, 18),
    );

    let mut x = min_x;
    while x <= max_x {
        painter.line_segment([to_screen(x, min_y), to_screen(x, max_y)], stroke);
        x += SCALE_GRID_STEP;
    }
    let mut y = min_y;
    while y <= max_y {
        painter.line_segment([to_screen(min_x, y), to_screen(max_x, y)], stroke);
        y += SCALE_GRID_STEP;
    }

    // Scale readout, anchored to the viewport's bottom-left corner.
    painter.text(
        egui::pos2(
            viewport_rect.left() + crate::theme::SPACE_SM,
            viewport_rect.bottom() - crate::theme::SPACE_LG,
        ),
        egui::Align2::LEFT_BOTTOM,
        format!("{:.0} units / grid", SCALE_GRID_STEP),
        egui::FontId::monospace(crate::theme::SIZE_MICRO),
        egui::Color32::from_rgba_premultiplied(255, 255, 255, 90),
    );
}

/// Draws a rectangle outline at the world boundary (±[`WORLD_HALF_EXTENT`])
/// using the same world→screen transform as the vision-cone overlay, so it
/// stays put under panning/zooming. Visual only — does not affect physics.
fn render_world_boundary(
    ctx: &egui::Context,
    state: &crate::WorkbenchState,
    viewport_rect: egui::Rect,
) {
    let screen_center = viewport_rect.center();
    let ppp = ctx.pixels_per_point();

    let to_screen = |pos: common::Vec2| {
        egui::pos2(
            screen_center.x + (pos.x - state.camera_pos.x) * state.camera_zoom / ppp,
            screen_center.y - (pos.y - state.camera_pos.y) * state.camera_zoom / ppp,
        )
    };

    let mut painter = ctx.layer_painter(egui::LayerId::background());
    painter.set_clip_rect(viewport_rect);

    let e = WORLD_HALF_EXTENT;
    let corners = [
        to_screen(common::Vec2::new(-e, -e)),
        to_screen(common::Vec2::new(e, -e)),
        to_screen(common::Vec2::new(e, e)),
        to_screen(common::Vec2::new(-e, e)),
    ];

    painter.add(egui::Shape::closed_line(
        corners.to_vec(),
        egui::Stroke::new(
            2.0,
            egui::Color32::from_rgba_premultiplied(255, 200, 0, 220),
        ),
    ));
}
