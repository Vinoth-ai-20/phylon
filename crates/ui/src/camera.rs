//! The canonical 3D camera (Phase 8, ADR-P8-02) — replaces the pre-Phase-8
//! flat `camera_pos: Vec2` + `camera_zoom: f32` pair on `WorkbenchState`.
//!
//! [`Camera3d`] is the single object every renderer and every input
//! controller consumes; `view_proj()` and `screen_to_ray()` are its only two
//! projection-related methods (per the ADR — no renderer or controller ever
//! derives its own projection matrix again). [`OrbitController`] (the
//! default mode) and [`FlyController`] (opt-in) are the two ways user input
//! produces a `Camera3d` — see [`CameraController`].
//!
//! ## World/camera axis convention
//!
//! Organisms live in the world XY plane (Z fixed at `0.0` until Epic 8.6).
//! `Z` is therefore the camera's "altitude" axis: the default orbit
//! configuration sits above the origin at a fixed height, looking straight
//! down (`-Z`), with world `+Y` appearing "up" on screen — this exactly
//! reproduces the pre-Phase-8 flat top-down view (see
//! `WorkbenchState::camera_pos_2d`/`camera_zoom_2d`, the temporary bridge
//! that feeds the not-yet-migrated 2D renderers this same information).

use common::{Mat3, Mat4, Quat, Vec2, Vec3};

/// The single canonical 3D camera object (Phase 8, ADR-P8-02).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera3d {
    /// World-space eye position.
    pub position: Vec3,
    /// World-space orientation. Local `-Z` is "forward," local `+Y` is
    /// "up," matching `glam`'s usual right-handed convention.
    pub orientation: Quat,
    /// Vertical field of view, in radians.
    pub fov_y: f32,
    /// Near clip distance.
    pub near: f32,
    /// Far clip distance.
    pub far: f32,
}

impl Camera3d {
    /// Default vertical FOV — an untuned-but-reasonable ~50°, same status as
    /// every other not-yet-measured constant introduced this phase.
    pub const DEFAULT_FOV_Y: f32 = 50.0_f32.to_radians();
    /// Default near clip.
    pub const DEFAULT_NEAR: f32 = 1.0;
    /// Default far clip — comfortably beyond the simulation's world bounds
    /// (`app::render::WORLD_BOUNDS` is 1500.0) at any orbit distance this
    /// module allows.
    pub const DEFAULT_FAR: f32 = 20_000.0;

    /// World-space forward vector (local `-Z`, rotated by `orientation`).
    pub fn forward(&self) -> Vec3 {
        self.orientation * Vec3::NEG_Z
    }

    /// World-space up vector (local `+Y`, rotated by `orientation`).
    pub fn up(&self) -> Vec3 {
        self.orientation * Vec3::Y
    }

    /// World-space right vector (local `+X`, rotated by `orientation`).
    pub fn right(&self) -> Vec3 {
        self.orientation * Vec3::X
    }

    /// The combined view-projection matrix for the given viewport aspect
    /// ratio (`width / height`). The one and only place any renderer should
    /// obtain a projection matrix from (ADR-P8-02).
    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let view = Mat4::look_to_rh(self.position, self.forward(), self.up());
        let proj = Mat4::perspective_rh(self.fov_y, aspect.max(0.0001), self.near, self.far);
        proj * view
    }

    /// Produces a world-space ray `(origin, direction)` from a screen-space
    /// point — the one and only unproject pathway (ADR-P8-02), replacing
    /// the 3 independently hand-derived screen↔world transforms the Phase 8
    /// audit found (`app::pick_entity`, and two closures in
    /// `ui::plugins::viewport`).
    ///
    /// `screen_pos` and `viewport_size` are both in the same pixel units
    /// (physical or logical, as long as both agree).
    pub fn screen_to_ray(&self, screen_pos: Vec2, viewport_size: Vec2) -> (Vec3, Vec3) {
        let aspect = if viewport_size.y > 0.0 {
            viewport_size.x / viewport_size.y
        } else {
            1.0
        };
        // NDC in [-1, 1], with Y flipped since screen space grows downward.
        let ndc_x = (screen_pos.x / viewport_size.x.max(1.0)) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_pos.y / viewport_size.y.max(1.0)) * 2.0;

        let tan_half_fov = (self.fov_y * 0.5).tan();
        let view_dir = Vec3::new(ndc_x * tan_half_fov * aspect, ndc_y * tan_half_fov, -1.0);
        let world_dir = (self.orientation * view_dir).normalize();
        (self.position, world_dir)
    }
}

impl Default for Camera3d {
    fn default() -> Self {
        OrbitController::default().camera()
    }
}

/// Intersects a world-space ray with the `Z = 0` plane (where every
/// organism/food/mineral/corpse still lives until Epic 8.6) — the shared
/// primitive behind every "screen click/hover → world position" call site
/// this epic consolidates. Not a `Camera3d` method: the `Z = 0` plane is a
/// simulation-space convention the camera itself has no reason to know
/// about (mirrors `ADR-P8-05`'s "world-space fields stay a flat plane"
/// precedent).
pub fn ray_intersect_z0(origin: Vec3, direction: Vec3) -> Option<Vec2> {
    if direction.z.abs() < 1e-6 {
        return None;
    }
    let t = -origin.z / direction.z;
    if t <= 0.0 {
        return None;
    }
    Some((origin + direction * t).truncate())
}

/// Arcball orbit around a focus point — the default camera mode (ADR-P8-02),
/// matching the Blender-style scientific-tool convention this project's UX
/// already leans on elsewhere.
///
/// `pitch` is measured from nadir (`0.0` = looking straight down at
/// `focus`, matching the pre-Phase-8 default view exactly) rather than from
/// the horizon — the natural zero for a tool whose primary view has always
/// been top-down. `yaw` is the azimuth of the tilt direction; kept
/// well-defined at `pitch == 0.0` (where azimuth is otherwise meaningless)
/// by simply leaving it unchanged rather than resetting it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitController {
    /// World-space point the camera looks at and orbits around.
    pub focus: Vec3,
    /// Distance from `focus` to the camera's eye position.
    pub distance: f32,
    /// Azimuth of the tilt direction, in radians.
    pub yaw: f32,
    /// Tilt away from straight-down, in radians. `0.0` is nadir (today's
    /// default top-down view); clamped to `[0.0, MAX_PITCH]`.
    pub pitch: f32,
}

impl OrbitController {
    /// Distance at which the default view reproduces the pre-Phase-8
    /// `camera_zoom = 1.0` default (1 world unit == 1 pixel) at the
    /// application's default 720px-tall window (`data/default.ron`) —
    /// `360.0 / tan(Camera3d::DEFAULT_FOV_Y / 2.0)`, see
    /// `WorkbenchState::camera_zoom_2d`'s doc comment. Precomputed rather
    /// than derived at const-eval time since `f32::tan` isn't `const fn`.
    pub const DEFAULT_DISTANCE: f32 = 772.02;
    /// Closest the camera is allowed to approach its focus point.
    pub const MIN_DISTANCE: f32 = 20.0;
    /// Farthest the camera is allowed to sit from its focus point.
    pub const MAX_DISTANCE: f32 = 8_000.0;
    /// Maximum tilt away from nadir — stops just short of the horizon
    /// (`90°`) so the camera never crosses into or past looking sideways
    /// along the ground plane.
    pub const MAX_PITCH: f32 = 89.0_f32.to_radians();

    /// The world-space forward direction this orbit configuration looks
    /// along, independent of `focus`/`distance`.
    fn forward(&self) -> Vec3 {
        let (sin_p, cos_p) = self.pitch.sin_cos();
        let (sin_y, cos_y) = self.yaw.sin_cos();
        Vec3::new(-sin_p * sin_y, sin_p * cos_y, -cos_p)
    }

    /// Builds the `Camera3d` this orbit configuration currently describes.
    pub fn camera(&self) -> Camera3d {
        let forward = self.forward();
        let position = self.focus - forward * self.distance;
        let orientation = orientation_from_forward_and_reference_up(forward, Vec3::Y);
        Camera3d {
            position,
            orientation,
            fov_y: Camera3d::DEFAULT_FOV_Y,
            near: Camera3d::DEFAULT_NEAR,
            far: Camera3d::DEFAULT_FAR,
        }
    }

    /// Rotates the orbit by a screen-space drag delta (radians per pixel is
    /// baked into the caller's scale factor — this method just applies the
    /// resulting angle deltas and clamps pitch).
    pub fn orbit(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.yaw += delta_yaw;
        self.pitch = (self.pitch + delta_pitch).clamp(0.0, Self::MAX_PITCH);
    }

    /// Pans `focus` along the camera's own right/up-on-ground axes, scaled
    /// by `distance` so a given screen-space drag covers the same *visual*
    /// fraction of the viewport regardless of zoom level — the direct
    /// analog of the pre-Phase-8 `camera_pos -= drag_delta / camera_zoom`.
    pub fn pan(&mut self, screen_delta: Vec2, viewport_height: f32) {
        if viewport_height <= 0.0 {
            return;
        }
        // Same proportionality the old flat camera used: one world unit
        // spans `viewport_height / (2 * distance * tan(fov_y / 2))` pixels
        // at the focus plane — invert that to convert a pixel delta into a
        // world delta.
        let world_per_pixel =
            (2.0 * self.distance * (Camera3d::DEFAULT_FOV_Y * 0.5).tan()) / viewport_height;
        let camera = self.camera();
        let right = camera.right();
        let up = camera.up();
        self.focus -= right * screen_delta.x * world_per_pixel;
        self.focus += up * screen_delta.y * world_per_pixel;
    }

    /// Multiplies `distance` by `1 / factor` (a `factor > 1.0` zooms in,
    /// matching the pre-Phase-8 `camera_zoom *= factor` convention where
    /// bigger `camera_zoom` meant "closer"), clamped to
    /// `[MIN_DISTANCE, MAX_DISTANCE]`.
    pub fn zoom_by(&mut self, factor: f32) {
        if factor > 0.0 {
            self.distance = (self.distance / factor).clamp(Self::MIN_DISTANCE, Self::MAX_DISTANCE);
        }
    }

    /// Snaps `focus` to `target` without changing distance/yaw/pitch — a
    /// one-shot recenter (`MenuAction::FocusSelection`), distinct from
    /// continuous camera-follow tracking.
    pub fn focus_on(&mut self, target: Vec3) {
        self.focus = target;
    }

    /// Resets to the exact pre-Phase-8 default view: origin, default
    /// distance, looking straight down.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Builds an orbit configuration that looks at `focus` from `position`
    /// — used when switching from `FlyController` back to `OrbitController`
    /// so the transition is coherent rather than snapping to the default
    /// view.
    pub fn looking_at(focus: Vec3, position: Vec3) -> Self {
        let offset = position - focus;
        let distance = offset
            .length()
            .clamp(Self::MIN_DISTANCE, Self::MAX_DISTANCE);
        if distance < 1e-4 {
            return Self {
                focus,
                distance: Self::DEFAULT_DISTANCE,
                ..Self::default()
            };
        }
        let forward = (-offset / offset.length().max(1e-6)).normalize();
        // Invert `forward()`: forward = (-sin(p)sin(y), sin(p)cos(y), -cos(p)).
        let pitch = (-forward.z)
            .clamp(-1.0, 1.0)
            .acos()
            .clamp(0.0, Self::MAX_PITCH);
        let sin_p = pitch.sin();
        let yaw = if sin_p > 1e-4 {
            (-forward.x / sin_p).atan2(forward.y / sin_p)
        } else {
            0.0
        };
        Self {
            focus,
            distance,
            yaw,
            pitch,
        }
    }

    /// Builds a coherent orbit configuration from a `FlyController`'s
    /// current camera — see this module's transition-continuity note on
    /// [`OrbitController::looking_at`].
    pub fn from_fly(camera: &Camera3d) -> Self {
        let focus = camera.position + camera.forward() * Self::DEFAULT_DISTANCE;
        Self::looking_at(focus, camera.position)
    }
}

impl Default for OrbitController {
    fn default() -> Self {
        Self {
            focus: Vec3::ZERO,
            distance: Self::DEFAULT_DISTANCE,
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

/// Free WASD + mouselook camera — opt-in (ADR-P8-02); direct control over
/// position/orientation, no focus/distance concept.
///
/// `pitch` here uses the conventional FPS zero (`0.0` = horizontal,
/// matching a fresh fly-mode session starting by looking across the
/// horizon rather than straight down) — deliberately a different zero than
/// `OrbitController::pitch`'s nadir-relative convention, since the two
/// controllers have unrelated "natural" defaults and neither reads the
/// other's angle fields directly (mode switches go through `Camera3d`
/// position/orientation, not shared angle state).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlyController {
    /// World-space eye position.
    pub position: Vec3,
    /// Horizontal look angle, in radians.
    pub yaw: f32,
    /// Vertical look angle, in radians (`0.0` = horizontal).
    pub pitch: f32,
}

impl FlyController {
    /// Maximum pitch magnitude — stops just short of straight up/down to
    /// avoid the forward/up basis degenerating.
    pub const MAX_PITCH: f32 = 89.0_f32.to_radians();
    /// World units moved per second at the base fly speed.
    pub const BASE_SPEED: f32 = 400.0;

    fn forward(&self) -> Vec3 {
        let (sin_y, cos_y) = self.yaw.sin_cos();
        let (sin_p, cos_p) = self.pitch.sin_cos();
        Vec3::new(cos_p * cos_y, cos_p * sin_y, sin_p)
    }

    /// Builds the `Camera3d` this fly configuration currently describes.
    pub fn camera(&self) -> Camera3d {
        let forward = self.forward();
        let orientation = orientation_from_forward_and_reference_up(forward, Vec3::Z);
        Camera3d {
            position: self.position,
            orientation,
            fov_y: Camera3d::DEFAULT_FOV_Y,
            near: Camera3d::DEFAULT_NEAR,
            far: Camera3d::DEFAULT_FAR,
        }
    }

    /// Applies a mouselook delta (radians).
    pub fn look(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.yaw += delta_yaw;
        self.pitch = (self.pitch + delta_pitch).clamp(-Self::MAX_PITCH, Self::MAX_PITCH);
    }

    /// Rotates in place to look at `target` — the `Fly`-mode half of
    /// `MenuAction::FocusSelection` (a one-shot snap, matching
    /// `OrbitController::focus_on`'s "look at this, don't teleport there"
    /// semantics, just for a controller with no separate focus point to
    /// move).
    pub fn look_at(&mut self, target: Vec3) {
        let to_target = target - self.position;
        if to_target.length_squared() < 1e-6 {
            return;
        }
        let forward = to_target.normalize();
        self.pitch = forward
            .z
            .clamp(-1.0, 1.0)
            .asin()
            .clamp(-Self::MAX_PITCH, Self::MAX_PITCH);
        self.yaw = forward.y.atan2(forward.x);
    }

    /// Moves the camera relative to its own orientation: `forward`/`right`/
    /// `up` are each in `[-1.0, 1.0]` (typically from WASD/space/ctrl held
    /// state), `dt` is the frame's elapsed seconds, `speed_multiplier`
    /// scales `BASE_SPEED` (e.g. a "sprint" modifier).
    pub fn move_relative(
        &mut self,
        forward: f32,
        right: f32,
        up: f32,
        dt: f32,
        speed_multiplier: f32,
    ) {
        let camera = self.camera();
        let speed = Self::BASE_SPEED * speed_multiplier * dt;
        self.position += camera.forward() * forward * speed;
        self.position += camera.right() * right * speed;
        self.position += Vec3::Z * up * speed;
    }

    /// Builds a fly configuration starting from wherever `camera` currently
    /// is/looks — used when switching from `OrbitController` to
    /// `FlyController` so the transition is coherent.
    pub fn from_camera(camera: &Camera3d) -> Self {
        let forward = camera.forward();
        let pitch = forward.z.clamp(-1.0, 1.0).asin();
        let yaw = forward.y.atan2(forward.x);
        Self {
            position: camera.position,
            yaw,
            pitch: pitch.clamp(-Self::MAX_PITCH, Self::MAX_PITCH),
        }
    }
}

/// Builds a right-handed orientation quaternion from an explicit forward
/// vector and a reference "up" hint, via a basis-vectors-to-quaternion
/// construction — avoids the Euler-angle gimbal ambiguity a from/to-angle
/// composition would hit exactly at the two controllers' own zero
/// configurations (nadir for orbit, horizontal for fly).
///
/// Falls back to `Vec3::X` as the reference if `forward` is (near-)parallel
/// to `reference_up`, so this never produces a degenerate/NaN basis.
fn orientation_from_forward_and_reference_up(forward: Vec3, reference_up: Vec3) -> Quat {
    let forward = forward.normalize();
    let reference_up =
        if forward.abs_diff_eq(reference_up, 1e-3) || forward.abs_diff_eq(-reference_up, 1e-3) {
            Vec3::X
        } else {
            reference_up
        };
    let right = forward.cross(reference_up).normalize();
    let up = right.cross(forward).normalize();
    // Columns: local +X -> right, local +Y -> up, local +Z -> backward
    // (since local -Z is "forward" by convention).
    Quat::from_mat3(&Mat3::from_cols(right, up, -forward))
}

/// Which camera mode is currently active (ADR-P8-02) — `Orbit` is the
/// default; `Fly` is opt-in via `MenuAction::ToggleCameraMode`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CameraController {
    /// Arcball orbit around a focus point (default mode).
    Orbit(OrbitController),
    /// Free WASD + mouselook (opt-in mode).
    Fly(FlyController),
}

impl CameraController {
    /// Builds the `Camera3d` the active controller currently describes.
    pub fn camera(&self) -> Camera3d {
        match self {
            Self::Orbit(orbit) => orbit.camera(),
            Self::Fly(fly) => fly.camera(),
        }
    }

    /// True while `Fly` mode is active.
    pub fn is_fly(&self) -> bool {
        matches!(self, Self::Fly(_))
    }

    /// Switches to the other mode, seeding it from the current camera state
    /// so the transition is visually continuous (ADR-P8-02's "camera
    /// transitions" requirement) rather than snapping to a default.
    pub fn toggle(&mut self) {
        *self = match self {
            Self::Orbit(_) => Self::Fly(FlyController::from_camera(&self.camera())),
            Self::Fly(_) => Self::Orbit(OrbitController::from_fly(&self.camera())),
        };
    }
}

impl Default for CameraController {
    fn default() -> Self {
        Self::Orbit(OrbitController::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_orbit_looks_straight_down_at_the_origin() {
        let camera = OrbitController::default().camera();
        assert!(camera.position.truncate().abs_diff_eq(Vec2::ZERO, 1e-4));
        assert!(camera.position.z > 0.0);
        assert!(camera.forward().abs_diff_eq(Vec3::NEG_Z, 1e-4));
        assert!(camera.up().abs_diff_eq(Vec3::Y, 1e-4));
    }

    #[test]
    fn orbit_distance_matches_camera_height_when_looking_straight_down() {
        let orbit = OrbitController {
            distance: 500.0,
            ..OrbitController::default()
        };
        let camera = orbit.camera();
        assert!((camera.position.z - 500.0).abs() < 1e-3);
    }

    #[test]
    fn default_distance_reproduces_a_one_to_one_pixel_to_world_ratio_at_720p() {
        // The formula this constant is derived from (see its own doc
        // comment): half_h_world = distance * tan(fov_y / 2); zoom = 1.0
        // means (viewport_height / 2) == half_h_world.
        let half_h_world =
            OrbitController::DEFAULT_DISTANCE * (Camera3d::DEFAULT_FOV_Y * 0.5).tan();
        assert!((half_h_world - 360.0).abs() < 0.5);
    }

    #[test]
    fn orbit_zoom_by_increases_distance_for_a_sub_one_factor() {
        let mut orbit = OrbitController::default();
        let start = orbit.distance;
        orbit.zoom_by(1.0 / 1.1);
        assert!(orbit.distance > start);
        orbit.zoom_by(1.1 * 1.1); // net > 1 relative to the last step
        assert!(orbit.distance < start);
    }

    #[test]
    fn orbit_pitch_clamps_to_max_and_never_goes_negative() {
        let mut orbit = OrbitController::default();
        orbit.orbit(0.0, -10.0);
        assert_eq!(orbit.pitch, 0.0);
        orbit.orbit(0.0, 10.0);
        assert_eq!(orbit.pitch, OrbitController::MAX_PITCH);
    }

    #[test]
    fn orbit_pan_moves_focus_in_the_ground_plane_when_looking_straight_down() {
        let mut orbit = OrbitController::default();
        orbit.pan(Vec2::new(100.0, 0.0), 720.0);
        assert!(orbit.focus.x.abs() > 0.0);
        assert!(
            orbit.focus.z.abs() < 1e-3,
            "pan must not move focus off the Z=0 plane"
        );
    }

    #[test]
    fn screen_to_ray_at_viewport_center_points_straight_down_for_the_default_orbit() {
        let camera = OrbitController::default().camera();
        let viewport = Vec2::new(1280.0, 720.0);
        let (origin, dir) = camera.screen_to_ray(viewport * 0.5, viewport);
        assert!(origin.abs_diff_eq(camera.position, 1e-4));
        assert!(dir.abs_diff_eq(Vec3::NEG_Z, 1e-3));
    }

    #[test]
    fn ray_intersect_z0_finds_the_origin_for_a_straight_down_ray_from_above() {
        let hit = ray_intersect_z0(Vec3::new(5.0, -3.0, 100.0), Vec3::NEG_Z);
        assert_eq!(hit, Some(Vec2::new(5.0, -3.0)));
    }

    #[test]
    fn ray_intersect_z0_returns_none_for_a_ray_parallel_to_the_plane() {
        assert_eq!(ray_intersect_z0(Vec3::new(0.0, 0.0, 10.0), Vec3::X), None);
    }

    #[test]
    fn fly_controller_default_forward_is_horizontal() {
        let fly = FlyController {
            position: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
        };
        let camera = fly.camera();
        assert!(camera.forward().z.abs() < 1e-4);
    }

    #[test]
    fn toggling_mode_twice_returns_to_a_coherent_orbit_view() {
        let mut controller = CameraController::default();
        let before = controller.camera();
        controller.toggle();
        assert!(controller.is_fly());
        controller.toggle();
        assert!(!controller.is_fly());
        let after = controller.camera();
        // Round-tripping through Fly is lossy: both controllers clamp pitch
        // a degree short of the true pole (`MAX_PITCH = 89°`) to keep their
        // forward/up basis non-degenerate, so a straight-down orbit view
        // can only round-trip to within about that same margin, not
        // exactly — the resulting orbit must still look in approximately
        // the same direction, not identically.
        assert!(before.forward().abs_diff_eq(after.forward(), 0.05));
    }
}
