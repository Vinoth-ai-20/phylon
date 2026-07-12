//! The single canonical viewport-interaction layer.
//!
//! ## Purpose
//!
//! Camera-affecting input can arrive through more than one platform path:
//! an egui-routed path (`ui::plugins::viewport` gathering drag/scroll
//! gestures into `CanvasInteraction`) and a winit-routed path (`app::events`
//! reading raw `WindowEvent::KeyboardInput` for WASD/arrow keys). Without a
//! shared intermediate representation, each path would need its own logic
//! for interpreting gestures and mutating the camera, and a new input
//! source (3D mouse, VR controller, touch, a synthetic/replay-driven input
//! stream for automated testing) would have nowhere to plug in without
//! duplicating that logic again.
//!
//! ## Architecture
//!
//! `ViewportInput` is a plain, platform-agnostic struct describing "what
//! interaction happened this frame," entirely ignorant of egui/winit. Each
//! input source is an *adapter* that fills in a `ViewportInput` — see
//! `ViewportInput::from_canvas_interaction` for the egui adapter;
//! `app::events`'s keyboard handler is the winit adapter, building one
//! inline since it has no equivalent shared gesture type to adapt from.
//! `apply_to_camera` is the single point where any of this input actually
//! mutates a camera, so a new adapter needs only to produce a
//! `ViewportInput`, never touch `CameraController`/`OrbitController`/
//! `FlyController` directly.
//!
//! ## Data flow
//!
//! Each frame: an input source gathers raw platform events -> an adapter
//! converts them into one `ViewportInput` -> `apply_to_camera` reads that
//! struct once and applies the appropriate deltas to whichever
//! `CameraController` variant (`Orbit` or `Fly`) is currently active,
//! consulting `WorkbenchState` only for the pieces it needs (viewport
//! height for pan scaling, the active controller, camera-follow state).
//!
//! ## Design decisions
//!
//! Discrete, one-shot commands (Home/reset, zoom-in/out from the menu,
//! frame-selected, toggle camera mode) are deliberately **not** folded into
//! `ViewportInput` — those already have exactly one dispatch path each
//! (`ui::types::MenuAction`), so there is no duplicated-gesture problem for
//! them to solve. `ViewportInput` exists only for *continuous, per-frame*
//! interaction (orbit/pan/zoom/fly).
//!
//! ## Related modules
//!
//! - `camera.rs` — the `CameraController`/`OrbitController`/`FlyController`
//!   types this module drives.
//! - `plugins/viewport.rs` — gathers `CanvasInteraction` from egui pointer
//!   state, the input this module's egui adapter consumes.

use crate::types::CanvasInteraction;

/// One frame's worth of canonical viewport interaction, produced by an
/// adapter (egui or winit today; a future 3D-mouse/VR/touch/replay source
/// tomorrow) and consumed exactly once by `apply_to_camera`.
#[derive(Default, Clone, Copy, Debug)]
pub struct ViewportInput {
    /// Left-drag pan delta, screen pixels, not yet scaled by the window's
    /// backing-buffer scale factor — Orbit mode only.
    pub pan_delta: common::Vec2,
    /// Middle-drag rotate delta, screen pixels, not yet scale-adjusted —
    /// orbits in Orbit mode, looks around in Fly mode.
    pub rotate_delta: common::Vec2,
    /// Multiplicative zoom factor this frame (`1.0` = no change) — mouse
    /// wheel / trackpad pinch. Discrete keyboard zoom (`+`/`-`) is a
    /// `MenuAction`, not this field — see this module's doc comment.
    pub zoom_delta: f32,
    /// WASD/arrow-key discrete pan step this frame, in world units applied
    /// directly to the orbit focus — Orbit mode only.
    pub key_pan_step: common::Vec2,
    /// WASD/arrow-key fly-move axes this frame, each in `[-1.0, 1.0]`:
    /// `(forward, right)` — Fly mode only.
    pub key_fly_move: (f32, f32),
    /// Whether a genuine drag/pan happened this frame (as opposed to a
    /// sub-pixel trackpad micro-movement) — detaches camera-follow, so
    /// that trackpad noise doesn't silently cancel an active follow.
    pub detach_follow: bool,
}

impl ViewportInput {
    /// The egui adapter: translates `ui::plugins::viewport`'s
    /// `CanvasInteraction` (mouse drag/scroll gathered from the egui
    /// pointer this frame) into a canonical `ViewportInput`.
    pub fn from_canvas_interaction(interaction: &CanvasInteraction) -> Self {
        let pan_delta = common::Vec2::new(interaction.drag_delta.x, interaction.drag_delta.y);
        Self {
            pan_delta,
            rotate_delta: common::Vec2::new(interaction.rotate_delta.x, interaction.rotate_delta.y),
            zoom_delta: interaction.zoom_delta,
            key_pan_step: common::Vec2::ZERO,
            key_fly_move: (0.0, 0.0),
            // A 3-pixel-squared threshold: sub-pixel trackpad noise
            // shouldn't be enough to detach an active camera-follow.
            detach_follow: pan_delta.length_squared() > 9.0,
        }
    }
}

/// The single point where any adapted input actually mutates the camera.
/// Framework-agnostic: takes nothing egui- or winit-specific, only the
/// already-adapted input plus the workspace state/scale it needs to apply
/// it.
///
/// `scale` is the window's backing-buffer scale factor (physical / logical
/// pixels), applied to screen-space deltas before they reach
/// `OrbitController::pan` so panning covers the same physical screen
/// distance regardless of display scaling.
pub fn apply_to_camera(state: &mut crate::WorkbenchState, input: &ViewportInput, scale: f32) {
    if input.zoom_delta != 1.0 && input.zoom_delta > 0.0 {
        state.zoom_by(input.zoom_delta);
    }

    if input.pan_delta.length_squared() > 0.0 {
        if let crate::camera::CameraController::Orbit(orbit) = &mut state.camera_controller {
            let viewport_h = state
                .canvas_rect
                .map(|[_, _, _, h]| h as f32)
                .unwrap_or(720.0);
            orbit.pan(input.pan_delta * scale, viewport_h);
        }
    }

    if input.key_pan_step.length_squared() > 0.0 {
        if let crate::camera::CameraController::Orbit(orbit) = &mut state.camera_controller {
            orbit.focus.x += input.key_pan_step.x;
            orbit.focus.y += input.key_pan_step.y;
        }
    }

    if input.rotate_delta.length_squared() > 0.0 {
        // Untuned-but-reasonable radians-per-pixel mouselook sensitivity.
        const ROTATE_SENSITIVITY: f32 = 0.005;
        let dx = input.rotate_delta.x * ROTATE_SENSITIVITY;
        let dy = input.rotate_delta.y * ROTATE_SENSITIVITY;
        match &mut state.camera_controller {
            crate::camera::CameraController::Orbit(orbit) => orbit.orbit(-dx, dy),
            crate::camera::CameraController::Fly(fly) => fly.look(-dx, -dy),
        }
    }

    if input.key_fly_move.0 != 0.0 || input.key_fly_move.1 != 0.0 {
        if let crate::camera::CameraController::Fly(fly) = &mut state.camera_controller {
            // One key-repeat event = one fixed step, relying on the OS's
            // own key-repeat rate for continuous movement while held.
            const FLY_STEP_DT: f32 = 1.0 / 60.0;
            fly.move_relative(
                input.key_fly_move.0,
                input.key_fly_move.1,
                0.0,
                FLY_STEP_DT,
                1.0,
            );
        }
    }

    if input.detach_follow {
        state.set_follow(None);
    }
}
