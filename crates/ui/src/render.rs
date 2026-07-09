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
    track_recent_selections(state);
    track_trajectory_history(state, world);
    // Accessibility pass 2 (Phase 2, M18) — reactive every frame so toggling
    // either setting in the Settings tab takes effect immediately.
    crate::theme::apply_style(ctx, state.high_contrast);
    ctx.set_zoom_factor(state.ui_scale.clamp(0.5, 3.0));
    // Reset each frame; whichever panel's row the cursor is over this frame
    // (if any) sets it again while rendering — see `panel_hover_entity`'s
    // doc comment.
    state.panel_hover_entity = None;

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

    // ── World-space scale grid ───────────────────────────────────────────────
    // Low-opacity, so it reads as a subtle scale reference rather than
    // competing with the simulation — the audit's "no viewport scale
    // reference" finding. On by default (cheap — line count is bounded by
    // the visible world extent divided by the grid step, not by zoom level)
    // but toggleable, e.g. for a clean screenshot/recording.
    if state.show_scale_grid {
        render_scale_grid(ctx, state, interact_response.rect);
    }

    // ── World boundary overlay ──────────────────────────────────────────────
    if state.show_world_boundary {
        render_world_boundary(ctx, state, interact_response.rect);
    }

    // ── Minimap overlay (Phase 2, M17) ───────────────────────────────────────
    if state.show_minimap {
        render_minimap(ctx, state, world, interact_response.rect);
    }

    // ── Organism labels (Phase 5, SX-5a) ─────────────────────────────────────
    // Priority 5 (cosmetic/identity, lowest) per the Numeric priority
    // hierarchy — drawn before every other biological overlay so labels
    // never sit on top of (obscure) a Behavior glyph, Health/Disease badge,
    // or Death/Reproduction burst; those all paint after this.
    if state.show_organism_labels {
        render_organism_labels(ctx, state, world, interact_response.rect);
    }

    // ── Trajectory trail (Phase 5, SX-5c) ────────────────────────────────────
    // Priority 4 (Ecological status), tied to the tracked entity only —
    // reuses `state.trajectory_history`, the data SX-4c's Inspector
    // Relationships/History section already populates every tick; this is
    // the viewport-visual half of that same feature, not a second tracking
    // mechanism. No separate toggle: tracking an entity (`tracked_entity`)
    // is already the opt-in gesture.
    if state.tracked_entity.is_some() {
        render_trajectory_trail(ctx, state, interact_response.rect);
    }

    // ── Behavior-state glyph overlay (Phase 5, SX-1b) ───────────────────────
    // Population-wide, not opt-in — per `docs/design/biological_visual_language.md`'s
    // Behavior entry, a Priority-4 state.
    render_behavior_glyphs(ctx, state, world, interact_response.rect);

    // ── Physiology science overlay (Phase 4, P4-V2) ─────────────────────────
    // Priority 4 (Tertiary/opt-in detail) — drawn before the Priority 2/3
    // timed-effect overlay below, for the same reason Behavior is: it must
    // never paint over a Death/Reproduction burst.
    if state.physiology_overlay.is_some() {
        render_physiology_overlay(ctx, state, world, interact_response.rect);
    }

    // ── Timed interaction-effect overlay (Phase 4, P4-V1) ───────────────────
    // Renders `events::TimedEffects` — the data-side framework P4-E1 built
    // but deliberately left unrendered (see that milestone's execution log).
    // Phase 5, SX-1e: moved to *last* among the biological overlays — Death/
    // Reproduction bursts are Priority 2/3, strictly above Behavior's and
    // Physiology's Priority 4, so they must paint on top of both, never
    // underneath. Re-audited the previous order (this call preceded both)
    // and found it violated the mandatory priority hierarchy: a Behavior
    // glyph or Physiology ring could visually sit on top of (obscure) a
    // same-position death/birth burst. Painter calls composite in call order
    // within the same egui layer, so this reorder is the entire fix — no new
    // drawing logic.
    render_timed_effects(ctx, state, world, interact_response.rect);

    // ── Command Palette overlay (Phase 2, M15) ──────────────────────────────
    crate::plugins::command_palette::command_palette_ui(ctx, state, &mut actions);

    // ── Workspace Manager overlay (Phase 7, W3c) ────────────────────────────
    crate::plugins::workspace_manager::workspace_manager_ui(ctx, state, &mut actions);

    // ── Global Search overlay (Phase 7, W6a) ────────────────────────────────
    crate::plugins::global_search::global_search_ui(ctx, world, state);

    // ── Toast notifications overlay ─────────────────────────────────────────
    render_toasts(ctx, state);

    (interact_response, actions)
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Pushes `selected_entity` onto `recent_selections` whenever it changes
/// from the previous frame (Phase 2, M13) — deliberately done here, once per
/// frame, rather than at each of `selected_entity`'s ~20 existing write
/// sites, so none of them needed to change. Deselecting (a change to `None`)
/// is not recorded — only a change *to* some entity counts as "selecting"
/// it. A re-selection of an already-most-recent entity is a no-op rather
/// than a duplicate push.
fn track_recent_selections(state: &mut crate::WorkbenchState) {
    if state.selected_entity != state.previous_selected_entity {
        if let Some(entity) = state.selected_entity {
            if state.recent_selections.front() != Some(&entity) {
                state.recent_selections.retain(|&e| e != entity);
                state.recent_selections.push_front(entity);
                state
                    .recent_selections
                    .truncate(crate::state::RECENT_SELECTIONS_CAPACITY);
            }
        }
        state.previous_selected_entity = state.selected_entity;
    }
}

/// Samples `tracked_entity`'s current position into
/// `WorkbenchState::trajectory_history`, once per *simulation tick* (not per
/// render frame — reading `metabolism::GlobalAtmosphere.ticks` and skipping
/// unless it's changed since the last sample avoids capturing dozens of
/// near-duplicate points per real second at typical frame rates, which
/// would waste almost all of the bounded buffer on redundant data while the
/// simulation is paused or between ticks). Phase 5, SX-4c.
///
/// Resets the history whenever `tracked_entity` changes (including to
/// `None`) — a trail belongs to one entity at a time, not a blended path
/// across whichever entities happened to be tracked previously.
fn track_trajectory_history(state: &mut crate::WorkbenchState, world: &mut world::World) {
    if state.tracked_entity != state.trajectory_entity {
        state.trajectory_history.clear();
        state.trajectory_entity = state.tracked_entity;
        state.trajectory_last_tick = None;
    }

    let Some(entity) = state.tracked_entity else {
        return;
    };

    let current_tick = world
        .ecs
        .get_resource::<metabolism::GlobalAtmosphere>()
        .map_or(0, |a| a.ticks);
    if state.trajectory_last_tick == Some(current_tick) {
        return;
    }

    let mut node_q = world.ecs.query::<&physics::ParticleNode>();
    if let Ok(node) = node_q.get(&world.ecs, entity) {
        state.trajectory_history.push_back(node.position);
        if state.trajectory_history.len() > crate::state::TRAJECTORY_HISTORY_CAPACITY {
            state.trajectory_history.pop_front();
        }
        state.trajectory_last_tick = Some(current_tick);
    }
}

/// # Trajectory Trail
///
/// ## 1. What Happens
/// Draws `state.trajectory_history` (populated by `track_trajectory_history`,
/// SX-4c) as a short, fading polyline behind the tracked entity — oldest
/// samples nearly transparent, newest fully opaque.
///
/// ## 2. Why It Happens
/// Population-wide trails would be visual noise, not signal (this
/// milestone's own framing) — hundreds of overlapping paths would obscure
/// the viewport rather than clarify it. Limited to the one entity the
/// researcher already opted into tracking, the same restraint
/// `render_physiology_overlay` already applies to per-segment detail.
///
/// ## 3. How It Happens
/// No new data — this is the visual half of SX-4c's already-populated
/// `trajectory_history`, not a second tracking mechanism. Segment alpha
/// scales linearly by position in the buffer (oldest→newest), so the trail
/// visibly fades rather than cutting off abruptly at its start.
fn render_trajectory_trail(
    ctx: &egui::Context,
    state: &crate::WorkbenchState,
    viewport_rect: egui::Rect,
) {
    if state.trajectory_history.len() < 2 {
        return;
    }

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

    let total = state.trajectory_history.len();
    let [r, g, b, _] = crate::theme::ACCENT.to_normalized_gamma_f32();
    for (i, (a, b_pos)) in state
        .trajectory_history
        .iter()
        .zip(state.trajectory_history.iter().skip(1))
        .enumerate()
    {
        let progress = (i + 1) as f32 / total as f32; // 0 (oldest) .. 1 (newest)
        let alpha = (progress * 200.0) as u8;
        let color = egui::Color32::from_rgba_unmultiplied(
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8,
            alpha,
        );
        painter.line_segment(
            [to_screen(*a), to_screen(*b_pos)],
            egui::Stroke::new(2.0, color),
        );
    }
}

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
                    .size(crate::theme::SIZE_SPLASH_TITLE)
                    .strong()
                    .color(crate::theme::ACCENT),
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
                    egui::Button::new(
                        egui::RichText::new("New Simulation")
                            .size(crate::theme::SIZE_SPLASH_BUTTON),
                    ),
                )
                .clicked()
            {
                actions.push(MenuAction::StartSimulation);
            }
            ui.add_space(crate::theme::SPACE_SM);

            if ui
                .add_sized(
                    btn_size,
                    egui::Button::new(
                        egui::RichText::new("Load State…").size(crate::theme::SIZE_SPLASH_BUTTON),
                    ),
                )
                .clicked()
            {
                actions.push(MenuAction::LoadState);
            }
            ui.add_space(crate::theme::SPACE_SM);

            if ui
                .add_sized(
                    btn_size,
                    egui::Button::new(
                        egui::RichText::new("Settings").size(crate::theme::SIZE_SPLASH_BUTTON),
                    ),
                )
                .clicked()
            {
                state.active_tab = SidebarTab::Settings;
                actions.push(MenuAction::StartSimulation);
            }
            ui.add_space(crate::theme::SPACE_SM);

            if ui
                .add_sized(
                    btn_size,
                    egui::Button::new(
                        egui::RichText::new("About").size(crate::theme::SIZE_SPLASH_BUTTON),
                    ),
                )
                .clicked()
            {
                state.show_about = true;
            }
            ui.add_space(crate::theme::SPACE_SM);

            if ui
                .add_sized(
                    btn_size,
                    egui::Button::new(
                        egui::RichText::new("Quit")
                            .size(crate::theme::SIZE_SPLASH_BUTTON)
                            .color(crate::theme::DANGER),
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
        let offset_y =
            -(idx as f32) * crate::theme::TOAST_STACK_OFFSET - crate::theme::TOAST_BOTTOM_MARGIN;
        let (bg_color, border_color) = toast_colors(toast.severity);

        egui::Window::new(format!("__toast_{}", idx))
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(
                egui::Align2::RIGHT_BOTTOM,
                egui::vec2(-crate::theme::TOAST_RIGHT_MARGIN, offset_y),
            )
            .fixed_size(crate::theme::TOAST_SIZE)
            .frame(
                egui::Frame::none()
                    .fill(bg_color)
                    .stroke(egui::Stroke::new(
                        crate::theme::TOAST_STROKE_WIDTH,
                        border_color,
                    ))
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
                            .color(crate::theme::TEXT_PRIMARY)
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
                state.set_follow(Some(new_target));
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

/// # Timed Interaction-Effect Overlay
///
/// ## 1. What Happens
/// Draws every currently-active `events::TimedEffects::FloatingText` at its
/// world position, converted to screen space — the rendering P4-E1's
/// `TimedEffects` framework deliberately deferred (see that milestone's
/// module doc comment: "Epic 8's job").
///
/// ## 2. Why It Happens
/// P4-E1 proved the data-side framework works (a real predation death spawns
/// a real, correctly-expiring `TimedEffect`) but drawing it was explicitly
/// out of scope, per ADR-P4-05's Epic 6/Epic 8 split. This is that drawing
/// step, for whichever event types by then have a real producer (see
/// `crates/app/src/systems.rs` for the current producers: predation,
/// reproduction, disease transmission, decomposition).
///
/// ## 3. How It Happens
/// Same world→screen transform and background-layer `Painter` pattern as
/// `render_vision_cones` above. Text fades linearly over its last third of
/// remaining lifetime rather than popping off abruptly, using
/// `TimedEffects`' own `expires_at_tick` and the current tick (read from
/// `metabolism::GlobalAtmosphere`, the same tick source every P4-F/E-tier
/// system this phase already uses).
fn render_timed_effects(
    ctx: &egui::Context,
    state: &crate::WorkbenchState,
    world: &mut world::World,
    viewport_rect: egui::Rect,
) {
    let Some(effects) = world.ecs.get_resource::<events::TimedEffects>() else {
        return;
    };
    if effects.active.is_empty() {
        return;
    }
    let current_tick = world
        .ecs
        .get_resource::<metabolism::GlobalAtmosphere>()
        .map_or(0, |a| a.ticks);

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

    // Fade-out window: the last third of an effect's remaining lifetime at
    // the moment it's drawn is not knowable without its original duration,
    // so this fades over a fixed tail instead — simple and effect-agnostic.
    const FADE_TICKS: u64 = 20;

    for effect in world
        .ecs
        .get_resource::<events::TimedEffects>()
        .into_iter()
        .flat_map(|e| e.active.iter())
    {
        let events::TimedEffectKind::FloatingText { text, color } = &effect.kind;
        let remaining = effect.expires_at_tick.saturating_sub(current_tick);
        let alpha = if remaining < FADE_TICKS {
            (remaining as f32 / FADE_TICKS as f32).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let color32 = egui::Color32::from_rgba_unmultiplied(
            (color[0] * 255.0) as u8,
            (color[1] * 255.0) as u8,
            (color[2] * 255.0) as u8,
            (alpha * 255.0) as u8,
        );
        painter.text(
            to_screen(effect.position),
            egui::Align2::CENTER_BOTTOM,
            text,
            egui::FontId::proportional(14.0),
            color32,
        );
    }
}

/// # Behavior-State Glyph Overlay
///
/// ## 1. What Happens
/// Draws a small glyph above every organism whose `behavior::BehaviorState`
/// is not `Idle` — one canonical icon+color pair per state, per
/// `docs/design/biological_visual_language.md`'s Behavior entry. `Idle`
/// (the most common state) draws nothing, so the viewport isn't cluttered
/// with a badge on every resting organism — absence *is* the encoding.
///
/// ## 2. Why It Happens
/// SX-1a/SX-2a's investigation found Phylon's dominant readability problem
/// isn't motion, it's communication: nothing in the viewport currently
/// reflects what an organism is *doing*. `BehaviorState` is already computed
/// every tick by `behavior::behavior_system` and already shown live in the
/// Inspector — this overlay is the same data, population-wide, in the one
/// place a researcher is actually looking most of the time.
///
/// ## 3. How It Happens
/// Same world→screen transform and background-layer `Painter` pattern as
/// `render_timed_effects` above — reused, not reinvented. `BehaviorState`
/// lives on the same entity as `physics::ParticleNode` (confirmed by reading
/// `behavior::behavior_system`'s own query tuple), so this is a single flat
/// query, no per-organism graph walk needed. The glyph is drawn via
/// `egui_remixicon`, matching every panel's existing icon usage, at a fixed
/// size — no decorative animation, per this phase's engineering rule (the
/// glyph's *presence*, not any motion on it, carries the meaning).
fn render_behavior_glyphs(
    ctx: &egui::Context,
    state: &crate::WorkbenchState,
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

    let mut painter = ctx.layer_painter(egui::LayerId::background());
    painter.set_clip_rect(viewport_rect);

    let mut query = world
        .ecs
        .query::<(&physics::ParticleNode, &behavior::BehaviorState)>();
    for (node, behavior_state) in query.iter(&world.ecs) {
        let (glyph, color) = match behavior_state {
            behavior::BehaviorState::Idle => continue,
            behavior::BehaviorState::Hunting => (
                egui_remixicon::icons::ARROW_UP_S_LINE,
                egui::Color32::from_rgb(230, 140, 30),
            ),
            behavior::BehaviorState::Fleeing => (
                egui_remixicon::icons::ALERT_LINE,
                egui::Color32::from_rgb(220, 60, 60),
            ),
            behavior::BehaviorState::Foraging => (
                egui_remixicon::icons::LEAF_LINE,
                egui::Color32::from_rgb(80, 190, 90),
            ),
            behavior::BehaviorState::Mating => (
                egui_remixicon::icons::HEART_LINE,
                egui::Color32::from_rgb(230, 110, 170),
            ),
            behavior::BehaviorState::Sleeping => (
                egui_remixicon::icons::ZZZ_LINE,
                egui::Color32::from_rgb(100, 140, 220),
            ),
        };

        let screen_pos = to_screen(node.position);
        painter.text(
            screen_pos - egui::vec2(0.0, 14.0),
            egui::Align2::CENTER_BOTTOM,
            glyph,
            egui::FontId::proportional(14.0),
            color,
        );
    }
}

/// # Organism Labels
///
/// ## 1. What Happens
/// Draws a small text label ("`<Diet> {Idx, Gen}`", the same format
/// `inspector_ui`'s header already uses) above every organism head within
/// `state.show_organism_labels`'s scope: the selected/tracked entity
/// (always, if one exists), plus the nearest
/// `crate::state::ORGANISM_LABEL_MAX_COUNT` other organisms to the camera
/// center.
///
/// ## 2. Why It Happens
/// Opt-in, per the roadmap's own framing — most research sessions don't
/// want a label on every organism at once. "Density-aware" is the other
/// half: even with labels enabled, the count actually drawn is bounded
/// regardless of total population (hundreds to thousands at typical
/// scales), so turning this on doesn't turn the viewport into unreadable
/// text clutter — the exact failure this milestone's own name warns
/// against.
///
/// ## 3. How It Happens
/// Nearest-to-camera-center selection reuses the same "sort by distance,
/// take N" shape `inspector_ui`'s Relationships section already established
/// for its nearby-organisms list (SX-4c) — a different distance origin
/// (camera center here, vs. a specific organism there), same pattern, not a
/// new one invented for this milestone.
fn render_organism_labels(
    ctx: &egui::Context,
    state: &crate::WorkbenchState,
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

    let mut painter = ctx.layer_painter(egui::LayerId::background());
    painter.set_clip_rect(viewport_rect);

    let label_for = |diet: &ecology::Diet, entity: bevy_ecs::entity::Entity| -> String {
        format!("{:?} {{Idx: {}}}", diet, entity.index())
    };

    let mut query = world.ecs.query::<(
        bevy_ecs::entity::Entity,
        &physics::ParticleNode,
        &ecology::Diet,
    )>();

    let mut always_labeled: std::collections::HashSet<bevy_ecs::entity::Entity> =
        std::collections::HashSet::new();
    for pinned in [state.selected_entity, state.tracked_entity]
        .into_iter()
        .flatten()
    {
        if let Ok((entity, node, diet)) = query.get(&world.ecs, pinned) {
            always_labeled.insert(entity);
            painter.text(
                to_screen(node.position) - egui::vec2(0.0, 18.0),
                egui::Align2::CENTER_BOTTOM,
                label_for(diet, entity),
                egui::FontId::proportional(12.0),
                crate::theme::ACCENT,
            );
        }
    }

    let mut nearby: Vec<(bevy_ecs::entity::Entity, common::Vec2, ecology::Diet, f32)> = query
        .iter(&world.ecs)
        .filter(|(e, ..)| !always_labeled.contains(e))
        .map(|(e, node, diet)| {
            (
                e,
                node.position,
                diet.clone(),
                node.position.distance(state.camera_pos),
            )
        })
        .collect();
    nearby.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal));

    for (entity, position, diet, _dist) in nearby
        .into_iter()
        .take(crate::state::ORGANISM_LABEL_MAX_COUNT)
    {
        painter.text(
            to_screen(position) - egui::vec2(0.0, 18.0),
            egui::Align2::CENTER_BOTTOM,
            label_for(&diet, entity),
            egui::FontId::proportional(12.0),
            crate::theme::DISABLED_FG,
        );
    }
}

/// # Physiology Science Overlay
///
/// ## 1. What Happens
/// For the selected/tracked organism, draws a colored ring at each Body
/// Graph segment's world position, sized/colored by whichever physiology
/// layer `state.physiology_overlay` is set to (Circulation → ATP level,
/// Hormone → dominant channel, Immune → infection severity) — toggled from
/// the corresponding P4-R1-R4 Viewer panel's own "Show on viewport" control.
///
/// ## 2. Why It Happens
/// P4-R1-R4's panels are tables in a side dock — useful for precise values,
/// but they don't show *where on the body* something is happening at a
/// glance. ADR-P4-05 frames Epic 8's visualization needs (blood flow, ATP
/// transport, hormone diffusion, immune activity) as viewport-space
/// overlays, the same category as P4-V1's timed effects, not another table.
///
/// ## 3. How It Happens
/// **Disclosed scope simplification:** this draws current per-segment
/// *magnitude* as a static colored ring, not animated directional flow
/// particles along Body Graph edges — matching Circulation Viewer's own
/// "levels, not flow rate" disclosure (P4-R2). A future milestone could add
/// particle-trail animation along edges if that fidelity is wanted; this is
/// a real, live, per-segment visualization, not a placeholder.
fn render_physiology_overlay(
    ctx: &egui::Context,
    state: &crate::WorkbenchState,
    world: &mut world::World,
    viewport_rect: egui::Rect,
) {
    let Some(layer) = state.physiology_overlay else {
        return;
    };
    let Some(entity) = state.selected_entity.or(state.tracked_entity) else {
        return;
    };
    let Some(graph) = world
        .ecs
        .query::<&organisms::DevelopmentalGraph>()
        .get(&world.ecs, entity)
        .ok()
        .cloned()
    else {
        return;
    };

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

    let mut node_q = world.ecs.query::<&physics::ParticleNode>();
    let mut chem_q = world.ecs.query::<&metabolism::ChemicalEconomy>();
    let mut hormone_q = world.ecs.query::<&brain::HormoneLevel>();
    let mut severity_q = world.ecs.query::<&ecology::disease::SegmentInfection>();

    for graph_node in &graph.nodes {
        let Some(seg_entity) = graph_node.entity else {
            continue;
        };
        let Ok(node) = node_q.get(&world.ecs, seg_entity) else {
            continue;
        };

        let (intensity, color) = match layer {
            PhysiologyOverlayLayer::Circulation => {
                let Ok(chem) = chem_q.get(&world.ecs, seg_entity) else {
                    continue;
                };
                let frac = if chem.max_atp > 0.0 {
                    (chem.atp / chem.max_atp).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                (frac, egui::Color32::from_rgb(220, 90, 90))
            }
            PhysiologyOverlayLayer::Hormone => {
                let Ok(level) = hormone_q.get(&world.ecs, seg_entity) else {
                    continue;
                };
                let frac = level.dopamine.max(level.serotonin).max(level.noradrenaline);
                (frac.clamp(0.0, 1.0), egui::Color32::from_rgb(160, 120, 220))
            }
            PhysiologyOverlayLayer::Immune => {
                let Ok(segment_infection) = severity_q.get(&world.ecs, seg_entity) else {
                    continue;
                };
                (
                    segment_infection.severity.clamp(0.0, 1.0),
                    egui::Color32::from_rgb(90, 200, 120),
                )
            }
        };

        let screen_pos = to_screen(node.position);
        let radius = 6.0 + intensity * 14.0;
        let alpha = (60.0 + intensity * 180.0) as u8;
        painter.circle_stroke(
            screen_pos,
            radius,
            egui::Stroke::new(2.0, color.gamma_multiply(alpha as f32 / 255.0)),
        );
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

/// Fixed size of the minimap's inset square, in screen points.
const MINIMAP_SIZE: f32 = 160.0;

/// Draws a fixed-size overview map in the viewport's bottom-right corner
/// (Phase 2, M17): every organism's head position as a Diet-colored dot, plus
/// a rectangle showing the main camera's currently-visible world extent.
/// Unlike `render_scale_grid`/`render_world_boundary`, this has its own
/// independent world→minimap-space transform (always shows the *whole*
/// bounded world, regardless of the main camera's zoom).
///
/// **Scope note:** click-to-recenter-camera was considered but not built —
/// this overlay is drawn on the background painter layer, not through a
/// `Sense`d widget, so a manual click check here would double-fire alongside
/// the viewport's own click-to-select handling for the same screen
/// coordinates with no easy way to verify the interaction doesn't conflict
/// without a live visual test. Left as a visual reference only; noted here
/// rather than silently attempted.
fn render_minimap(
    ctx: &egui::Context,
    state: &crate::WorkbenchState,
    world: &mut world::World,
    viewport_rect: egui::Rect,
) {
    if !viewport_rect.is_positive() {
        return;
    }
    let margin = 12.0;
    let minimap_rect = egui::Rect::from_min_size(
        egui::pos2(
            viewport_rect.right() - MINIMAP_SIZE - margin,
            viewport_rect.bottom() - MINIMAP_SIZE - margin,
        ),
        egui::vec2(MINIMAP_SIZE, MINIMAP_SIZE),
    );

    let e = WORLD_HALF_EXTENT;
    let world_to_minimap = |pos: common::Vec2| {
        egui::pos2(
            minimap_rect.left() + (pos.x + e) / (2.0 * e) * MINIMAP_SIZE,
            minimap_rect.bottom() - (pos.y + e) / (2.0 * e) * MINIMAP_SIZE,
        )
    };

    let mut painter = ctx.layer_painter(egui::LayerId::background());
    painter.set_clip_rect(viewport_rect);

    painter.rect(
        minimap_rect,
        crate::theme::RADIUS_TIGHT,
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 140),
        egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_premultiplied(255, 255, 255, 100),
        ),
    );

    let mut query = world
        .ecs
        .query::<(&physics::ParticleNode, Option<&ecology::Diet>)>();
    for (node, diet) in query.iter(&world.ecs) {
        if node.segment_type != 0 {
            continue; // head nodes only — one dot per organism
        }
        let color = diet
            .map(crate::theme::chart_color)
            .unwrap_or(crate::theme::DISABLED_FG);
        painter.circle_filled(world_to_minimap(node.position), 1.5, color);
    }

    // Main camera's visible world extent, so the minimap also shows "you are
    // here" — reuses the same screen-size/zoom math `render_scale_grid` uses
    // to derive visible world bounds from `camera_zoom`.
    let ppp = ctx.pixels_per_point();
    let half_w_world = (viewport_rect.width() / 2.0) / (state.camera_zoom / ppp);
    let half_h_world = (viewport_rect.height() / 2.0) / (state.camera_zoom / ppp);
    let frustum = egui::Rect::from_min_max(
        world_to_minimap(common::Vec2::new(
            state.camera_pos.x - half_w_world,
            state.camera_pos.y + half_h_world,
        )),
        world_to_minimap(common::Vec2::new(
            state.camera_pos.x + half_w_world,
            state.camera_pos.y - half_h_world,
        )),
    );
    painter.rect_stroke(
        frustum.intersect(minimap_rect),
        0.0,
        egui::Stroke::new(1.0, crate::theme::ACCENT),
    );
}
