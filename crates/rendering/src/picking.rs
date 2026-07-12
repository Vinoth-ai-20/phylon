//! # Ray-vs-Capsule Picking
//!
//! ## Purpose
//! Entity selection (mouse picking) needs to find which organism bone, if
//! any, a screen-space click ray passes through. This module computes an
//! exact 3D ray-vs-capsule intersection against the same `pos_a`/`pos_b`/
//! `radius` primitives [`crate::organism::OrganismRenderer`] draws, so what
//! the user sees on screen is exactly what's clickable — no separately-tuned
//! pick radius or flattened 2D approximation.
//!
//! ## Geometry
//! A capsule is the set of points within `radius` of a line segment (its
//! "core", from `pos_a` to `pos_b`). Intersecting a ray with it reduces to a
//! classic *closest point between two lines* problem: find the ray
//! parameter `s` and segment parameter `t` that minimize the distance
//! between a point on the ray (`ray_origin + s * ray_dir`) and a point on
//! the core segment (`pos_a + t * (pos_b - pos_a)`), clamping `t` to `[0,
//! 1]` so the closest point stays on the segment rather than its infinite
//! extension. If that minimum distance is within `radius`, the ray hits the
//! capsule at parameter `s`. Degenerate capsules (`pos_a == pos_b`, i.e. a
//! sphere — food/mineral/corpse pellets and any point-entity) fall out of
//! the same formula with no special-casing at the call site: the segment
//! collapses to a point and `t` becomes irrelevant.
//!
//! This is a closest-approach test, not a full ray-vs-cylinder surface
//! intersection — the returned `s` is where the ray comes nearest the
//! capsule's core, not the exact point where it crosses the capsule's skin.
//! That's sufficient (and cheaper) for picking: it correctly determines
//! hit/miss and ranks multiple overlapping candidates by approximate depth,
//! which is all a picking query needs.

/// Ray parameter (distance along the ray, `s >= 0`) at the point of closest
/// approach between `ray` and the capsule's core segment (`pos_a`-`pos_b`,
/// inflated by `radius`), if that closest approach falls within `radius` of
/// the segment — `None` if the ray misses the capsule entirely. See this
/// module's doc comment for the underlying closest-point-between-two-lines
/// geometry.
///
/// `ray_dir` must be normalized. The returned value is the closest-approach
/// distance along the ray, not the exact surface entry point — see the
/// module doc comment for why that's sufficient for picking.
pub fn ray_capsule_hit(
    ray_origin: glam::Vec3,
    ray_dir: glam::Vec3,
    pos_a: glam::Vec3,
    pos_b: glam::Vec3,
    radius: f32,
) -> Option<f32> {
    const EPSILON: f32 = 1e-6;
    let d2 = pos_b - pos_a;
    let e = d2.dot(d2);

    let s = if e <= EPSILON {
        // Degenerate capsule (a point/sphere) — closest point on the ray to
        // a single point, clamped to the front of the ray.
        (pos_a - ray_origin).dot(ray_dir).max(0.0)
    } else {
        let r = ray_origin - pos_a;
        let b = ray_dir.dot(d2);
        let c = ray_dir.dot(r);
        let f = d2.dot(r);
        // `a == 1.0` throughout below since `ray_dir` is unit-length.
        let denom = e - b * b;

        let mut s = if denom > EPSILON {
            ((b * f - c * e) / denom).max(0.0)
        } else {
            // Ray parallel to the capsule's core segment.
            0.0
        };
        let mut t = (b * s + f) / e;
        if t < 0.0 {
            t = 0.0;
            s = (-c).max(0.0);
        } else if t > 1.0 {
            t = 1.0;
            s = (b - c).max(0.0);
        }
        let _ = t;
        s
    };

    let closest_on_ray = ray_origin + ray_dir * s;
    let t_seg = if e <= EPSILON {
        0.0
    } else {
        ((closest_on_ray - pos_a).dot(d2) / e).clamp(0.0, 1.0)
    };
    let closest_on_segment = pos_a + d2 * t_seg;
    let dist = (closest_on_ray - closest_on_segment).length();

    (dist <= radius).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn ray_through_the_center_of_a_point_sphere_hits_at_the_projection_distance() {
        let hit = ray_capsule_hit(
            Vec3::new(0.0, 0.0, -10.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::ZERO,
            Vec3::ZERO,
            1.0,
        );
        assert!((hit.unwrap() - 10.0).abs() < 1e-4);
    }

    #[test]
    fn ray_missing_a_point_sphere_by_more_than_its_radius_is_none() {
        let hit = ray_capsule_hit(
            Vec3::new(0.0, 5.0, -10.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::ZERO,
            Vec3::ZERO,
            1.0,
        );
        assert!(hit.is_none());
    }

    #[test]
    fn ray_grazing_a_point_sphere_within_radius_hits() {
        let hit = ray_capsule_hit(
            Vec3::new(0.0, 0.9, -10.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::ZERO,
            Vec3::ZERO,
            1.0,
        );
        assert!(hit.is_some());
    }

    #[test]
    fn ray_through_the_middle_of_a_cylinder_body_hits() {
        // Capsule core runs along +Y from (0,-5,0) to (0,5,0); ray travels
        // along +Z, crossing the segment's midpoint region perpendicularly.
        let hit = ray_capsule_hit(
            Vec3::new(0.0, 0.0, -10.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, -5.0, 0.0),
            Vec3::new(0.0, 5.0, 0.0),
            2.0,
        );
        assert!((hit.unwrap() - 10.0).abs() < 1e-4);
    }

    #[test]
    fn ray_missing_past_a_segment_endpoint_is_none() {
        // Perpendicular to the segment but far beyond its `pos_b` end —
        // closest point on the (clamped) segment is farther than `radius`.
        let hit = ray_capsule_hit(
            Vec3::new(0.0, 20.0, -10.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, -5.0, 0.0),
            Vec3::new(0.0, 5.0, 0.0),
            2.0,
        );
        assert!(hit.is_none());
    }

    #[test]
    fn ray_origin_already_inside_the_capsule_hits_at_zero() {
        let hit = ray_capsule_hit(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, -1.0), // pointing away from the capsule
            Vec3::new(0.0, -5.0, 0.0),
            Vec3::new(0.0, 5.0, 0.0),
            2.0,
        );
        assert_eq!(hit, Some(0.0));
    }

    #[test]
    fn nearer_hit_has_a_smaller_t_than_a_farther_one() {
        let origin = Vec3::new(0.0, 0.0, -10.0);
        let dir = Vec3::new(0.0, 0.0, 1.0);
        let near = ray_capsule_hit(origin, dir, Vec3::ZERO, Vec3::ZERO, 1.0).unwrap();
        let far = ray_capsule_hit(
            origin,
            dir,
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::new(0.0, 0.0, 5.0),
            1.0,
        )
        .unwrap();
        assert!(near < far);
    }
}
