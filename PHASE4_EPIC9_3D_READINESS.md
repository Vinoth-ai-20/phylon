# Phase 4, Epic 9 — 3D-Readiness Design Document (P4-A1)

## 0. Scope and Status

Per `PHASE4_ROADMAP.md`'s milestone table: *"3D-readiness design document: dimension-independent math/body-graph/physics-interface design, renderer abstraction, migration strategy — **audit and design only, no 3D implementation**."* This document is that deliverable. It is independent of every other Phase 4 epic (no code depends on it; it depends on no other epic landing first — see the roadmap's own dependency graph, where this node has no incoming edges).

**This document does not implement 3D support.** It audits exactly how far today's codebase is from supporting it, proposes a dimension-independent design where one is actually achievable, and honestly identifies the layers where "dimension-independent" is not achievable without a parallel rewrite — recommending a migration strategy that accounts for that, rather than promising a generalization that doesn't exist.

---

## 1. Current-State Audit

This expands `PHASE4_ROADMAP.md` §2.8's table with concrete reasoning per layer, not just a verdict.

### 1.1 Core vector type — `common::Vec2`

`crates/common/src/lib.rs:31,34` re-exports `glam::Vec2` directly as `common::Vec2`, with no wrapper type, no trait abstraction, no generic parameter. A workspace-wide search finds 311 matches across 39 files using it directly as a concrete type in struct fields, function signatures, and shader-adjacent buffer layouts.

**Implication:** there is no single swap point. "Making `Vec2` generic over dimension" is not a small change — it would touch every one of those 311 call sites, and many of them (see below) don't have a well-defined 3D meaning anyway. Treating this as "just add a type parameter" would be the wrong mental model for this migration; see §3.

### 1.2 CPU physics — comparatively portable

`crates/physics/src/lib.rs`'s `ParticleNode`/`Spring` integrator (`physics_integration_system`, lines ~260-276) and constraint solver (`spring_force_system`, lines ~191-244) use only `.length()`, `.dot()`, and vector add/scale — operations `glam::Vec3` supports identically. No cross product, no 2D-specific perpendicular-vector trick, no heading/angle concept appears in this file.

**Implication:** if `Vec2` were replaced with `Vec3` throughout this one file (and its callers updated to supply a `z` component), the CPU integrator would very likely keep working with no logic changes — this is the one layer in the whole audit where "dimension-independent" is close to already true, by construction rather than by design.

### 1.3 GPU physics — the largest obstacle, not a generalization candidate

`crates/gpu/src/physics.wgsl` is hardcoded to 2D at three independent levels:

1. **Split force accumulation buffers**: `atomic_forces_x`/`atomic_forces_y` (lines ~39-40) — two parallel `atomic<i32>` buffers, no `z`. Adding a third buffer is mechanical, but see point 2.
2. **A 2D perpendicular-vector trick with no 3D equivalent**: `vec2(-dir.y, dir.x)` (line ~59-76) is used to compute a direction orthogonal to a bone/muscle's axis — in 2D this is a single well-defined vector (a 90° rotation); in 3D, "the direction perpendicular to this axis" is an entire plane, not a vector — the shader would need an actual orientation/reference-frame representation (e.g. a per-bone quaternion or a stored "up" vector) to disambiguate which perpendicular direction is meant. This is not a generalization of the existing math; it requires a different representation of orientation that doesn't exist in today's model at all.
3. **A flat 2D broad-phase collision grid** (line ~130) — a 3D equivalent is a fundamentally different data structure (a voxel grid or a bounding-volume hierarchy), not a templated version of the same grid.

**Implication, stated plainly:** a 3D physics shader is a **parallel rewrite**, not a generalization of the existing one — and this is the shader the actual simulation runs on in every normal configuration (the CPU fallback in §1.2 exists for tests/headless CI only, per its own doc comment). Any 3D-readiness plan that treats GPU physics as "just extend the buffers" would be wrong; §3's migration strategy accounts for this directly.

### 1.4 Rendering — the projection swap is contained, but the shape model is not

`crates/rendering/src/debug.rs:180-184` and `sdf_skin.rs:481-484` build the render pipeline on a real `glam::Mat4::orthographic_rh` projection matrix, not raw 2D screen-space drawing — swapping to `Mat4::perspective_rh` (or keeping an orthographic 3D view) is a contained, well-understood change at the projection-matrix level.

However, the SDF/skin shape model itself — 2D circle/capsule signed-distance fields representing each body segment's visual "skin" — is 2D-specific by construction, with no mesh/model rendering path at all. A 3D organism would need either a 3D SDF primitive set (sphere/capsule SDFs, which do exist in the general SDF literature and are a smaller lift than the physics rewrite) or a conventional mesh-based renderer — a second, separate rendering paradigm, not an extension of the current one.

### 1.5 Spatial index — a different data structure, not a generic parameter

`crates/spatial/src/index.rs:21,26,35` defines a `SpatialIndex` trait already hardcoded to `Vec2` in its method signatures; `quadtree.rs:40,49-58`'s `Quadtree` is inherently 2D (exactly 4 children per subdivision — the "quad" in "quadtree" is not incidental). A 3D equivalent is an octree (8 children), a genuinely different structure with different subdivision math, not `Quadtree<Vec3>`.

### 1.6 The developmental pipeline — already dimension-agnostic, by accident

`crates/organisms/src/developmental_graph.rs`'s `DevelopmentalNode.position` is a `usize` body-axis index (this position's rank along the growth sequence), not a spatial coordinate — the graph's topology (segment type, parent index, branch order, and — as of this phase's own P4-F1 — a live entity link) carries zero `Vec2`/angle/coordinate data anywhere. The 2D embedding — turning "this is body position 4" into an actual `Vec2` in the world — happens one layer later and in exactly one place: `crates/organisms/src/spawning.rs`'s heading→position conversion (`start_pos + Vec2::new(heading.cos(), heading.sin()) * -segment_length`, confirmed at the call sites this phase's own P4-L1 work touched, e.g. `spawning.rs:95`, and `systems.rs`'s equivalent growth-tick embedding).

**Implication — the single most useful finding in this audit:** the *topology* layer (what a body plan structurally is) needs zero changes for 3D. Only the *materialization* layer (turning that topology into physical `Vec2`/`Vec3` positions) does. This means P4-F1 through P4-F6's entire body-graph/physiology infrastructure — built this phase — is **already 3D-ready as designed**, without having been built with 3D in mind. It was a side effect of keeping the graph's topology genuinely abstract (position-as-index, not position-as-coordinate), a design choice already made for unrelated reasons (Phase 3's ADR-P3-02, decoding by position not by template).

### 1.7 Net finding

Ranked by how large a 3D migration each layer actually requires:

| Rank | Layer | Effort class |
|---|---|---|
| 1 (smallest) | Developmental graph topology | None — already dimension-agnostic |
| 2 | CPU physics integrator | Small — direct `Vec3` substitution, no logic change |
| 3 | Rendering projection matrix | Small-Medium — contained matrix swap |
| 4 | Core vector type call sites | Medium — mechanical but touches 311 call sites |
| 5 | Spatial index | Medium-Large — new data structure (octree), not a template |
| 6 | Rendering shape model (SDF skin) | Large — new primitive set or a wholly different renderer |
| 7 (largest) | GPU physics shader | Large — parallel rewrite, not a generalization; blocked on an orientation representation that doesn't exist today |

---

## 2. Dimension-Independent Design Proposal

Given §1's ranking, a genuinely useful "dimension-independent" design effort should focus where dimension-independence is actually achievable cheaply, and should NOT pretend the GPU physics/SDF-rendering layers can be made generic — they need parallel implementations, and pretending otherwise would produce a design that looks clean on paper and is wrong in practice.

### 2.1 What should become genuinely dimension-independent

- **The developmental graph topology** (§1.6) already is — no design work needed, just don't regress it. Any future Phase 4/5 milestone touching `DevelopmentalNode` should keep treating `position` as an abstract index, never let a `Vec2`/`Vec3` leak into that struct.
- **The CPU physics integrator** (§1.2) should be made literally generic, since the underlying math already is. Concretely: parametrize `ParticleNode`/`Spring`'s position/velocity/force fields over a `Point: VectorSpace` trait bound (implemented for both `glam::Vec2` and `glam::Vec3`) rather than hardcoding `Vec2`. This is a real, achievable, low-risk generalization — not a rewrite.
- **The rendering projection** (§1.4) should expose an explicit `ProjectionMode::Orthographic2D | Perspective3D` choice at the point `Mat4::orthographic_rh` is currently hardcoded, so a future 3D renderer swaps one enum value, not the whole pipeline.

### 2.2 What should NOT be forced into a shared abstraction

- **GPU physics** (§1.3): recommend a **parallel** `physics_3d.wgsl` (own buffers, own orientation representation, own broad-phase structure) when 3D is actually pursued, sharing only the CPU-side dispatch/readback plumbing (`crates/gpu/src/physics_pipeline.rs`'s Rust-side buffer management), not the shader math itself. Forcing a single generic shader would produce worse code on both sides (2D performance regressions from unused 3D bookkeeping, or an under-specified 3D model trying to reuse 2D assumptions).
- **The SDF skin shape model** (§1.4): recommend a parallel 3D shape/rendering path (3D SDF primitives or a mesh renderer), not a generalization of the 2D one. A 2D capsule SDF and a 3D capsule SDF are different enough (different distance functions, different parameter counts) that sharing an abstraction would cost more than it saves.
- **The spatial index** (§1.5): recommend implementing `SpatialIndex` for an `Octree` type as a genuinely separate implementation (mirroring `Quadtree`'s existing shape) once 3D is pursued, not a templated `Tree<N>` — the subdivision math differs enough (8-way vs. 4-way, different node-capacity heuristics) that a shared generic would be more complex than two concrete implementations, the same lesson §1.5 already states.

### 2.3 Body Graph / physics-interface design

The Body Graph (topology, §1.6) needs no interface changes. The *physics interface* — the boundary between "the graph says segment N connects to segment N+1" and "there is a `Spring` entity linking two `ParticleNode` positions" — already goes through `organisms::compile_segment`/`growth_system`'s spring-spawning logic, which constructs a `Spring { node_a, node_b, ... }` from two entity references, never touching `Vec2` directly except to compute `rest_length`/`stiffness` from segment type. This interface is already dimension-agnostic in the same accidental way §1.6 describes: the graph-to-physics boundary passes entities and scalars, not coordinates. No redesign is needed here either — it inherits §1.6's finding for free.

---

## 3. Migration Strategy

A phased plan, at the same resolution level `PHASE4_ROADMAP.md` itself uses for its own epics — **not implementation-ready milestones**, since actual 3D implementation is explicitly out of this document's scope.

| Phase | Goal | Depends on | Risk |
|---|---|---|---|
| **3D-M1** | Generic CPU physics: parametrize `ParticleNode`/`Spring` over a `VectorSpace`-like trait bound; existing 2D behavior must be bit-identical (regression tests same-seed-same-output). | None | Low |
| **3D-M2** | Renderer projection-mode abstraction (§2.1's third bullet) — still 2D-only in practice, just removes the hardcoded assumption. | None | Low |
| **3D-M3** | Design (not implement) the 3D orientation representation GPU physics needs (§1.3's point 2) — this is its own audit-and-design sub-effort, likely as large as this document, since "what represents a bone's 3D orientation" (quaternion? per-node local frame? something else?) has real tradeoffs this document does not resolve. | 3D-M1 | Medium (design risk, not code risk) |
| **3D-M4** | Implement `physics_3d.wgsl` as a parallel shader per §2.2, once 3D-M3's design is settled. | 3D-M3 | High (the largest single implementation effort in this whole migration) |
| **3D-M5** | Implement `Octree`/3D `SpatialIndex` per §2.2. | 3D-M4 (3D positions must exist to index) | Medium |
| **3D-M6** | Implement a 3D shape/rendering path (3D SDF primitives or mesh renderer) per §2.2. | 3D-M4 | High |
| **3D-M7** | Update `organisms::spawning`'s materialization layer (§1.6) to embed body positions in 3D instead of 2D — the one place the developmental graph's abstraction boundary is actually crossed. | 3D-M1, 3D-M4 | Medium |

**This table is not an authorization to implement any of the above.** Per this document's own scope (§0), it exists so that *if* a future phase decides to pursue 3D, the decision-makers at that time have an accurate map of what's actually required — including the honest, unflattering parts (GPU physics and rendering are large parallel rewrites, not generalizations) — rather than discovering it mid-implementation.

---

## 4. Risk Assessment

| Risk | Mitigation |
|---|---|
| Treating "3D readiness" as "make `Vec2` generic" and stopping there | §1.3/§1.4/§1.5 explicitly document why this undersells the real GPU physics/rendering/spatial-index effort |
| Retrofitting a shared abstraction onto GPU physics or the SDF renderer, producing worse code on both the 2D and hypothetical 3D sides | §2.2 explicitly recommends parallel implementations instead, for both layers |
| A future contributor assuming the developmental graph needs 3D-specific changes | §1.6/§2.3 document that it already doesn't, so this should be a non-event when 3D-M7 is reached |
| This document going stale if `common::Vec2`'s 311 call sites grow substantially before any 3D work begins | Not mitigated — this is a point-in-time audit; a future 3D effort should re-run the call-site count before relying on this document's specific numbers |

## 5. Executive Summary

Three findings matter most: (1) the developmental graph topology built this phase (P4-F1 onward) is already dimension-agnostic, by accident of an unrelated Phase 3 design choice (position-as-index) — this is genuinely good news requiring no design work. (2) GPU physics and the SDF rendering shape model are the two layers where "dimension-independent" is not an honest goal — they need parallel 3D implementations, and a design that pretends otherwise would be wrong. (3) The CPU physics integrator and the rendering projection matrix are the two layers where a real, low-risk generalization is achievable now, independent of any decision to actually pursue 3D later.

**This document is audit and design only.** No 3D implementation work is authorized or has been started by it, per its own stated scope.
