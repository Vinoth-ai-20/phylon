# Phase 8 — Native 3D Scientific Simulation Engine

## Planning Program: Repository Audit → Architecture → Migration Roadmap → Epics

**Status: PLANNING COMPLETE. NO CODE HAS BEEN CHANGED IN THIS PHASE. Implementation has not begun and must not begin until this document is explicitly approved.**

This document was produced by an audit-first, multi-angle investigation (5 independent parallel investigations across physics/ECS/spatial, rendering/GPU/shaders, biology/evo-devo, UI/interaction, and serialization/testing — each reading the real code, not guessing) followed by architecture design, internal self-critique, a migration strategy, and a dependency-ordered epic roadmap. Every claim below traces to a specific file and line found during the audit; nothing here is invented or assumed.

---

## 1. Executive Summary

**What Phase 8 actually is.** Phylon simulates organisms as spring-mass particle bodies grown procedurally by a genome→CPPN→developmental-graph pipeline, in a 2D world, rendered via a 2-pass SDF "metaball" technique. Phase 8 is not a renderer upgrade — it is a genuine dimensional migration touching the physics substrate, the body-plan growth algorithm, the GPU compute layer, the spatial-indexing layer, the sensory model, and the entire rendering pipeline, while the genome/development/physiology/evolution/neural layers turn out to be **already dimension-agnostic** and require no change at all. That asymmetry — some subsystems untouched, others requiring ground-up redesign — is the central finding of the audit and shapes the entire migration strategy.

**Migration philosophy.** Never touch a system the audit didn't prove needs touching. Land one compiling, testable, reversible milestone at a time. Do the cheap, safe, high-value thing first (a real 3D camera and a mesh-based renderer, with organisms still growing in a Z=0 plane) before the hard, risky thing (true 3D growth orientation, volumetric diffusion). Treat anything that changes the simulation's scientific meaning (what "bilateral symmetry" means in 3D) or its visual identity (abandoning the SDF metaball look) as a decision requiring explicit human sign-off, not something an autonomous pass decides unilaterally.

**Highest-risk systems** (in order): (1) the GPU physics broad-phase grid, which naively extended to 3D is a **~128× memory blowup** at equal per-axis resolution; (2) the world-space GPU diffusion field, which naively made volumetric is a **~256× memory/bandwidth blowup**; (3) the SDF organism-skin renderer, whose 2-pass accumulate/threshold "metaball" technique has no 3D equivalent and must become a different rendering algorithm entirely; (4) the growth system's single-scalar `heading: f32`, which cannot represent 3D direction at all (there is no natural generalization of `heading.cos()/heading.sin()` to 3D) and forces a real scientific decision about what "bilateral symmetry" means once embedded in 3D space.

**Lowest-risk systems** (confirmed dimension-agnostic by direct code reading, zero changes needed): `DevelopmentalGraph` topology, Hox/segment-identity decoding (`genetics::develop_at_position`), intra-organism morphogen diffusion (graph-edge based, not spatial), physiology (`transport_system`/`endocrine_diffusion_system`, also graph-edge based), the CTRNN brain itself, evolution/lineage tracking, most of reproduction, all of analytics, the entire docking/workspace/layout UI system, and the graph-canvas-based Neural/GRN/HOX Viewer panels.

**Expected timeline.** This is a multi-quarter program for a small team, not a single session's work — the roadmap below sequences 13 epics across roughly 4 tiers of dependency, with the first tier (camera + mesh renderer + Vec3 foundation, organisms still in a Z=0 plane) being the fastest path to a genuinely useful, low-risk 3D milestone, and the hardest tier (true 3D growth orientation, 3D GPU physics, 3D vision) requiring real research-grade design work each.

**Success criteria for Phase 8 as a whole**: determinism preserved (every existing `*_is_deterministic_for_a_given_seed` test still passes, plus new 3D-specific ones); no scientific model silently simplified (every biology system's audit-confirmed dimension-agnostic status is preserved, and the one real scientific decision — 3D bilateral symmetry — is made explicitly and documented, not implicitly); the repository compiles and passes its full test suite at every milestone boundary; no milestone is irreversible; performance is measured before any architecture change that trades memory/compute for capability (per the project's own standing "profile before optimizing" rule, now applied to *architecture* choices, not just optimizations).

---

## 2. Repository Dependency Graph

```
                     ┌─────────────────────────────────────────────────┐
                     │                    app (composition root)        │
                     └───────────────────────┬───────────────────────────┘
                                              │ depends on everything
        ┌───────────────┬───────────────┬────┴────┬───────────────┬──────────────┐
        ▼               ▼               ▼         ▼               ▼              ▼
      ui            rendering          gpu     storage         analytics     research/network
        │               │               │         │               │
        │  (2D graph-   │ (SDF/debug/   │ (physics/│ (snapshot/    │ (metrics,
        │   canvas,     │  field         │ diffusion│  replay,      │  lineage,
        │   docking —   │  renderers —   │ /brain   │  bincode-     │  colony
        │   UNCHANGED)  │  MIGRATION     │  compute │  encoded      │  graph —
        │               │  TARGET)       │  pipelines│  Vec2 —      │  UNCHANGED)
        │               │               │  — MIGR- │  MIGRATION)   │
        │               │               │  ATION)  │               │
        └───────┬───────┴───────┬───────┴────┬─────┘               │
                ▼               ▼            ▼                     │
          spatial (UniformGrid/SpatialHash/Quadtree — MIGRATION)   │
                ▼                                                   │
          physics (ParticleNode/Spring — MIGRATION, math itself     │
                    is dimension-agnostic)                          │
                ▼                                                   │
          organisms (DevelopmentalGraph UNCHANGED; GrowthState      │
                      heading/placement — MIGRATION)                │
                ▼                                                   │
          genetics (Hox/CPPN decode — UNCHANGED, confirmed 1D/      │
                     positional, no spatial embedding at all)       │
                ▼                                                   │
          metabolism / brain / behavior / sensing / evolution /     │
          reproduction / ecology / diffusion / environment          │
          (mostly UNCHANGED — sensing's vision cone is the one      │
           real exception, MIGRATION; diffusion's world-space grid  │
           is a MIGRATION candidate but recommended to stay a       │
           plane, not go volumetric — see §9)                       │
                ▼                                                   │
          common (Vec2 → introduce Vec3 alongside, foundation)  ────┘
```

### Dependency classification

| Subsystem | Depends on (hard) | Depends on (soft) | Circular? | Migration tier |
|---|---|---|---|---|
| `common` | — | — | No | Tier 0 (foundation) |
| `physics` | `common` | — | No | Tier 0 |
| `spatial` | `common` | — | No | Tier 0 |
| `gpu` (physics/diffusion/brain compute) | `common`, `physics` | `spatial` (broad-phase concept) | No | Tier 2 (hardest — GPU buffer layout) |
| `organisms` | `common`, `physics`, `genetics`, `metabolism` | `spatial` | No | Tier 3 (growth orientation redesign) |
| `genetics` | `common` (barely — only for `Vec2` in a couple of CPPN input contexts, not body-plan decode itself) | — | No | Tier 0 (no change) |
| `sensing` | `common`, `physics`, `spatial`, `diffusion` | — | No | Tier 3 (vision-cone redesign) |
| `diffusion` / `gpu::diffusion_pipeline` | `common` | — | No | Tier 2 (optional — recommend staying planar) |
| `metabolism`, `brain`, `behavior`, `evolution`, `reproduction`, `ecology`, `environment` | `common`, `physics` (positions only) | — | No | Tier 0-1 (mechanical `Vec2`→`Vec3` at call sites only) |
| `rendering` | `common`, `gpu` | — | No | Tier 1 (camera) + Tier 2 (organism renderer rewrite) |
| `ui` | `common`, `world`, everything it displays | — | No | Tier 1 (camera/picking call sites); docking/graph-canvas subsystems: **no change** |
| `storage` | `common`, `physics`, `organisms`, `brain` (serialized types) | — | No | Tier 4 (schema bump, last — depends on every type it serializes being final) |
| `analytics`, `evolution` (lineage) | (topological only) | — | No | Tier 0 (no change) |
| `tests`, `benchmarks` | everything | — | No | Continuous (updated alongside each tier, not a separate tier) |
| CI | (build tooling only) | — | No | Tier 0 (should be addressed early — see Epic 8.12) |

**No circular dependencies were found** — the existing crate-dependency-graph discipline (documented in `docs/reference/crate_graph.md`, corrected during Phase 7 W1e) holds up under this audit; Phase 8 does not need to break any cycle, only extend types along the existing DAG.

**Blocking systems**: `common::Vec3`'s introduction (Tier 0) blocks everything downstream. The GPU physics buffer layout (Tier 2) blocks any 3D physics behavior. The growth-orientation redesign (Tier 3) blocks true 3D body plans (until it lands, organisms can exist and render in 3D space but still *grow* as if embedded in a Z=0 plane — a deliberate, valid, and useful intermediate state, not a bug).

---

## 3. Architecture Decision Records

### ADR-P8-01 — Introduce `common::Vec3` alongside `Vec2`; do not replace `Vec2` project-wide

- **Context**: `common::Vec2` (a re-export of `glam::Vec2`) is used 359 times across 49 files, including many contexts that are legitimately 2D forever (egui graph-canvas layouts for Neural/GRN Viewer, UI-only concerns like `GraphViewState`, `CameraBookmark`'s 2D-era shape before this migration).
- **Decision**: add `pub use glam::Vec3;` to `common`. Migrate simulation-space position/velocity/force fields (`physics::ParticleNode`, `organisms::GrowthState`, ecology entity positions) to `Vec3`. Leave UI-internal 2D graph-canvas types on `Vec2` — they represent abstract 2D layouts (node-link graphs), not simulation space, and migrating them would be actively wrong (a brain's CTRNN graph layout has no "3D" meaning).
- **Alternatives considered**: (a) a generic `Position<const N: usize>` — rejected, adds real complexity (const-generic vector math, trait bound sprawl) for no benefit since Phylon will never support N≠3 for simulation space; (b) replace `Vec2` everywhere including UI graph canvases — rejected, conflates simulation-space and UI-layout-space, which the audit confirmed are already cleanly separated (the graph-canvas module's own doc comment: "never owns WHAT a node/edge means").
- **Risk**: none — purely additive.
- **Rollback**: trivial (remove the re-export; nothing depends on it until call sites are migrated).
- **Future implications**: this is the one and only place a future N-dimensional experiment (unlikely, but worth naming) would need to start from.

### ADR-P8-02 — One `Camera3d` type; orbit and fly are controllers over it, not separate camera representations

- **Context**: the audit found the camera today is `camera_pos: Vec2` + `camera_zoom: f32` on `WorkbenchState`, with the corresponding orthographic projection matrix **hand-derived independently 4 times in Rust and once inline in WGSL** (`sdf_skin.rs` ×2, `debug.rs`, `app.rs`'s picking code, `field_overlay.wgsl`), plus **3 separate, uncoordinated copies of the screen↔world unproject transform** (`app.rs::pick_entity`, and two independent closures inside `viewport.rs`). This is exactly the kind of "single canonical pathway" violation the project's own `ADR-W0-01` (selection state) and `ADR-W0-02` (recent items) precedents exist to prevent.
- **Decision**: introduce one `Camera3d { position: Vec3, orientation: Quat, fov_y: f32, near: f32, far: f32 }`, owned by `WorkbenchState` (replacing `camera_pos`/`camera_zoom`), with exactly one method producing `view_proj: Mat4` and exactly one method producing a world-space ray from a screen-space point (`screen_to_ray`). Every renderer and every input controller consumes this one object; none derives its own projection matrix again.
  - `OrbitController` — arcball orbit around a focus point + distance (the default mode; matches the Blender/scientific-tool convention this project already follows elsewhere).
  - `FlyController` — WASD + mouselook, direct position/orientation control (opt-in, for free exploration).
  - Both controllers only ever write into the same `Camera3d` fields — there is no second camera state.
- **Alternatives considered**: (a) keep 2D `camera_pos`/`camera_zoom` and bolt on a separate 3D camera for a "3D mode" — rejected, recreates exactly the dual-pathway problem this ADR exists to prevent, and the UI audit found no evidence any 2D-only camera mode needs to survive Phase 8; (b) a full 6DOF-only camera with no orbit concept — rejected, scientific inspection workflows (focus on an organism, orbit around it) are a stated interaction requirement and are better served by an explicit orbit primitive than by asking users to fly precisely around a point.
- **Risk**: touches ~12 call sites across `app/src/events.rs`, `app/src/render.rs`, `ui/src/state.rs`, `ui/src/plugins/viewport.rs` (per the UI audit's own enumeration) — wide fan-out, but each site's change is mechanical once `Camera3d`'s API exists.
- **Rollback**: keep `Camera3d` behind a feature flag during migration is not recommended (violates "no parallel architectures"); instead, land it as one atomic milestone (Epic 8.1) with the old 2D camera fields removed in the same milestone, verified by a full interactive smoke test before merge.
- **Future implications**: this is the single object any future VR/stereo-camera or multi-viewport feature would extend, not replace.

### ADR-P8-03 — Organism rendering becomes mesh-based capsule instancing with a real depth buffer, PBR shading, and standard shadow mapping; the 2-pass SDF metaball technique is retired

- **Context**: the current renderer (`SdfSkinRenderer`) is a 2-pass technique — additively accumulate a `capsule_sdf` density value per bone into an offscreen 2D texture with no depth test, then threshold/anti-alias in a composite pass. This is confirmed to be a fundamentally planar technique (order-independent additive blending only works because there is no "behind" in 2D) with **no depth buffer anywhere in the codebase today**. The explicit Phase 8 requirements list demands mesh rendering, PBR, lighting, shadows, skeletal animation, LOD, and instancing — all of which are solved, well-understood problems in a mesh-rasterization pipeline and are not natural fits for a raymarched-SDF or metaball-accumulation approach.
- **Alternatives evaluated**:
  1. **Mesh-based capsule/rounded-cylinder instancing** (chosen) — a shared, tiny, procedurally-generated capsule mesh (hemisphere caps + cylinder body), instanced per bone with a per-instance `(pos_a: Vec3, pos_b: Vec3, radius: f32, color, health)` buffer — nearly the same *shape* as today's `SdfBoneInstance`, just with `Vec3` endpoints instead of `Vec2`. The vertex shader orients the mesh per-instance via a look-at construction from `pos_a` to `pos_b` (a standard billboard-cylinder-alignment technique), so **no per-instance rotation/quaternion needs to be stored** — this materially reduces the instance-format migration cost the initial audit estimated, since the actual instance *data* barely changes shape; only the *shader algorithm* (oriented rasterized mesh vs. metaball density accumulation) changes.
     - Native support for: real depth buffer (occlusion falls out for free), standard shadow mapping, PBR (standard normal/roughness/metallic shading on the capsule surface), LOD (reduce radial mesh segments, or billboard-impostor beyond a distance threshold), instancing (this *is* instancing), skeletal animation (the "skeleton" already exists — it's the spring/bone graph itself; capsule instances simply follow bone transforms each frame, which is what already happens today), picking (ray-vs-capsule, a solved, cheap intersection test), and clipping planes (a trivial per-fragment `discard` test, natural with rasterization+depth — much harder to do cleanly with additive blending).
     - Cost: this is the **single largest visual-identity change in the entire migration** — organisms will look like rounded, faceted-at-the-joints creatures rather than smoothly blended metaballs, at least until an optional joint-smoothing refinement (below) is built.
  2. **Raymarched 3D SDF** (rejected) — preserves the organic blobby look most faithfully, but: shadows require a second expensive raymarch pass (shadow-raymarching) rather than standard shadow maps; there is no natural mesh to skin, so "skeletal animation" as normally understood doesn't apply — the primitives already are the skeleton, which sounds elegant but means every animation/tooling convention this project might want later (glTF import, standard DCC-tool workflows) doesn't apply; LOD has no standard technique (you cannot decimate an SDF the way you decimate a mesh); per-pixel cost scales with the number of nearby primitives sampled per raymarch step, which needs its own bespoke spatial acceleration structure *inside the shader* — a harder, more novel piece of shader engineering than a standard rasterization pipeline, for a project whose team is not shown by this audit to already have raymarching expertise banked (no existing raymarch code exists anywhere in the codebase to build from).
  3. **Hybrid (rasterized capsules + a post-process or shader-level joint-blend to fake metaball continuity)** — accepted as a **future, non-blocking refinement**, not part of the core migration: once capsules are rasterizing correctly with depth/shadows/PBR, a follow-on epic can add small-radius overlap geometry or a screen-space blend limited to organism silhouette edges to soften visible joint seams, without touching the core rendering architecture again.
- **Decision**: mesh-based capsule instancing (option 1), with joint-blending as an explicitly separate future epic (not blocking, not assumed necessary — build the mesh pipeline first, then evaluate whether the joint seams are actually a problem worth solving, per the "measure before changing" discipline).
- **Risk**: **High — user-facing visual identity change.** This ADR is flagged for explicit human sign-off before Epic 8.2 begins; it is not something an autonomous pass should decide unilaterally on the project's behalf, since it changes what the simulation *looks like* to every researcher who has used it, independent of any engineering merit.
- **Rollback**: the old `SdfSkinRenderer` code should be deleted only after the new renderer is verified end-to-end (screenshots compared, interactive smoke test passed) — not deleted preemptively "to keep things clean."
- **Future implications**: this decision also resolves the debug-renderer and field-renderer migrations for free — `DebugRenderer`'s badges become camera-facing billboards (a smaller version of the same oriented-quad technique), and `FieldRenderer` becomes a plane-slice sampler re-using its existing full-screen-quad-plus-2D-texture technique nearly unchanged (see ADR-P8-05).

### ADR-P8-04 — GPU physics broad-phase becomes a spatial hash, not a dense 3D grid

- **Context**: the current GPU broad-phase (`physics.wgsl`) is a dense `128×128` grid (`GRID_DIM=128`, flat-indexed). A naive 3D extension (`128×128×128`) is a **~128× memory increase** for equal per-axis resolution — the audit flags this explicitly as a real design decision, not a mechanical port.
- **Decision**: replace the dense grid with a **GPU-side spatial hash** (extending the same 2D→3D mixing-function change `crates/spatial::SpatialHash` already needs on the CPU side, so the CPU and GPU broad-phase share one conceptual design instead of two independent ones) — a fixed-size hash table sized for the *expected* organism count, not the *volume* of the world, so memory scales with population, not with world size cubed.
- **Alternatives considered**: (a) dense 3D grid at reduced per-axis resolution to cap memory — rejected as a default, since it directly degrades broad-phase precision (more false-positive neighbor candidates, more narrow-phase work) as a side effect of a memory constraint, trading one performance problem for another without measurement; (b) a GPU BVH — rejected as unnecessary complexity for this population scale (thousands, not millions, of nodes) and a much larger engineering lift than a hash table.
- **Risk**: Medium — this is new GPU shader engineering (atomic-bucket insertion into a hash table has known techniques but is more complex than a dense grid's direct indexing).
- **Rollback**: land behind a benchmark (extend `crates/benchmarks` with a GPU broad-phase benchmark, mirroring the CPU-side `foraging_scaling` benchmark this project already built in Phase 7 W7) before/after comparison at multiple population sizes, so a regression is caught by data, not assumed.
- **Future implications**: unifies the CPU (`crates/spatial`) and GPU broad-phase designs conceptually, which should make a future "spatial index" ADR easier to reason about as one concept instead of two.

### ADR-P8-05 — World-space diffusion field stays a 2D (or few-layer) plane by default; true volumetric diffusion is an explicit, separately-measured future epic, not part of Phase 8's default architecture

- **Context**: the audit found the world-space GPU diffusion field (pheromones/energy/O2/CO2/morphogen) is a `256×256×5-layer` texture array. A naive volumetric extension (`256³×5`) is a **~256× memory/bandwidth increase**, and the per-tick CPU readback this pipeline already performs scales the same way — this is flagged as the single most compute-risky item in the entire audit.
- **Decision**: keep the diffusion field as a small number of discrete horizontal layers (e.g. a "ground plane" concentration field, or 3-5 fixed height-bands) rather than a full volumetric grid, for Phase 8. Organisms sample whichever height-band their position falls nearest to. This preserves the field's actual purpose (environmental gradient sensing) without paying a 256× cost nobody has measured the need for.
- **Alternatives considered**: (a) full volumetric 3D texture — rejected by default, exactly the "never optimize/redesign without measurement" violation this project's own standing rules prohibit; kept as an explicit optional Phase 9 candidate, gated on real profiling once organisms actually inhabit meaningful vertical space (which itself depends on Epic 8.5's growth-orientation redesign landing first — there is no vertical body-plan variation to sense until then); (b) sparse/windowed volumetric diffusion (only allocate cells near active organisms) — a legitimate future option, more complex than the layered-plane approach, deferred until the layered approach is proven insufficient.
- **Risk**: Low for the chosen option (mechanical extension of existing 2D infrastructure); the rejected volumetric option is explicitly named High-risk-if-attempted-without-measurement so a future contributor doesn't reach for it by default.
- **Rollback**: trivial — this is the conservative default, nothing to roll back from.
- **Future implications**: if Phase 9 ever needs true volumetric diffusion, this ADR's own reasoning (measure first, layered-plane is the fallback) should be revisited with real profiling data, not superseded by assumption.

### ADR-P8-06 — 3D bilateral symmetry is implemented as strict mirror-symmetry about a sagittal plane (direction-of-travel + a body-fixed dorsal "up" vector); radial symmetry is an explicitly separate, optional future body-plan variant, not bundled into this migration

- **Context**: the biology audit found the current fin-placement math (`perp = Vec2::new(-dir.y, dir.x)`) computes the *unique* in-plane perpendicular to the heading direction — a construction that is **only well-defined in 2D**. In 3D, "perpendicular to a direction" is an entire circle of vectors, not one, so this code has no direct generalization; a body-fixed reference (a dorsal/ventral "up" axis) must be introduced to disambiguate which perpendicular direction is "left" vs. "right." The audit explicitly frames this as a scientific-correctness decision, not just an engineering one: does 3D branching mean strict bilaterian mirror-symmetry (the evident intent of today's "exactly 2 fins per branch point" code), or should radial arrangements (3/4/6-way appendages) also be representable?
- **Decision**: `GrowthState` gains an explicit per-organism `dorsal: Vec3` (or an equivalent orientation quaternion carrying it), maintained alongside the existing direction-of-travel, so "left fin" and "right fin" are defined as `±(dorsal × forward)` (a proper 3D cross product, now well-defined because a second reference vector exists) — this is the direct, meaning-preserving 3D generalization of the current model's evident intent (strict bilaterian symmetry), not a new biological hypothesis. Radial symmetry (N-fold appendage arrangements) is explicitly **not** built in this migration — it would be a new evolvable trait (a "symmetry-type" gene) representing a different body-plan hypothesis than bilaterian, and bundling it into a dimensional-migration epic would silently expand scope into new biology, which Phase 8's own governing brief prohibits ("no biological shortcut should compromise scientific correctness" cuts both ways — don't invent new biology either).
- **Alternatives considered**: (a) pick an arbitrary global "up" (e.g. world +Z) instead of a per-organism dorsal vector — rejected, this would make "bilateral" depend on which way an organism happens to be facing relative to the world, not an intrinsic body-plan property, a real scientific regression from the current model's (2D, but organism-relative) definition; (b) build radial symmetry now since 3D makes it possible — rejected as scope creep, explicitly deferred to a named future epic instead of silently expanded into this one.
- **Risk**: Medium — touches `growth_system`, the same code Phase 7's W5a milestone carefully extracted into `wire_brain_for_completed_organism`/`decode_next_segment`/`spawn_grown_segment` with an existing 11-test safety net; this migration must extend that same test suite with 3D-specific determinism/correctness tests, not bypass it.
- **Rollback**: gated behind the existing `growth_system_*` test suite passing unchanged for every 2D-equivalent case (an organism grown with `dorsal` always equal to a fixed world axis should reproduce today's 2D behavior exactly, as a regression check).
- **Future implications**: a future "radial symmetry" epic (candidate for Phase 9) would add a new gene/trait and a new branch-count/branch-angle-distribution model, built on top of this ADR's `dorsal` vector, not replacing it.

### ADR-P8-07 — Vision/sensing becomes an azimuth×elevation binned cone, preserving the existing "cheap heuristic over expensive raycasting" performance philosophy

- **Context**: `HeadVision`'s current 3-bin (Left/Center/Right) model is built on a *signed 2D angle* between two `Vec2`s (`forward.angle_to(dir)`), which has no direct 3D analogue (a signed angle between two 3D vectors is only meaningful relative to a rotation axis, which the current code doesn't have). The system's own doc comment explicitly states this binned approach was chosen *because* true per-agent raycasting is too expensive at population scale — this design philosophy should be preserved, not abandoned, when redesigning for 3D.
- **Decision**: extend the single azimuth bin (Left/Center/Right) into a small azimuth×elevation grid (e.g. 3×3), computed via the same `dorsal`/`forward` body-frame ADR-P8-06 introduces (azimuth = angle in the forward-right plane, elevation = angle in the forward-up plane) — still a cheap binned heuristic, still O(candidates-in-range), not a raycast.
- **Alternatives considered**: (a) true raycasting now that a 3D engine exists anyway — rejected, no evidence the original performance concern has gone away (population scale is unchanged by this migration), and this would be exactly the kind of unmeasured architecture change the project's standing rules prohibit; (b) keep only azimuth bins (no elevation) — rejected as an unnecessary loss of information once bodies have real vertical extent (post ADR-P8-06), though this is the cheaper fallback if the 3×3 grid proves to cost more than the population-scale budget allows (to be measured, not assumed).
- **Risk**: Low-medium — isolated to `crates/sensing`, with `SensoryState.inputs` (the flat float vector the CTRNN brain consumes) simply gaining more slots; the brain itself needs no change (confirmed dimension-agnostic by the biology audit).
- **Rollback**: straightforward — sensing is a leaf-ish crate relative to the brain (brain only ever sees flat floats, doesn't know what they mean).
- **Future implications**: none beyond this crate; sets the pattern for any future sensory-channel addition (cheap binned heuristic first, raycast only if measurement proves it necessary).

### ADR-P8-08 — Save-file and replay-file schema break is expected and accepted, following the project's own existing precedent; no migration tooling is built

- **Context**: the serialization audit found `SchemaVersion` has been bumped 3 times already (v1→4) with **no migration path ever built** — the documented, consistent precedent is "a version bump means old files stop loading," not "old files get migrated forward."
- **Decision**: bump `SchemaVersion` again for the `Vec2`→`Vec3` position change (and again if/when `GrowthState.heading`→orientation lands), following the exact same precedent as the prior 3 bumps. No migration tooling is built, matching existing project convention. This must be communicated to users/researchers explicitly (old `.phylon` save files and `.phylon-replay` bundles will stop loading) rather than silently discovered.
- **Alternatives considered**: building a real migration path (deserialize old schema, re-embed a default Z=0, re-serialize) — a legitimate option, more user-friendly, but a **net-new capability this project has never built for any of its 3 prior schema bumps**; introducing it now, only for this bump, would be inconsistent and is not requested by any stated Phase 8 requirement. Left as a candidate suggestion for whoever owns backward-compatibility policy, not decided unilaterally here.
- **Risk**: Low (process risk only) — precedented behavior, but must be communicated, not hidden.
- **Rollback**: N/A — this is a documentation/communication decision, not a code change.
- **Future implications**: if backward-compatible migration ever becomes a real product requirement, it should be built as its own initiative applying to *all* schema bumps retroactively, not bolted on ad hoc for this one.

### ADR-P8-09 — CI gains explicit, documented GPU-testing posture before the 3D renderer rewrite lands

- **Context**: the audit found CI (`ubuntu-latest`, single job) runs `cargo test --all` unconditionally with **no GPU-specific handling anywhere** — no `#[ignore]` gates, no documented software-rasterizer fallback, no headless-display setup step. This apparently works today only because of undocumented reliance on Mesa/llvmpipe's software Vulkan/GL fallback on the GitHub-hosted runner.
- **Decision**: before Epic 8.2 (the renderer rewrite) lands, add an explicit CI step documenting and pinning this reliance (e.g. explicit Mesa/llvmpipe package install rather than hoping it's preinstalled, and a comment explaining why), and audit whether any *new* GPU-dependent tests this migration adds (e.g. a render-output screenshot comparison) need explicit `#[ignore]` gating for CI vs. local-only execution.
- **Alternatives considered**: doing nothing and hoping the same implicit behavior continues to work for a substantially more complex renderer — rejected, this is exactly the kind of undocumented assumption the project's "no silent assumptions" rule prohibits, and a 3D renderer with a real depth buffer, shadow maps, and PBR shading is meaningfully more likely to expose software-rasterizer limitations than the current simple 2-pass 2D technique.
- **Risk**: Low effort, but High if skipped (a broken, undiagnosed CI failure mid-migration would stall every subsequent epic).
- **Rollback**: N/A — additive documentation/CI-config work.
- **Future implications**: establishes the project's first explicit GPU-testing convention, which any future rendering work should follow.

---

## 4. 3D Engine Architecture

### Scene representation

No new "scene graph" abstraction is introduced. Phylon's scene *is* the `bevy_ecs::World` — organisms, food, minerals, corpses are already entities with a position component; a 3D migration widens that component, it does not add a parallel scene-graph layer (which would violate "no duplicated systems / no parallel architectures"). The one new resource is `Camera3d` (ADR-P8-02) and a `RenderInstances` gather step (already precedented by Phase 7 W2d's `world_instances.rs` pattern — this exact "gather this frame's render instances from World state" module is where 3D instance construction slots in, unchanged in *shape*, just wider vectors).

### Camera architecture

One `Camera3d` (ADR-P8-02), two controllers (`OrbitController` default, `FlyController` opt-in), one `view_proj()` method, one `screen_to_ray()` method. Every renderer (organism, debug, field) and every interaction system (picking, box-select, gizmos) consumes these two methods and none other — this directly resolves the audit's finding of 6 duplicated projection-matrix derivations and 3 duplicated screen↔world transforms.

### Rendering abstraction

A thin `RenderPass` trait is **not** introduced — the audit found the existing pass sequence (background/heatmap → organism → debug → highlight → egui → present) is already a clear, linear, well-understood sequence in `app/src/render.rs`; adding an abstraction layer over 5-6 passes that don't need dynamic reordering would be exactly the kind of "unnecessary abstraction" the self-critique pass (§16) is instructed to search for. Each pass remains a plain Rust struct with a `render(&self, device, queue, view, ...)` method, as today.

### Mesh pipeline

Procedurally generated, not asset-imported (no glTF pipeline needed for Phase 8 — organisms are entirely procedural, there is no authored art). One shared low-poly capsule mesh (hemisphere caps + cylinder body, generated once at startup, stored as a single vertex/index buffer) is instanced per bone via the oriented-look-at vertex shader technique from ADR-P8-03. LOD is a per-instance mesh-detail selection (full capsule vs. a cheaper billboard impostor beyond a distance threshold), decided per-frame from camera distance, not a separate mesh asset per LOD tier (kept simple: 2 tiers, not a full LOD chain, until profiling says more are needed).

### Material pipeline

A single PBR material model (metallic/roughness) parameterized per-instance by the existing per-bone `color` (from `OrganismColor`) plus new scalar roughness/metallic values initially fixed to reasonable organic-material defaults (not evolvable — that would be new biology, out of scope). No material graph/shader-permutation system is introduced; one shader, one material model, matching the project's existing "no duplicated systems" discipline.

### Lighting

A single directional "sun" light (matching the existing day/night `GlobalAtmosphere.sunlight` value, which already drives the background clear-color tint — the same scalar now also drives light intensity, a natural, already-precedented reuse) plus a fixed low-intensity ambient term. No dynamic point-light system is needed for Phase 8 (no light-casting entities exist in this simulation's biology).

### Shadows

One cascaded or single-frustum shadow map from the directional sun light, rendered as an extra depth-only pass before the main organism pass. This is the first pass in the codebase to use a depth buffer at all (ADR-P8-03) — the shadow map and the main scene's own depth buffer are two separate depth textures, standard practice.

### Depth pipeline

A new `wgpu::Texture` with `DEPTH_STENCIL` usage is created in `init_gpu`/`init_gpu_headless` (today, neither exists — confirmed by the audit) sized to the swapchain/viewport. The organism, debug, and shadow passes write to it; the egui pass continues to omit it entirely (ADR confirmed by the rendering audit: egui's pass is already fully separated via its own encoder and `LoadOp::Load`, so it needs no change — this is the one piece of the whole migration the audit found requires **zero** modification).

### Selection rendering

Today's white/green outline highlight (a second SDF pass at a larger radius) becomes: render selected/hovered organisms' capsules a second time with a slightly inflated radius and a flat unlit color, depth-tested *behind* the main pass (so the outline only shows where it extends past the silhouette) — a standard, well-known "inverted hull" outline technique, natural in a depth-buffered mesh pipeline, awkward in the old accumulate-blend model.

### Debug rendering

Health/disease/category badges become camera-facing billboards (quads that always face the camera, computed in the vertex shader from `Camera3d`'s orientation) rather than 2D screen-space AABB-quads — the same instance-gather code path (`organism_visuals.rs`, Phase 7 W2a) feeds this, just producing `Vec3` positions instead of `Vec2`.

### Scientific overlays

Built on the same capsule/billboard primitives: vector-field glyphs (arrows) are billboarded or camera-facing instanced meshes; heatmap plane-slices reuse `FieldRenderer`'s existing full-screen-quad technique (ADR-P8-05) with the screen→world derivation now going through `Camera3d::screen_to_ray()` intersected with the chosen slice plane, instead of the old flat orthographic remap.

### Instancing

Already the current architecture's own pattern (`SdfBoneInstance`/`DebugInstance` are already instance buffers) — Phase 8 widens the vector fields, it does not introduce instancing as a new concept.

### Resource ownership

Unchanged from today: `GpuContext` (owned by `PhylonApp`) holds the device/queue/surface; each renderer struct (`OrganismRenderer` replacing `SdfSkinRenderer`, `DebugRenderer`, `FieldRenderer`, plus the new `ShadowRenderer`) is constructed once in `init_gpu` and owns its own pipelines/buffers/textures, exactly the existing pattern.

### Frame graph

Not introduced as a formal abstraction (same reasoning as "rendering abstraction" above) — the linear pass sequence in `app/src/render.rs` remains hand-ordered, now with the shadow pass inserted before the organism pass and the depth buffer threaded through background→organism→debug→highlight, egui still last and depth-less.

---

## 5. ECS Evolution Plan

| Component/field | Disposition |
|---|---|
| `physics::ParticleNode::{position, velocity, force}` | `Vec2` → `Vec3` (Tier 0/1) |
| `physics::Spring` (all scalar fields) | Unchanged — carries no vectors |
| `organisms::DevelopmentalGraph`/`DevelopmentalNode` | **Unchanged** — pure topology, confirmed dimension-agnostic |
| `organisms::GrowthState::current_pos` | `Vec2` → `Vec3` (mechanical) |
| `organisms::GrowthState::heading: f32` | **Redesigned** → `forward: Vec3` + `dorsal: Vec3` (ADR-P8-06), not a type swap |
| `genetics::*` (Hox/CPPN decode) | **Unchanged** — confirmed 1D/positional, no spatial embedding |
| `organisms::morphogen_field::MorphogenLevel` | **Unchanged** — graph-edge diffusion, not spatial |
| `metabolism::ChemicalEconomy` and physiology (`transport`/`endocrine`) | **Unchanged** — graph-edge based |
| `brain::*` (CTRNN) | **Unchanged** — flat float vectors only |
| `sensing::HeadVision`/`SensoryState` | **Redesigned** (ADR-P8-07) — azimuth×elevation bins replace the single 2D angle bin |
| `ecology::{FoodPellet, MineralPellet, Corpse}::position` | `Vec2` → `Vec3` (mechanical) |
| `evolution::*` (lineage, species) | **Unchanged** — purely genomic/topological |
| `reproduction::*` | Mechanical `Vec2`→`Vec3` at offset-placement/mate-distance call sites only |
| `spatial::{UniformGrid, SpatialHash}` | Mechanical extension (3rd axis in `cell_of`/hash mixing) |
| `spatial::Quadtree` | **Redesigned** → `Octree` (8 children, 3-axis quadrant test) |
| `ui::WorkbenchState::{camera_pos, camera_zoom}` | **Replaced** by `Camera3d` (ADR-P8-02) |
| `ui::CameraBookmark` | Extended: `position: Vec2` → `Vec3`, add `orientation: Quat` |
| `ui::graph_canvas::GraphViewState` and all Neural/GRN/HOX Viewer node-link layouts | **Unchanged** — confirmed abstract 2D layout space, unrelated to simulation dimensionality |
| `storage::SerializedVec2` and all snapshot/replay position fields | `Vec2`→`Vec3` shape, `SchemaVersion` bump (ADR-P8-08) |
| `analytics::*` | **Unchanged** — confirmed purely topological/statistical |

**Migration sequence**: `common::Vec3` → `physics::ParticleNode` → `organisms`/`ecology` position fields (mechanical) → `spatial` indices (mechanical, then Quadtree→Octree) → `gpu` compute buffers → `rendering` instance formats/camera → `ui::Camera3d` and interaction → `organisms::GrowthState` orientation (the real redesign, ADR-P8-06) → `sensing` (ADR-P8-07) → `storage` schema bump (last, once every serialized type's final shape is known).

**Compatibility strategy**: no dual-mode "2D or 3D" runtime switch is built (would be a parallel architecture, prohibited). During the migration window, organisms grow with `dorsal` fixed to a constant world axis (equivalent to today's 2D behavior, verified by the existing test suite reproducing identical results) until ADR-P8-06 lands for real; this is the deliberate, useful "2D-embedded-in-3D" intermediate state named in §1, not a compatibility shim to be ashamed of.

---

## 6. Biology Evolution Plan

**Preserved exactly, no simplification** (confirmed dimension-agnostic by direct code reading, not assumption): Body Graph topology (`DevelopmentalGraph`), Hox/segment-identity decoding, intra-organism morphogen diffusion, circulation/hormone transport (`transport_system`/`endocrine_diffusion_system`, both graph-edge based), immune system (`SegmentInfection`/`SegmentImmunity`, graph/topology-based per the Phase 4/5 physiology work, no spatial dependency found), neural systems (CTRNN brain, flat-float interface), evolution/speciation/lineage tracking.

**Genuinely redesigned, with an explicit scientific decision made and documented** (ADR-P8-06): the growth/development *placement* algorithm — `heading: f32` becomes `forward`/`dorsal: Vec3`, fin/branch placement becomes a proper 3D cross-product against a body-fixed dorsal axis, preserving the current model's evident bilaterian-symmetry intent rather than silently reinterpreting it. Radial symmetry is named as an explicit future body-plan hypothesis, not smuggled into this migration.

**Genuinely redesigned, preserving existing design philosophy** (ADR-P8-07): sensing/vision — the binned-cone heuristic (chosen for performance, not accuracy, per the sensing module's own doc comment) is extended to azimuth×elevation bins, keeping the same performance-first philosophy rather than escalating to true raycasting.

**No biological shortcut taken**: nowhere in this plan is a biological model simplified to make the 3D migration easier — every "unchanged" system was confirmed unchanged by evidence (the audit reading the actual decode/diffusion/physiology code), not assumed unchanged for convenience, and the two systems that do need redesign (growth placement, vision) are redesigned to preserve their existing scientific/design intent, not to approximate it away.

---

## 7. Physics Migration

**Dimension-independent, trivial swap** (confirmed by direct reading of the CPU physics crate): the Symplectic Euler integrator and Hooke's-law spring-force computation are pure `glam` vector algebra with no 2D-specific tricks — `Vec2` operations generalize to `Vec3` via the same operator overloads, no logic change needed.

**Broad phase**: CPU-side `spatial::UniformGrid`/`SpatialHash` get a mechanical 3rd-axis extension; `Quadtree` is redesigned into an `Octree` (ADR noted in §5, standard, well-precedented CS technique, low scientific risk, real engineering effort). GPU-side broad phase moves from a dense grid to a spatial hash (ADR-P8-04), explicitly to avoid a ~128× memory blowup.

**Narrow phase**: unchanged in kind (distance-based constraint/collision checks) — the math generalizes; the "unique 2D perpendicular" trick used for anisotropic fin drag (`vec2(-dir.y, dir.x)`) does **not** generalize and is redesigned against the same `dorsal`-vector body frame ADR-P8-06 introduces for growth, so both the growth-placement and physics-drag code share one consistent notion of a body's orientation rather than two independent, potentially-divergent ones.

**Constraints**: `ConstraintType::Rotational` is confirmed to be a *declared but never implemented* enum variant today (no CPU or GPU code branches on it distinctly from Elastic/Passive) — Phase 8 does not need to invent 3D rotational-constraint math for something that doesn't exist yet; if a future milestone implements it, that milestone inherits the `dorsal`-vector body frame as its natural foundation.

**Muscles/springs**: unchanged in representation (topology + scalars); actuation math (`muscle_actuation.wgsl`) is confirmed purely scalar (no vector math at all) and needs no change.

**Collisions**: the existing PBD-style distance-constraint projection is dimension-agnostic (confirmed by direct shader reading) and generalizes via wider vectors with no algorithmic change.

**Determinism strategy**: every new piece of 3D-specific logic (the hash-based GPU broad phase, the `dorsal`-vector fin-drag redesign) gets its own `*_is_deterministic_for_a_given_seed`-style test, matching the existing precedent this project already applies to `catastrophe_system`/`food_spawner_system` — determinism is verified per-change, not assumed to survive a redesign.

**GPU interaction**: `GpuParticleNode`'s `position`/`velocity`/`force` become `vec3<f32>` in WGSL, with the associated 16-byte-alignment consequence (WGSL pads `vec3` storage-buffer fields to 16 bytes, unlike `vec2`'s 8-byte layout) explicitly accounted for in the buffer-size/offset math, not discovered as a bug later. The dual `atomic_forces_x`/`atomic_forces_y` fixed-point accumulation buffers gain a third parallel `atomic_forces_z` buffer, with every one of the 4 call sites that touch the existing two buffers (`compute_forces`, `integrate`, `pbd_projection`, `apply_pbd`) updated in lockstep — enumerated explicitly in the audit, not left to be discovered mid-implementation.

---

## 8. Rendering Migration

Covered in full in ADR-P8-03 above. Summary of the decision: **mesh-based capsule/rounded-cylinder instancing, rasterized with a real depth buffer, standard PBR shading, and standard shadow mapping — the 2-pass SDF metaball technique is retired.** Raymarched SDF and a rasterize+post-process-blend hybrid were both evaluated and are documented as rejected/deferred respectively, with reasons. **This is the highest-visibility decision in the entire roadmap and requires explicit user sign-off before Epic 8.2 begins implementation** — it is a genuine visual-identity change, not a pure engineering upgrade, and the autonomous process deliberately does not treat "engineering is right" as sufficient license to decide a product's visual identity unilaterally.

---

## 9. Scientific Visualization Roadmap

- **3D heatmaps / diffusion / hormone / morphogen visualization**: plane-slice rendering (ADR-P8-05) — reuses `FieldRenderer`'s existing full-screen-quad + 2D-texture-sample technique almost unchanged, re-deriving the screen→world mapping from `Camera3d::screen_to_ray()` intersected against a user-movable slice plane. Low cost, high reuse of existing, working code.
- **Volume rendering**: explicitly deferred (ADR-P8-05) pending real measurement of whether plane-slices are insufficient for actual research workflows — not built speculatively.
- **Vector fields / flow visualization** (e.g. force vectors, circulation direction): new capability — small instanced arrow/glyph meshes, positioned and oriented per-sample from the relevant field (force, hormone-transport direction) — built on the same instancing infrastructure as organism rendering, not a separate system.
- **Cross-sections / clipping planes**: a per-fragment `discard`-based clip test in the organism/debug shaders, driven by a draggable plane gizmo (position + normal) — a natural, low-cost fit for the chosen rasterized mesh pipeline (ADR-P8-03's own stated advantage over the old accumulate-blend technique, which could not clip cleanly at all).
- **Measurement tools**: extend the existing `measure_mode`/`measure_result` (already 2D-world-space, already the right shape) to 3D via `Camera3d::screen_to_ray()` — the state field, its "persist last result" semantics, and its toolbar-toggle pattern (identified as reusable by the UI audit) carry over unchanged; only the two endpoints' unprojection becomes a 3D ray-cast instead of a 2D unproject.
- **Scientific annotations**: new capability, billboarded text/icon markers pinned to world positions — same billboard-instancing technique as debug badges.
- **Experiment overlays / temporal & lineage visualization / developmental replay**: these are UI/data-presentation concerns (Research Dashboard, Lineage Explorer, Replay Browser) already confirmed by the UI audit to be dimension-agnostic egui panels unrelated to the simulation viewport's own 3D-ness — no migration needed beyond whatever their own data sources require (e.g., replay's position fields, covered under storage in §3/ADR-P8-08).

---

## 10. Interaction Architecture

- **Orbit camera**: `OrbitController`, default mode — arcball rotation around a focus point + zoom-as-distance, matching the scientific-tool convention (Blender-style) this project's UX already leans on elsewhere.
- **Fly camera**: `FlyController`, opt-in — WASD + mouselook, direct 6DOF position/orientation control, for free exploration outside the orbit-around-a-subject workflow.
- **Pan**: a per-controller concern (orbit-plane pan in orbit mode, strafe in fly mode) — both write into the same `Camera3d`, never a second camera state.
- **Focus**: `MenuAction::FocusSelection` (already exists) becomes "set orbit focus point + distance to the selected entity's position," a mechanical reinterpretation of the existing action, not a new one.
- **Selection / box selection**: ray-based picking (`Camera3d::screen_to_ray()` + ray-vs-capsule intersection, reusing the exact same capsule primitives the renderer already draws — no separate picking geometry) replaces the flat 2D nearest-point scan; box-select becomes a screen-rect-to-frustum test against candidate positions.
- **Lasso**: screen-space polygon test against each candidate's *projected* position (via `Camera3d`'s own project-to-screen, the inverse of `screen_to_ray`) — cheaper than a per-pixel raycast, matching the UI audit's own recommendation.
- **Measurement / cross-sections / slicing**: covered in §9.
- **Bookmarks**: `CameraBookmark` extended with `orientation: Quat` alongside its existing position field (its `zoom: f32` is superseded by `Camera3d`'s FOV/distance model, mapped at restore time).
- **Annotations**: covered in §9.
- **Comparison workspace integration**: out of scope for Phase 8 — this is Epic W8 (Comparative Analysis Workspace), a separate, already-named future epic from the Phase 7 roadmap, not something Phase 8's dimensional migration needs to touch.
- **3D inspector interaction**: the Inspector panel itself is an egui data-display panel (confirmed dimension-agnostic UI chrome) — its "Go to Head"/selection-sync behavior needs no redesign, only its underlying position data widens.

---

## 11. Performance Strategy

- **GPU memory**: the capsule mesh is tiny and shared (one mesh, instanced) — GPU memory growth is dominated by (a) the depth buffer and shadow-map textures (new, but standard-sized, proportional to viewport/shadow resolution, not population), and (b) the GPU physics broad-phase moving from a dense grid to a hash table sized by population rather than world-volume (ADR-P8-04, explicitly chosen to avoid the 128× blowup a dense 3D grid would cost).
- **CPU cost**: largely unchanged — the CPU-side physics crate's vector math generalizes at the same asymptotic cost; the CPU-side spatial indices (`UniformGrid`/`SpatialHash`) gain a 3rd-axis loop in their radius queries (linear cost increase, not exponential, since the *number* of populated cells scales with organism count, not grid volume, for a hash-based or sparse-populated grid).
- **Simulation cost**: growth/decode logic (`genetics`, confirmed dimension-agnostic) is unaffected; the new `dorsal`-vector maintenance in `GrowthState` is a handful of extra float operations per growth tick, negligible.
- **Rendering cost**: mesh instancing + a real depth buffer + one shadow pass is a well-understood, standard 3D-rendering cost profile — expected to be *more* GPU work than the old 2-pass 2D metaball technique (a depth-tested rasterization pipeline with shadows is inherently more expensive than an additive 2D blend), but this is the necessary and expected cost of supporting the explicitly requested capabilities (PBR, shadows, LOD), not a regression to be avoided.
- **Diffusion cost**: unchanged from today by design (ADR-P8-05 keeps the field planar specifically to avoid a 256× cost increase that nothing has justified yet).
- **LOD / instancing / culling**: standard techniques (distance-based mesh-detail/billboard-impostor switch, per-instance frustum culling via bounding sphere, reuse of the existing spatial index for broad-phase culling) — all well-precedented, low-novelty engineering, not requiring new research.
- **Streaming/chunking**: explicitly flagged as **not needed** at Phylon's current population scales (thousands of organisms, not millions) — naming it as a non-goal now prevents speculative complexity; revisit only if a future population-scale target changes this assumption, backed by measurement.
- **Expected scalability**: every one of the two genuinely expensive architecture choices in this roadmap (dense-grid-avoidance for GPU broad-phase, planar-not-volumetric diffusion) was already redirected specifically to preserve scalability at the project's actual population scale, rather than defaulting to the "more dimensions, more resolution everywhere" naive extension. Every future decision to add resolution/dimensionality anywhere in this system should be preceded by a benchmark, per this project's own standing rule — not assumed necessary because "more real" sounds better.

---

## 12. Migration Roadmap

Dependency-ordered; each epic leaves the repository compiling, is independently testable, and is reversible (via version control revert, since no epic is designed to be irreversible — no destructive one-way data transformation happens until Epic 8.13's schema bump, which is explicitly the last epic for exactly this reason).

### Tier 0 — Foundation (no visible behavior change, pure enablement)

**Epic 8.0 — `Vec3` foundation & mechanical position-field migration**
- Goal: introduce `common::Vec3`; migrate `physics::ParticleNode`, `organisms`/`ecology` position fields, `spatial` index cell-keys to 3D (Z initialized to 0.0 everywhere — no behavior change yet).
- Dependencies: none.
- Difficulty: Low (mechanical, wide fan-out).
- Risk: Low — every existing test should pass unchanged with Z always 0.
- Verification: full workspace build/clippy/test; every `*_is_deterministic_for_a_given_seed` test passes bit-identically (since Z=0 everywhere reproduces 2D behavior exactly).
- Rollback: trivial — pure type-widening, revertable file-by-file.
- Deliverables: `Vec3`-typed simulation, zero visible/behavioral change.
- Completion criteria: workspace compiles, full test suite green, interactive smoke run shows no visual difference (organisms still render via the old 2D pipeline at this point).

### Tier 1 — Camera & Interaction Foundation (first genuinely new capability)

**Epic 8.1 — `Camera3d` + orbit/fly controllers**
- Goal: replace `camera_pos`/`camera_zoom` with `Camera3d` (ADR-P8-02); implement `OrbitController`/`FlyController`; consolidate the 3 duplicated screen↔world transforms into `Camera3d::screen_to_ray()`.
- Dependencies: Epic 8.0.
- Difficulty: Medium (wide fan-out, ~12 call sites per the UI audit's enumeration).
- Risk: Medium — camera is the single most-touched piece of interactive state.
- Verification: build/clippy/test; manual interactive verification of orbit/fly/pan/zoom/focus (same disclosed-limitation pattern as every prior milestone — no GUI-automation harness exists); a real screenshot-based before/after is not meaningful here since the visual output is unchanged until Epic 8.2 (rendering is still 2D-orthographic-driven-by-a-3D-camera-pointed-straight-down at this point, a valid intermediate state).
- Rollback: revertable as one atomic commit per ADR-P8-02's own guidance.
- Deliverables: working 3D camera, still rendering the existing 2D scene from directly overhead (organisms still at Z=0).
- Completion criteria: full orbit/fly/pan/focus interaction working, verified interactively; existing 2D-equivalent view (camera looking straight down) reproduces today's visual output.

### Tier 2 — Rendering Rewrite (the flagged, sign-off-required visual identity change)

**Epic 8.2 — Mesh-based capsule renderer, depth buffer, basic lighting** *(requires explicit user sign-off on ADR-P8-03 before starting)*
- Goal: replace `SdfSkinRenderer` with capsule-mesh instancing; add depth buffer; basic directional lighting (no shadows yet).
- Dependencies: Epic 8.1.
- Difficulty: High (largest single rendering rewrite in the roadmap).
- Risk: High (visual identity), Medium (engineering — well-precedented technique, just new to this codebase).
- Verification: build/clippy/test; screenshot-based before/after comparison (this milestone's visual output is *expected* to differ from the old SDF look — the comparison is to confirm organisms render coherently and recognizably, not pixel-identical); interactive smoke test.
- Rollback: keep old `SdfSkinRenderer` code in version control history; do not delete until this epic is fully verified.
- Deliverables: organisms visible as instanced, depth-correct, lit capsule bodies.
- Completion criteria: full population renders without corruption/z-fighting at typical population scales; frame time measured and reported (expected to be different from the old technique — reported honestly, not hidden).

**Epic 8.3 — Debug/highlight billboards, shadows, PBR polish**
- Goal: port debug badges to camera-facing billboards; add shadow mapping; tune PBR material defaults.
- Dependencies: Epic 8.2.
- Difficulty: Medium.
- Risk: Low-medium.
- Verification: standard + shadow-specific visual check (does an organism cast a recognizable shadow onto the ground plane).
- Deliverables: full lit/shadowed scene with debug overlays.

**Epic 8.4 — 3D picking, box-select, lasso**
- Goal: replace flat 2D nearest-point picking with ray-vs-capsule picking; frustum-based box-select; polygon-based lasso.
- Dependencies: Epic 8.2 (needs the capsule primitives to intersect against).
- Difficulty: Medium.
- Risk: Medium (selection is a load-bearing interaction across the whole UI).
- Verification: interactive click/box-select/lasso testing against known organism positions.

**Epic 8.5 — Field renderer plane-slice migration, clipping planes**
- Goal: `FieldRenderer` becomes a `Camera3d`-driven plane-slice sampler (ADR-P8-05); add clipping-plane gizmo + shader clip test.
- Dependencies: Epic 8.2 (shares the depth buffer for correct clip-plane interaction with organism geometry).
- Difficulty: Low-medium (reuses most of the existing `FieldRenderer` shader).
- Risk: Low.

### Tier 3 — Real 3D Biology (the second flagged scientific decision)

**Epic 8.6 — Growth orientation redesign (`heading` → `forward`/`dorsal`)** *(requires explicit user sign-off on ADR-P8-06's symmetry decision before starting)*
- Goal: implement ADR-P8-06; extend the `growth_system_*` test suite with 3D-specific determinism/correctness tests; verify 2D-equivalent (`dorsal` fixed to a constant axis) reproduces today's behavior exactly as a regression check.
- Dependencies: Epic 8.0 (Vec3 foundation).
- Difficulty: High (core simulation logic, real redesign, not type-swap).
- Risk: High (this is the one epic that changes what the simulation's biology actually does, not just how it's viewed/rendered).
- Verification: full existing `growth_system_*` suite (11 tests) passes for the fixed-dorsal regression case; new tests for real 3D branching; determinism test for the new orientation math.
- Rollback: the existing 11-test safety net (built during Phase 7 W5a) is the primary regression guard — any failure here blocks the epic, full stop, per this project's own "if verification fails, stop, find the root cause" rule.

**Epic 8.7 — 3D vision/sensing redesign**
- Goal: implement ADR-P8-07 (azimuth×elevation binned cone).
- Dependencies: Epic 8.6 (needs the `forward`/`dorsal` body frame).
- Difficulty: Medium.
- Risk: Medium (affects organism behavior, needs careful validation that evolved brains still receive meaningful signal).
- Verification: sensing-specific unit tests; a real interactive run confirming organisms still forage/flee/hunt coherently (behavioral sanity check, not just unit correctness).

**Epic 8.8 — Fin-drag / anisotropic physics redesign**
- Goal: replace the 2D-only `vec2(-dir.y, dir.x)` perpendicular trick in GPU physics with the `dorsal`-vector body frame from Epic 8.6.
- Dependencies: Epic 8.6.
- Difficulty: Medium-high (GPU shader work).
- Risk: Medium (affects locomotion physics realism).
- Verification: swim/locomotion behavioral check (do organisms still move coherently); determinism test.

### Tier 4 — Spatial/Compute Scaling & Persistence (last, since it depends on every upstream type being final)

**Epic 8.9 — CPU spatial index 3D extension + Octree**
- Goal: `UniformGrid`/`SpatialHash` 3rd-axis extension; `Quadtree`→`Octree` rewrite.
- Dependencies: Epic 8.0.
- Difficulty: Medium (Octree is real, well-precedented rework).
- Risk: Low-medium.
- Verification: spatial-index-specific unit tests (insert/query/remove correctness at known 3D positions); benchmark comparison vs. the 2D baseline.

**Epic 8.10 — GPU physics 3D buffers + hash-based broad phase**
- Goal: implement ADR-P8-04; widen `GpuParticleNode` to `vec3`, add the third atomic-force buffer, replace the dense grid with a spatial hash.
- Dependencies: Epic 8.0, Epic 8.9 (shares broad-phase design philosophy).
- Difficulty: High (GPU shader engineering, WGSL alignment considerations).
- Risk: High (core physics correctness + performance).
- Verification: new GPU broad-phase benchmark (mirroring the CPU-side `foraging_scaling` precedent from Phase 7 W7) at multiple population sizes, before/after; determinism test; visual/behavioral sanity check.

**Epic 8.11 — CI GPU-testing posture (ADR-P8-09)**
- Goal: document and pin the software-rasterizer CI dependency explicitly; add `#[ignore]` gating conventions for any new GPU-dependent tests this migration introduced.
- Dependencies: none strictly, but sequenced before Epic 8.2 in practice (see note below) — listed here in the tier ordering for completeness since it's a CI/process change, not a runtime one.
- Difficulty: Low.
- Risk: Low effort, High-if-skipped (silent CI breakage risk).
- **Note**: although listed in Tier 4 for document-organization purposes, this epic's actual recommended execution order is *before* Epic 8.2, since it's cheap, low-risk, and de-risks every subsequent rendering epic's CI reliability.

**Epic 8.12 — Test/benchmark suite 3D migration**
- Goal: update the ~135+ direct `Vec2::new(...)` construction sites across `organisms`/`ecology`/`spatial`/`app`/`ui`/`behavior`/`reproduction`/`storage`/`benchmarks` (per the serialization/testing audit's own count) to 3D positions.
- Dependencies: every prior epic that changes a type these tests construct (effectively, this epic is continuous/interleaved with Epics 8.0-8.10 in practice — listed as its own epic here for tracking/completion-criteria purposes, not because it should literally wait until every other epic finishes).
- Difficulty: Low individually, Medium in aggregate (volume, not complexity).
- Risk: Low.
- Verification: full test suite green at 100% of its current test count (no test silently deleted/skipped to make this easier).

**Epic 8.13 — Storage schema bump (`SchemaVersion` v4→v5) & replay format update** *(last, deliberately)*
- Goal: implement ADR-P8-08; bump schema version; update `SerializedVec2`→`SerializedVec3`; update the CSV export header; communicate the save/replay-file breakage explicitly (release notes / changelog, not silent).
- Dependencies: every upstream type change (this must be last, since it serializes the final shape of every other type).
- Difficulty: Low (precedented, mechanical).
- Risk: Low (process risk only, precedented 3 times already) — but the user-communication step is not optional.
- Verification: save/load round-trip test with real 3D data; confirm old-schema files are rejected cleanly (not silently corrupted) per the existing `UnsupportedSchema` error path.

---

## 13. Verification Strategy

Every epic above completes the following before being considered done, extending (not replacing) this project's existing standing discipline:

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo build --workspace --all-targets`
4. `cargo test --workspace` (0 failures, and — critically — the *same or greater* test count as before the epic; no test silently deleted to make a migration easier)
5. `cargo doc` (no broken intra-doc links, matching the discipline already established this session)
6. **Benchmark verification** (new for Phase 8): any epic touching physics/spatial/diffusion performance characteristics gets a `crates/benchmarks` before/after comparison, extending the precedent Phase 7 W7 already established (`foraging_scaling.rs`) rather than inventing a new measurement convention.
7. **Determinism validation** (new, explicit): any epic touching simulation logic (growth, physics, sensing) adds or re-verifies a `*_is_deterministic_for_a_given_seed`-style test, matching existing precedent.
8. **Scientific validation**: any epic touching biology (Epics 8.6-8.8) includes a behavioral sanity pass — organisms still forage/flee/hunt/reproduce coherently in a real interactive run — not just unit-test-green.
9. **Visual validation**: rendering epics (8.2-8.5) include a screenshot-based before/after, disclosed honestly as "expected to differ" where the ADR says the visual identity is intentionally changing (Epic 8.2) vs. "expected to match" where it's a pure refactor (e.g. Epic 8.1's straight-down camera view).
10. **Regression validation**: every epic that touches code with an existing test suite (growth, physics, storage) runs that exact suite unchanged first, before adding new tests — a failure here means stop and find the root cause, per this project's own standing rule, never silently adjusted to "make the test pass."
11. **Interactive smoke run**: the real windowed binary, launched and driven for a real interaction pass, at the end of every epic — the same discipline applied to every Phase 7 milestone, continued here.

Acceptance checklist per epic = all of the above, plus that epic's own stated "Completion criteria" from §12.

---

## 14. Risk Register

| Risk | Category | Likelihood | Impact | Mitigation | Fallback | Owner |
|---|---|---|---|---|---|---|
| Rendering rewrite changes visual identity users have grown attached to | Rendering/Product | Certain (by design) | High | Explicit sign-off gate before Epic 8.2 (ADR-P8-03); old renderer code kept until new one is fully verified | Revert Epic 8.2 wholesale; the mesh-based approach is still recommended even on revert-and-retry, since the alternatives were rejected on engineering merit, not aesthetics | Whoever owns product/visual-identity decisions (not decided unilaterally by this planning pass) |
| GPU physics broad-phase memory blowup if naively ported | GPU/Performance | Medium (if ADR-P8-04 ignored) | High | Hash-based broad phase instead of dense 3D grid, benchmarked before/after | Reduce world/organism-density expectations, or accept the memory cost with explicit sign-off if measurement shows it's actually fine | GPU/Performance engineer |
| Volumetric diffusion compute/memory blowup | GPU/Performance | Low (only if someone reaches for it against this roadmap's recommendation) | High | ADR-P8-05 explicitly recommends staying planar by default | Sparse/windowed volumetric diffusion as a measured, opt-in future epic | GPU/Performance engineer |
| Growth-orientation redesign introduces a scientific regression (bilateral symmetry silently reinterpreted) | Biology/Scientific correctness | Medium (real redesign, not mechanical) | High | ADR-P8-06's explicit sign-off gate + regression test (fixed-dorsal case reproduces 2D behavior exactly) | Revert to Epic 8.0's Z=0 intermediate state; re-attempt with more design review | Computational biology owner |
| Determinism regression anywhere in the physics/growth/sensing redesigns | Scientific correctness | Medium | High (this project treats determinism as non-negotiable) | Explicit determinism test at every simulation-logic-touching epic | Stop, root-cause, do not proceed until fixed (per standing project rule) | Whoever owns the epic in question |
| Test-suite migration volume (~135+ call sites) creates schedule risk | Testing/Process | High (large, known volume) | Medium (effort, not correctness) | Track as its own epic (8.12) with a clear completion criterion (100% of current test count green) | Extend timeline; this is a volume problem, not a design problem, so there's no architectural fallback needed | Testing engineer |
| Save/replay file breakage surprises users | Product/Communication | Certain (by design, ADR-P8-08) | Medium | Explicit changelog/release-note communication, not silent | N/A — this is accepted, precedented behavior; the only failure mode is *not communicating* it | Release owner |
| CI silently breaks or silently masks a real GPU rendering bug via software-rasterizer quirks | CI/Testing | Medium (currently undocumented reliance) | Medium-High (could stall the whole rendering-epic tier) | ADR-P8-09, sequenced *before* Epic 8.2 in practice | Add local-only GPU test gating if CI's software rasterizer proves insufficient for the new pipeline | Build & CI engineer |
| Octree rewrite introduces a spatial-index correctness bug (missed neighbors, incorrect pruning) | Physics/Correctness | Medium (real rewrite, not mechanical) | Medium | Dedicated spatial-index unit tests (insert/query/remove at known 3D positions) before integrating into physics/sensing | Fall back to a flat 3D `UniformGrid` (already planned as the simpler, mechanical extension) if the octree proves buggy under time pressure — explicitly an acceptable fallback, not a failure | Physics/ECS engineer |
| Scope creep: an epic silently expands into new biology (e.g., radial symmetry) or new rendering features not in this roadmap | Process | Medium (a known temptation once 3D is "possible") | Medium | ADR-P8-06 explicitly names and defers radial symmetry; this roadmap is the single source of truth for what's in-scope per epic | Any such addition requires its own ADR and explicit sign-off, not silent inclusion in an existing epic | Architecture Review (whoever approves epics) |

---

## 15. Phase 8 Master Roadmap

**Epic sequence** (recommended execution order, respecting dependencies and risk-front-loading of sign-off gates):

1. Epic 8.11 — CI GPU-testing posture *(cheap, de-risks everything after; moved earlier in practice despite Tier 4 listing)*
2. Epic 8.0 — `Vec3` foundation
3. Epic 8.1 — `Camera3d` + orbit/fly controllers
4. **[SIGN-OFF GATE: ADR-P8-03, rendering visual identity]**
5. Epic 8.2 — Mesh-based capsule renderer + depth buffer
6. Epic 8.3 — Debug billboards + shadows + PBR polish
7. Epic 8.4 — 3D picking, box-select, lasso
8. Epic 8.5 — Field renderer plane-slice migration + clipping planes
9. Epic 8.9 — CPU spatial index 3D extension + Octree
10. **[SIGN-OFF GATE: ADR-P8-06, 3D bilateral-symmetry scientific decision]**
11. Epic 8.6 — Growth orientation redesign
12. Epic 8.7 — 3D vision/sensing redesign
13. Epic 8.8 — Fin-drag/anisotropic physics redesign
14. Epic 8.10 — GPU physics 3D buffers + hash-based broad phase
15. Epic 8.12 — Test/benchmark suite migration *(continuous alongside 2-14 in practice; tracked to completion here)*
16. Epic 8.13 — Storage schema bump *(last, deliberately)*

**Milestones**: end of step 3 = "3D navigation works, scene still looks like today's 2D sim viewed from above" (a real, demoable, low-risk milestone). End of step 8 = "full 3D rendering, picking, and scientific overlay pipeline, organisms still grow in a Z=0 plane" (a second real, demoable milestone, the natural point to pause and gather feedback on the new visual identity before touching biology). End of step 14 = "true 3D biology and physics." End of step 16 = "Phase 8 complete."

**Stopping conditions** (unchanged from this program's own governing instructions, restated for the implementation phase): continuing would risk breaking determinism; multiple architectures remain equally valid after investigation; repository integrity would be at risk; or project goals fundamentally conflict. Additionally, per this roadmap's own two sign-off gates: **implementation must not proceed past step 3 into step 5, or past step 9 into step 11, without explicit human approval of ADR-P8-03 and ADR-P8-06 respectively** — these are not routine architectural questions, they are product/scientific-identity decisions this planning process deliberately did not resolve unilaterally.

**Success criteria** (restated from §1, now dependency-ordered): every epic's own completion criteria (§12) met in sequence; full determinism preserved throughout; no biological model simplified without an explicit, documented, signed-off decision (ADR-P8-06 is the only one, and it preserves rather than simplifies the existing model's intent); the repository compiles and passes its full test suite at every epic boundary; performance measured (not assumed) before any epic that changes a memory/compute trade-off (Epics 8.5, 8.10 specifically).

**Future Phase 9 dependencies/recommendations** (named, not built): true volumetric diffusion (ADR-P8-05's deferred option, pending real measurement); radial body-plan symmetry as a new evolvable trait (ADR-P8-06's deferred option); a full LOD chain beyond the 2-tier capsule/billboard split if population scale grows; skeletal-animation-adjacent tooling (glTF import, standard DCC workflows) if Phylon ever needs authored (non-procedural) organism assets; a real save-file migration tool if backward-compatibility ever becomes a product requirement (ADR-P8-08's deferred alternative); Epic W8 — Comparative Analysis Workspace (already named in the Phase 7 roadmap, unaffected by but potentially enriched by 3D visualization once it lands).

---

## 16. Self-Critique (internal architecture review, performed before finalizing this document)

- **Hidden coupling found and addressed**: the initial pass toward ADR-P8-02 considered leaving `pick_entity`'s screen→world math as its own thing "since it's just picking" — rejected on review, since the UI audit found 3 independent copies of essentially the same transform already, and leaving picking as a 4th independent implementation would repeat the exact mistake this migration should fix. `Camera3d::screen_to_ray()` is deliberately the *only* place this math exists.
- **Unnecessary abstraction avoided**: an earlier draft of §4 considered a formal `RenderPass`/frame-graph trait — rejected on self-review as solving a problem (dynamic pass reordering) this project doesn't have; the existing linear pass sequence is clear and well-understood, and adding an abstraction over ~6 passes that don't need dynamic reordering would itself be the kind of "unnecessary abstraction" this critique step is instructed to search for.
- **Duplicated architecture avoided**: GPU and CPU broad-phase were initially planned independently (dense-grid-fix for GPU, separate hash design for CPU) — on review, unified into one conceptual design (spatial hash on both sides, ADR-P8-04) specifically to avoid maintaining two independently-evolving broad-phase philosophies for the same underlying problem.
- **Determinism risk surfaced, not glossed over**: the fin-drag/anisotropic-physics redesign (Epic 8.8) was initially scoped as "port the existing formula" — on review, flagged as requiring its own explicit determinism test, since it's genuinely new math (body-frame-relative perpendicular, not the old formula with wider vectors), not a mechanical port, and this project treats determinism as non-negotiable.
- **Scientific regression risk surfaced, not assumed away**: the bilateral-symmetry question (ADR-P8-06) was the single most important thing this self-critique pass looked for — an earlier informal framing treated "3D branching" as purely an engineering problem (how do fins get 3D coordinates); the audit's own biology specialist findings forced the recognition that this is a scientific-meaning question first, engineering second, which is why it's one of only two sign-off gates in the entire roadmap.
- **Performance regression risk surfaced twice**: both the GPU broad-phase (ADR-P8-04) and world-diffusion-field (ADR-P8-05) naive-3D-extension costs were caught specifically because the audit was instructed to measure/estimate blast radius rather than assume "add a dimension" is a uniformly cheap operation — both are now explicitly redirected away from their naive (dense-grid, full-volume) defaults.
- **No further major architectural issues found** after this pass; the two sign-off gates (rendering identity, biological symmetry) are the two decisions this document deliberately does not resolve unilaterally, by design, not by oversight.

---

*This document is the complete Stage 1-4 planning deliverable. No code has been modified. Implementation (Stage 5 proper — Epic 8.11 onward) requires explicit approval to begin, and Epics 8.2 and 8.6 additionally require explicit sign-off on ADR-P8-03 and ADR-P8-06 respectively before starting, per this document's own stated gates.*
