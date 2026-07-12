//! Viewport navigation gizmos (Phase 9, P9.5).
//!
//! Per ADR-P9-02 (frozen after P9.3/P9.4): camera interaction and viewport
//! visualization are separate responsibilities from here on. Every function
//! in this module only *reads* camera/selection state and *issues commands*
//! through the existing, already-frozen APIs (`Camera3d::world_to_screen`
//! for projection, `MenuAction::SetCameraPreset`/`FrameSelected` for
//! commands) — nothing here ever mutates `OrbitController`/`FlyController`
//! directly, or adds new orientation math. All drawing is a plain egui
//! overlay on the background layer, the same technique
//! `render_behavior_glyphs`/`render_timed_effects` already use — no new
//! wgpu/shader work, and the organism rendering pipeline is untouched.

use crate::camera::Camera3d;

/// World-space direction each of the six preset views looks *along* —
/// mirrors `app::events`'s `MenuAction::SetCameraPreset` handler exactly
/// (duplicated as data, not logic: this module only needs to know which
/// preset the camera's *current* forward vector is closest to, for
/// highlighting, not how to reach it — reaching it is still only ever
/// done by pushing the same `MenuAction`).
const PRESET_DIRECTIONS: [(crate::types::CameraPreset, common::Vec3); 6] = [
    (
        crate::types::CameraPreset::Top,
        common::Vec3::new(0.0, 0.0, -1.0),
    ),
    (
        crate::types::CameraPreset::Bottom,
        common::Vec3::new(0.0, 0.0, 1.0),
    ),
    (
        crate::types::CameraPreset::Front,
        common::Vec3::new(0.0, 1.0, 0.0),
    ),
    (
        crate::types::CameraPreset::Back,
        common::Vec3::new(0.0, -1.0, 0.0),
    ),
    (
        crate::types::CameraPreset::Right,
        common::Vec3::new(-1.0, 0.0, 0.0),
    ),
    (
        crate::types::CameraPreset::Left,
        common::Vec3::new(1.0, 0.0, 0.0),
    ),
];

fn preset_label(preset: crate::types::CameraPreset) -> &'static str {
    match preset {
        crate::types::CameraPreset::Top => "Top",
        crate::types::CameraPreset::Bottom => "Bottom",
        crate::types::CameraPreset::Front => "Front",
        crate::types::CameraPreset::Back => "Back",
        crate::types::CameraPreset::Right => "Right",
        crate::types::CameraPreset::Left => "Left",
    }
}

/// Semi-transparent panel background for this module's two floating
/// overlays (nav cube, scene info) — routed through `theme::CHROME_BG`
/// rather than an independent hand-picked literal, matching the same
/// base-token-plus-alpha convention `render.rs`'s `toast_colors` already
/// established for this codebase's other floating surfaces (Phase 9,
/// P9.7 polish pass).
fn gizmo_panel_fill(alpha: u8) -> egui::Color32 {
    let c = crate::theme::CHROME_BG;
    egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

/// Renders every gizmo this milestone adds, in one call — the single entry
/// point `app::render`'s overlay sequence should call, right after the last
/// biological overlay (`render_timed_effects`) and before transient UI
/// chrome (Command Palette, Toasts), matching the existing "biological
/// content, then navigation chrome, then transient popups" paint order.
#[allow(clippy::too_many_arguments)]
pub fn render_gizmos(
    ctx: &egui::Context,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
    viewport_rect: egui::Rect,
    actions: &mut Vec<crate::types::MenuAction>,
) {
    let camera = state.camera();
    let ppp = ctx.pixels_per_point();
    let viewport_size_px =
        common::Vec2::new(viewport_rect.width() * ppp, viewport_rect.height() * ppp);

    render_world_origin_indicator(ctx, &camera, viewport_rect, viewport_size_px, ppp);
    render_pivot_indicator(ctx, state, &camera, viewport_rect, viewport_size_px, ppp);
    render_selection_bounding_box(
        ctx,
        state,
        world,
        &camera,
        viewport_rect,
        viewport_size_px,
        ppp,
    );
    render_axis_triad(ctx, &camera, viewport_rect);
    render_navigation_cube(ctx, &camera, viewport_rect, actions);
    render_scene_info_overlay(ctx, state, &camera, viewport_rect);
}

/// Projects a world-space point to a screen position within `viewport_rect`
/// (logical points, matching every other overlay in this codebase) — the
/// same physical-pixel round-trip `ui::plugins::viewport`'s own
/// `to_screen` closure uses, factored out since every gizmo below needs it.
fn project(
    camera: &Camera3d,
    world_pos: common::Vec3,
    viewport_rect: egui::Rect,
    viewport_size_px: common::Vec2,
    ppp: f32,
) -> Option<egui::Pos2> {
    camera
        .world_to_screen(world_pos, viewport_size_px)
        .map(|s| {
            egui::pos2(
                s.x / ppp + viewport_rect.min.x,
                s.y / ppp + viewport_rect.min.y,
            )
        })
}

/// World origin indicator — a small crosshair at world `(0, 0, 0)`, when
/// it's in view. Purely a reference marker; no interaction.
fn render_world_origin_indicator(
    ctx: &egui::Context,
    camera: &Camera3d,
    viewport_rect: egui::Rect,
    viewport_size_px: common::Vec2,
    ppp: f32,
) {
    let Some(screen_pos) = project(
        camera,
        common::Vec3::ZERO,
        viewport_rect,
        viewport_size_px,
        ppp,
    ) else {
        return;
    };
    if !viewport_rect.contains(screen_pos) {
        return;
    }
    let painter = ctx.layer_painter(egui::LayerId::background());
    let t = crate::theme::TEXT_PRIMARY;
    let color = egui::Color32::from_rgba_unmultiplied(t.r(), t.g(), t.b(), 140);
    let r = 6.0;
    painter.line_segment(
        [
            screen_pos - egui::vec2(r, 0.0),
            screen_pos + egui::vec2(r, 0.0),
        ],
        egui::Stroke::new(1.5, color),
    );
    painter.line_segment(
        [
            screen_pos - egui::vec2(0.0, r),
            screen_pos + egui::vec2(0.0, r),
        ],
        egui::Stroke::new(1.5, color),
    );
}

/// Camera pivot indicator — a small diamond at `OrbitController::focus`
/// (Orbit mode only; Fly mode has no pivot concept, matching every other
/// Orbit-only feature in this codebase). Read-only: this function never
/// writes to `focus`, only displays it.
fn render_pivot_indicator(
    ctx: &egui::Context,
    state: &crate::WorkbenchState,
    camera: &Camera3d,
    viewport_rect: egui::Rect,
    viewport_size_px: common::Vec2,
    ppp: f32,
) {
    let crate::camera::CameraController::Orbit(orbit) = &state.camera_controller else {
        return;
    };
    let Some(screen_pos) = project(camera, orbit.focus, viewport_rect, viewport_size_px, ppp)
    else {
        return;
    };
    if !viewport_rect.contains(screen_pos) {
        return;
    }
    let painter = ctx.layer_painter(egui::LayerId::background());
    let color = crate::theme::ACCENT;
    let r = 5.0;
    let points = vec![
        screen_pos + egui::vec2(0.0, -r),
        screen_pos + egui::vec2(r, 0.0),
        screen_pos + egui::vec2(0.0, r),
        screen_pos + egui::vec2(-r, 0.0),
    ];
    painter.add(egui::Shape::closed_line(
        points,
        egui::Stroke::new(1.5, color),
    ));
}

/// Selection bounding-box gizmo — a wireframe box around every
/// `ParticleNode` sharing the selected entity's `organism_id`, using the
/// same "walk the population, filter by organism_id" pattern
/// `MenuAction::FrameSelected` already established (not a new query shape).
#[allow(clippy::too_many_arguments)]
fn render_selection_bounding_box(
    ctx: &egui::Context,
    state: &crate::WorkbenchState,
    world: &mut world::World,
    camera: &Camera3d,
    viewport_rect: egui::Rect,
    viewport_size_px: common::Vec2,
    ppp: f32,
) {
    let Some(selected) = state.selected_entity else {
        return;
    };
    let mut nodes = world.ecs.query::<&physics::ParticleNode>();
    let Ok(selected_node) = nodes.get(&world.ecs, selected) else {
        return;
    };
    let organism_id = selected_node.organism_id;
    let mut min = common::Vec3::splat(f32::MAX);
    let mut max = common::Vec3::splat(f32::MIN);
    let mut any = false;
    for node in nodes.iter(&world.ecs) {
        if node.organism_id != organism_id {
            continue;
        }
        any = true;
        min = min.min(node.position);
        max = max.max(node.position);
    }
    if !any {
        return;
    }
    // A small margin so the box doesn't clip exactly through the
    // organism's own surface.
    const MARGIN: f32 = 8.0;
    min -= common::Vec3::splat(MARGIN);
    max += common::Vec3::splat(MARGIN);

    let corners = [
        common::Vec3::new(min.x, min.y, min.z),
        common::Vec3::new(max.x, min.y, min.z),
        common::Vec3::new(max.x, max.y, min.z),
        common::Vec3::new(min.x, max.y, min.z),
        common::Vec3::new(min.x, min.y, max.z),
        common::Vec3::new(max.x, min.y, max.z),
        common::Vec3::new(max.x, max.y, max.z),
        common::Vec3::new(min.x, max.y, max.z),
    ];
    let screen: Vec<Option<egui::Pos2>> = corners
        .iter()
        .map(|&c| project(camera, c, viewport_rect, viewport_size_px, ppp))
        .collect();

    const EDGES: [(usize, usize); 12] = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0), // bottom face
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4), // top face
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7), // verticals
    ];
    let painter = ctx.layer_painter(egui::LayerId::background());
    let t = crate::theme::TEXT_PRIMARY;
    let color = egui::Color32::from_rgba_unmultiplied(t.r(), t.g(), t.b(), 90);
    for (a, b) in EDGES {
        if let (Some(pa), Some(pb)) = (screen[a], screen[b]) {
            painter.line_segment([pa, pb], egui::Stroke::new(1.0, color));
        }
    }
}

/// Axis triad — a small, fixed-position (bottom-left corner) X/Y/Z
/// indicator showing the camera's current orientation relative to world
/// axes, the standard "gizmo in the corner" technique: project each world
/// axis through the camera's *orientation only* (ignoring position/depth),
/// so the triad always fits in a small fixed screen region regardless of
/// zoom or distance.
fn render_axis_triad(ctx: &egui::Context, camera: &Camera3d, viewport_rect: egui::Rect) {
    let center = viewport_rect.left_bottom() + egui::vec2(48.0, -48.0);
    let length = 28.0;
    let right = camera.right();
    let up = camera.up();

    let project_axis = |world_axis: common::Vec3| {
        // Screen-space direction of a world axis, ignoring depth — the
        // axis's components along the camera's own right/up vectors.
        // Screen Y grows downward, so the up-component is negated.
        let dx = world_axis.dot(right);
        let dy = -world_axis.dot(up);
        egui::vec2(dx, dy) * length
    };

    let axes = [
        (common::Vec3::X, egui::Color32::from_rgb(220, 70, 70), "X"),
        (common::Vec3::Y, egui::Color32::from_rgb(90, 200, 90), "Y"),
        (common::Vec3::Z, egui::Color32::from_rgb(80, 130, 230), "Z"),
    ];
    let painter = ctx.layer_painter(egui::LayerId::background());
    // Draw back-to-front (axis pointing away from the viewer first) so a
    // foreshortened axis doesn't visually overlap a nearer one incorrectly.
    let mut ordered = axes;
    ordered.sort_by(|(a, ..), (b, ..)| {
        a.dot(camera.forward())
            .partial_cmp(&b.dot(camera.forward()))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for (axis, color, label) in ordered {
        let tip = center + project_axis(axis);
        painter.line_segment([center, tip], egui::Stroke::new(2.0, color));
        painter.text(
            tip,
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::monospace(crate::theme::SIZE_SMALL),
            color,
        );
    }
}

/// Navigation gizmo — a compact, always-visible cluster of six preset-view
/// buttons (top-right corner), the closest-matching view highlighted
/// against the camera's current forward direction. A simplified stand-in
/// for a true rendered 3D view-cube (which would need real mesh geometry
/// and its own pick-ray hit-testing, out of scope for an egui overlay) —
/// functionally equivalent for the purpose Blender's own cube serves here
/// (quick preset switching + at-a-glance orientation feedback), disclosed
/// as a simplification rather than silently presented as the real thing.
/// Clicking a button only ever pushes the same `MenuAction::SetCameraPreset`
/// the keyboard shortcuts already use (per ADR-P9-02 — a command, not new
/// camera logic).
fn render_navigation_cube(
    ctx: &egui::Context,
    camera: &Camera3d,
    viewport_rect: egui::Rect,
    actions: &mut Vec<crate::types::MenuAction>,
) {
    let forward = camera.forward();
    let current = PRESET_DIRECTIONS
        .iter()
        .max_by(|(_, a), (_, b)| {
            a.dot(forward)
                .partial_cmp(&b.dot(forward))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(preset, _)| *preset);

    egui::Area::new(egui::Id::new("nav_cube"))
        .fixed_pos(viewport_rect.right_top() + egui::vec2(-172.0, 8.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .fill(gizmo_panel_fill(180))
                .show(ui, |ui| {
                    ui.set_width(160.0);
                    ui.horizontal(|ui| {
                        nav_cube_button(ui, crate::types::CameraPreset::Back, current, actions);
                        nav_cube_button(ui, crate::types::CameraPreset::Top, current, actions);
                        nav_cube_button(ui, crate::types::CameraPreset::Left, current, actions);
                    });
                    ui.horizontal(|ui| {
                        nav_cube_button(ui, crate::types::CameraPreset::Front, current, actions);
                        nav_cube_button(ui, crate::types::CameraPreset::Bottom, current, actions);
                        nav_cube_button(ui, crate::types::CameraPreset::Right, current, actions);
                    });
                });
        });
}

fn nav_cube_button(
    ui: &mut egui::Ui,
    preset: crate::types::CameraPreset,
    current: Option<crate::types::CameraPreset>,
    actions: &mut Vec<crate::types::MenuAction>,
) {
    let is_current = current == Some(preset);
    let button =
        egui::Button::new(egui::RichText::new(preset_label(preset)).size(crate::theme::SIZE_MICRO))
            .fill(if is_current {
                crate::theme::ACCENT_SOFT
            } else {
                egui::Color32::TRANSPARENT
            });
    if ui.add_sized([48.0, 20.0], button).clicked() {
        actions.push(crate::types::MenuAction::SetCameraPreset(preset));
    }
}

/// Scientific-context overlay (Phase 9, P9.5's "beyond Blender" addition) —
/// a compact text readout beneath the navigation cube showing state a
/// researcher benefits from seeing at a glance: coordinate convention,
/// active projection, active navigation mode, and clip-plane status.
/// Read-only — reports state, never changes it.
fn render_scene_info_overlay(
    ctx: &egui::Context,
    state: &crate::WorkbenchState,
    camera: &Camera3d,
    viewport_rect: egui::Rect,
) {
    let projection = if camera.ortho_half_height.is_some() {
        "Orthographic"
    } else {
        "Perspective"
    };
    let nav_mode = if state.camera_controller.is_fly() {
        "Fly"
    } else {
        "Orbit"
    };
    let clip = if state.clip_plane.enabled {
        format!("Clip: on (z={:.0})", state.clip_plane.height)
    } else {
        "Clip: off".to_string()
    };
    let scale = world_scale_label(state, camera);

    egui::Area::new(egui::Id::new("scene_info_overlay"))
        .fixed_pos(viewport_rect.right_top() + egui::vec2(-172.0, 68.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .fill(gizmo_panel_fill(150))
                .show(ui, |ui| {
                    ui.set_width(160.0);
                    ui.label(
                        egui::RichText::new(format!("Z-up · {projection} · {nav_mode}"))
                            .size(crate::theme::SIZE_MICRO)
                            .monospace(),
                    );
                    ui.label(
                        egui::RichText::new(clip)
                            .size(crate::theme::SIZE_MICRO)
                            .monospace(),
                    );
                    ui.label(
                        egui::RichText::new(scale)
                            .size(crate::theme::SIZE_MICRO)
                            .monospace(),
                    );
                });
        });
}

/// World-scale indicator — the world-space height spanned by the viewport
/// at the current focus depth, so a researcher can read off "how zoomed in"
/// the view is in simulation units rather than an abstract zoom percentage.
/// Orbit mode uses the real focus distance; Fly mode has no pivot, so a
/// fixed reference depth is used instead and labeled as such.
fn world_scale_label(state: &crate::WorkbenchState, camera: &Camera3d) -> String {
    let half_height = if let Some(ortho_half_height) = camera.ortho_half_height {
        ortho_half_height
    } else {
        let depth = match &state.camera_controller {
            crate::camera::CameraController::Orbit(orbit) => orbit.distance,
            crate::camera::CameraController::Fly(_) => 100.0,
        };
        depth * (camera.fov_y * 0.5).tan()
    };
    let label = if state.camera_controller.is_fly() && camera.ortho_half_height.is_none() {
        format!("Scale: ~{:.0}u @ 100u ref", half_height * 2.0)
    } else {
        format!("Scale: {:.0}u across", half_height * 2.0)
    };
    label
}
