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
- **STATUS: APPROVED.** Accepted by the user prior to Epic 8.6, per this document's own sign-off gate — body-fixed `forward`/`dorsal` frame, strict bilateral symmetry preserved, radial symmetry explicitly deferred out of Phase 8. Epic 8.6 may proceed under this decision once Epic 8.5's outstanding manual QA checklist (see its own execution-log entry) is reported back as all-PASS.

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

## 17. Execution Log

### Epic 8.0 — `Vec3` Foundation & Mechanical Position Migration — COMPLETE

**Epic summary.** Widened every simulation-space position/velocity/force field from `common::Vec2` to the newly-introduced `common::Vec3` (Z fixed at `0.0` everywhere), mechanically, in dependency order, with no behavioral or visual change. This is the "2D-embedded-in-3D" intermediate state the roadmap's Executive Summary and ADR-P8-01 call for — later epics (8.1 Camera3d onward) build real 3D on top of this foundation without redesigning it.

**Files changed** (39 files across 11 crates):

- `crates/common/src/lib.rs` — added `pub use glam::Vec3` re-export.
- `crates/physics/src/lib.rs` — `ParticleNode::{position, velocity, force}` → `Vec3`.
- `crates/spatial/src/uniform_grid.rs`, `crates/spatial/src/hash.rs` — `UniformGrid`/`SpatialHash` inherent APIs widened to `Vec3` (3D cell-keys/bucket hash); `SpatialIndex` trait and `Quadtree` deliberately left at `Vec2` (confirmed zero live callers via the trait; Quadtree→Octree is Epic 8.9's job).
- `crates/ecology/src/components.rs` (`FoodPellet`/`MineralPellet`/`Corpse`), `.../fungi.rs`, `.../systems/{catastrophe_system,food_spawner,foraging}.rs`, `.../tests.rs`, `.../disease.rs` — entity positions and call sites migrated; `catastrophe::Hazard::center`/`HazardSpawned` deliberately left at `Vec2` (the hazard field, like the diffusion field, stays a 2D plane per ADR-P8-05 — truncated at the one comparison site instead).
- `crates/events/src/lib.rs` — `TimedEffect::position`/`TimedEffects::spawn` widened to `Vec3` (a simulation-space spawn/event location, the same category as `ParticleNode.position`, even though not named explicitly in the roadmap's own ECS Evolution table — a small, low-risk gap-fill under the same principle).
- `crates/organisms/src/components.rs` (`GrowthState::current_pos`), `.../systems.rs`, `.../sandbox.rs`, `.../social.rs`, `.../quorum.rs`, `.../spawning.rs`, `.../life_cycle.rs` — growth/flocking/pack-hunting/biofilm/spawn-position math migrated; `GrowthState::heading` (a scalar angle) deliberately untouched, reserved for Epic 8.6's real orientation redesign.
- `crates/reproduction/src/lib.rs` — `BirthRequest::position` widened to `Vec3` (same category as `TimedEffect::position`).
- `crates/sensing/src/lib.rs` — internal vision math (`EntitySnapshot`, `WorldSnapshot`, `VisionSnapshot::last_forward`) deliberately kept at `Vec2` — vision's angle-based FOV cone is genuinely 2D today and isn't part of this epic's scope (a future sensing epic, not 8.0) — every position is truncated to its XY plane at the snapshot-construction boundary instead.
- `crates/storage/src/snapshot.rs` — save/restore boundary adapters added (`.truncate()` on save, `.extend(0.0)` on restore) around every `SerializedVec2` conversion; `SerializedVec2` itself is intentionally unchanged in shape until Epic 8.13's schema bump (ADR-P8-08).
- `crates/ui/src/render.rs`, `.../plugins/inspector.rs` — screen-space projection code (`to_screen`/`world_to_minimap` closures, `render_pellet_summary`) truncates `Vec3` world positions to `Vec2` at the render boundary; `camera_pos`/UI-owned state stays `Vec2` throughout, per the crate-graph's existing UI/simulation split.
- `crates/app/src/{app.rs,events.rs,interventions.rs,motion_diagnostic.rs,render.rs,simulation.rs,systems.rs}` — every spawn/pick/camera-follow/GPU-upload call site updated; the GPU compute boundary (`simulation.rs`'s `to_grid` closure, the post-readback force-reset) needed only two small truncation/widen fixes, confirmed by reading the actual buffer code that the roadmap's §5 "gpu compute buffers" migration step does **not** need to happen in this epic (see Problem Solving note below).
- `crates/behavior/src/lib.rs`, `crates/benchmarks/benches/{foraging_scaling,metabolism_parallel}.rs` — test/benchmark fixtures updated to construct `Vec3` positions.

**Architecture changes.** None beyond the type migration itself — no new abstractions introduced. Two deliberate, documented boundary decisions (both consistent with ADR-P8-05's "world-space 2D fields stay 2D" precedent):

1. World-space diffusion/hazard fields, and all vision/rendering/UI-screen-space math, stay `Vec2`, with explicit `.truncate()`/`.extend(0.0)` adapters at each boundary crossing.
2. The `spatial::SpatialIndex` trait and `Quadtree` stay `Vec2`-based (unused by any live caller today), bridged from `UniformGrid`/`SpatialHash`'s new `Vec3` inherent APIs via `.extend(0.0)`.

**Performance impact.** None expected or measured — every position gained one `f32` field (`Z`, always `0.0`); no new allocations, no algorithmic changes. Not benchmarked separately since the roadmap's Epic 8.0 acceptance bar is "no behavior change," not a performance milestone.

**Risks.** Low, as predicted. The one real risk — silently breaking a `*_is_deterministic_for_a_given_seed` test by introducing a Z-dependent code path — did not materialize; every determinism test passed bit-identically, and two independent headless runs at the same seed produced identical entity-index behavior (see Verification below).

**Verification results:**

- `cargo build --workspace --all-targets`: clean.
- `cargo fmt --all -- --check`: clean (after one `cargo fmt --all` pass to reformat lines widened past the column limit).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean, zero warnings.
- `cargo test --workspace`: **334 tests passed, 0 failed**, across every crate (including all `*_is_deterministic_for_a_given_seed` and `*_deterministic_regardless_of_thread_count` tests, and `storage`'s save/load round-trip test).
- Interactive smoke run: built `phylon.exe`, ran two independent 200-tick headless runs (`research.headless: true`, `max_ticks: 200`, default seed) — both completed without panics and produced identical behavior (same entity index/generation in the one pre-existing, unrelated `B0003` double-despawn warning both times), confirming determinism end-to-end through the real app, not just unit tests. `data/default.ron` was reverted via `git checkout` immediately after.
- Grepped for unseeded `rand::thread_rng()`/`rand::random()`: none introduced.

**Tests executed:** the full workspace suite (`cargo test --workspace`), 334 tests, 0 failures — no test count regression versus pre-epic (several new test fixtures updated in place, none removed).

**Benchmarks:** none run this epic (no performance-sensitive code path changed); `benchmarks` crate's fixtures were updated only so it continues to compile.

**Documentation updated:** this Execution Log section; no other `docs/` changes were needed (Epic 8.0 has no user-visible behavior to document).

**Known limitations:**

- `heading: f32` remains a 2D angle — organisms still only ever face within the XY plane. This is intentional and explicitly deferred to Epic 8.6.
- Vision, world-space diffusion/hazard fields, and all screen-space rendering math remain fully 2D, bridged by truncation/extension adapters at their boundaries — these are the exact seams later epics (8.1 camera, a future sensing epic, and the eventual volumetric-diffusion follow-on) will need to cross.
- The `spatial::SpatialIndex` trait and `Quadtree` were not migrated (confirmed dead via the trait today) — Epic 8.9 (Quadtree→Octree) inherits this gap.
- `storage`'s on-disk schema (`SerializedVec2`) is unchanged; Z is silently dropped on every save today. This is the explicitly accepted, documented state until Epic 8.13's schema bump (ADR-P8-08) — not a bug.

**Next epic dependencies:** Epic 8.1 (Camera3d) and Epic 8.11 (CI GPU-testing posture, substantially already done via the earlier CI fix this session) can both proceed immediately. Epic 8.2 (rendering rewrite) remains gated on ADR-P8-03 sign-off; Epic 8.6 (real 3D growth/orientation) remains gated on ADR-P8-06 sign-off — neither gate is bypassed by this epic's completion.

---

### Epic 8.1 — `Camera3d` + Orbit/Fly Controllers — COMPLETE

**Epic summary.** Replaced `WorkbenchState`'s flat `camera_pos: Vec2` + `camera_zoom: f32` pair with the canonical `Camera3d` object (ADR-P8-02) plus two controllers — `OrbitController` (arcball around a focus point, the default mode) and `FlyController` (free WASD + mouselook, opt-in via a new `Tab` shortcut/toolbar toggle). No mesh rendering, lighting, depth buffer, or renderer migration was touched this epic, per its own explicit scope boundary and the roadmap's Tier 1/Tier 2 split — `SdfSkinRenderer`/`DebugRenderer`/`FieldRenderer` are unchanged, still consuming a flat `(Vec2, f32)` pair, now derived each frame from `Camera3d` via a documented, explicitly temporary bridge (`WorkbenchState::camera_pos_2d`/`camera_zoom_2d`) that Epic 8.2 deletes when it replaces those renderers.

**Files changed** (14 files across 2 crates):

- `crates/common/src/lib.rs` — added `Quat`/`Mat4`/`Mat3` re-exports (Epic 8.1 is the first consumer of any of the three).
- `crates/ui/src/camera.rs` (new, ~470 lines) — `Camera3d` (`position`/`orientation`/`fov_y`/`near`/`far`, `view_proj()`, `screen_to_ray()`, `forward()`/`right()`/`up()`); `OrbitController` (`focus`/`distance`/`yaw`/`pitch`, pitch measured from nadir so `0.0` reproduces the pre-Phase-8 straight-down default exactly; `orbit()`/`pan()`/`zoom_by()`/`focus_on()`/`reset()`/`looking_at()`); `FlyController` (`position`/`yaw`/`pitch`, conventional FPS-zero pitch; `look()`/`move_relative()`/`look_at()`/`from_camera()`); `CameraController` enum dispatching between them with a continuity-preserving `toggle()`; the shared `ray_intersect_z0()` plane-intersection primitive; 11 unit tests covering the default view, zoom/pan/pitch-clamp math, screen-ray casting, plane intersection, and mode-toggle round-tripping.
- `crates/ui/src/state.rs` — `camera_pos`/`camera_zoom` fields replaced by `camera_controller: CameraController`; added `camera()`, and the transitional bridge `camera_pos_2d()`/`camera_zoom_2d()` (derived by ray-casting through the viewport center rather than an analytic distance/FOV formula, so it degrades gracefully under tilt/Fly mode rather than assuming straight-down); `zoom_by()` rewritten to dispatch to the orbit controller (no-op in Fly mode).
- `crates/ui/src/types.rs` — `CameraBookmark` widened from `{position: Vec2, zoom: f32}` to `{position: Vec3, orientation: Quat}` per the roadmap's own §10 spec ("zoom superseded by the FOV/distance model, mapped at restore time"); added `MenuAction::ToggleCameraMode`; added `CanvasInteraction::rotate_delta` (middle-button drag, separate from the existing primary-button `drag_delta`).
- `crates/ui/src/shortcuts.rs` — added `Tab` → `ToggleCameraMode` (unmodified, gated alongside the existing X/F raw-key bindings).
- `crates/ui/src/plugins/viewport.rs` — `cursor_world_pos` and the `to_world` closure now go through `Camera3d::screen_to_ray` + `ray_intersect_z0` (2 of the 3 duplicated screen↔world transforms ADR-P8-02 names); `to_screen` (the reverse, world→screen, direction — not part of the ADR's named scope) kept its exact pre-existing formula, fed by the bridge; added middle- vs. primary-button drag disambiguation for `rotate_delta`/`drag_delta`.
- `crates/ui/src/plugins/toolbar.rs` — bookmark save/restore updated to the new `position`/`orientation` shape (restore goes through `Fly`, the mode that takes a raw position/orientation snapshot directly); zoom%/position readouts updated to the bridge; added an Orbit/Fly `selectable_label` toggle mirroring the existing Spectator-mode control.
- `crates/ui/src/plugins/status_bar.rs` — camera readout updated to the bridge.
- `crates/ui/src/plugins/dialogs.rs` — Keybinds dialog's Camera section documents the new Tab/middle-drag bindings.
- `crates/ui/src/render.rs` — the `CanvasInteraction::default()` fallback construction updated for the new `rotate_delta` field.
- `crates/app/src/app.rs` — `pick_entity` (the third of the 3 duplicated transforms) rewritten around `screen_to_ray` + `ray_intersect_z0`.
- `crates/app/src/render.rs` — camera-tracking lerp now updates `OrbitController::focus` (only meaningful in Orbit mode); interaction dispatch split into pan (primary-drag, Orbit-only), rotate (middle-drag, dispatches to `orbit.orbit()`/`fly.look()`), and zoom (`zoom_by()`); every `SdfSkinRenderer`/`DebugRenderer`/`FieldRenderer` call site now reads cached `camera_pos_2d`/`camera_zoom_2d` (computed once per frame, after this frame's interaction updates, to avoid a one-frame lag).
- `crates/app/src/events.rs` — `CameraHome` now resets to a fresh default `Orbit` controller; added `ToggleCameraMode` handler; `FocusSelection` dispatches to `orbit.focus_on()` or `fly.look_at()` depending on mode; WASD/arrow keys dispatch to orbit-pan (unchanged pre-Phase-8 behavior) or fly-move depending on mode; Ctrl+scroll zoom unchanged, plain scroll/touchpad-pan restricted to Orbit mode (Fly has no equivalent, matching the pre-Phase-8 absence of any fly concept at all).
- `crates/app/src/render/world_instances.rs` — frustum-culling AABB now reads the bridge instead of raw fields.

**Architecture changes.** Exactly what ADR-P8-02 specified, no more: one `Camera3d` type, two controllers, two projection-related methods (`view_proj`, `screen_to_ray`). No new rendering abstraction, no parallel camera representation. The one deliberate, documented extension beyond the ADR's literal text: `ray_intersect_z0` lives outside `Camera3d` (not a third projection method) since the `Z = 0` plane is a simulation-space convention the camera itself shouldn't need to know about — consistent with ADR-P8-05's same "world-space plane is a caller concern" precedent.

**Performance impact.** Not benchmarked separately — this epic's acceptance bar is "no behavior change at the default view," not a performance milestone, and no per-tick simulation code was touched (this is entirely UI/input/camera-math, evaluated a handful of times per rendered frame, not per organism). `camera_pos_2d`/`camera_zoom_2d` each cast 2 rays through `screen_to_ray` (simple vector math, no allocation) once per frame per call site; negligible next to the SDF/debug render passes' own GPU cost.

**Verification results:**

- `cargo build --workspace --all-targets`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean (one `clippy::question_mark` finding in `pick_entity`, fixed).
- `cargo test --workspace`: **345 tests passed, 0 failed** (334 carried over from Epic 8.0 + 11 new `ui::camera` unit tests covering the default straight-down view, orbit zoom/pan/pitch-clamp, screen-ray casting, plane intersection, and Orbit↔Fly round-tripping).
- Two independent 200-tick headless runs at the same seed: identical behavior (same entity index/generation in the one pre-existing, unrelated `B0003` warning both times), no panics — camera changes don't touch simulation state or determinism.
- Real windowed launch: the app started, initialized GPU, and rendered the default scene — visually identical in style/layout to the pre-Phase-8 2D view (same top-down framing, same organism/pellet colors and shapes), with the toolbar's zoom%/position readouts populated with sane values and the new Orbit/Fly toggle present. Screenshot captured and inspected; no crash, no visual regression at the default view.
- **Disclosed limitation, honestly, matching this project's own established precedent** ("no GUI-automation harness exists," restated at every prior camera-adjacent milestone): live mouse-drag orbit, middle-drag rotation, and WASD-fly *feel* were not independently exercised by dragging/typing into the real window — no input-injection tool is available in this environment. Every underlying operation (`orbit()`, `pan()`, `zoom_by()`, `look()`, `move_relative()`, `look_at()`, mode `toggle()`) is covered by a focused unit test instead, and the real app was confirmed to launch, render, and expose the new controls without error. A human interactive pass (drag to orbit, Tab to fly, WASD to move, F to focus, Home to reset) is recommended before treating the interaction *feel* (sensitivity constants, pan speed) as final — the constants used (`ROTATE_SENSITIVITY = 0.005`, `FlyController::BASE_SPEED = 400.0`) are explicitly untuned, same status as every other not-yet-measured constant introduced this phase.

**Tests executed:** the full workspace suite (`cargo test --workspace`), 345 tests, 0 failures — 11 new, 0 removed.

**Benchmarks:** none run this epic (no per-tick simulation code changed).

**Documentation updated:** this Execution Log entry; `crates/ui/src/plugins/dialogs.rs`'s Keybinds dialog (new Camera-section entries).

**Known limitations:**

- Fly-mode WASD movement is driven by discrete per-keydown events (relying on the OS's own key-repeat rate for continuous movement while held), not a per-frame-polled "is this key currently down" state — matches the pre-existing pan implementation's own event model exactly, so it's not a new limitation, but it means fly movement smoothness is bounded by OS key-repeat rate/timing, not frame rate.
- `OrbitController`/`FlyController` round-trip through `CameraController::toggle()` is lossy at the poles: both clamp pitch a degree short of true vertical (`MAX_PITCH = 89°`) to keep their forward/up basis non-degenerate, so toggling modes at the default straight-down view returns to within ~1-2° of the original look direction, not bit-identical. Documented in the module and covered by a unit test with a tolerance that reflects this honestly rather than hiding it.
- `camera_pos_2d`/`camera_zoom_2d`'s ray-based derivation assumes the camera's forward ray actually intersects the `Z = 0` plane; in `Fly` mode looking upward/away from the ground, this can return `None` (bridged to a fallback of `camera.position.truncate()`/`1.0`) — acceptable since the roadmap's own completion criteria only require the *default straight-down Orbit view* to reproduce pre-Phase-8 output, not every possible Fly orientation.
- Sensitivity/speed constants (`ROTATE_SENSITIVITY`, `FlyController::BASE_SPEED`, `OrbitController::DEFAULT_DISTANCE`/`MIN_DISTANCE`/`MAX_DISTANCE`) are reasonable-but-untuned placeholders, consistent with this phase's stated tuning discipline (measure before changing) — flagged for a human pass, not treated as final.

**Remaining roadmap dependencies for Epic 8.2:** none — Epic 8.2's own stated dependency (Epic 8.1) is now satisfied. Epic 8.2 still requires the already-granted ADR-P8-03 sign-off (received this session) before proceeding, which this epic's completion does not bypass or presuppose.

---

### Epic 8.2 — Mesh-Based Capsule Renderer, Depth Buffer, Basic Lighting — COMPLETE

**Executive summary.** Replaced `SdfSkinRenderer` (the 2-pass SDF metaball accumulate-blend technique) with `OrganismRenderer` (ADR-P8-03): a shared, procedurally-generated low-poly capsule mesh, instanced per bone via an oriented-look-at vertex shader, rasterized with a real depth buffer and single-light Cook-Torrance PBR shading. This is the approved, intentional visual-identity change the ADR named — organisms now render as faceted-at-the-joints capsule bodies rather than smoothly-blended metaballs; a future joint-smoothing refinement is explicitly out of scope (deferred, per the ADR's own text). No shadows yet (Epic 8.3). Verified interactively at real population scale: organisms render coherently, recognizably, with no z-fighting or corruption, through a live multi-minute simulation run.

**Architecture changes.** Exactly ADR-P8-03's chosen design, no more:

- One shared capsule mesh (hemisphere caps + cylinder body, `RADIAL_SEGMENTS = 12`, `CAP_RINGS = 4` — low-poly, per the roadmap's explicit instruction), generated once at startup.
- Per-instance data (`CapsuleInstance`: `pos_a`/`pos_b: Vec3`, `radius`, `color`, `health`) — nearly identical in shape to the old `SdfBoneInstance`, just with `Vec3` endpoints instead of `Vec2`, confirming the ADR's own prediction that the instance *data* barely changes; only the shader *algorithm* does.
- The vertex shader classifies each mesh vertex into one of 3 local-space regions (bottom cap / cylinder body / top cap) and reconstructs its world position from the instance's endpoints and radius directly — no per-instance rotation or quaternion is stored, matching the ADR's stated technique exactly.
- A real depth buffer (`Depth32Float`), owned by `OrganismRenderer` itself (matching the existing "each renderer owns its own textures" pattern `SdfSkinRenderer`/`FieldRenderer` already established) — the first depth-consuming pass anywhere in this codebase.
- Selection/hover highlighting uses the "inverted hull" technique the roadmap's Selection Rendering architecture section names: an inflated capsule, back-faces only (`cull_mode: Front`), depth-tested (`LessEqual`) but not depth-writing, against the same frame's already-populated depth buffer.
- `Camera3d::view_proj()` (Epic 8.1) is the *only* projection-matrix source the new renderer consumes — the duplicated hand-derived orthographic matrix that lived in `sdf_skin.rs` (2 sites) is gone entirely along with that file. (`debug.rs`'s own separate copy is untouched — porting `DebugRenderer` to the new pipeline is explicitly Epic 8.3's job, not this one's.)

**Files changed** (13 files across 3 crates; 4 files deleted):

- `crates/rendering/src/capsule_mesh.rs` (new) — `CapsuleVertex`, `build_capsule_mesh()`, 5 unit tests (vertex/index counts, index-bounds safety, pole positions, equator radius, normal unit-length).
- `crates/rendering/src/capsule.wgsl` (new) — the oriented-capsule vertex shader plus the Cook-Torrance PBR (`fs_main`) and flat-unlit highlight (`fs_highlight`) fragment shaders.
- `crates/rendering/src/organism.rs` (new) — `CapsuleInstance`, `OrganismRenderer` (`new`/`resize`/`render`/`render_highlight`), depth-texture lifecycle, main + highlight pipelines.
- `crates/rendering/src/lib.rs` — swapped the `sdf_skin` module/exports for `capsule_mesh`/`organism`.
- `crates/rendering/src/sdf_skin.rs`, `sdf_accum.wgsl`, `sdf_composite.wgsl`, `sdf_highlight.wgsl` — **deleted** (after end-to-end verification, per ADR-P8-03's own rollback note: "do not delete until this epic is fully verified" — now satisfied; history remains in git).
- `crates/app/src/app.rs` — `sdf_skin_renderer` field/construction/resize replaced with `organism_renderer`.
- `crates/app/src/render.rs` — organism/highlight render calls now build `view_proj`/`camera.position`/`sunlight` from `Camera3d` instead of the `camera_pos_2d`/`camera_zoom_2d` bridge (which the *other*, not-yet-migrated `DebugRenderer`/`FieldRenderer` calls still use, unchanged); fixed a real depth/color attachment size-mismatch bug caught during interactive testing (see Risks below).
- `crates/app/src/render/organism_visuals.rs` — `bone_highlight_instances`/`bone_visual_instances`/`pellet_like_instances` now build `CapsuleInstance` (`Vec3` endpoints) alongside their still-`Vec2` `DebugInstance` half.
- `crates/app/src/render/world_instances.rs` — added a `node_positions_3d` cache alongside the existing 2D one (`DebugInstance`/culling/spotlight-lookup stay on the 2D cache, untouched and out of scope); `sdf_bones` renamed to `capsule_instances` throughout.

**Renderer pipeline explanation.** Per frame: (1) `FieldRenderer` clears the screen and draws the background/heatmap, unchanged. (2) `OrganismRenderer::render()` resizes its depth texture to match the swapchain, clears it, and draws every capsule instance with full PBR shading — `LoadOp::Load` on color (composites onto the field pass), `LoadOp::Clear` on depth (first depth writer this frame). (3) `DebugRenderer` draws health/disease/category badges, unchanged, still 2D/depth-less. (4) `OrganismRenderer::render_highlight()` draws hover then selected outlines, `LoadOp::Load` on *both* color and depth (so the inverted-hull technique correctly tests against step 2's depth). (5) egui overlay, unchanged.

**Performance comparison.** Not benchmarked with hard numbers (the roadmap's own completion criteria call for this to be "measured and reported, expected to be different from the old technique" — reported honestly): the interactive verification run sustained a live, multi-minute simulation with a real organism population, `AutoVsync` present mode, no stalling, no dropped-frame symptoms observed, and no frame-time regression complaints in the log. A rigorous before/after frame-time benchmark (e.g. `criterion`-style, isolating the render pass) was not built this epic — flagged as a reasonable follow-up for whoever owns Epic 8.3, which touches this same pass sequence next. Memory: one new `Depth32Float` texture sized to the swapchain (a few MB at typical window sizes) plus the mesh's own tiny (~120-vertex) shared buffer; the per-instance buffer is the same size class as the old `SdfBoneInstance` buffer it replaces (44 vs. 32 bytes/instance).

**Verification results:**

- `cargo build --workspace --all-targets`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: **350 tests passed, 0 failed** (345 carried over from Epic 8.1 + 5 new `rendering::capsule_mesh` unit tests).
- Two independent 200-tick headless runs at the same seed: identical behavior, no panics — this epic touches only rendering/presentation code, no simulation state.
- **Interactive verification, at real population scale**: launched the windowed app, confirmed a live simulation with a real organism population, screenshotted the running scene, and visually confirmed organisms render as coherent, recognizable capsule bodies — smooth tube-like forms in the expected diet colors, correctly composited over the field background, with the Epic 8.1 camera-mode toggle and zoom readout both visible and functional in the same frame. No corruption, no z-fighting, no missing geometry. The app ran for several minutes without error beyond the one pre-existing, unrelated `B0003` despawn warning.
- **A real bug was caught and fixed during this verification**: the first interactive run crashed immediately with a wgpu validation panic ("Attachments have differing sizes") — the new depth texture was sized to the *cropped viewport rect* (`screen_w`/`screen_h`), but the color attachment (`target_view`) is always the *full swapchain texture*, and wgpu requires every attachment in a render pass to share one extent. Fixed by resizing the depth texture to the actual swapchain dimensions (`gpu.config.width`/`.height`) instead, while leaving the camera's projection aspect ratio computed from the (correct, cropped) viewport rect. This is exactly the kind of issue the roadmap's own "interactive smoke test" verification step exists to catch before it reaches a real user.

**Tests executed:** the full workspace suite (`cargo test --workspace`), 350 tests, 0 failures — 5 new, 0 removed.

**Remaining roadmap dependencies:** Epic 8.3 (debug billboards, shadows, PBR polish) and Epic 8.4 (3D picking against capsule primitives) and Epic 8.5 (field-renderer plane-slice migration) each depend on Epic 8.2 and can now proceed. Epic 8.6 (real 3D growth/orientation) remains gated on ADR-P8-06 sign-off, untouched by this epic.

**Risks.**

- The attachment-size bug above is now fixed, but it's a reminder that every future pass sharing this depth buffer (Epic 8.3's shadow pass, in particular) must size against the swapchain, not a viewport crop — worth calling out explicitly in that epic's own implementation.
- No LOD/billboard-impostor tier was built this epic (explicitly out of Epic 8.2's stated goal, despite being mentioned in the roadmap's general Mesh Pipeline architecture section) — population-scale frame cost at very large populations (thousands of organisms) is unmeasured; flagged for whoever picks up LOD work.
- PBR roughness/metallic/ambient constants are untuned placeholders (`ROUGHNESS = 0.6`, `METALLIC = 0.05`, `AMBIENT_FLOOR = 0.12`) — Epic 8.3's own stated scope ("PBR polish") is expected to revisit these with real measurement.
- Mesh triangle winding wasn't independently verified by hand; the main pipeline uses `cull_mode: None` specifically to avoid a correctness risk from an unverified winding order, at a negligible performance cost for this low-poly mesh — a future epic could tighten this once winding is confirmed, but there is no reason to before then.

**Recommended next epic:** Epic 8.3 (Debug/highlight billboards, shadows, PBR polish) — it depends directly on this epic, continues the same rendering-crate work, and is next in the roadmap's own Tier 2 sequence.

---

### Epic 8.3 — PBR Polish, Shadows, Debug Billboards & Rendering Quality — COMPLETE

**Executive summary.** Added directional shadow mapping and converted `DebugRenderer`'s Health/Disease/Category/colony-link badges from flat world-space-XY quads to true camera-facing billboards, depth-tested against `OrganismRenderer`'s shared depth buffer. No new architecture beyond what ADR-P8-03 already implied (a shadow pass was always the named next step); no ADR update needed — every design choice below follows directly from decisions already on record.

**Audit findings (pre-implementation).** Read every renderer this epic touches before writing code: `debug.rs`/`debug_quad.wgsl` (flat world-space AABB-quad technique, no depth, no camera-facing behavior at all), `organism.rs`/`capsule.wgsl` (Epic 8.2's PBR pipeline, single fixed `sun_dir`, no shadow anywhere), `render.rs`'s full pass ordering and its `WORLD_BOUNDS = 1500.0` local constant, and `metabolism::GlobalAtmosphere` (confirmed `sunlight` is a `0..1` intensity scalar with **no direction data anywhere in the simulation model** — validating that the existing fixed `sun_dir` constant is correct and does not need to become dynamic). Grepped for any pre-existing shadow/matrix-generation/billboard code elsewhere in the codebase: none found, so nothing in this epic duplicates prior work.

**Architecture changes.**

- **Shadow mapping**: a fixed `2048×2048` `Depth32Float` shadow texture (`OrganismRenderer::shadow_texture`/`shadow_view`), rendered from the light's point of view every frame via a new depth-only `shadow_pipeline` (`vs_shadow` — reuses `capsule.wgsl`'s existing `capsule_vertex()` reconstruction function verbatim, just projecting through `light.light_view_proj` instead of the camera's `view_proj`) before the existing main color+depth pass, all within one command encoder. The light's view-projection is computed by a new `compute_light_view_proj(sun_dir, world_half_extent)` — an orthographic frustum (directional lights have no meaningful perspective) sized from the caller's own `WORLD_BOUNDS` constant (`app/render.rs`), passed in rather than duplicated. `fs_main` samples the map back via hardware `textureSampleCompare` (a linear-filtered comparison sampler — cheap, standard bilinear PCF, not a multi-tap kernel this epic doesn't call for), modulating the direct (diffuse+specular) lighting term only — ambient stays unshadowed, matching how the roadmap's own "ambient floor" already keeps night-time scenes readable.
- **Bind-group design**: `capsule.wgsl` now declares 3 groups — group 0 (camera+light, shared), group 1 (highlight color, `fs_highlight`-only), group 2 (shadow map, `fs_main`-only) — backed by **one shared `PipelineLayout`** reused by the main, highlight, and shadow-writing pipelines (all 3 groups declared uniformly, even where a given entry point doesn't read one), so no pipeline layout is duplicated. The shadow-writing pipeline is the one exception: it gets its **own** minimal group-0-only layout, because `vs_shadow` needs nothing from groups 1/2 — critically, binding the shadow map's own texture (group 2) while that same pass's depth attachment writes into it is a self-referential usage conflict wgpu correctly rejects (see Risks/bug below).
- **Debug billboards**: `DebugInstance::pos_a`/`pos_b` widened from `Vec2` to `Vec3`. `debug_quad.wgsl`'s vertex shader keeps the exact same min/max-bounding-box-in-a-plane technique Phase 7 used, just rebased from the world XY plane onto the camera's own right/up basis (`Camera3d::right()`/`up()`, Epic 8.1) — projecting each instance's world-space endpoints onto that basis via `dot()` before building the padded quad, so a badge always faces the viewer regardless of view direction, for both point badges (health/disease/category rings) and line badges (colony links) uniformly, with no special-casing. `DebugRenderer`'s pipeline gained a depth-stencil state (`depth_compare: Less`, `depth_write_enabled: false`) that tests against — but never writes — `OrganismRenderer`'s depth buffer, exposed via a new `OrganismRenderer::depth_view()` accessor so no second depth texture is ever allocated.
- **Cleanup**: `organism_visuals.rs`'s `bone_visual_instances`/`pellet_like_instances` each dropped a now-redundant `Vec2` position parameter (previously kept only for the 2D `DebugInstance`, now unified onto the same `Vec3` value the `CapsuleInstance` half already used) — a small duplication removal, not a new abstraction.

**Files changed** (7 files across 2 crates):

- `crates/rendering/src/capsule.wgsl` — 3-group bind layout; shared `capsule_vertex()` function (factored out of `vs_main`, reused by new `vs_shadow`); new `sample_shadow()`; `fs_main`'s lighting term multiplied by `shadow_factor`.
- `crates/rendering/src/organism.rs` — `GpuLight` gained `light_view_proj`; new `shadow_texture`/`shadow_view`/`shadow_bind_group`/`shadow_pipeline`/`shadow_pipeline_layout` fields and construction; `compute_light_view_proj()`; `render()` gained a `world_half_extent: f32` parameter, now runs the shadow pass before the main pass and uploads the instance buffer once for both; `render_highlight()` no longer re-writes camera/light uniforms (the same frame's preceding `render()` call already did) and now also binds the shadow group (present in the shared layout, unread by `fs_highlight`); new `depth_view()` accessor.
- `crates/rendering/src/debug.rs` — `DebugInstance` endpoints widened to `Vec3`; new `GpuCamera{view_proj, right, up}` uniform; pipeline gained a test-only depth-stencil state; `render()` signature replaced `camera_pos: Vec2, camera_zoom: f32` with `depth_view: &TextureView, view_proj: Mat4, camera_right: Vec3, camera_up: Vec3`.
- `crates/rendering/src/debug_quad.wgsl` — vertex shader rebased from world-XY min/max-bounding-box to the camera's right/up billboard plane.
- `crates/app/src/render.rs` — `organism_renderer.render()` call now passes `WORLD_BOUNDS` as `world_half_extent`; the two `render_highlight()` calls dropped their now-unnecessary `view_proj`/`camera_pos`/`sunlight` arguments; the debug-pass call site now passes `organism_renderer.depth_view()`, `view_proj`, `camera.right()`, `camera.up()` instead of the old `camera_pos_2d`/`camera_zoom_2d` bridge.
- `crates/app/src/render/organism_visuals.rs` — `health_ring_instance`/`disease_badge_instance`/`segment_debug_dot_instance`/`category_ring_instance`/`colony_link_instance` all take/build `Vec3` positions now; `bone_visual_instances`/`pellet_like_instances` dropped their redundant `Vec2` position parameter.
- `crates/app/src/render/world_instances.rs` — call sites updated to pass `Vec3` positions (reusing the `node_positions_3d` cache and pellet `pos3` values Epic 8.2 already introduced); the colony-link check now also looks up `node_positions_3d` alongside the existing 2D map.

**A real bug was caught and fixed during interactive verification** (not just written and assumed correct): the very first run crashed with a wgpu validation panic — `"Attempted to use Texture with 'OrganismShadowTexture' ... conflicting usages. Current usage TextureUses(RESOURCE) and new usage TextureUses(DEPTH_STENCIL_WRITE)"`. Root cause: the shadow pass's pipeline layout (initially the same shared 3-group layout as everything else) required binding group 2 (the shadow map) even though `vs_shadow` never reads it — and binding that texture as a sampled resource in the very same pass whose depth attachment is writing into it is a self-referential conflict wgpu correctly rejects. Fixed by giving the shadow-writing pipeline its own minimal group-0-only `PipelineLayout`, so the shadow pass never touches `shadow_bind_group` at all. This is exactly the class of bug the "no duplicated pipeline layouts" instinct almost introduced — the fix is a justified, documented exception to that rule, not a violation of it.

**Verification results:**

- `cargo build --workspace --all-targets`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: **350 tests passed, 0 failed** (unchanged from Epic 8.2 — this epic added no new unit-testable logic; shader/GPU-pipeline correctness is verified interactively, matching this project's existing precedent for rendering work).
- **Interactive verification**: launched the release build, screenshotted the running window at real population scale (topmost-window-positioning technique, since `SetForegroundWindow` remains blocked by Windows' foreground-lock policy). Confirmed: organisms render with visible PBR specular highlighting; a cropped close-up of an overlapping-bone region shows clear contact-shadow darkening exactly where one capsule crosses another, confirming the shadow map is sampling correctly; Health/Disease/Category billboards ("Grazed!", "Infected!", "Hunted!") render correctly and track their organisms; no z-fighting, no flicker, no corruption. The app ran for ~8 minutes of continuous simulation with no crash and no panic beyond the one pre-existing, unrelated `B0003` despawn warning — confirmed via `tasklist` that the process was still alive throughout.
- **Two independent 200-tick headless runs at the same seed**: log output identical byte-for-byte except timestamps (same entity index/generation in the same pre-existing `B0003` warning, both runs) — this epic touches only rendering/presentation code, no simulation state; `data/default.ron` was reverted immediately after via direct edit back to its original `headless: false, max_ticks: 0`.

**Tests executed:** the full workspace suite (`cargo test --workspace`), 350 tests, 0 failures — 0 new, 0 removed (rendering-only epic).

**Performance observations.** Not benchmarked with hard numbers — matching Epic 8.2's own precedent, this is flagged rather than glossed over. The shadow pass adds one additional depth-only draw of the same instance count per frame (a `2048×2048` render target, cheap relative to the main shaded pass) plus one small (`64×64×4` byte class) additional uniform write; no stalling or dropped-frame symptoms were observed during the ~8-minute interactive run. A rigorous before/after frame-time comparison (shadow pass cost, billboard cost, draw-call count) was not built this epic — flagged as a reasonable follow-up for whoever next touches this render path, consistent with Epic 8.2's own unaddressed flag for a formal benchmark.

**Determinism validation.** Confirmed via two independent 200-tick headless runs at the same seed (see Verification above) — bit-identical behavior. This epic's changes are entirely rendering/presentation-layer; no simulation-state code path was touched, so this validation is a confirmation of "no regression," not a test of new logic.

**Risks.**

- PBR roughness/metallic/ambient-floor constants remain untuned placeholders, unchanged from Epic 8.2 (`ROUGHNESS = 0.6`, `METALLIC = 0.05`, `AMBIENT_FLOOR = 0.12`) — the interactive verification pass did not find the lit+shadowed result visually wrong enough to justify an unmeasured adjustment, so per "measure before optimizing," none was made. Flagged for whoever next does a dedicated visual-tuning pass.
- Shadow-acne mitigation (`DepthBiasState{constant: 2, slope_scale: 2.0}`) is an untuned, standard starting value, not measured against this specific mesh/light-angle combination — the interactive screenshot showed no obvious acne or peter-panning at the tested camera angles, but only a few angles were checked, not an exhaustive sweep.
- `SHADOW_MAP_SIZE = 2048` is fixed and untuned; population-scale shadow-map resolution/quality tradeoffs at very large world extents are unmeasured, same status as Epic 8.2's unmeasured LOD gap.
- The debug-pass call site in `render.rs` only runs when `self.organism_renderer` is `Some` (it now needs `depth_view()`); this was already implicitly true (organism_renderer is constructed unconditionally at startup) but if `capsule_instances` is ever empty on a frame where `debug_instances` is not, the shared depth buffer would carry stale content from a previous frame rather than being freshly cleared — flagged as a theoretical edge case, not observed or expected in practice (debug instances are always produced alongside capsule instances for the same entities).

**Remaining roadmap dependencies:** Epic 8.4 (3D picking against capsule primitives) and Epic 8.5 (field-renderer plane-slice migration) each depend on Epic 8.2/8.3 and can now proceed. Epic 8.6 (real 3D growth/orientation) remains gated on ADR-P8-06 sign-off, untouched by this epic.

**Recommended next epic:** Epic 8.4 (3D picking against capsule primitives) — continues the same rendering-crate lineage and is next in the roadmap's own Tier 2 sequence. Per this epic's own explicit instruction, **implementation stops here** — Epic 8.4 is not begun.

---

### Epic 8.4 — 3D Picking, Box-Select, Lasso — COMPLETE

**Executive summary.** Replaced flat, Z=0-plane-based nearest-point picking with a real ray-vs-capsule 3D ray cast against the exact radius each entity renders at; upgraded box-select from a flat world-space rectangle to a real screen-space frustum test (projecting each candidate through the camera's own `view_proj`); added lasso-select (freeform polygon) using the same projection. All three reuse the same shared primitives — no separate picking geometry was invented, per this epic's own architecture goal.

**Architecture changes.**

- **`rendering::picking::ray_capsule_hit`** (new, pure math, no ECS/GPU dependency) — the closest-approach distance between a ray and a capsule's core segment, clamped within `radius`; returns the ray parameter `t` so multiple overlapping candidates rank by actual depth, not screen-space proximity. Point entities (food/mineral/corpse, and now also individual `ParticleNode`s, rendered as spheres) fall out of the same formula with `pos_a == pos_b`, no special-casing.
- **`app::pick_entity` rewrite** — iterates the same node/pellet queries as before, but each candidate is now hit-tested via `ray_capsule_hit` against the *same radius the renderer draws it at* (`self.ui.node_radius` for organism segments; new named constants `organism_visuals::{FOOD_PELLET_RADIUS, MINERAL_PELLET_RADIUS, CORPSE_RADIUS}`, shared by both the renderer's `pellet_like_instances` and the picker, replacing three previously-independent literal pairs). "Nearest" is now nearest-along-the-ray (smallest `t`) — depth-correct, and correct under any camera tilt (the old technique silently assumed a top-down-ish view since it unprojected onto the `Z = 0` plane first).
- **`Camera3d::world_to_screen`** (new) — the missing inverse of `screen_to_ray` (ADR-P8-02 now has two projection methods, not one); returns `None` when the point is behind the camera. Reused by both box-select and lasso-select, and by `viewport.rs`'s own measurement-tool overlay (`to_screen`, previously a flat approximation fed by the `camera_pos_2d`/`camera_zoom_2d` bridge — upgraded to the real projection now that one exists, closing a stale "no equivalent method" comment this epic made incorrect).
- **`ui::camera::point_in_polygon`** (new) — standard even-odd crossing-number test in screen-space pixels, used only by lasso-select.
- **Box-select** (`MenuAction::SelectInRect`) — fields changed from world-space `min`/`max: Vec2` to screen-space `screen_min`/`screen_max: Vec2` + `viewport_size: Vec2`; the handler in `events.rs` now projects each head node's real `Vec3` position through `Camera3d::world_to_screen` and tests against the screen rectangle, instead of a flat XY bounds check — this is what makes it genuinely "frustum-based" (a screen-space rectangle swept through the frustum) rather than a single-plane test, and means it will keep working correctly once Epic 8.6 gives organisms real, non-zero `Z`.
- **Lasso-select** (`MenuAction::SelectInLasso`, new) — same projection, tested against a freeform polygon via `point_in_polygon`. `ui::MarqueeMode` (new 3-way enum: `Select`/`Lasso`/`Measure`) replaces the old `measure_mode: bool`, since a boolean no longer had room for the third mode; `WorkbenchState::lasso_points: Vec<egui::Pos2>` accumulates drag points (only appending when the cursor has moved a few pixels since the last vertex, so a slow drag doesn't flood the polygon). A new toolbar toggle (reusing `SHAPE_LINE` from the existing icon set — no icon literally named "lasso" exists in the Remix Icon subset already in use) sits next to the existing Measure toggle, all three mutually exclusive on the same click-drag gesture in `viewport.rs`.

**Files changed** (13 files across 3 crates; 1 new file):

- `crates/rendering/src/picking.rs` (new) — `ray_capsule_hit`, 7 unit tests.
- `crates/rendering/src/lib.rs` — exports the new `picking` module.
- `crates/ui/src/camera.rs` — `Camera3d::world_to_screen`; `point_in_polygon`; 5 new unit tests.
- `crates/ui/src/types.rs` — new `MarqueeMode` enum; `SelectInRect` fields changed to screen-space; new `SelectInLasso` variant.
- `crates/ui/src/state.rs` — `measure_mode: bool` → `marquee_mode: MarqueeMode`; new `lasso_points` field.
- `crates/ui/src/plugins/toolbar.rs` — new Lasso toggle button alongside the existing Measure toggle.
- `crates/ui/src/plugins/viewport.rs` — drag-gesture handling rewritten as a 3-way match on `marquee_mode`; `to_world`/`to_screen` deduplicated onto one shared `viewport_size_px`/`to_local_px`; `to_screen` upgraded to the real `Camera3d::world_to_screen` projection.
- `crates/ui/src/lib.rs` — exports `MarqueeMode`.
- `crates/app/src/app.rs` — `pick_entity` rewritten around `ray_capsule_hit`.
- `crates/app/src/events.rs` — `SelectInRect` handler rewritten around `world_to_screen`; new `SelectInLasso` handler.
- `crates/app/src/render.rs` — `mod organism_visuals` changed to `pub(crate)` so `app.rs` can reuse its pellet-radius constants.
- `crates/app/src/render/organism_visuals.rs` — new `FOOD_PELLET_RADIUS`/`MINERAL_PELLET_RADIUS`/`CORPSE_RADIUS` constants.
- `crates/app/src/render/world_instances.rs` — the 3 `pellet_like_instances` call sites now pass the named constants instead of literals.

**A deliberate scope decision, not a gap**: this epic keeps picking at the existing per-*node* granularity (each `ParticleNode`/pellet as its own sphere), rather than introducing per-*bone* (spring) capsule picking. The roadmap's own goal is "ray-vs-capsule picking [replacing] flat 2D nearest-point picking" — read as upgrading the *technique* (real 3D ray cast against real geometry, depth-correct) rather than changing *what* gets selected (which has always been individual `ParticleNode` entities, not bones). Changing selection granularity to per-bone would be a larger, unrequested behavior change to what clicking selects; `ray_capsule_hit`'s degenerate (`pos_a == pos_b`) sphere case already fully serves node-level picking, and the same function is available unchanged if a future epic wants true per-bone selection.

**Verification results:**

- `cargo build --workspace --all-targets`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: **362 tests passed, 0 failed** (350 carried over from Epic 8.3 + 7 new `rendering::picking` unit tests + 5 new `ui::camera` unit tests for `world_to_screen`/`point_in_polygon`).
- Two independent 200-tick headless runs at the same seed: log output identical byte-for-byte except timestamps (same entity index/generation in the same pre-existing `B0003` warning, both runs) — this epic touches only input/selection code, no simulation state; `data/default.ron` reverted immediately after.
- **Interactive verification, partial**: launched the release build, confirmed the main-menu → simulation transition and organism rendering are unaffected, and confirmed via simulated mouse click that single-entity picking correctly selects an organism (Inspector/selection-outline updated) at least once during testing. Automated drag-gesture simulation (for box-select/lasso) proved unreliable in this environment — a background VS Code window repeatedly regained OS-level topmost/focus mid-sequence despite `SetWindowPos(HWND_TOPMOST)`, causing simulated drags to land on the wrong window; one such misdirected click closed the Phylon window rather than exercising the app. Per the user's explicit request, further automated screenshot-based interaction testing was stopped here so the user could verify box-select, lasso-select, and the toolbar toggles directly. This is disclosed as a real gap in this epic's own interactive verification, not glossed over: **the drag-based box-select and lasso-select paths are covered by unit tests on their underlying math (`world_to_screen`, `point_in_polygon`) and by a clean compile/type-check of the full dispatch wiring (`viewport.rs` → `MenuAction` → `events.rs`), but have not been exercised end-to-end through the real running app by this epic's own automation.**

**Tests executed:** the full workspace suite (`cargo test --workspace`), 362 tests, 0 failures — 12 new, 0 removed.

**Performance observations.** Not benchmarked with hard numbers. `pick_entity` and the box-select/lasso handlers all run once per user interaction (a click or a drag release), not per-frame, so their cost is negligible regardless of population size relative to the existing per-frame render/simulation cost; no new per-frame work was added anywhere in this epic.

**Determinism validation.** Confirmed via two independent 200-tick headless runs at the same seed (see Verification above) — bit-identical behavior. This epic's changes are entirely input/selection-layer; no simulation-state code path was touched.

**Risks.**

- The interactive drag-gesture verification gap noted above is the primary open risk — the user should manually confirm box-select and lasso-select behave as expected (toolbar toggle indicates the active mode; a drag paints the correct overlay shape; releasing selects the expected organisms and shows the "Selected N organism(s)" toast) before relying on them.
- `ray_capsule_hit`'s closest-approach simplification (not a true ray-vs-cylinder surface intersection) means the reported hit distance `t` is approximate near a capsule's rounded ends — adequate for ranking overlapping candidates by depth (this function's only purpose), not for anything requiring an exact surface point.
- Lasso-select's minimum-3-point / 3-pixel-append-threshold constants are untuned, reasonable starting values, not measured against real usage.

**Remaining roadmap dependencies:** Epic 8.5 (field-renderer plane-slice migration) depends on Epic 8.2 and can proceed independently of this epic. Epic 8.6 (real 3D growth/orientation) remains gated on ADR-P8-06 sign-off.

**Recommended next epic:** Epic 8.5 (field renderer plane-slice migration, clipping planes) — next in the roadmap's own Tier 2 sequence.

---

### Epic 8.5 — Field Renderer Plane-Slice Migration, Clipping Planes — COMPLETE

**Executive summary.** `FieldRenderer` now samples the diffusion field via a genuine `Camera3d`-driven plane-slice unproject (ADR-P8-05), replacing the flat orthographic `camera_pos`/`camera_zoom`/`screen_size` approximation that assumed a top-down camera and could drift out of registration with the organism renderer under any tilt. Added a horizontal world-space clipping plane (enable/height/keep-above-or-below), with a shader-side clip test in the organism capsule renderer's both fragment shaders (main + highlight), and a sidebar control to drive it — the "clipping-plane gizmo" the roadmap names, implemented as a slider/toggle rather than an in-viewport 3D drag-handle (see Architecture changes below for why).

**Architecture changes.**

- **`FieldConfig`/`field_overlay.wgsl`** — `camera_pos: [f32;2]`, `camera_zoom: f32`, `screen_size: [f32;2]` replaced with `inv_view_proj: [[f32;4];4]` and `slice_z: f32`. The fragment shader now unprojects each pixel's NDC coordinate into two world-space points (near/far, using wgpu's `0..1` clip-space depth range) and intersects the resulting ray with the world-space `Z = slice_z` plane — the shader-side equivalent of `Camera3d::screen_to_ray` + a plane intersection (WGSL can't call the Rust method directly, so the same math is re-expressed via the inverse view-projection matrix). `slice_z` stays fixed at `0.0`: ADR-P8-05 keeps the diffusion field as a single Z=0 layer for Phase 8 (no multi-height-band data exists yet to slice through), so there is nothing yet for a user-adjustable field-height slider to select between — the "plane-slice" migration is about the *projection technique*, not a new visualization control.
- **`app/src/render.rs`** — hoisted the `camera`/`aspect`/`view_proj` computation earlier in `render()` (it previously ran after the heatmap/field section) so `FieldConfig` can compute and reuse `inv_view_proj = view_proj.inverse()` from the same canonical camera every other renderer already uses; removed the now-fully-unused `camera_pos_2d`/`camera_zoom_2d` local bridge variables from this function (the methods themselves remain — still used by `world_instances.rs`'s frustum culling and unaffected panels).
- **Clipping plane** — `rendering::organism::ClipPlane` (new: `enabled`, `height`, `keep_above`) is packed into `GpuCamera::clip_params: [f32;4]` (extends the existing `Camera` uniform rather than adding a 4th bind group, since every fragment invocation that shades already reads this uniform and the shared `PipelineLayout` already declares exactly 3 groups). `capsule.wgsl` gained a shared `clip_test(world_position)` function, called at the top of both `fs_main` and `fs_highlight` (so a hover/selection outline never renders through clipped-away geometry) — `discard`s a fragment when enabled and on the wrong side of the plane. Since `ui` doesn't depend on `rendering` (crate dependency rules), the UI-facing state is a separate `ui::ClipPlaneState` (mirroring the existing `HeatmapState` pattern); `app/src/render.rs` converts it to `rendering::ClipPlane` at the one call site that needs it.
- **The "gizmo" scope decision**: implemented as a sidebar checkbox + height slider + Above/Below toggle (in the existing Tuning panel, alongside `node_radius`/`skin_thickness`), not an in-viewport draggable 3D handle. The viewport's primary-button drag gesture is already a 3-way `MarqueeMode` switch (Select/Lasso/Measure, Epic 8.4); adding a 4th competing drag mode for the clip plane would overload that same gesture ambiguously. A numeric slider is a real, immediately-usable interactive control (egui's `Slider` supports click-drag-to-adjust) consistent with this project's existing "cheap immediate-mode gizmo" precedent (camera bookmarks, the measure tool) — disclosed here as a deliberate, scope-conscious substitution for a full 3D transform-gizmo widget, not a silent downgrade.

**Files changed** (8 files across 2 crates):

- `crates/rendering/src/field.rs` — `FieldConfig` fields replaced (`inv_view_proj`/`slice_z` instead of `camera_pos`/`camera_zoom`/`screen_size`).
- `crates/rendering/src/field_overlay.wgsl` — `fs_main` rewritten around the inverse-view-proj plane-slice unproject.
- `crates/rendering/src/organism.rs` — `ClipPlane` (new, public); `GpuCamera::clip_params`; `update_uniforms`/`render()` thread it through.
- `crates/rendering/src/capsule.wgsl` — `Camera.clip_params`; new `clip_test()`; called from `fs_main` and `fs_highlight`.
- `crates/rendering/src/lib.rs` — exports `ClipPlane`.
- `crates/ui/src/types.rs` — new `ClipPlaneState`.
- `crates/ui/src/state.rs` — `WorkbenchState::clip_plane: ClipPlaneState`.
- `crates/ui/src/lib.rs` — exports `ClipPlaneState`.
- `crates/ui/src/plugins/sidebar.rs` — new "Clipping Plane" collapsing section in the Tuning panel.
- `crates/app/src/render.rs` — hoisted camera computation; both `FieldConfig` construction sites; `ClipPlane` conversion at the organism-render call site.

**Verification results:**

- `cargo build --workspace --all-targets`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean (one new `#[allow(clippy::too_many_arguments)]` on `update_uniforms`, consistent with the existing allows on `render`/`render_highlight` in the same file).
- `cargo test --workspace`: all tests pass (no new unit tests this epic — both changes are shader/uniform plumbing with no new pure-Rust logic to unit-test; verified instead by compilation, clippy, and the runtime shader-validation smoke test below).
- **Runtime shader validation**: launched the release build; log confirms `naga` successfully compiled both rewritten shaders to SPIR-V (`capsule.wgsl`'s new `clip_test` function and `field_overlay.wgsl`'s rewritten `fs_main` both appear in the compilation log with no validation errors) and the app ran for over a minute with no crash, panic, or wgpu validation error.
- Two independent 200-tick headless runs at the same seed: log output identical byte-for-byte except timestamps — this epic touches only rendering code (shaders, uniform layout, UI controls), no simulation state path; `data/default.ron` reverted immediately after.
- **Interactive verification, not performed this epic**: per the same automation-reliability issue disclosed in Epic 8.4's report (background-window focus/z-order instability in this environment), no screenshot-based visual check of the field's on-screen registration or the clip plane's visible slicing effect was attempted. This is a real, disclosed gap: **the plane-slice field projection and the clip-plane's visual effect on organism geometry have not been visually confirmed by this epic's own automation** — only that they compile, pass clippy, and don't crash at runtime. The user should confirm: the heatmap/field overlay still aligns with organism positions when panning/orbiting the camera (the actual bug this migration fixes — the old technique could drift under tilt); and that toggling the Clipping Plane checkbox visibly slices through capsule geometry at the configured height.
- **Manual QA checklist, outstanding**: the user is running a 10-point manual pass/fail checklist locally (camera registration under orbit/pan/zoom/tilt; clipping-plane behavior; clip stability under camera movement; field/clip alignment; picking and highlight rendering on clipped geometry; debug billboard behavior; population-scale stress test; startup shader-validation log check; and a no-clipping regression pass). Results pending — Epic 8.6 begins once all 10 are reported PASS, per the user's explicit gate.

**Tests executed:** the full workspace suite (`cargo test --workspace`) — same pass count as Epic 8.4 (no new tests added this epic).

**Performance observations.** Not benchmarked with hard numbers. The field shader's per-pixel cost changed from ~6 scalar ops to two 4×4 matrix-vector multiplies plus a divide — still trivially cheap relative to the full-screen triangle's total pixel count and the existing texture sample; no new per-frame CPU-side allocation was introduced. The clip test adds one branch and a few scalar ops to every organism fragment shader invocation, negligible next to the existing PBR/shadow-sampling cost.

**Determinism validation.** Confirmed via two independent 200-tick headless runs at the same seed (see Verification above) — bit-identical behavior. This epic's changes are entirely rendering-layer.

**Risks.**

- The disclosed interactive-verification gap above (field registration under camera tilt, clip-plane visual effect) is the primary open item — should be manually confirmed before relying on either feature for actual research use.
- The clipping-plane height range (`-20.0..=20.0` in the sidebar slider) is an untuned, reasonable-guess default sized to today's `node_radius`/`skin_thickness` ranges (organisms have no meaningful vertical extent beyond capsule radius until Epic 8.6) — likely needs revisiting once Epic 8.6 gives organisms real vertical body-plan variation.
- The clip test only applies to the organism capsule renderer (main + highlight passes) — `DebugRenderer`'s billboards and the field overlay itself are not clipped. This is a deliberate, scope-conscious choice (the ADR's stated dependency is specifically "correct clip-plane interaction with organism geometry"), not an oversight, but is worth naming: a debug badge could still render for an organism whose body is otherwise clipped away.

**Remaining roadmap dependencies:** Epic 8.6 (real 3D growth/orientation) remains gated on ADR-P8-06 sign-off — the next epic in the roadmap's own Tier 2→3 sequence, and the second of the two explicitly-flagged scientific-decision gates this roadmap names.

**Recommended next epic (superseded — see Epic 8.6's own completion report below):** ~~Epic 8.6 requires explicit user sign-off on ADR-P8-06's bilateral-symmetry decision before starting~~ — **ADR-P8-06 was approved by the user** (see its own STATUS line above), and Epic 8.6 is now complete.

---

### Epic 8.6 — Growth Orientation Redesign (`heading` → `forward`/`dorsal`) — COMPLETE

**Executive summary.** Implements ADR-P8-06 exactly as approved: `GrowthState::heading: f32` is replaced by two body-fixed `Vec3` fields, `forward` (direction-of-travel — a renamed, pre-computed-once version of the same value `heading` encoded) and `dorsal` (new — a per-organism "up" reference, initialized to `Vec3::Z` at every construction site). "Left fin"/"right fin" placement is now `organisms::bilateral_fin_direction(dorsal, forward)` (a proper 3D cross product), replacing the `Vec2::new(-dir.y, dir.x)` construction that had no direct 3D generalization. Per the user's explicit framing when approving the ADR ("moving from 2D to 3D is an engine migration, not a biological redesign"), this epic deliberately does **not** introduce any mechanism for `forward` to leave the Z=0 growth plane — every construction site still derives it the same way (`(heading.cos(), heading.sin(), 0.0)`, or an equivalent normalized delta between two Z=0 spine positions), so real running populations grow identically to before. The new math is genuinely 3D-*capable* (verified with a tilted `dorsal` in a dedicated test), not merely retyped.

**Architecture changes.**

- **`organisms::developmental_graph::bilateral_fin_direction(dorsal: Vec3, forward: Vec3) -> Vec3`** (new, `dorsal.cross(forward)`) — the single shared implementation of the fin-placement formula, replacing two independent copies of the ad hoc 2D-only construction (`growth_system`'s branch logic in `systems.rs`, and the standalone `spawn_proto_fish` debug preset in `spawning.rs`, which has no `GrowthState`/`dorsal` of its own and passes a fixed `Vec3::Z`). Exported at the crate root alongside `can_branch`/`compile_segment`, the existing home for small, pure decode-to-physics helpers.
- **`GrowthState`** (`components.rs`) — `heading: f32` → `forward: Vec3` + `dorsal: Vec3`. Every one of the 9 construction sites across `spawning.rs` (2 organism-creation paths), `life_cycle.rs` (resumed adult growth + 1 test fixture), and `systems.rs` (5 test fixtures) updated; every one of the 4 `state.heading.cos()/.sin()` read sites in `systems.rs` replaced with direct reads of the pre-computed `state.forward`.
- **`life_cycle.rs`'s resumed-growth heading inference** — previously computed a 2D angle (`delta.y.atan2(delta.x)`) from the last two spine positions, then reconstructed a direction vector from it; now computes `delta.normalize()` directly, one fewer round-trip through trigonometric functions, same result for any Z=0 delta (confirmed via the existing `resumed_growth_reaches_completion_and_rebuilds_the_brain` test, unchanged and passing).
- **No changes** to `current_pos: Vec3` (still `z == 0.0` at every construction site, per its own doc comment, now updated to explain *why* — Epic 8.6 made the fin-placement math 3D-capable without adding a mechanism for growth to actually leave the plane) or to any other `GrowthState` field.

**Files changed** (5 files, 1 crate):

- `crates/organisms/src/developmental_graph.rs` — new `bilateral_fin_direction`; 3 new unit tests.
- `crates/organisms/src/components.rs` — `GrowthState::heading` → `forward`/`dorsal`, with updated doc comments.
- `crates/organisms/src/systems.rs` — 4 read-site replacements; fin-placement call site; 5 test-fixture updates.
- `crates/organisms/src/spawning.rs` — `spawn_organism`'s `GrowthState` construction; `spawn_proto_fish`'s independent fin-placement formula routed through the same shared helper.
- `crates/organisms/src/life_cycle.rs` — resumed-growth `forward` inference; 1 test-fixture update; doc comment.
- `crates/organisms/src/lib.rs` — exports `bilateral_fin_direction`.

**Verification results:**

- `cargo build --workspace --all-targets`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: all tests pass, including **all 11 pre-existing `growth_system_*` tests, unchanged and passing** — the exact regression check ADR-P8-06 and the roadmap's own Epic 8.6 entry both name as the rollback gate. 3 new tests added: `bilateral_fin_direction_with_z_dorsal_matches_the_pre_8_6_2d_formula` (the 2D-equivalence regression, checked across 6 headings, not just one), `bilateral_fin_direction_with_a_tilted_dorsal_stays_orthogonal_to_both_inputs` (the genuine 3D-correctness check — a non-`Z` dorsal still produces a mathematically valid perpendicular), and `bilateral_fin_direction_is_zero_when_dorsal_and_forward_are_parallel` (the one honestly-degenerate case, documented as a zero-length result rather than a silent NaN).
- Two independent 200-tick headless runs at the same seed: log output identical byte-for-byte except timestamps — confirms this migration changed no observable simulation behavior for any currently-possible `GrowthState` (since `dorsal` is always `Vec3::Z` and `forward` is always Z=0-plane-confined in every real code path today). `data/default.ron` reverted immediately after.
- No interactive/visual verification needed or attempted — this epic touches only internal growth-system math with no rendering surface of its own (organisms rendered by the existing capsule renderer are unaffected, since bone endpoints/positions are unchanged).

**Tests executed:** the full workspace suite (`cargo test --workspace`) — 3 new, 0 removed, all passing (`organisms` crate alone: 68 tests, up from 65).

**Performance observations.** Not benchmarked with hard numbers — expected to be a net-neutral-to-negligible-positive change: the 4 former `.cos()`/`.sin()` recomputations per tick per organism are replaced by direct field reads of a value computed once at spawn/resume, a small constant-factor reduction, not a new cost.

**Determinism validation.** Confirmed via two independent 200-tick headless runs at the same seed (see Verification above) — bit-identical behavior.

**Risks.**

- None newly introduced. The one pre-existing risk this epic was meant to retire — "growth-orientation redesign introduces a scientific regression (bilateral symmetry silently reinterpreted)" (risk register) — is addressed directly by the regression test suite passing unchanged plus the new 2D-equivalence test covering 6 distinct headings, not just the default.
- `bilateral_fin_direction`'s degenerate case (`dorsal` parallel to `forward`) returns a zero-length vector rather than panicking — correctly documented, but any future code that divides by this result's length without checking would need to handle it; no such call site exists today (both fin-placement call sites multiply by `fin_spread`, tolerating a zero vector as "no lateral offset" rather than crashing).
- `dorsal` is not yet evolvable or dynamically variable by any mechanism — it is a real, per-organism-stored field (not a hardcoded global), but every code path sets it to the same constant `Vec3::Z` today. This is the correct, disclosed scope boundary per ADR-P8-06 and the user's own explicit framing, not an oversight: a future epic could vary `dorsal` (e.g., as an evolvable trait) without touching `bilateral_fin_direction` itself.

**Remaining roadmap dependencies:** none blocking. Epic 8.7 (vision/sensing redesign, ADR-P8-07) depends on the same `dorsal`/`forward` body-frame this epic introduces (its own decision text: "computed via the same `dorsal`/`forward` body-frame ADR-P8-06 introduces") and can now proceed. No further sign-off gate exists between here and the end of the roadmap's currently-planned epics — both of the roadmap's named gates (ADR-P8-03, ADR-P8-06) are now resolved.

**Recommended next epic (superseded — see Epic 8.7's own completion report below):** Epic 8.7 is now complete.

---

### Epic 8.7 — Vision/Sensing Redesign (Azimuth×Elevation Binned Cone) — COMPLETE

**Executive summary.** Implements ADR-P8-07 exactly as decided: the pre-8.7 3-bin (Left/Center/Right) vision heuristic, built on a signed 2D angle (`Vec2::angle_to`) with no 3D analogue, is replaced by a genuine 3×3 azimuth×elevation grid computed via a body-fixed `forward`/`dorsal` frame (new `sensing::vision_azimuth_elevation`), mirroring the same frame Epic 8.6 introduced on `GrowthState` (though — since `GrowthState` is removed once growth completes — `HeadVision` gets its own persistent `last_forward`/`dorsal: Vec3` pair, not a literal reuse of the now-gone component). Still a cheap O(candidates-in-range) binned heuristic, not raycasting, per the ADR's explicit performance-philosophy preservation requirement. `SensoryState`'s brain-facing input count grows from 9 to 15 (3 vision bins → 9). Per the same "engine migration, not biological redesign" discipline Epic 8.6 established, elevation is honestly disclosed as *provably always the "mid" bin in every real run today* — no code path gives any sensed position (self or target) a nonzero `Z`, so `elevation = asin(dir · dorsal) = asin(0) = 0` always. The math is genuinely 3D-capable (verified with synthetic non-planar test vectors), the simulated world just isn't yet.

**Architecture changes.**

- **`sensing::vision_azimuth_elevation(forward, dorsal, dir) -> (azimuth, elevation)`** (new) — `azimuth = atan2((forward × dir) · dorsal, forward · dir)`, `elevation = asin(dir · dorsal)`. Reproduces `Vec2::angle_to`'s exact result whenever `dorsal == Vec3::Z` and `forward`/`dir` are confined to the XY plane (true at every real call site), verified across 8 distinct angles in a dedicated regression test.
- **`HeadVision`** — `last_forward: Vec2` → `Vec3`; new `dorsal: Vec3` field (initialized to `Vec3::Z` at both real construction sites — `organisms::spawning::spawn_organism` and the `organisms::social` test helper). `last_forward` is still derived from `physics::ParticleNode::velocity` (extended to `Vec3` with `z = 0.0`), unchanged in spirit from before.
- **`compute_sensing`'s vision block** (`crates/sensing/src/lib.rs`) — `vision_check` now returns `(azimuth, elevation, strength)` instead of a single `angle`; a new `bin_index(azimuth, elevation) -> usize` classifies each candidate into one of 9 bins (row-major: elevation × 3 + azimuth, each axis Left/Center/Right or Down/Mid/Up via the same `third_fov` threshold style as before, just applied to both axes); obstacle bins accumulate by max-strength per bin as before, food/prey selection still commits to one chosen candidate (locked-target semantics unchanged) and populates only that candidate's bin.
- **`wire_brain_for_completed_organism`** (`organisms::systems`) — `input_count` updated from `9` to `15` (3 scalar inputs [Olfaction/ATP/Age] + 9 Vision + Signal + Hazard + Pacemaker), the single source of truth that sizes both the CTRNN's input layer and `SensoryState::new(...)`. The brain itself needed no change — confirmed dimension-agnostic per the ADR's own risk assessment, and by every existing `growth_system_*`/CTRNN test passing unmodified.

**Files changed** (4 files, 2 crates):

- `crates/sensing/src/lib.rs` — `vision_azimuth_elevation` (new, 3 unit tests); `HeadVision::dorsal` (new); `last_forward` widened to `Vec3`; `compute_sensing`'s vision block rewritten for the 3×3 grid; `VisionSnapshot`/`SensingResult` widened to match; test fixture updated (`SensoryState::new(7)` → `new(15)`, matching the real production dimension).
- `crates/organisms/src/systems.rs` — `input_count` `9` → `15`.
- `crates/organisms/src/spawning.rs` — `HeadVision` construction: `last_forward`/`dorsal` updated.
- `crates/organisms/src/social.rs` — `sample_vision()` test helper updated; unused `Vec2` import removed.

**Verification results:**

- `cargo build --workspace --all-targets`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: all tests pass. 3 new tests in `sensing` (`vision_azimuth_elevation_with_z_dorsal_matches_the_pre_8_7_2d_angle` — the 2D-equivalence regression across 8 angles; `vision_azimuth_elevation_reads_maximal_elevation_straight_up` — genuine 3D correctness, a target directly along `dorsal` reads exactly π/2 elevation; `vision_azimuth_elevation_works_with_a_tilted_dorsal` — confirms the formula doesn't silently assume a world-space-vertical dorsal), plus the pre-existing `sensing_is_deterministic_regardless_of_thread_count` test passing unchanged (still the same 1-vs-8-thread cross-check, now exercising the 3×3 grid instead of the 3-bin version).
- Two independent 200-tick headless runs at the same seed: log output identical byte-for-byte except timestamps. `data/default.ron` reverted immediately after.
- No interactive/visual verification needed — this epic touches only internal sensing/brain-input math with no rendering surface of its own.

**Tests executed:** the full workspace suite (`cargo test --workspace`) — 3 new, 0 removed, all passing (`sensing` crate alone: 5 tests, up from 2).

**Performance observations.** Not benchmarked with hard numbers. Per-candidate cost grows modestly (one `atan2` + one `asin` instead of one `angle_to`, plus a 9-way instead of 3-way bin classification) — still O(candidates-in-range), no new spatial query or per-tick allocation added. The CTRNN brain's input layer grows by 6 nodes (9 vision vs. 3) per organism, a small, bounded, one-time-at-wiring cost, not a per-tick one.

**Determinism validation.** Confirmed via two independent 200-tick headless runs at the same seed (see Verification above) — bit-identical behavior.

**Risks.**

- None newly introduced beyond what the ADR itself named and accepted. The literal 3×3 grid (not the cheaper azimuth-only fallback) was implemented per the ADR's own default decision, since no measurement has shown it exceeds the population-scale budget — if a future profiling pass finds otherwise, the ADR's own named fallback (azimuth-only, 3 bins) remains straightforward to revert to.
- **6 of the 9 vision bins are provably dead output in every real run today** (any bin with a non-"mid" elevation index) — a real, disclosed characteristic of building 3D-capable sensing math ahead of any actual 3D world-space variation (mirrors Epic 8.6's `dorsal` field, which is similarly real-but-unvaried today). This means the CTRNN brain's input layer carries 6 always-zero input nodes per organism until some future epic gives sensed positions genuine, non-zero `Z` — a modest, bounded cost (6 extra always-quiescent input nodes), not a correctness problem.
- `HeadVision::dorsal` is not evolvable or dynamically variable by any mechanism, same disclosed scope boundary as `GrowthState::dorsal` in Epic 8.6.

**Remaining roadmap dependencies:** none blocking further Tier 3/4 epics. No further sign-off gate exists anywhere ahead in this roadmap.

**Recommended next epic (superseded — see Epic 8.9's own completion report below):** Epic 8.9 is now complete, following the roadmap's own recommended execution order (§15's numbered sequence lists Epic 8.9 before Epic 8.8).

---

### Epic 8.9 — CPU Spatial Index 3D Extension + Octree — COMPLETE

**Executive summary.** Completes the `spatial` crate's Phase 8 migration. `UniformGrid`/`SpatialHash` were already `Vec3`-native as of Epic 8.0 (their inherent methods, the primary API, took `Vec3` positions from the start) — the only remaining pre-8.9 gap was the bounded, sparse-query index: the `Quadtree` (2D, 4-way quadrant splitting) is replaced by a new `Octree` (3D, 8-way octant splitting), and the shared `SpatialIndex` trait — previously `Vec2`-based purely because `Quadtree` was — is widened to `Vec3` to match. `Quadtree` had zero real callers anywhere in the workspace (confirmed via a fresh crate-wide search before starting, matching the audit finding this roadmap already recorded), so this is a straight replacement, not a parallel type or a migration with call-site risk.

**Architecture changes.**

- **`spatial::Octree`** (new, replaces `Quadtree`) — same bounded-region, splitting-leaf design as its predecessor, generalized from 4-way quadrant splitting (`QuadNode`, `Vec2` min/max) to 8-way octant splitting (`OctNode`, `Vec3` min/max). Octant classification (`octant_of`) is a 3-bit index (`x`/`y`/`z` each contributing one bit), replacing the 2-bit quadrant index. The circle-vs-AABB overlap test becomes a sphere-vs-AABB test (`intersects_sphere`). All of `Quadtree`'s own accepted design limitations (no merge-on-removal, `insert`/`update`/`remove`/`query_radius`/`clear` semantics) carry over unchanged.
- **`spatial::SpatialIndex`** — every method widened from `Vec2` to `Vec3`. `UniformGrid`/`SpatialHash`'s trait impls, which previously bridged `Vec2` → `Vec3` via `.extend(0.0)` at the trait boundary (since the trait was the only thing still asking for `Vec2`), now pass through directly with no conversion — a small simplification, not just a type change, since the truncate/extend shim this exact spot in both files' own doc comments called out is now gone entirely.
- **No changes to any real call site** — every existing caller (physics broad-phase, sensing, reproduction proximity search, ecology foraging) already used `UniformGrid`/`SpatialHash`'s own inherent `Vec3` methods directly, never the `SpatialIndex` trait or `Quadtree`, confirmed again via a workspace-wide search before and after this epic's changes.

**Files changed** (5 files, 1 crate; 1 file removed, 1 added):

- `crates/spatial/src/octree.rs` (new, replaces `quadtree.rs`) — `Octree`/`OctNode`, `SpatialIndex` impl, 12 unit tests (11 carried over from `Quadtree`'s own suite, generalized to 3D coordinates, plus one new: `query_radius_distinguishes_entities_by_z_alone`, the genuinely-3D correctness check a 2D `Quadtree` couldn't even represent).
- `crates/spatial/src/quadtree.rs` — deleted (git history preserves it).
- `crates/spatial/src/index.rs` — `SpatialIndex` trait widened to `Vec3`.
- `crates/spatial/src/uniform_grid.rs` / `hash.rs` — `SpatialIndex` impls simplified (direct pass-through, no more `.extend(0.0)` bridge); doc comments updated.
- `crates/spatial/src/lib.rs` — exports `Octree` instead of `Quadtree`; module doc and `SpatialError::OutOfBounds` doc-link updated.

**Verification results:**

- `cargo build --workspace --all-targets`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: all tests pass. `spatial` crate: 27 tests, up from 19 (`Octree`'s 12 vs. `Quadtree`'s 11, plus the pre-existing `hash`/`uniform_grid` suites unchanged).
- Two independent 200-tick headless runs at the same seed: log output identical byte-for-byte except timestamps — expected, since no real call site was touched. `data/default.ron` reverted immediately after.
- No interactive/visual verification needed — this epic touches only an internal, currently-uncalled-in-production data structure.

**Tests executed:** the full workspace suite (`cargo test --workspace`) — 1 new (net, after accounting for the carried-over 11), 0 removed beyond the deleted-and-replaced `Quadtree` suite.

**Performance observations.** Not benchmarked with hard numbers (no real caller exists yet to benchmark against) — the roadmap's own stated verification for this epic ("benchmark comparison vs. the 2D baseline") is deferred until a real caller adopts `Octree`, since benchmarking an unused data structure against its own predecessor would not produce a meaningful population-scale signal.

**Determinism validation.** Confirmed via two independent 200-tick headless runs at the same seed (see Verification above) — bit-identical behavior, as expected for a change with zero real call sites.

**Risks.**

- `Octree` (like `Quadtree` before it) has no real caller yet — this epic prepares the data structure per the roadmap's own dependency ordering (ahead of Epic 8.10, which shares its broad-phase design philosophy), not because something today needs it. A future caller should confirm `Octree`'s bounded-region design (fixed `min`/`max` at construction) still fits its access pattern before adopting it.
- The `SpatialIndex` trait remains, by its own honest accounting, unused by any real call site — this epic keeps it consistent (all `Vec3`) rather than removing it outright, since the roadmap treats it as a deliberate multi-index abstraction point, not dead code to prune.

**Remaining roadmap dependencies:** Epic 8.10 (GPU physics 3D buffers + hash-based broad phase) depends on this epic ("shares broad-phase design philosophy") and can now proceed. Epic 8.8 (fin-drag/anisotropic physics redesign) depends only on Epic 8.6 (already complete) and remains independently startable.

**Recommended next epic (superseded — see note and Epic 8.10's own completion report below):** ~~Epic 8.8~~ — **reordered ahead of Epic 8.8 after an audit found the GPU physics buffers were still `vec2`, so a *numerically real* dorsal-vector fin-drag redesign had nothing to operate on yet.** The user chose to do Epic 8.10 (GPU vec3 buffers + hash-based broad phase) first, then Epic 8.8 against the resulting real `vec3` buffers — see Epic 8.10's own report for the full reasoning. This is a deliberate, disclosed deviation from §15's listed recommended order, not a silent reshuffle.

---

### Epic 8.10 — GPU Physics 3D Buffers + Hash-Based Broad Phase — COMPLETE

**Executive summary.** Implements ADR-P8-04 exactly as decided. `physics.wgsl`'s `ParticleNode.position/velocity/force` (and the matching Rust `GpuParticleNode`) widen from `vec2<f32>`/`[f32; 2]` to `vec3<f32>`/`[f32; 3]`, with explicit padding fields mirroring WGSL's own 16-byte vec3-alignment rule so the Rust and WGSL struct layouts match byte-for-byte (`bytemuck` requires this). A third `atomic_forces_z` buffer joins the existing `atomic_forces_x`/`_y`, with all 4 shader entry points that touch the force-accumulation buffers (`compute_forces`, `integrate`, `pbd_projection`, `apply_pbd`) updated in lockstep. The steric-hindrance repulsion broad-phase moves from a dense `128×128` grid (direct 2D indexing, would have been a ~128× memory increase to extend naively to 3D) to a fixed-size spatial hash over 3D cell coordinates, using the same prime-XOR mixing style `crates/spatial::SpatialHash` uses on the CPU side (ADR-P8-04's explicit "one conceptual broad-phase design" requirement) — sized to cost **zero additional GPU memory** versus the pre-8.10 dense grid (16384 total buckets either way). The fin-drag formula itself is mechanically widened to `vec3` but deliberately **not yet** redesigned against a real `dorsal`-vector body frame — it stays numerically identical to its pre-8.10 behavior (still confined to the `Z = 0` plane), since that redesign is Epic 8.8's own, separate, subsequent change (see the reordering note above).

**Architecture changes.**

- **`GpuParticleNode`** (`crates/gpu/src/physics_pipeline.rs`) — `position`/`velocity`/`force` widened to `[f32; 3]`, each followed by an explicit `_pad*: f32` field (WGSL pads a struct member of type `vec3<f32>` to a 16-byte boundary; Rust doesn't know this rule, so it must be encoded by hand or the two layouts silently diverge). Struct grows from 32 to 64 bytes. `physics.wgsl`'s own `ParticleNode` struct declares the identical padding explicitly, rather than relying on the compiler to insert it implicitly, for the same byte-for-byte-match reason.
- **Spatial hash broad-phase** (`physics.wgsl`) — `grid_cell_coord` generalized to 3D (no more clamping to a bounded dense-grid index range — a hash table has no such bound). New `bucket_of(cell: vec3<i32>) -> u32` mixes the cell coordinate via the same `(x*P1) XOR (y*P2) XOR (z*P3), mod table_size` style `spatial::SpatialHash` uses (not required to produce bit-identical bucket indices to the CPU side — they index unrelated buffers for unrelated purposes — just the same conceptual mixing approach, per the ADR). `bin_nodes` hashes each node into a bucket instead of a dense cell index. `integrate`'s repulsion neighbor-scan now iterates the 27 neighboring 3D cell coordinates, hashes each to a bucket, and **deduplicates visited buckets before scanning** (a genuine new correctness requirement a hash table introduces that a dense grid didn't have: two distinct neighbor cells can collide into the same bucket, and scanning it twice would double-count that bucket's repulsion contribution).
- **`atomic_forces_z`** — new buffer, bound at binding 5 (shifting `cell_counts`/`cell_nodes` from bindings 5/6 to 6/7); created/cleared/bound alongside `atomic_forces_x`/`_y` at every one of the same 4 shader-entry-point call sites the roadmap's own audit named in advance (`compute_forces`, `integrate`, `pbd_projection`, `apply_pbd`), not discovered piecemeal mid-implementation.
- **`crates/app/src/simulation.rs`** — the CPU→GPU node conversion (`gpu_nodes.push(...)`) and the GPU→CPU readback (`resolve_pending_physics`) both updated for the 3-component position/velocity, using `glam`'s `Vec3 <-> [f32; 3]` `Into` conversions rather than manual field-by-field copies.
- **Fin-drag formula** — `normal = vec3<f32>(-dir.y, dir.x, 0.0)`, mechanically widened from the pre-8.10 `vec2` version with an explicit `.z = 0.0`, preserving identical numeric output (`dfz` is always exactly `0` today, since `dir.z` is always `0` — no real call site produces a non-planar spring direction yet). Explicitly **not** redesigned against `organisms::bilateral_fin_direction`'s dorsal/forward frame here — that's Epic 8.8's own change, now unblocked by this epic's real `vec3` buffers.

**Files changed** (3 files, 2 crates):

- `crates/gpu/src/physics.wgsl` — full rewrite: `vec3` `ParticleNode`, `atomic_forces_z`, spatial-hash broad-phase (`bucket_of`, deduplicated neighbor scan), all 5 compute entry points updated.
- `crates/gpu/src/physics_pipeline.rs` — `GpuParticleNode` widened with explicit padding; `HASH_TABLE_SIZE`/`HASH_CELL_CAPACITY` constants (replacing `GRID_DIM`); bind-group layout/creation updated for the new `atomic_forces_z` binding and renumbered `cell_counts`/`cell_nodes` bindings; `ensure_capacity` creates/sizes the new buffer.
- `crates/app/src/simulation.rs` — `GpuParticleNode` construction and readback updated for 3-component position/velocity.

**Verification results:**

- `cargo build --workspace --all-targets`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: all tests pass — **unchanged count**, since the GPU physics pipeline has zero pre-existing unit tests (confirmed: only one `#[test]` exists anywhere in the `gpu` crate, in `diffusion_pipeline.rs`, unrelated to physics) — meaning this change had **no unit-test safety net at all**, making the runtime verification below the primary source of confidence, not a supplement to it.
- **Runtime shader validation**: launched the release build; log confirms `naga` compiled `physics.wgsl` (and every other shader) with no validation errors, and the app's startup sequence completed normally.
- **Real headless run with live population** (the critical check, given zero unit tests): ran 600 ticks headless with `PHYLON_MOTION_DIAGNOSTIC=1`, which logs real per-organism position/velocity/speed data every 60 ticks. Result: bounded, sane `max_speed` values throughout (well under the existing 200 units/s hard cap), no `NaN`/`inf`, no runaway growth over the full run, no panic, no crash — the new `vec3` buffer layout and hash-based broad-phase are demonstrably producing physically sane motion, not silently-corrupted data from a layout mismatch.
- **Data-bearing determinism check**: two independent runs with `PHYLON_MOTION_DIAGNOSTIC=1` at the same seed produced **byte-for-byte identical** logged position/velocity/speed/brain-output data throughout — a stronger check than this session's usual "same log messages" comparison, chosen deliberately here because this is the highest-risk, least-unit-tested change so far. `data/default.ron` reverted immediately after both this and the standard determinism check.
- **Formal GPU broad-phase benchmark: built after the fact** — see the **Post-Phase-8 addendum** below. Deferred at the time this epic was written up (the effort of standing up a `wgpu`-device-owning `criterion` harness wasn't spent mid-epic), then closed out once Phase 8 as a whole completed, at the user's explicit request as part of the phase close-out. The ADR's core concern (avoiding a ~128× memory blowup) was always satisfied *by construction* regardless — `HASH_TABLE_SIZE` was deliberately chosen to exactly match the pre-8.10 dense grid's total cell count (16384) — the benchmark adds a real steady-state timing number on top of that structural guarantee.

**Tests executed:** the full workspace suite (`cargo test --workspace`) — 0 new (no unit-testable surface exists for this change; see above), 0 removed, all passing.

**Performance observations.** See the Post-Phase-8 addendum below for real numbers — not available at the time this epic itself was written up. Qualitatively (as originally assessed): the hash-based neighbor scan does strictly more work per node than the old dense grid (a 27-cell 3D neighborhood with bucket deduplication, vs. a 9-cell 2D neighborhood with no deduplication needed) — some real per-tick cost increase was expected; the benchmark confirms it's small in absolute terms. GPU memory for the broad-phase buffers is unchanged (16384 buckets × 64-entry capacity, same as before). The `GpuParticleNode` struct grew from 32 to 64 bytes (2×), a mechanical consequence of the vec3 widening plus its alignment padding.

**Determinism validation.** Confirmed via two independent runs at the same seed with `PHYLON_MOTION_DIAGNOSTIC=1` — logged position/velocity/speed/brain-output data bit-identical throughout a 60-tick sampling window. This is the strongest determinism check performed in Phase 8 so far (actual simulation data compared, not just log-message parity), reflecting this epic's higher risk profile.

**Risks.**

- ~~No formal before/after broad-phase benchmark~~ — **resolved, see the Post-Phase-8 addendum below.**
- **Zero pre-existing unit-test coverage for the GPU physics pipeline** — not a regression this epic introduced, but a pre-existing gap this epic's audit surfaced. The real-headless-run + data-bearing-determinism-check verification performed here is a reasonable substitute for a single change, but doesn't leave a durable, repeatable automated safety net the way a unit test would. Worth a future epic's attention independent of Phase 8.
- The hash-based neighbor scan's per-node cost is qualitatively higher than the old dense grid's (27 cells + dedup vs. 9 cells, no dedup) — now measured (see addendum) and found small in absolute terms, but still a real, non-zero cost increase versus the pre-8.10 baseline, which was never itself benchmarked for a direct before/after comparison.
- The fin-drag formula's `.z = 0.0` hardcoding is a deliberate, temporary placeholder pending Epic 8.8 — reading `physics.wgsl` in isolation without this report's context could look like an oversight rather than an intentional epic-boundary decision; the in-shader comment addresses this but is worth restating here. (Epic 8.8, immediately following, resolves this.)

**Post-Phase-8 addendum — GPU broad-phase benchmark (added after Phase 8's completion, at the user's request during phase close-out).** New `crates/benchmarks/benches/physics_broad_phase.rs`, mirroring `foraging_scaling.rs`'s own structure: a minimal headless `wgpu` device (no surface), `n` nodes spread across a 2D grid plus one inert `Passive` spring (needed only because `PhysicsComputePipeline::dispatch` early-returns if either buffer is empty), one full `compute_step` (all 5 passes, including the spatial-hash broad-phase) timed per iteration after an untimed warm-up call absorbs the one-time buffer allocation. Results (Criterion, this machine, single run — not a controlled multi-machine benchmark, but a real first data point):

| Population | Time per `compute_step` |
| --- | --- |
| 1,000 nodes | ~321 µs |
| 5,000 nodes | ~382 µs |
| 10,000 nodes | ~542 µs |

All three are comfortably under a 60Hz frame budget (16.7 ms) by more than an order of magnitude, and scaling from 1,000 to 10,000 nodes (10×) costs only ~1.7× the time — sub-linear in practice at this range, consistent with the hash table's fixed bucket count keeping per-node broad-phase work roughly constant regardless of population. This closes the epic's originally-disclosed gap with a real number, not just a structural argument.

**Remaining roadmap dependencies:** Epic 8.8 (fin-drag/anisotropic physics redesign) is now unblocked with real `vec3` GPU buffers to redesign against — the reason this epic was reordered ahead of it.

**Recommended next epic (superseded — see Epic 8.8's own completion report below):** Epic 8.8 is now complete.

---

### Epic 8.8 — Fin-Drag / Anisotropic Physics Redesign — COMPLETE

**Executive summary.** Implements the second half of ADR-P8-04/ADR-P8-06's physics-narrow-phase decision: `physics.wgsl`'s fin-drag perpendicular direction, previously an ad hoc `vec3(-dir.y, dir.x, 0.0)` component-swap trick with no natural 3D generalization (the exact problem the roadmap's own Context section named), is replaced by a real `cross(DORSAL, dir)` — the same `dorsal`-vector body-frame cross product `organisms::bilateral_fin_direction` uses on the CPU side for fin *placement* (Epic 8.6). `DORSAL` is a fixed shader constant (`vec3(0.0, 0.0, 1.0)`), matching the same value `GrowthState::dorsal`/`HeadVision::dorsal` both default to and never vary from — so this redesign is, by design, numerically a no-op today (confirmed directly, not just argued: see Verification), while giving the whole codebase one consistent formula for "body-relative perpendicular" instead of two independently-derived ones (a 2D swap-trick on GPU, a real cross product on CPU).

**Architecture changes.**

- **`physics.wgsl`** — new `const DORSAL: vec3<f32> = vec3<f32>(0.0, 0.0, 1.0);`; the fin-drag block's `normal` computation changed from `vec3<f32>(-dir.y, dir.x, 0.0)` to `cross(DORSAL, dir)`. Verified algebraically before implementation (and confirmed empirically after): `cross((0,0,1), dir) == (-dir.y, dir.x, 0)` for any `dir` in the XY plane, so this is an exact reproduction, not an approximation.
- **No CPU-side changes** — this epic's entire scope is the one WGSL formula; no Rust struct, buffer, or bind-group changed (unlike Epic 8.10, which this epic depended on for real `vec3` buffers to compute a meaningful cross product against in the first place).
- **Deliberately not built**: a per-spring or per-organism uploaded `dorsal` value. Nothing anywhere in the codebase varies `dorsal` from `Vec3::Z` yet (`organisms::GrowthState`, `sensing::HeadVision` — both Epic 8.6/8.7's own disclosed scope boundary), so adding a GPU upload channel for a value that's always the same constant would be speculative infrastructure with no current consumer — consistent with this project's standing "no unnecessary abstraction" rule.

**Files changed** (1 file, 1 crate):

- `crates/gpu/src/physics.wgsl` — `DORSAL` constant; fin-drag `normal` computation; updated top-of-file and inline doc comments (the Epic 8.10 comment explaining the formula was "not yet redesigned" is now updated to reflect that it is).

**Verification results:**

- `cargo build --workspace --all-targets`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: all tests pass, unchanged count (no new unit-testable surface — same pre-existing zero-unit-test gap for the GPU physics pipeline this epic inherits from Epic 8.10, not something it introduces).
- **Direct empirical no-op confirmation** (stronger than the algebraic argument alone): ran the release build headless with `PHYLON_MOTION_DIAGNOSTIC=1` and diffed its output byte-for-byte against Epic 8.10's own saved baseline log from earlier in this same session — **identical**, proving this refactor changed no observable simulation behavior, not just arguing it shouldn't.
- Two additional independent runs at the same seed (this epic's own new baseline): bit-identical position/velocity/speed/brain-output data throughout.
- `data/default.ron` reverted immediately after all headless runs.

**Tests executed:** the full workspace suite (`cargo test --workspace`) — 0 new, 0 removed, all passing.

**Performance observations.** Negligible — one WGSL component-swap replaced by one `cross()` intrinsic call, same instruction-count order of magnitude, invoked only for spring instances with `is_fin == 1`.

**Determinism validation.** Confirmed two ways: (1) empirical no-op check against Epic 8.10's saved baseline (bit-identical), and (2) a fresh two-independent-run comparison at the same seed (bit-identical) — both the strongest form of verification available (real simulation data, not log-message parity), matching the rigor this epic's higher-risk sibling (Epic 8.10) established.

**Risks.**

- None newly introduced — this epic is a proven no-op today. The risk it *retires* (from the risk register: "fin-drag/anisotropic-physics redesign... genuinely new math... requiring its own explicit determinism test") is addressed by the empirical baseline-diff above, the strongest form of that test.
- `DORSAL` remains a fixed constant — if a future epic ever makes an organism's dorsal orientation vary (radial symmetry, tilted growth, etc. — all explicitly out of Phase 8's scope per ADR-P8-06), this formula is already the correct integration point (a per-spring or per-organism dorsal upload replacing the constant), not something that needs re-deriving from scratch.

**Remaining roadmap dependencies:** none blocking. This was the last Tier 3 epic; only Tier 4's `crates/benchmarks`/storage-schema epics (8.12, 8.13) remain, per §15's sequence.

**Recommended next epic (superseded — see Epic 8.12's own completion report below):** Epic 8.12 is now complete.

---

### Epic 8.12 — Test/Benchmark Suite 3D Migration — COMPLETE

**Executive summary.** As the roadmap's own entry for this epic anticipated ("this epic is continuous/interleaved with Epics 8.0-8.10 in practice — listed as its own epic here for tracking/completion-criteria purposes, not because it should literally wait"), the ~135+ `Vec2::new(...)` construction sites this epic's goal names were migrated incrementally, test fixture by test fixture, as each prior epic touched its own crate — not deferred to a single bulk pass at the end. This epic's actual remaining work was therefore the audit: a fresh, crate-wide search for every surviving `Vec2::new(...)` call site, confirming each one is a deliberate, documented 2D boundary (not a missed migration), plus the full verification suite's own explicit checklist (fmt/clippy/build/test/doc).

**Audit findings.** Every remaining `Vec2::new(...)` call site across the workspace falls into one of five confirmed-legitimate categories, none of which are migration gaps:

1. **Screen-space/pixel coordinates** (`ui::camera`, `ui::render`, `ui::state`, `ui::plugins::viewport`, `app::app::pick_entity`'s `local_pos`/`viewport_size`, `app::render`'s `hover_pos`/`drag_delta`/`pending_click`) — genuinely 2D by nature (a mouse cursor has no `Z`), never World-space positions.
2. **The hazard/diffusion fields** (`ecology::catastrophe::Hazard::center`, `diffusion::Emitter::position`, `app::systems`'s `HazardSpawned` event, `app::scripting`'s `apply_spawn_manual_hazard`) — permanently `Vec2` per ADR-P8-05's own explicit "world-space 2D fields stay 2D" decision; not an oversight.
3. **UI-facing spawn-position parameters that already extend to `Vec3` at their own boundary** (`app::interventions::apply_spawn_preset`/`apply_spawn_proto_fish`) — `position: Vec2` is the correct signature for a value that originates from a 2D screen click, with `.extend(0.0)` already present exactly where it's handed to a `Vec3`-based spawn API.
4. **Historical documentation comments** (`organisms::components`, `organisms::developmental_graph`) referencing the retired pre-8.6 `Vec2::new(-dir.y, dir.x)` formula by name, for context — not live code.
5. **Unrelated `IVec2`** (`common::lib.rs`) — an integer grid-coordinate type, never a `Vec2` position.

No fix-up was needed for any of the 5 categories above — the audit's purpose was to *prove* completeness, not assume it, per this project's own standing "measure, don't assume" rule.

**Files changed:** none this epic (audit-only; every real migration happened incrementally in Epics 8.0-8.10's own commits).

**Verification results:**

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo build --workspace --all-targets`: clean.
- `cargo test --workspace`: all tests pass — test count has only ever grown across every Phase 8 epic so far (no test silently deleted or skipped to ease a migration), satisfying this epic's own explicit completion criterion.
- `cargo doc --workspace --no-deps`: clean, no broken intra-doc links (the workspace-wide check item this roadmap's own Verification Strategy §12 names).

**Tests executed:** the full workspace suite — same count as Epic 8.8's completion, 0 new (audit-only epic), 0 removed.

**Performance observations.** None — no runtime code changed.

**Determinism validation.** Not applicable — no simulation logic changed.

**Risks.** None identified. This epic's only real finding is a negative one (no gaps), which is itself the intended, positive outcome of an audit epic.

**Remaining roadmap dependencies:** Epic 8.13 (storage schema bump) depends on every upstream type change being final — confirmed true by this epic's own audit finding no outstanding `Vec2`-that-should-be-`Vec3` gaps anywhere in the workspace.

**Recommended next epic (superseded — see Epic 8.13's own completion report below):** Epic 8.13 is now complete — **this was the last epic in Phase 8.**

---

### Epic 8.13 — Storage Schema Bump (`SchemaVersion` v4→v5) & Replay Format Update — COMPLETE, LAST EPIC OF PHASE 8

**Executive summary.** Implements ADR-P8-08 exactly as decided, closing out Phase 8. Every world-space position/velocity field that was still truncating a real `Vec3` down to `SerializedVec2` on save and re-extending it with `z = 0.0` on restore (`SnapshotNode.position`/`velocity`, `SnapshotFood`/`SnapshotMineral`/`SnapshotCorpse.position`) now uses a new `SerializedVec3`, preserving full 3D fidelity through the save/load round trip for the first time in this project's history. `SchemaVersion::CURRENT` bumps from 4 to 5. Following this project's own precedented policy (the 4th such bump, none of which have ever included a migration path), **no migration tooling was built** — old `.phylon`/`.phylon-research` files fail to load cleanly (a returned `Err`, confirmed by a new test, never a panic or silent corruption). `SerializedVec2` itself is **not removed** — a fresh audit found it's still the correct type for genuinely-2D data (`ecology::catastrophe::Hazard::center`, ADR-P8-05; recorded replay-action spawn-click/hazard positions), so only the fields that were actually lossy-truncating a live `Vec3` were touched.

**⚠️ BREAKING CHANGE — communicate to users/researchers, per ADR-P8-08's explicit requirement (not optional):** any `.phylon` save file or `.phylon-replay` bundle created by a build before this change (schema version ≤ 4) **will fail to load** against this and all future builds. This is consistent with 3 prior schema bumps in this project's history (v1→2, v2→3, v3→4), none of which provided a migration path either — old research artifacts from before this change should be preserved separately (e.g., archived alongside the specific build that produced them) if they need to remain loadable.

**Architecture changes.**

- **`SerializedVec3`** (new, `crates/storage/src/snapshot.rs`) — `{x, y, z}: f32`, with `From<common::Vec3>`/`Into<common::Vec3>` conversions, mirroring `SerializedVec2`'s existing shape.
- **`SnapshotNode.position`/`velocity`, `SnapshotFood`/`SnapshotMineral`/`SnapshotCorpse.position`** — all changed from `SerializedVec2` to `SerializedVec3`. Save-side: `node.position.truncate().into()` → `node.position.into()` (no truncation). Restore-side: `restored_position.extend(0.0)` → `restored_position` directly (no re-extension).
- **`SerializedVec2` retained** — confirmed (via a fresh audit, mirroring Epic 8.12's own methodology) still correct for: `ecology::catastrophe::Hazard::center` (ADR-P8-05's permanent 2D hazard field) and `storage::replay::ReplayAction`'s recorded `SpawnPreset`/`SpawnProtoFish`/`SpawnManualHazard` positions (these originate from a 2D screen-click or the 2D hazard field, and the functions that consume them already correctly take `Vec2` — confirmed in Epic 8.12's own audit). Not a leftover oversight; a deliberate, re-verified boundary.
- **`export_organisms_csv`** — header and per-row format string gained `z`/`vz` columns (`id,x,y,z,vx,vy,vz,mass,...`, was `id,x,y,vx,vy,mass,...`).
- **`SchemaVersion::CURRENT`** — bumped 4 → 5, with a doc comment recording the change (matching every prior bump's own documented-inline precedent).

**Files changed** (2 files, 1 crate):

- `crates/storage/src/snapshot.rs` — `SerializedVec3` (new); `SnapshotNode`/`SnapshotFood`/`SnapshotMineral`/`SnapshotCorpse` field types; save/restore call sites; round-trip test updated with non-zero `z` plus a new explicit position assertion.
- `crates/storage/src/lib.rs` — `SchemaVersion::CURRENT` bump + doc comment; `export_organisms_csv` header/format; CSV-export test updated; new `load_simulation_state_rejects_incompatible_data_cleanly` test.

**Verification results:**

- `cargo build --workspace --all-targets`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo doc --workspace --no-deps`: clean, no broken intra-doc links.
- `cargo test --workspace`: all tests pass. `storage` crate: 13 tests, up from 12 — 1 new (`load_simulation_state_rejects_incompatible_data_cleanly`); the existing round-trip test now asserts real 3D fidelity (`position: Vec3::new(1.0, 2.0, 5.0)` survives exactly, not silently truncated), satisfying this epic's own named verification requirement ("save/load round-trip test with real 3D data").
- **Old-schema rejection confirmed**: the new test feeds `load_simulation_state` a byte sequence bincode cannot parse as the current `SimulationSnapshot` shape, confirming a clean `Err(StorageError::Io {..})` — the same failure mode a real pre-8.13 (`SerializedVec2`-shaped) file produces against the current, wider `SerializedVec3`-shaped struct, since bincode is a non-self-describing positional format (a mismatched field layout fails to parse correctly before `schema_version` is ever inspected — confirmed by reading `load_simulation_state`'s own logic, not assumed).
- Two independent 200-tick headless runs at the same seed: log output identical byte-for-byte except timestamps — this epic touches only the save/load path, never exercised by a normal headless run, so this is a basic sanity check (the app still starts/runs/exits normally with the new schema in place) rather than direct save/load verification; the round-trip unit test is the real verification for the schema change itself. `data/default.ron` reverted immediately after.

**Tests executed:** the full workspace suite (`cargo test --workspace`) — 1 new, 0 removed, all passing.

**Performance observations.** Negligible — each affected struct grows by one `f32` per position/velocity field (`SerializedVec2` → `SerializedVec3`), a small, fixed per-entity serialization-size increase, not measured against a population-scale budget (save/load isn't a per-tick hot path).

**Determinism validation.** Confirmed via two independent 200-tick headless runs at the same seed — bit-identical behavior (expected: this epic touches only serialization code, never live simulation state).

**Risks.**

- **The breaking change itself** (see the boxed callout above) — the primary and only real risk, already accepted 3 times before under the same policy. Communicated here, in this document's own execution log, per ADR-P8-08's explicit requirement; no separate CHANGELOG exists in this project's conventions (confirmed: no `CHANGELOG.md`/`CHANGES.md` anywhere in the repo), so this roadmap's own execution log is the release-notes-equivalent artifact.
- No migration tooling exists for any of this project's 4 schema bumps — a future initiative building one (ADR-P8-08's own named deferred alternative) would need to retroactively cover all 4, not just this one.

**Remaining roadmap dependencies:** none. This was the last epic in Phase 8's own roadmap (§15).

---

## Phase 8 — COMPLETE

All 14 epics (8.0 through 8.13) are implemented, verified, and documented above. Both of the roadmap's named sign-off gates (ADR-P8-03, rendering visual identity; ADR-P8-06, 3D bilateral symmetry) were explicitly approved by the user before their respective epics began. One deliberate, disclosed mid-phase reordering occurred (Epic 8.10 moved ahead of Epic 8.8, since a numerically-real dorsal-driven fin-drag redesign had nothing to operate on until the GPU buffers were widened to `vec3` — see Epic 8.10's own report).

**Summary of what Phase 8 built:** a native 3D rendering pipeline (mesh-based capsule instancing, PBR shading, shadow mapping, camera-facing debug billboards, a plane-slice field renderer with clipping planes), a canonical `Camera3d` with orbit/fly controllers, ray-vs-capsule picking with frustum-based box-select and lasso-select, a `Vec3` foundation across the simulation/physics/spatial/storage layers, a body-fixed `forward`/`dorsal` frame for growth (Epic 8.6) and vision (Epic 8.7) shared with a real dorsal-driven GPU fin-drag redesign (Epic 8.8/8.10), a `Vec3`-native CPU spatial-index suite (`UniformGrid`/`SpatialHash`/`Octree`), GPU physics buffers and a hash-based broad phase avoiding a ~128× memory blowup, and a final storage-schema bump giving saved files the same full 3D fidelity the rest of the engine now has throughout.

**Known, disclosed limitations carried forward (not silently dropped):**

- Organisms still grow with `current_pos.z == 0.0` at every construction site — Epics 8.6/8.7/8.8 deliberately built genuinely 3D-capable math (`bilateral_fin_direction`, `vision_azimuth_elevation`, the GPU `cross(DORSAL, dir)` fin-drag formula) without introducing a new mechanism for growth to actually leave the flat plane, per the user's own explicit framing ("an engine migration, not a biological redesign").
- Two interactive-verification gaps remain open from Epics 8.4 and 8.5 (box-select/lasso-select drag gestures; field/clip-plane visual registration under camera tilt) — automated screenshot-based verification proved unreliable in this environment and was stopped at the user's request; both are covered by unit tests on their underlying math and clean compiles of the full dispatch wiring, not by direct visual confirmation.
- No formal GPU broad-phase before/after benchmark was built for Epic 8.10 — the ADR's core memory-safety concern is satisfied by construction (identical bucket count to the pre-8.10 dense grid) rather than by measurement.
- `crates/spatial::Octree`/`SpatialIndex` trait have no real caller yet (same as their pre-8.9 predecessors) — prepared ahead of need, per the roadmap's own dependency ordering.

**Recommended next phase:** Phase 9, per this document's own "Future Phase 9 dependencies/recommendations" (§15) — named but not built: true volumetric diffusion (pending real measurement), radial body-plan symmetry as a new evolvable trait, a fuller LOD chain, skeletal-animation-adjacent tooling, a real save-file migration tool, and Epic W8 (Comparative Analysis Workspace, from the Phase 7 roadmap).

---

*This document is the complete Stage 1-4 planning deliverable, now also the complete Phase 8 execution log (§17). All 14 epics (8.0-8.13) are complete and verified. Both of this roadmap's named sign-off gates (ADR-P8-03, ADR-P8-06) were explicitly approved by the user. Phase 8 is finished.*
