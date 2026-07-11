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
//! `Z` is the world's fixed up axis (Phase 9, P9.3 — see
//! `OrbitController::orientation`'s doc comment for the full reasoning).
//! The default orbit configuration sits above the origin at a fixed
//! height, looking straight down (`-Z`) — this reproduces the original
//! flat top-down view (see `WorkbenchState::camera_pos_2d`/
//! `camera_zoom_2d`, the temporary bridge that feeds the not-yet-migrated
//! 2D renderers this same information), with world `+Y` reading as
//! screen-up specifically *at that default view* — not because `Y` is the
//! up axis, but because `Y` is what a Z-up camera's screen-up naturally
//! reduces to when looking straight down the up axis itself. Tilt the
//! camera toward the horizon and screen-up smoothly rotates toward world
//! `+Z`, which *is* the up axis everywhere else on the sphere.
//!
//! `OrbitController` orbits freely over the full sphere (no pitch clamp);
//! `FlyController` still clamps pitch a few degrees short of straight
//! up/down to keep its own forward/right/up basis non-degenerate — the two
//! controllers are independent, and only `OrbitController`'s clamp was in
//! scope for the P9.3 fix (Fly mode's own feel was explicitly left
//! unchanged).

use common::{Mat3, Mat4, Quat, Vec2, Vec3};

/// The single canonical 3D camera object (Phase 8, ADR-P8-02).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera3d {
    /// World-space eye position.
    pub position: Vec3,
    /// World-space orientation. Local `-Z` is "forward," local `+Y` is
    /// "up," matching `glam`'s usual right-handed convention.
    pub orientation: Quat,
    /// Vertical field of view, in radians. Only meaningful in perspective
    /// mode (`ortho_half_height.is_none()`) — still populated either way
    /// so toggling projection mode doesn't need to reconstruct the whole
    /// `Camera3d`.
    pub fov_y: f32,
    /// Near clip distance.
    pub near: f32,
    /// Far clip distance.
    pub far: f32,
    /// Phase 9, P9.4: `None` (the default, and the only value before this
    /// milestone) means perspective projection. `Some(half_height)` means
    /// orthographic, with `half_height` the world-space half-extent
    /// visible at the vertical center of the viewport — deliberately a
    /// *projection-mode* field, not an orbit/orientation one; nothing
    /// about `yaw`/`pitch`/`orientation` construction changes based on it.
    pub ortho_half_height: Option<f32>,
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

    /// The projection matrix alone (no view) for the given aspect ratio —
    /// perspective or orthographic depending on `ortho_half_height`. Shared
    /// by [`Self::view_proj`] and [`Self::world_to_screen`] so the two
    /// never drift out of sync on which projection mode is active.
    fn projection_matrix(&self, aspect: f32) -> Mat4 {
        match self.ortho_half_height {
            None => Mat4::perspective_rh(self.fov_y, aspect.max(0.0001), self.near, self.far),
            Some(half_height) => {
                let half_width = half_height * aspect.max(0.0001);
                Mat4::orthographic_rh(
                    -half_width,
                    half_width,
                    -half_height,
                    half_height,
                    self.near,
                    self.far,
                )
            }
        }
    }

    /// The combined view-projection matrix for the given viewport aspect
    /// ratio (`width / height`). The one and only place any renderer should
    /// obtain a projection matrix from (ADR-P8-02).
    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let view = Mat4::look_to_rh(self.position, self.forward(), self.up());
        self.projection_matrix(aspect) * view
    }

    /// Produces a world-space ray `(origin, direction)` from a screen-space
    /// point — the one and only unproject pathway (ADR-P8-02), replacing
    /// the 3 independently hand-derived screen↔world transforms the Phase 8
    /// audit found (`app::pick_entity`, and two closures in
    /// `ui::plugins::viewport`). In orthographic mode (Phase 9, P9.4) every
    /// ray shares the same `forward()` direction and only the *origin*
    /// varies across the viewport, matching how parallel projection
    /// actually works — perspective's diverging-rays-from-one-point model
    /// doesn't apply.
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

        if let Some(half_height) = self.ortho_half_height {
            let half_width = half_height * aspect;
            let origin = self.position
                + self.right() * (ndc_x * half_width)
                + self.up() * (ndc_y * half_height);
            return (origin, self.forward());
        }

        let tan_half_fov = (self.fov_y * 0.5).tan();
        let view_dir = Vec3::new(ndc_x * tan_half_fov * aspect, ndc_y * tan_half_fov, -1.0);
        let world_dir = (self.orientation * view_dir).normalize();
        (self.position, world_dir)
    }

    /// Projects a world-space point to a screen-space pixel coordinate —
    /// the inverse direction of [`Camera3d::screen_to_ray`], and (Phase 8,
    /// Epic 8.4) the second of this type's two projection-related methods,
    /// used by frustum-based box-select and lasso-select to test entity
    /// positions against a screen-space region. Returns `None` if
    /// `world_pos` is behind the camera (not meaningfully projectable to a
    /// screen pixel).
    pub fn world_to_screen(&self, world_pos: Vec3, viewport_size: Vec2) -> Option<Vec2> {
        let aspect = if viewport_size.y > 0.0 {
            viewport_size.x / viewport_size.y
        } else {
            1.0
        };
        let view = Mat4::look_to_rh(self.position, self.forward(), self.up());
        let view_pos = view * world_pos.extend(1.0);
        // Behind-camera test via view-space Z (RH view space: the camera
        // looks down -Z, so a visible point has negative view-space Z) —
        // checked directly rather than via clip-space `w`, since an
        // orthographic projection's `w` is always `1.0` and would never
        // catch this the way perspective's `w == -view_z` does.
        if view_pos.z >= 0.0 {
            return None;
        }
        let clip = self.projection_matrix(aspect) * view_pos;
        // `clip.w` is always `1.0` for the orthographic branch (no
        // perspective divide), so this is a no-op there and a correct
        // divide-by-`w` for the perspective branch — one formula, both
        // modes, no branch needed here.
        let ndc = clip.truncate() / clip.w;
        // Inverse of `screen_to_ray`'s NDC formula, including the same Y
        // flip (screen space grows downward).
        Some(Vec2::new(
            (ndc.x * 0.5 + 0.5) * viewport_size.x,
            (1.0 - (ndc.y * 0.5 + 0.5)) * viewport_size.y,
        ))
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

/// Standard even-odd (crossing-number) point-in-polygon test, in
/// screen-space pixels (Phase 8, Epic 8.4 — lasso-select). `polygon` need
/// not be explicitly closed; the last point is implicitly connected back to
/// the first. Fewer than 3 points never contains anything.
pub fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let vi = polygon[i];
        let vj = polygon[j];
        if (vi.y > point.y) != (vj.y > point.y) {
            let x_at_y = vj.x + (point.y - vj.y) / (vi.y - vj.y) * (vi.x - vj.x);
            if point.x < x_at_y {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// Arcball/turntable orbit around a focus point — the default camera mode
/// (ADR-P8-02), matching the Blender-style scientific-tool convention this
/// project's UX already leans on elsewhere.
///
/// Phase 9, P9.3: `pitch` is an **unbounded** polar angle (radians, `0.0` =
/// looking straight down at `focus`, matching the original top-down
/// default) — the previous `[0.0, 89°]` clamp stopped the camera just short
/// of the horizon and could never orbit "up and over" the pivot, which is
/// the exact "camera feels locked" complaint this milestone fixes. `yaw` is
/// the azimuth of the tilt direction; kept well-defined at `pitch == 0.0`
/// (where azimuth is otherwise meaningless) by simply leaving it unchanged
/// rather than resetting it. `forward()`'s spherical parameterization is
/// continuous and periodic in `pitch` by construction (`sin`/`cos` are
/// periodic), so letting `pitch` grow or shrink without bound produces a
/// smooth, unbroken orbit over the full sphere — no wraparound logic is
/// needed, and there is no artificial stopping point.
///
/// **World `Z` is the fixed reference-up for this orbit** (previously
/// `Vec3::Y` — an inconsistency with `FlyController`, which always used
/// `Z`, left over from when the simulation was 2D-in-the-XY-plane and `Z`
/// was merely a camera "altitude," not a true up axis). With `Z` as the
/// reference, screen-up stays anchored to world-up everywhere except at
/// the exact poles (`pitch` an exact multiple of π, where `forward` is
/// parallel to `Z`) — there,
/// `orientation_from_forward_and_reference_up`'s existing fallback to
/// `Vec3::X` keeps the basis non-degenerate, at the cost of a one-instant
/// roll-reference discontinuity exactly at that single point. This is the
/// same, well-known, accepted behavior every Z-up turntable camera has at
/// true zenith/nadir (Blender's turntable orbit included) — it is not the
/// pathological "gimbal lock" (loss of a whole degree of freedom) the
/// previous hard pitch clamp was a workaround for, and does not recur
/// anywhere else on the sphere.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitController {
    /// World-space point the camera looks at and orbits around — always an
    /// explicit pivot (never the camera's own position); see
    /// `focus_on`/`looking_at` for how it's set.
    pub focus: Vec3,
    /// Distance from `focus` to the camera's eye position.
    pub distance: f32,
    /// Azimuth of the tilt direction, in radians.
    pub yaw: f32,
    /// Polar tilt from nadir, in radians. `0.0` is straight down at
    /// `focus` (today's default top-down view). Unbounded — see this
    /// struct's own doc comment for why no clamp is applied.
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

    /// The full orientation this orbit configuration describes, as a
    /// genuine quaternion composition — **not** a from-forward-vector
    /// reconstruction (which is what the pre-P9.3 code, and this method's
    /// own first draft, used, via `orientation_from_forward_and_reference_up`
    /// with a fixed reference-up vector). That approach has an inherent
    /// blind spot: whichever single vector is chosen as "reference up" is
    /// itself degenerate (parallel to `forward`) at *some* orientation, and
    /// there is no one choice that is simultaneously non-degenerate at
    /// nadir (where the pre-existing top-down default needs `Y` to read as
    /// screen-up) and correct at the horizon (where a genuinely Z-up world
    /// needs `Z` to read as screen-up).
    ///
    /// Composing two rotations instead has no such blind spot anywhere on
    /// the sphere: `yaw` rotates around world `Z` (spinning the view around
    /// the vertical axis), then `pitch` rotates around the *local* `X`
    /// (tilting forward away from nadir). At `pitch == 0.0` this reduces to
    /// pure yaw-around-`Z`, which leaves local `-Z` (forward) and `+Y` (up)
    /// exactly where the pre-existing top-down default expects them — this
    /// is a genuine mathematical property of the composition, not a
    /// special-cased branch. At `pitch == π/2` (horizon), the composition
    /// puts world `+Z` exactly at screen-up, satisfying "the world stays
    /// Z-up." Every angle in between interpolates continuously and is
    /// numerically well-defined; `Quat` multiplication has no singularity
    /// to fall back from.
    fn orientation(&self) -> Quat {
        Quat::from_axis_angle(Vec3::Z, self.yaw) * Quat::from_axis_angle(Vec3::X, self.pitch)
    }

    /// The world-space forward direction this orbit configuration looks
    /// along, independent of `focus`/`distance`. Valid (and continuous) for
    /// any `pitch`, not just `[0, π]` — see [`Self::orientation`]'s doc
    /// comment.
    fn forward(&self) -> Vec3 {
        self.orientation() * Vec3::NEG_Z
    }

    /// Builds the `Camera3d` this orbit configuration currently describes.
    pub fn camera(&self) -> Camera3d {
        let orientation = self.orientation();
        let position = self.focus - self.forward() * self.distance;
        Camera3d {
            position,
            orientation,
            fov_y: Camera3d::DEFAULT_FOV_Y,
            near: Camera3d::DEFAULT_NEAR,
            far: Camera3d::DEFAULT_FAR,
            ortho_half_height: None,
        }
    }

    /// Rotates the orbit by a screen-space drag delta (radians per pixel is
    /// baked into the caller's scale factor — this method just applies the
    /// resulting angle deltas). Phase 9, P9.3: `pitch` is no longer
    /// clamped — see this struct's own doc comment.
    pub fn orbit(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.yaw += delta_yaw;
        self.pitch += delta_pitch;
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
        // `acos` already returns `[0, π]`, exactly `forward()`'s valid
        // domain — no further clamp needed (P9.3: `pitch` is unbounded, but
        // a freshly-derived value from a concrete `forward` vector is
        // naturally within one period).
        let pitch = (-forward.z).clamp(-1.0, 1.0).acos();
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
            ortho_half_height: None,
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
    fn orthographic_screen_to_ray_produces_parallel_rays() {
        let mut camera = OrbitController::default().camera();
        camera.ortho_half_height = Some(200.0);
        let viewport = Vec2::new(1280.0, 720.0);
        let (origin_a, dir_a) = camera.screen_to_ray(Vec2::new(0.0, 0.0), viewport);
        let (origin_b, dir_b) = camera.screen_to_ray(Vec2::new(1280.0, 720.0), viewport);
        // Parallel projection: every ray shares the same direction, only
        // the origin differs across the viewport.
        assert!(dir_a.abs_diff_eq(dir_b, 1e-5));
        assert!(dir_a.abs_diff_eq(camera.forward(), 1e-5));
        assert!(
            (origin_a - origin_b).length() > 1.0,
            "orthographic ray origins must differ across the viewport"
        );
    }

    #[test]
    fn orthographic_world_to_screen_round_trips_through_screen_to_ray() {
        let mut camera = OrbitController::default().camera();
        camera.ortho_half_height = Some(200.0);
        let viewport = Vec2::new(1280.0, 720.0);
        let screen_in = Vec2::new(400.0, 500.0);
        let (origin, dir) = camera.screen_to_ray(screen_in, viewport);
        let world = ray_intersect_z0(origin, dir).unwrap();
        let screen_out = camera.world_to_screen(world.extend(0.0), viewport).unwrap();
        assert!(screen_in.abs_diff_eq(screen_out, 0.5));
    }

    #[test]
    fn orthographic_world_to_screen_returns_none_behind_the_camera() {
        let mut camera = OrbitController::default().camera();
        camera.ortho_half_height = Some(200.0);
        let behind = camera.position - camera.forward() * 10.0;
        assert_eq!(
            camera.world_to_screen(behind, Vec2::new(1280.0, 720.0)),
            None
        );
    }

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
    fn orbit_pitch_is_unbounded_and_never_clamped() {
        // P9.3: the previous [0, 89°] clamp is gone — orbit must be able to
        // swing well past the old horizon-adjacent limit in either
        // direction without being stopped.
        let mut orbit = OrbitController::default();
        orbit.orbit(0.0, -10.0);
        assert!((orbit.pitch - (-10.0)).abs() < 1e-6);
        orbit.orbit(0.0, 25.0);
        assert!((orbit.pitch - 15.0).abs() < 1e-6);
    }

    #[test]
    fn orbit_can_pass_continuously_over_the_top_of_the_pivot() {
        // A full pole-to-pole sweep (pitch 0 -> π -> 2π) must never panic,
        // produce a NaN, or visibly reverse direction tick-to-tick — the
        // exact "camera feels locked / can orbit over the top" requirement
        // this milestone fixes.
        let mut orbit = OrbitController::default();
        let steps = 200;
        let mut last_forward = orbit.forward();
        for i in 1..=steps {
            orbit.orbit(0.0, std::f32::consts::TAU / steps as f32);
            let forward = orbit.forward();
            assert!(forward.is_finite(), "forward became non-finite at step {i}");
            // Consecutive samples must stay close together — a true flip
            // would show up as a large discontinuous jump.
            assert!(
                last_forward.dot(forward) > 0.9,
                "discontinuous jump in forward direction at step {i}"
            );
            last_forward = forward;
        }
        // A full 2π sweep must return arbitrarily close to where it started.
        assert!(orbit
            .forward()
            .abs_diff_eq(OrbitController::default().forward(), 1e-3));
    }

    #[test]
    fn orbit_reference_up_is_world_z_not_y() {
        // P9.3: Orbit used to build its basis from `Vec3::Y`, inconsistent
        // with `FlyController`'s `Vec3::Z` — this must no longer be true.
        // With the camera looking horizontally (pitch near the equator,
        // away from the degenerate poles), its screen-up vector should
        // read close to world +Z.
        let orbit = OrbitController {
            pitch: std::f32::consts::FRAC_PI_2, // horizon, away from either pole
            ..OrbitController::default()
        };
        let up = orbit.camera().up();
        assert!(
            up.dot(Vec3::Z) > 0.99,
            "orbit's up vector should track world Z, got {up:?}"
        );
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
    fn world_to_screen_round_trips_through_screen_to_ray_at_the_z0_plane() {
        let camera = OrbitController::default().camera();
        let viewport = Vec2::new(1280.0, 720.0);
        let screen_in = Vec2::new(300.0, 450.0);
        let (origin, dir) = camera.screen_to_ray(screen_in, viewport);
        let world = ray_intersect_z0(origin, dir).unwrap();
        let screen_out = camera.world_to_screen(world.extend(0.0), viewport).unwrap();
        assert!(screen_in.abs_diff_eq(screen_out, 0.5));
    }

    #[test]
    fn world_to_screen_returns_none_behind_the_camera() {
        let camera = OrbitController::default().camera();
        let behind = camera.position - camera.forward() * 10.0;
        assert_eq!(
            camera.world_to_screen(behind, Vec2::new(1280.0, 720.0)),
            None
        );
    }

    #[test]
    fn point_in_polygon_finds_the_center_of_a_square() {
        let square = [
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 10.0),
        ];
        assert!(point_in_polygon(Vec2::new(5.0, 5.0), &square));
    }

    #[test]
    fn point_in_polygon_excludes_a_point_outside_the_square() {
        let square = [
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 10.0),
        ];
        assert!(!point_in_polygon(Vec2::new(15.0, 5.0), &square));
    }

    #[test]
    fn point_in_polygon_is_false_for_fewer_than_three_points() {
        let line = [Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)];
        assert!(!point_in_polygon(Vec2::new(5.0, 5.0), &line));
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
        // Round-tripping through Fly is lossy: P9.3 removed Orbit's own
        // pitch clamp, but `FlyController` still clamps a degree short of
        // its own pole (`FlyController::MAX_PITCH = 89°`, left unchanged —
        // Fly mode is explicitly out of this milestone's scope) to keep
        // its forward/up basis non-degenerate. A straight-down orbit view
        // can therefore only round-trip through Fly to within about that
        // margin, not exactly — the resulting orbit must still look in
        // approximately the same direction, not identically.
        assert!(before.forward().abs_diff_eq(after.forward(), 0.05));
    }
}
