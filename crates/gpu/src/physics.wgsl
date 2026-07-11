// Phase 8, Epic 8.10 (ADR-P8-04): `ParticleNode.position/velocity/force`
// widened from `vec2<f32>` to `vec3<f32>`, and the steric-hindrance
// broad-phase moved from a dense 2D grid (`GRID_DIM x GRID_DIM`, direct
// indexing) to a fixed-size spatial hash over 3D cell coordinates —
// extending a naive dense grid to 3D would have been a ~128x memory
// increase for equal per-axis resolution; a hash table sized for the
// expected *population*, not the *volume* of the world, avoids that
// entirely, mirroring `crates/spatial::SpatialHash`'s own CPU-side
// prime-XOR mixing technique so both broad-phases share one conceptual
// design.
//
// Phase 8, Epic 8.8: the fin-drag formula's perpendicular direction is now
// a real `cross(DORSAL, dir)` (see `DORSAL`'s own doc comment below),
// replacing the pre-8.8 `vec3(-dir.y, dir.x, 0.0)` component-swap trick —
// the same `dorsal`-vector body-frame convention
// `organisms::bilateral_fin_direction` uses on the CPU side for fin
// *placement*, so both systems share one consistent notion of a body's
// orientation. `DORSAL` is a fixed constant (not yet an uploaded per-spring
// value), so this is numerically identical to the pre-8.8 formula for
// every spring today (`cross((0,0,1), dir) == (-dir.y, dir.x, 0.0)` for
// any `dir` confined to the XY plane, as every real spring's `dir` still
// is) — an architecture-consistency redesign, not a behavior change, per
// this epic's own explicit scope (see ADR-P8-06's framing: an engine
// migration, not a biological redesign).
struct ParticleNode {
    position: vec3<f32>,
    _pad0: f32,
    velocity: vec3<f32>,
    _pad1: f32,
    force: vec3<f32>,
    _pad2: f32,
    mass: f32,
    organism_id: u32,
    _pad3: vec2<f32>,
}

struct Spring {
    node_a: u32,
    node_b: u32,
    constraint_type: u32, // 0 = Elastic, 1 = Rigid, 2 = Passive, 3 = Rotational
    rest_length: f32,
    base_length: f32,
    stiffness: f32,
    damping: f32,
    actuation_amplitude: f32,
    actuation_phase: f32,
    breaking_strain: f32,
    is_fin: u32,
    padding_2: u32,
}

struct PhysicsConfig {
    dt: f32,
    time: f32,
    // The nodes/springs buffers are capacity-sized (grown geometrically to
    // avoid reallocating every tick as population changes) so they can hold
    // more than the currently-live population — these two counts are the
    // actual number of live entries this tick, and must be used instead of
    // `arrayLength()` (which reflects buffer capacity, not live count) for
    // every loop bound / entry guard below.
    active_node_count: u32,
    active_spring_count: u32,
}

@group(0) @binding(0) var<storage, read_write> nodes: array<ParticleNode>;
@group(0) @binding(1) var<storage, read_write> springs: array<Spring>;
@group(0) @binding(2) var<uniform> config: PhysicsConfig;
@group(0) @binding(3) var<storage, read_write> atomic_forces_x: array<atomic<i32>>;
@group(0) @binding(4) var<storage, read_write> atomic_forces_y: array<atomic<i32>>;
@group(0) @binding(5) var<storage, read_write> atomic_forces_z: array<atomic<i32>>;
// Broad-phase spatial hash for the steric-hindrance repulsion loop below —
// fixed-size (not grown with population, and no longer tied to world
// volume): `cell_counts[b]` is how many nodes have hashed into bucket `b`
// this tick (capped for storage at HASH_CELL_CAPACITY, see `bin_nodes`),
// and `cell_nodes` holds up to HASH_CELL_CAPACITY node indices per bucket,
// laid out as `cell_nodes[b * HASH_CELL_CAPACITY + slot]`.
@group(0) @binding(6) var<storage, read_write> cell_counts: array<atomic<u32>>;
@group(0) @binding(7) var<storage, read_write> cell_nodes: array<u32>;

const FORCE_SCALE: f32 = 10000.0;

// Hash-table size and per-bucket capacity — MUST match `HASH_TABLE_SIZE`/
// `HASH_CELL_CAPACITY` in physics_pipeline.rs, which size the fixed
// `cell_counts`/`cell_nodes` buffers allocated on the Rust side. Chosen to
// match the total cell count the pre-8.10 dense `128 x 128` grid used
// (16384), so this change costs no additional GPU memory versus before —
// only the *indexing function* changes (hash mix instead of direct 2D
// indexing), which is what makes a 3rd axis free instead of a ~128x blowup.
const HASH_TABLE_SIZE: u32 = 16384u;
const HASH_CELL_CAPACITY: u32 = 64u;
const GRID_CELL_SIZE: f32 = 32.0;

// Same mixing primes `crates/spatial::SpatialHash` uses on the CPU side
// (ADR-P8-04's explicit "one conceptual broad-phase design, not two"
// requirement) — the two hash functions aren't required to produce
// identical bucket indices (they index unrelated buffers for unrelated
// purposes), just the same style of prime-XOR mixing.
const P1: i32 = 73856093;
const P2: i32 = 19349663;
const P3: i32 = 83492791;

// Body-fixed dorsal ("up") reference for fin-drag's perpendicular
// direction (Phase 8, Epic 8.8, ADR-P8-06) — the same fixed `Vec3::Z`
// value `organisms::GrowthState::dorsal`/`sensing::HeadVision::dorsal`
// both default to and never vary from today. A real per-spring/per-
// organism dorsal isn't uploaded to the GPU (nothing anywhere evolves or
// varies dorsal yet), so this stays a constant — the redesign is the
// *formula* (a real cross product against a named body-frame reference),
// not a new per-instance data channel.
const DORSAL: vec3<f32> = vec3<f32>(0.0, 0.0, 1.0);

// Maps a world position to its (unbounded — no clamping needed, unlike the
// pre-8.10 dense grid) integer cell coordinate.
fn grid_cell_coord(pos: vec3<f32>) -> vec3<i32> {
    return vec3<i32>(
        i32(floor(pos.x / GRID_CELL_SIZE)),
        i32(floor(pos.y / GRID_CELL_SIZE)),
        i32(floor(pos.z / GRID_CELL_SIZE)),
    );
}

// Hashes a 3D cell coordinate into a bucket index. `HASH_TABLE_SIZE` is a
// power of two, so masking with `HASH_TABLE_SIZE - 1` is equivalent to `%
// HASH_TABLE_SIZE` — the standard fast-path for power-of-two hash tables.
// WGSL's defined wrapping behavior for `i32` arithmetic makes this safe
// (and deliberately unconcerned with overflow) for arbitrary cell
// coordinates, the same way the CPU-side hash treats overflow as a
// harmless part of the mix rather than an error condition.
fn bucket_of(cell: vec3<i32>) -> u32 {
    let h = (cell.x * P1) ^ (cell.y * P2) ^ (cell.z * P3);
    return bitcast<u32>(h) & (HASH_TABLE_SIZE - 1u);
}

// Pass 0: Bin every live node into its spatial-hash bucket for this tick's
// broad-phase repulsion queries (see `integrate` below).
@compute @workgroup_size(64)
fn bin_nodes(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= config.active_node_count) { return; }

    let bucket = bucket_of(grid_cell_coord(nodes[index].position));
    let slot = atomicAdd(&cell_counts[bucket], 1u);
    if (slot < HASH_CELL_CAPACITY) {
        cell_nodes[bucket * HASH_CELL_CAPACITY + slot] = index;
    }
}

// Pass 1: Compute Forces
@compute @workgroup_size(64)
fn compute_forces(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= config.active_spring_count) { return; }

    let spring = springs[index];
    let a_idx = spring.node_a;
    let b_idx = spring.node_b;

    let pos_a = nodes[a_idx].position;
    let pos_b = nodes[b_idx].position;

    let delta = pos_b - pos_a;
    let dist = max(length(delta), 0.0001); // Prevent div-by-zero

    let dir = delta / dist;
    // Only apply standard forces for Elastic, Passive, and Rotational. Rigid is handled in PBD.
    if (spring.constraint_type != 1u) {
            let displacement = dist - spring.rest_length;
            let spring_force = dir * (displacement * spring.stiffness);

            let vel_a = nodes[a_idx].velocity;
            let vel_b = nodes[b_idx].velocity;
            let rel_vel = vel_b - vel_a;
            let damp_force = dir * (dot(rel_vel, dir) * spring.damping);

            var total_force = spring_force + damp_force;

            // Clamp force magnitude — an entangled high-relative-velocity pair
            // can otherwise produce an oversized damp_force from last tick's
            // already-large velocity, ratcheting energy upward tick over tick.
            let max_spring_force = spring.stiffness * 50.0 + 5000.0;
            let force_mag = length(total_force);
            if (force_mag > max_spring_force) {
                total_force = total_force * (max_spring_force / force_mag);
            }

            // Apply anisotropic drag if it's a Fin. `normal` (Phase 8, Epic
            // 8.8) is now a real `cross(DORSAL, dir)` body-frame
            // perpendicular — see `DORSAL`'s own doc comment for why this
            // is numerically identical to the pre-8.8 formula today.
            if (spring.is_fin == 1u) {
                let mid_vel = (vel_a + vel_b) * 0.5; // Velocity relative to fluid
                let normal = cross(DORSAL, dir);
                let v_norm = dot(mid_vel, normal);

                // High drag perpendicular to fin, low drag parallel
                let drag_force = -normal * (v_norm * abs(v_norm) * 50.0); // Quadratic drag

                // Add drag force to the nodes (divided equally)
                let half_drag = drag_force * 0.5;

                let dfx = i32(half_drag.x * FORCE_SCALE);
                let dfy = i32(half_drag.y * FORCE_SCALE);
                let dfz = i32(half_drag.z * FORCE_SCALE);
                atomicAdd(&atomic_forces_x[a_idx], dfx);
                atomicAdd(&atomic_forces_y[a_idx], dfy);
                atomicAdd(&atomic_forces_z[a_idx], dfz);
                atomicAdd(&atomic_forces_x[b_idx], dfx);
                atomicAdd(&atomic_forces_y[b_idx], dfy);
                atomicAdd(&atomic_forces_z[b_idx], dfz);
            }

            let fx = i32(total_force.x * FORCE_SCALE);
            let fy = i32(total_force.y * FORCE_SCALE);
            let fz = i32(total_force.z * FORCE_SCALE);

            atomicAdd(&atomic_forces_x[a_idx], fx);
            atomicAdd(&atomic_forces_y[a_idx], fy);
            atomicAdd(&atomic_forces_z[a_idx], fz);
            atomicAdd(&atomic_forces_x[b_idx], -fx);
            atomicAdd(&atomic_forces_y[b_idx], -fy);
            atomicAdd(&atomic_forces_z[b_idx], -fz);
    }
}

// Pass 2: Integrate
@compute @workgroup_size(64)
fn integrate(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= config.active_node_count) { return; }

    var node = nodes[index];

    // --- STERIC HINDRANCE (Node Repulsion) ---
    // Same-organism nodes repel strongly to give the body volume; different
    // organisms repel more gently (just enough to stop visual overlap
    // without fighting locomotion/actuation forces).
    let R = 15.0; // Rest distance threshold (intra-organism)
    let k_repel = 2000.0; // Repulsion strength. High enough to counter springs.
    let R_CROSS = 15.0; // Rest distance threshold (inter-organism)
    let k_repel_cross = 400.0; // Weaker: only needed to keep bodies apart
    var repel_force = vec3<f32>(0.0, 0.0, 0.0);

    // Broad-phase: scan the 3x3x3 cell neighborhood around this node,
    // hashing each neighbor cell coordinate to a bucket — correct as long
    // as GRID_CELL_SIZE >= max(R, R_CROSS) (checked: both are 15.0 <=
    // 32.0), so any node within repulsion range of this one is guaranteed
    // to fall in the center cell or one of its 26 neighbors. Distinct
    // neighbor cells can hash to the same bucket (a hash collision), so
    // visited buckets are deduplicated before scanning — otherwise a
    // colliding bucket's nodes would have their repulsion force
    // double-counted.
    let my_cell = grid_cell_coord(node.position);
    var visited_buckets: array<u32, 27>;
    var visited_count = 0u;

    for (var dz = -1; dz <= 1; dz = dz + 1) {
        for (var dy = -1; dy <= 1; dy = dy + 1) {
            for (var dx = -1; dx <= 1; dx = dx + 1) {
                let neighbor_cell = my_cell + vec3<i32>(dx, dy, dz);
                let bucket = bucket_of(neighbor_cell);

                var already_visited = false;
                for (var v = 0u; v < visited_count; v = v + 1u) {
                    if (visited_buckets[v] == bucket) {
                        already_visited = true;
                        break;
                    }
                }
                if (already_visited) { continue; }
                visited_buckets[visited_count] = bucket;
                visited_count = visited_count + 1u;

                let count = min(atomicLoad(&cell_counts[bucket]), HASH_CELL_CAPACITY);
                for (var s = 0u; s < count; s = s + 1u) {
                    let i = cell_nodes[bucket * HASH_CELL_CAPACITY + s];
                    if (i == index) { continue; }
                    let other = nodes[i];
                    let delta = node.position - other.position;
                    let d = length(delta);
                    if (other.organism_id == node.organism_id) {
                        if (d > 0.0001 && d < R) {
                            let dir = delta / d;
                            repel_force = repel_force + dir * (k_repel * (R - d));
                        }
                    } else {
                        if (d > 0.0001 && d < R_CROSS) {
                            let dir = delta / d;
                            repel_force = repel_force + dir * (k_repel_cross * (R_CROSS - d));
                        }
                    }
                }
            }
        }
    }

    let fx = f32(atomicLoad(&atomic_forces_x[index])) / FORCE_SCALE;
    let fy = f32(atomicLoad(&atomic_forces_y[index])) / FORCE_SCALE;
    let fz = f32(atomicLoad(&atomic_forces_z[index])) / FORCE_SCALE;
    let total_force = node.force + vec3<f32>(fx, fy, fz) + repel_force;

    // Symplectic Euler
    let acceleration = total_force / node.mass;
    node.velocity = node.velocity + acceleration * config.dt;

    // Global damping
    node.velocity = node.velocity * 0.99;

    // ── Hard velocity cap (BEFORE position update) ──────────────────────────
    // This must be here — not just in apply_pbd — because position is updated
    // below. apply_pbd runs after position update so it cannot prevent the
    // displacement from an uncapped velocity.
    let max_speed = 200.0; // world-units / s; typical locomotion is < 20
    let speed = length(node.velocity);
    if (speed > max_speed) {
        node.velocity = node.velocity * (max_speed / speed);
    }

    // ── Soft world-bounds reflection (BEFORE position update) ───────────────
    // Reflect nodes that are already out-of-bounds so they migrate back.
    // This runs before the position update so the reflected velocity produces
    // inward displacement this same tick.
    let bounds = 1500.0;
    if (abs(node.position.x) > bounds) {
        node.position.x = clamp(node.position.x, -bounds, bounds);
        node.velocity.x = -node.velocity.x * 0.5; // lose half energy on bounce
    }
    if (abs(node.position.y) > bounds) {
        node.position.y = clamp(node.position.y, -bounds, bounds);
        node.velocity.y = -node.velocity.y * 0.5;
    }
    // Z bound: organisms are still Z=0 by construction (no code path gives
    // vertical velocity/position yet — Epic 8.6/8.7's own disclosed scope
    // boundary), but the integrator itself is now genuinely 3D-capable;
    // clamped symmetrically with X/Y for defensiveness, in case a future
    // epic introduces real vertical forces.
    if (abs(node.position.z) > bounds) {
        node.position.z = clamp(node.position.z, -bounds, bounds);
        node.velocity.z = -node.velocity.z * 0.5;
    }

    node.position = node.position + node.velocity * config.dt;

    // Reset forces
    node.force = vec3<f32>(0.0, 0.0, 0.0);
    atomicStore(&atomic_forces_x[index], 0);
    atomicStore(&atomic_forces_y[index], 0);
    atomicStore(&atomic_forces_z[index], 0);

    nodes[index] = node;
}


// Pass 3: PBD Projection
@compute @workgroup_size(64)
fn pbd_projection(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= config.active_spring_count) { return; }

    let spring = springs[index];

    // Only process Rigid constraints
    if (spring.constraint_type == 1u) {
        let a_idx = spring.node_a;
        let b_idx = spring.node_b;

        let pos_a = nodes[a_idx].position;
        let pos_b = nodes[b_idx].position;

        let delta = pos_b - pos_a;
        let dist = max(length(delta), 0.0001); // Prevent div-by-zero

        // Dampen correction by 0.25 (relaxation factor) to prevent multi-spring atomicAdd explosions
        let correction_mag = (dist - spring.rest_length) * 0.5 * 0.25;
        let dir = delta / dist;
        let correction = dir * correction_mag;

            // To be thread-safe without atomic floats, PBD on GPU typically uses Graph Coloring or Jacobi methods.
            // For a simple implementation, we just atomic add the positional correction and divide later,
            // or we accept slight tearing. Here we use atomicAdd on fixed-point positions.

            let cx = i32(correction.x * FORCE_SCALE);
            let cy = i32(correction.y * FORCE_SCALE);
            let cz = i32(correction.z * FORCE_SCALE);

            atomicAdd(&atomic_forces_x[a_idx], cx);
            atomicAdd(&atomic_forces_y[a_idx], cy);
            atomicAdd(&atomic_forces_z[a_idx], cz);
            atomicAdd(&atomic_forces_x[b_idx], -cx);
            atomicAdd(&atomic_forces_y[b_idx], -cy);
            atomicAdd(&atomic_forces_z[b_idx], -cz);
    }
}

// Pass 4: Apply PBD Corrections
@compute @workgroup_size(64)
fn apply_pbd(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= config.active_node_count) { return; }

    var node = nodes[index];

    let cx = f32(atomicLoad(&atomic_forces_x[index])) / FORCE_SCALE;
    let cy = f32(atomicLoad(&atomic_forces_y[index])) / FORCE_SCALE;
    let cz = f32(atomicLoad(&atomic_forces_z[index])) / FORCE_SCALE;
    let correction = vec3<f32>(cx, cy, cz);

    node.position = node.position + correction;

    // Inject velocity from PBD correction — clamp to prevent explosion.
    // Without clamping, large corrections (from nodes far apart) inject
    // huge velocities that accumulate across ticks and cause fly-off.
    // Clamped by magnitude (not per-axis) — a per-axis clamp lets diagonal
    // corrections overshoot to max_pbd_vel * sqrt(2), which was one source
    // of the entanglement "lock and spiral" energy ratchet.
    let raw_vel_correction = correction / config.dt;
    let max_pbd_vel = 150.0; // world-units/s cap on PBD velocity injection
    let raw_vel_mag = length(raw_vel_correction);
    var pbd_vel = raw_vel_correction;
    if (raw_vel_mag > max_pbd_vel) {
        pbd_vel = raw_vel_correction * (max_pbd_vel / raw_vel_mag);
    }
    node.velocity = node.velocity + pbd_vel;

    // Damping — this pass runs 3x/tick (PBD iteration loop); without damping
    // here too, injected velocity compounds undamped across iterations
    // within a single tick, on top of the once-per-tick damping in
    // `integrate`. This was the primary cause of entangled organisms
    // spiraling apart with escalating rather than settling energy.
    node.velocity = node.velocity * 0.99;

    // Hard velocity cap — matches `integrate`'s cap so PBD injection can
    // never push a node faster than a normal tick's own ceiling allows.
    let max_speed = 200.0;
    let speed = length(node.velocity);
    if (speed > max_speed) {
        node.velocity = node.velocity * (max_speed / speed);
    }

    // Soft world-bounds: reflect nodes that drift too far so they stay
    // within a reasonable simulation area (±2000 world units).
    let bounds = 2000.0;
    if (abs(node.position.x) > bounds) {
        node.position.x = clamp(node.position.x, -bounds, bounds);
        node.velocity.x = node.velocity.x * -0.5;
    }
    if (abs(node.position.y) > bounds) {
        node.position.y = clamp(node.position.y, -bounds, bounds);
        node.velocity.y = node.velocity.y * -0.5;
    }
    if (abs(node.position.z) > bounds) {
        node.position.z = clamp(node.position.z, -bounds, bounds);
        node.velocity.z = node.velocity.z * -0.5;
    }

    atomicStore(&atomic_forces_x[index], 0);
    atomicStore(&atomic_forces_y[index], 0);
    atomicStore(&atomic_forces_z[index], 0);

    nodes[index] = node;
}
