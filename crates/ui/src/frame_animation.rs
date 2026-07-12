//! Smooth "Frame Selected" / "Frame All" camera transitions.
//!
//! ## Purpose
//!
//! Snapping the camera instantly to a new focus/distance when the user
//! triggers "Frame Selected" or "Frame All" is visually jarring; this
//! module provides a short, eased interpolation from wherever the camera
//! currently is to a computed target, driven once per rendered frame.
//!
//! ## Architecture
//!
//! Deliberately **not** part of `camera.rs`: `FrameAnimation` only ever
//! *reads* `OrbitController`'s existing public `focus`/`distance` fields
//! from the outside and writes new values into them each frame — it adds
//! no method, no field, and no orientation/orbit math to `Camera3d`/
//! `OrbitController`/`FlyController` themselves. `WorkbenchState` owns the
//! `Option<FrameAnimation>` (see `state.rs`'s `frame_animation` field) and
//! drives it via `start_frame_animation`/`tick_frame_animation`, writing
//! the interpolated values into the active `OrbitController` each frame.

use common::Vec3;

/// One in-progress "frame the pivot/distance" transition. Only meaningful
/// in `Orbit` mode (mirrors `WorkbenchState::zoom_by`'s own "no zoom
/// concept in Fly mode" precedent) — `yaw`/`pitch` are deliberately left
/// alone throughout, so a Frame Selected/Frame All action re-centers and
/// re-distances the view without spinning it.
#[derive(Debug, Clone, Copy)]
pub struct FrameAnimation {
    start_focus: Vec3,
    target_focus: Vec3,
    start_distance: f32,
    target_distance: f32,
    elapsed: f32,
    /// Total transition duration, seconds — 250ms, the midpoint of the
    /// requested 200-300ms range.
    duration: f32,
}

impl FrameAnimation {
    const DURATION_SECS: f32 = 0.25;

    /// Starts a new transition from the orbit's current focus/distance to
    /// the given target.
    pub fn new(
        start_focus: Vec3,
        start_distance: f32,
        target_focus: Vec3,
        target_distance: f32,
    ) -> Self {
        Self {
            start_focus,
            target_focus,
            start_distance,
            target_distance,
            elapsed: 0.0,
            duration: Self::DURATION_SECS,
        }
    }

    /// Cubic ease-in/ease-out (smoothstep) — starts and ends at zero
    /// velocity, matching the requested "ease-in/ease-out" feel.
    fn ease(t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// Advances the transition by `dt` seconds. Returns the current
    /// `(focus, distance)` this frame should apply, and whether the
    /// transition has finished (in which case the caller should drop this
    /// `FrameAnimation` — its final frame has already been applied).
    pub fn advance(&mut self, dt: f32) -> (Vec3, f32, bool) {
        self.elapsed += dt;
        let t = Self::ease(self.elapsed / self.duration);
        let focus = self.start_focus.lerp(self.target_focus, t);
        let distance = self.start_distance + (self.target_distance - self.start_distance) * t;
        (focus, distance, self.elapsed >= self.duration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advancing_by_the_full_duration_reaches_the_target_exactly() {
        let mut anim = FrameAnimation::new(Vec3::ZERO, 100.0, Vec3::new(10.0, 0.0, 0.0), 50.0);
        let (focus, distance, finished) = anim.advance(FrameAnimation::DURATION_SECS);
        assert!(finished);
        assert!(focus.abs_diff_eq(Vec3::new(10.0, 0.0, 0.0), 1e-4));
        assert!((distance - 50.0).abs() < 1e-4);
    }

    #[test]
    fn advancing_partway_does_not_finish_and_stays_between_start_and_target() {
        let mut anim = FrameAnimation::new(Vec3::ZERO, 100.0, Vec3::new(10.0, 0.0, 0.0), 50.0);
        let (focus, distance, finished) = anim.advance(FrameAnimation::DURATION_SECS * 0.5);
        assert!(!finished);
        assert!(focus.x > 0.0 && focus.x < 10.0);
        assert!(distance > 50.0 && distance < 100.0);
    }

    #[test]
    fn ease_is_monotonic_and_bounded() {
        let mut last = 0.0;
        for i in 0..=10 {
            let t = FrameAnimation::ease(i as f32 / 10.0);
            assert!((0.0..=1.0).contains(&t));
            assert!(t >= last);
            last = t;
        }
    }
}
