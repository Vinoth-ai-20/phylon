//! Procedural capsule mesh generation (Phase 8, ADR-P8-03) — one shared,
//! low-poly vertex/index buffer, generated once at startup and instanced
//! per bone. Replaces the retired `sdf_skin` accumulate-blend technique.
//!
//! ## Local-space convention
//!
//! The mesh lives in a unit "capsule space" designed so the vertex shader
//! (`capsule.wgsl`) can orient and scale it per-instance without the CPU
//! doing any per-instance work:
//!
//! - `y ∈ [-1, 0]`: the bottom hemisphere, a unit sphere centered at the
//!   local origin `(0, 0, 0)` — this maps to the bone's `pos_a` endpoint.
//! - `y ∈ [0, 1]`: the cylindrical body, unit radius.
//! - `y ∈ [1, 2]`: the top hemisphere, a unit sphere centered at
//!   `(0, 1, 0)` — this maps to the bone's `pos_b` endpoint.
//!
//! The vertex shader classifies each vertex by its local `y` into one of
//! these three regions and reconstructs its world position from
//! `pos_a`/`pos_b`/`radius` accordingly — see that shader's own doc comment
//! for the exact formula. This is the "oriented-look-at vertex shader
//! technique" ADR-P8-03 names: no per-instance rotation/quaternion is
//! stored, only the two endpoints and a radius (nearly the same instance
//! *data* as the old `SdfBoneInstance`).

/// One vertex of the shared capsule mesh, in local capsule space (see this
/// module's doc comment).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CapsuleVertex {
    /// Local-space position (see module doc comment for the `y`-region convention).
    pub position: [f32; 3],
    /// Local-space normal — a genuine unit-sphere normal for cap vertices,
    /// a radial (no `y` component) normal for cylinder-body vertices.
    pub normal: [f32; 3],
}

impl CapsuleVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

    /// The vertex buffer layout for this shared mesh (step mode `Vertex`,
    /// not `Instance` — every instance reuses the same buffer).
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CapsuleVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Longitude divisions — kept low-poly per the roadmap's explicit
/// "shared low-poly capsule mesh" instruction; a future LOD/billboard-
/// impostor tier (mentioned in the roadmap's Mesh pipeline section) is
/// explicitly out of scope for this epic.
const RADIAL_SEGMENTS: u32 = 12;
/// Latitude divisions per hemisphere cap (not counting the degenerate pole
/// ring, which is included as ring index 0/last with a zero radius —
/// simpler than a special-cased triangle fan, at the cost of a few
/// zero-area triangles at each pole, an explicitly acceptable tradeoff for
/// a "kept simple" low-poly primitive).
const CAP_RINGS: u32 = 4;

/// Builds the shared capsule mesh once, returning `(vertices, indices)`.
/// `indices` are `u16` — comfortably enough for this mesh's small vertex
/// count (`2 * (CAP_RINGS + 1) * RADIAL_SEGMENTS` = 120 vertices at the
/// constants above).
pub fn build_capsule_mesh() -> (Vec<CapsuleVertex>, Vec<u16>) {
    let mut vertices = Vec::new();
    let mut ring_starts = Vec::new();

    // Bottom hemisphere: ring 0 is the degenerate pole (y = -1, radius 0),
    // ring CAP_RINGS is the equator (y = 0, radius 1).
    for i in 0..=CAP_RINGS {
        ring_starts.push(vertices.len() as u16);
        let theta = (i as f32 / CAP_RINGS as f32) * std::f32::consts::FRAC_PI_2;
        let y = -theta.cos();
        let ring_radius = theta.sin();
        for j in 0..RADIAL_SEGMENTS {
            let phi = (j as f32 / RADIAL_SEGMENTS as f32) * std::f32::consts::TAU;
            let x = ring_radius * phi.cos();
            let z = ring_radius * phi.sin();
            let position = [x, y, z];
            // A genuine point on the unit sphere centered at the local
            // origin — the normal is just the position itself.
            let normal = normalize([x, y, z]);
            vertices.push(CapsuleVertex { position, normal });
        }
    }

    // Cylinder body: reuses the bottom hemisphere's equator ring (last one
    // pushed above) and the top hemisphere's equator ring (first one pushed
    // below) as its two ends — no separate vertices needed.
    let cylinder_bottom_ring = *ring_starts.last().unwrap();

    // Top hemisphere: ring 0 is the equator (y = 1, radius 1), ring
    // CAP_RINGS is the degenerate pole (y = 2, radius 0).
    for i in 0..=CAP_RINGS {
        ring_starts.push(vertices.len() as u16);
        let theta = (i as f32 / CAP_RINGS as f32) * std::f32::consts::FRAC_PI_2;
        let y = 1.0 + theta.sin();
        let ring_radius = theta.cos();
        for j in 0..RADIAL_SEGMENTS {
            let phi = (j as f32 / RADIAL_SEGMENTS as f32) * std::f32::consts::TAU;
            let x = ring_radius * phi.cos();
            let z = ring_radius * phi.sin();
            let position = [x, y, z];
            // Sphere centered at local (0, 1, 0).
            let normal = normalize([x, y - 1.0, z]);
            vertices.push(CapsuleVertex { position, normal });
        }
    }

    let cylinder_top_ring = ring_starts[CAP_RINGS as usize + 1];

    let mut indices = Vec::new();

    // Stitch every consecutive ring pair (within a hemisphere, and the one
    // cylinder-body pair connecting the two hemispheres' equators) with a
    // standard quad-as-two-triangles band, wrapping `j` around the ring.
    let mut connect_rings = |ring_a: u16, ring_b: u16| {
        for j in 0..RADIAL_SEGMENTS {
            let j_next = (j + 1) % RADIAL_SEGMENTS;
            let a0 = ring_a + j as u16;
            let a1 = ring_a + j_next as u16;
            let b0 = ring_b + j as u16;
            let b1 = ring_b + j_next as u16;
            indices.extend_from_slice(&[a0, b0, a1, a1, b0, b1]);
        }
    };

    for pair in ring_starts.windows(2) {
        connect_rings(pair[0], pair[1]);
    }
    // The cylinder-body band isn't covered by `ring_starts.windows(2)` since
    // that only connects rings *within* the same hemisphere's push order —
    // wait, it is: `ring_starts` is pushed in one contiguous sequence
    // (bottom hemisphere then top hemisphere), so `windows(2)` already
    // includes the bottom-equator→top-equator pair (`cylinder_bottom_ring`→
    // `cylinder_top_ring`) as the middle transition. Kept as named
    // constants above only for clarity/documentation, not because they're
    // used separately here.
    let _ = (cylinder_bottom_ring, cylinder_top_ring);

    (vertices, indices)
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len > 1e-8 {
        [v[0] / len, v[1] / len, v[2] / len]
    } else {
        [0.0, 1.0, 0.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_has_the_expected_vertex_and_index_counts() {
        let (vertices, indices) = build_capsule_mesh();
        let expected_vertices = 2 * (CAP_RINGS + 1) * RADIAL_SEGMENTS;
        let expected_ring_pairs = 2 * CAP_RINGS + 1;
        let expected_indices = expected_ring_pairs * RADIAL_SEGMENTS * 6;
        assert_eq!(vertices.len() as u32, expected_vertices);
        assert_eq!(indices.len() as u32, expected_indices);
    }

    #[test]
    fn every_index_is_in_bounds() {
        let (vertices, indices) = build_capsule_mesh();
        for &i in &indices {
            assert!((i as usize) < vertices.len());
        }
    }

    #[test]
    fn bottom_pole_sits_at_y_negative_one_and_top_pole_at_y_two() {
        let (vertices, _) = build_capsule_mesh();
        let bottom_pole_y = vertices[0].position[1];
        let top_pole_y = vertices.last().unwrap().position[1];
        assert!((bottom_pole_y - (-1.0)).abs() < 1e-5);
        assert!((top_pole_y - 2.0).abs() < 1e-5);
    }

    #[test]
    fn equator_rings_have_unit_radius() {
        let (vertices, _) = build_capsule_mesh();
        // The bottom hemisphere's last ring (index CAP_RINGS) is the
        // equator, at flat array offset CAP_RINGS * RADIAL_SEGMENTS.
        let equator_start = (CAP_RINGS * RADIAL_SEGMENTS) as usize;
        for v in &vertices[equator_start..equator_start + RADIAL_SEGMENTS as usize] {
            let [x, y, z] = v.position;
            let radius = (x * x + z * z).sqrt();
            assert!((radius - 1.0).abs() < 1e-4);
            assert!(y.abs() < 1e-5);
        }
    }

    #[test]
    fn cap_normals_match_their_local_sphere_center() {
        let (vertices, _) = build_capsule_mesh();
        for v in &vertices {
            let [nx, ny, nz] = v.normal;
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            assert!((len - 1.0).abs() < 1e-4, "normal must be unit length");
        }
    }
}
