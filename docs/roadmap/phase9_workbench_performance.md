# Phase 9 — Workbench UX, Performance & Optimization Roadmap

**Status: P9.1 through P9.4 implemented and verified. P9.5 (viewport gizmos) is next.** Everything in this document is either a direct measurement or a code-cited finding — no claim here is a guess or an assumption carried over from prior phases' documentation.

## P9.4 — Blender Navigation Parity: implemented, verified

Built entirely on top of P9.3's now-frozen camera math — nothing in `OrbitController`/`FlyController`'s orientation logic changed for this milestone; every feature below either reads/writes their existing public fields from the outside, or adds a narrowly-scoped, additive-only projection field to `Camera3d` (orthographic support — a projection-mode change, not an orientation one).

- **Smooth Frame Selected / Frame All** (`crates/ui/src/frame_animation.rs`, new): a `FrameAnimation` type holding a 250ms eased (smoothstep) transition of `focus`/`distance` only — yaw/pitch untouched, so framing re-centers and re-distances without ever spinning the view. Computes a real bounding sphere (centroid + farthest-point radius) over the selected organism's own nodes (`FrameSelected`) or the whole population (`FrameAll`), not a guessed fixed distance. Driven once per rendered frame from `render.rs`'s existing "Camera Tracking" step, via its own dedicated wall-clock timing field (kept separate from simulation-tick timing on purpose — camera smoothness shouldn't couple to simulation bookkeeping).
- **Six preset views** (Top/Bottom/Front/Back/Left/Right) — each just sets `yaw`/`pitch` to a fixed value, leaving `focus`/`distance` alone (Blender's own preset-view behavior: re-orient around the existing pivot, don't re-frame it).
- **Camera bookmark mode-preservation bug, fixed**: `CameraBookmark` now records `orbit_focus: Option<Vec3>` (`Some` if Orbit was active at save time, `None` if Fly was) — restoring a bookmark reconstructs the *same* mode instead of always forcing Fly, which was the previously-disclosed bug.
- **Orthographic/perspective toggle**: `Camera3d` gained `ortho_half_height: Option<f32>` (`None` = perspective, unchanged default everywhere). `view_proj`/`screen_to_ray`/`world_to_screen` all branch on it — orthographic rays are genuinely parallel (constant direction, varying origin), not a perspective approximation. Applied only inside `WorkbenchState::camera()` (the one accessor every consumer already reads through), sized to match perspective's own apparent scale at the moment of toggling, so flipping modes doesn't jarringly rescale the view. `OrbitController`/`FlyController` themselves always still produce a plain perspective `Camera3d`, untouched.
- **"Orbit Around Selection"**: not implemented as a separate persistent-mode toggle — Frame Selected already re-centers the pivot on the selected organism, which is the practical behavior Blender's own preference is for. Flagged as a deliberate scope decision, not an oversight.
- **Navigation gizmo / view pie menu**: deliberately not built here — the roadmap's own §3 already scopes gizmos to P9.5, and a view pie menu was explicitly optional ("if desired") in the brief; building either now would have meant redoing work once P9.5 lands its own gizmo/interaction surface.
- Wired into both the keyboard (`.` for Frame Selected, `Home` for Frame All — `Num0` keeps the original hard `CameraHome` reset, now a distinct action since the two are no longer synonyms — `1`/`3`/`7` and `Ctrl+1`/`3`/`7` for the presets) and a new "View → Camera" menu, so every action is reachable without memorizing a keybinding.

**Verification:** 3 new orthographic-projection tests (parallel rays, round-trip through `screen_to_ray`, behind-camera rejection) and 3 new `FrameAnimation` tests (reaches target exactly at full duration, stays properly interpolated partway, easing is monotonic/bounded) — all pass, alongside every pre-existing test unmodified. Full `fmt`/`clippy --workspace --all-targets -D warnings`/`build --workspace`/`test --workspace` clean, plus a windowed release smoke test.

**Disclosed limitation, same as P9.2/P9.3:** no automated input-injection tooling in this environment for a live "trigger Frame Selected and watch it ease smoothly" session — verified by the automated tests above (which exercise the same interpolation math the live app runs) plus a launch/stability smoke test, not a real interactive pass. If you can drive real input, confirming the 250ms ease actually reads as smooth (not just mathematically correct) is the one thing worth a manual check.

---

## P9.3 — Free Camera Orbit: implemented, verified

Re-audited `OrbitController` directly before touching it (per this phase's own "re-audit before every milestone" rule) and found the "camera feels locked" complaint traced to two compounding issues, not one:

1. `orbit()`'s pitch was hard-clamped to `[0°, 89°]` — the camera could never tilt past just-short-of-horizon, let alone orbit over the top of the pivot.
2. `OrbitController::camera()` built its orientation via `orientation_from_forward_and_reference_up(forward, Vec3::Y)` — using world `Y` as the reference-up, inconsistent with `FlyController`'s `Vec3::Z`, and itself only non-degenerate near the original top-down default view. Simply swapping that one vector to `Vec3::Z` (my own first attempt) turns out to be wrong on inspection: it makes the *default top-down view itself* degenerate (looking straight down **is** looking along `Z`), which would have silently rotated the most common view by 90° — a real feel regression the phase's own "measure before assuming a fix is correct" rule exists to catch.

**The actual fix** replaces the from-forward-vector reconstruction with a genuine quaternion composition — `orientation = Quat::from_axis_angle(Z, yaw) * Quat::from_axis_angle(X, pitch)` — which has no degenerate point anywhere on the sphere (composing two proper rotations never produces a NaN or ill-defined basis, unlike reconstructing an orientation from a forward vector plus a separate reference-up hint). This composition has a real, non-coincidental property: at `pitch == 0` it reduces to pure yaw-around-`Z`, leaving the pre-existing top-down default's screen-up exactly at world `+Y` (**zero feel change at the default view**, confirmed by the pre-existing `default_orbit_looks_straight_down_at_the_origin` test still passing unmodified); at `pitch == π/2` (horizon) it puts world `+Z` at screen-up, satisfying the Z-up requirement. `orbit()`'s pitch clamp was removed entirely — pitch is now an unbounded float, and the sinusoidal `forward()` formula (unchanged, and shown by direct derivation to already equal what the quaternion composition produces) is periodic by construction, so no wraparound logic was needed for continuous 360° orbit.

**Preserved exactly, per this milestone's tightened scope:** Orbit/Pan/Fly remain three distinct modes (nothing here merges them); all sensitivities (rotate/pan/zoom constants) untouched; orbit still always revolves around `OrbitController.focus`, an explicit pivot, never the camera itself; `FlyController` untouched (its own pitch clamp stays, deliberately out of scope); no inertia/damping/gizmos/bookmarks/view-presets added — those are P9.4/P9.5.

**Verification:**

- New tests: `orbit_pitch_is_unbounded_and_never_clamped`, `orbit_can_pass_continuously_over_the_top_of_the_pivot` (a 200-step full pole-to-pole-to-pole sweep asserting `forward()` stays finite and never jumps discontinuously frame-to-frame), `orbit_reference_up_is_world_z_not_y` (confirms screen-up reads as `+Z` at the horizon). All pre-existing camera tests, including the default-view and Orbit↔Fly round-trip tests, pass unmodified.
- `cargo fmt` / `clippy --workspace --all-targets -D warnings` / `build --workspace` / `test --workspace` all clean.
- Windowed release-build smoke test: launches and runs stably.
- **Disclosed limitation, same as P9.2:** no automated input-injection tooling in this environment to drive a live 360°-orbit-by-mouse session; verified by direct derivation (the quaternion composition was checked by hand against the existing `forward()` formula and found to produce identical output) and the automated sweep test above, not a live interactive pass. Picking/clipping/field-overlay registration were not touched by this change (only `OrbitController`'s own orientation math changed; `Camera3d::screen_to_ray`/`world_to_screen`/`view_proj` — what picking and field overlays actually consume — are unmodified), so they're expected to keep working unchanged, but a live confirmation is still worth doing if you can drive real input.

---

## P9.2 — One Canonical Input Layer: implemented, verified

ADR-P9-01 (below) was approved with Option 2 — a dedicated `ViewportInput` abstraction, not routing everything through egui. Built `crates/ui/src/viewport_input.rs`:

- **`ViewportInput`** — a plain, platform-agnostic struct (pan/rotate/zoom deltas, key-driven pan/fly-move axes, detach-follow flag).
- **`ViewportInput::from_canvas_interaction`** — the egui adapter, replacing `app::render`'s prior direct interpretation of `CanvasInteraction`.
- **`app::events`'s raw-keyboard handler** — rewritten as the winit adapter, building a `ViewportInput` instead of mutating `OrbitController`/`FlyController` fields directly. Its redundant raw `+`/`-` zoom handling (a straight duplicate of `shortcuts.rs`'s `Key::Plus`/`Key::Minus` → `MenuAction::CameraZoomIn/Out`) was deleted, not migrated — it was never a distinct input source.
- **`apply_to_camera`** — the single `ViewportController`, the only function that mutates the camera controllers. Both adapters converge here; every constant/formula (rotate sensitivity, fly step, pan speed, the trackpad-micro-movement follow-detach threshold) was carried over unchanged from wherever it used to live, not retuned.

`docs/explanation/camera_and_viewport.md` and `docs/roadmap/decisions.md` (ADR-P9-01) updated in the same pass. Full verification clean: `fmt`/`clippy --workspace --all-targets -D warnings`/`build --workspace`/`test --workspace`, plus a windowed smoke-test launch confirming the app still runs.

**Disclosed limitation:** no automated input-injection tooling exists in this environment, so orbit/pan/zoom/fly's *feel* was verified by code inspection (every formula/constant is identical to its pre-consolidation call site, only *where* it's called from changed) rather than a live interactive session. If you have a way to drive real mouse/keyboard input against the running app, a manual pass confirming orbit/pan/zoom/fly all still behave identically is the one verification step this pass couldn't do itself.

---

## P9.1 — Performance Foundation: implemented, measured

Fixed the four ranked bottlenecks from the original audit (§1 below), preserving rendering output and simulation behavior exactly (no visible or behavioral change, verified by inspection of every touched call site):

- `gather_world_render_instances`'s six intermediate lookup `HashMap`s moved into a persistent `PhylonApp::render_scratch` field (`RenderInstanceScratch`), `.clear()`-ed instead of reallocated every frame.
- `status_bar.rs`'s six full-population queries (food/mineral/corpse/diet/hunting/diseased counts) cached in a `thread_local!`, refreshed every 15 frames instead of every single frame — imperceptible staleness (~0.25s at 60Hz) for numbers that already only mattered at a glance.
- `render_behavior_glyphs` now skips the egui text-shaping call for glyphs outside the viewport (a screen-space visibility cull, not a user-facing toggle — the overlay stays "population-wide, not opt-in" per `docs/design/biological_visual_language.md`, since an off-screen glyph was already invisible).
- `update_simulation`'s per-tick GPU node/spring gathering (`entity_to_index`, `gpu_nodes`, `node_entities`, `gpu_springs`) moved into a persistent `PhylonApp::sim_scratch` field (`SimTickScratch`), same reuse pattern.

**Measured, same methodology as the original audit** (temporary `PHYLON_FPS_PROBE` env-gated log line on `analytics::MetricsState::smoothed_fps`, added and removed both before and after — not left in the codebase): at the same ~1,000+ organism population plateau, smoothed FPS improved from **~6.4 (before) to a steady ~6.9–7.0, with a transient peak near ~8.8 during the population ramp (after)**. A real, positive, verified improvement — not a full fix. §1's ranking already predicted this: these four fixes address allocation churn and ungated per-frame scans, not the underlying O(population) iteration cost itself, which several of them (rendering instance gathering in particular) still fundamentally pay every frame by design. Full verification: `cargo fmt`/`clippy --workspace --all-targets -D warnings`/`build --workspace`/`test --workspace` all clean.

**What P9.1 deliberately left alone**, and why, for whoever picks this up next: the brain-buffer gathering and 5 diffusion-emitter passes in `update_simulation` have the same allocate-fresh-every-tick shape as the fix already applied there, but weren't touched this pass (time-boxed scope, not a finding that they're safe) — a natural next increment if more headroom is needed before 60Hz. The deeper, not-yet-attempted question is whether `gather_world_render_instances`'s O(population) full-rebuild-every-frame *shape* itself (not just its allocation pattern) is the real ceiling — that would require frame-to-frame change detection (only rebuild entries for entities that moved/changed) rather than allocation reuse, a materially bigger change than P9.1's scope and worth its own measurement-first pass rather than being bundled in here.

---

## Original audit (unchanged below)

## 0. Corrections to the phase brief, made because the code disagreed

Per this project's own standing rule ("trust the implementation, update the documentation later — never the opposite"), two claims in the phase brief were checked against reality before being treated as fact:

- **"The current simulation runs around 5 FPS."** Measured directly (a temporary, since-reverted probe on `analytics::MetricsState::smoothed_fps`, sampled during a real windowed run): FPS starts at ~60 with an empty/near-empty world and degrades continuously as the founder population grows, reaching **~6.4 FPS once the population plateaus around 1,000+ organisms** (the same population scale this project's Phase 9 Goal 2/3 work already characterized). The brief's "~5 FPS" is confirmed essentially accurate at realistic population scale — it is not flat/constant overhead, it is a population-scaling problem, which matters for where the fix belongs.
- **"Many files exceed 1500 lines."** Checked directly: exactly **2** files exceed 1,500 lines (`crates/organisms/src/systems.rs` at 1,823 and `crates/app/src/app.rs` at 1,771). A further handful sit in the 700–1,400 range (`crates/ui/src/layout.rs` 1,391, `crates/ui/src/render.rs` 1,279, `crates/app/src/events.rs` 1,180, `crates/ui/src/plugins/inspector.rs` 1,087). Real, but "many exceed 1500" overstates it — the roadmap below scopes file-splitting to the 2 genuine outliers plus any file a *performance* fix below naturally needs to touch, not a blanket pass.

## 1. Measured performance root cause (ranked by confidence)

GPU physics compute itself is **not** the bottleneck — this project's own existing benchmark (`crates/benchmarks/benches/physics_broad_phase.rs`) already measured the GPU physics `compute_step` at 321–542µs for 1,000–10,000 nodes, comfortably inside a 60Hz frame budget. The ~6.4 FPS ceiling is CPU-side, and specifically:

1. **`gather_world_render_instances` (`crates/app/src/render/world_instances.rs:36-637`) re-scans the entire population from scratch every single frame, unconditionally.** It builds fresh `HashMap`s/`Vec`s sized to population every frame (node positions, organism-id lookup, per-organism health fraction, per-organism averaged disease severity via a *nested* per-segment `DevelopmentalGraph` walk, growth progress), issuing at least 7 separate ECS query constructions and iterations per frame, none of them reused or cached across frames. At 1,000+ organisms with several segments each, this is O(population × segments) of allocation and iteration every frame regardless of whether anything changed. **Highest-confidence single contributor.**
2. **`crates/ui/src/plugins/status_bar.rs:132-191` runs 6+ full-population ECS queries every frame with no visibility gate** (the status bar is on-screen essentially always, unlike the Metrics panel, which *is* correctly gated behind `ui.metrics_visible`) — food/mineral/corpse counts, a per-diet loop, hunting count, diseased count, plus a `sysinfo` process-refresh syscall that also runs a second time in `simulation.rs`.
3. **`render_behavior_glyphs` (`crates/ui/src/render.rs:205,798-855`) draws population-wide egui text every frame with no visibility toggle**, unlike the label/vision-cone/physiology overlays immediately around it, which do have toggles. egui text shaping per call is comparatively expensive at population scale.
4. **`update_simulation` (`crates/app/src/simulation.rs`) builds 5+ full-population `Vec`/`HashMap` structures fresh every tick** (GPU node/spring/brain buffers, 5 separate diffusion-emitter gathering passes), none with reused capacity across ticks, and up to `max_ticks_per_frame` ticks can run within one frame's 20ms budget — this multiplies the cost. This is expected work (feeding the GPU), but it currently allocates from scratch every time rather than reusing capacity.

Ruled out by direct code reading: GPU buffer churn (physics and organism-instance buffers already grow-and-reuse correctly, never recreate per frame) and same-frame blocking GPU readback stalls (physics/brain readback is deliberately pipelined one tick behind, per its own doc comment).

**Proposed fix shape (not yet implemented):** gate #1–#3 behind actual change-detection or visibility, the same way the Metrics panel already correctly does; reuse #1 and #4's allocations across frames/ticks (pre-sized, cleared-not-reallocated `Vec`s/`HashMap`s) rather than rebuilding from scratch. This is the highest-leverage, lowest-risk work in this whole phase — it doesn't touch rendering quality, camera, or simulation behavior at all.

## 2. Measured camera/input architecture findings

- **The "camera feels locked" complaint is real and code-confirmed, and its exact cause is a hard pitch clamp, not a fundamental design flaw.** `OrbitController.pitch` is clamped to `[0°, 89°]` measured from nadir (`crates/ui/src/camera.rs:211,240`) — the camera can never orbit past horizontal in orbit mode. `FlyController.pitch` is clamped to `[-89°, 89°]` from horizontal (camera.rs:366,392), closer to Blender's own range but still can't reach true vertical. Both controllers pin their up-reference to a fixed world axis (orbit uses `Vec3::Y`, fly uses `Vec3::Z` — inconsistent with each other, worth reconciling) rather than deriving it from the current camera state, which is also why roll is structurally impossible in either mode. `Camera3d.orientation` is a `Quat`, but both controllers still track yaw/pitch as separate `f32`s internally and only convert to a quaternion at the last step — not a gimbal-lock bug today (since orientation is built from an explicit forward vector, not composed Euler rotations), but worth being aware of if pitch range changes.
- **Real, confirmed gaps** (none of these exist anywhere in the codebase today, verified by direct search, not assumed absent): frame-selected, frame-all, smooth damping/inertia (every camera input is an instant field mutation, zero interpolation), orthographic projection (`Camera3d` hardcodes `Mat4::perspective_rh`), preset views (Top/Front/Right/etc.), a view-cube/navigation gizmo, a transform gizmo, and pivot-mode selection (median/bounding-box/3D-cursor — the only pivot concept today is `OrbitController.focus`, manually set).
- **Camera bookmarks already exist and are wired** (`CameraBookmark`, `WorkbenchState.bookmarks`, a Toolbar menu) — but restoring one always force-switches to Fly mode regardless of what mode was active when it was saved, and there's no keybinding, only a menu item. A real gap, but a much smaller one than "bookmarks don't exist."
- **Input handling is genuinely duplicated across at least two independent paths**, confirming the phase brief's "no duplicated gesture handling" goal is warranted: an egui-routed path (`crates/ui/src/plugins/viewport.rs:36-79` → `crates/app/src/render.rs:180-219`) and a separate winit-routed path (`crates/app/src/events.rs:948-1019`) both independently interpret raw camera input, and zoom specifically has **three** separate call sites (scroll/pinch in render.rs, raw `+`/`-` keys in events.rs, and `MenuAction::CameraZoomIn/Out` from shortcuts.rs) rather than one input layer.

## 3. Prioritized epics (roadmap, not yet implemented)

Ordered by measured impact and risk, per this project's own "never optimize without measurement" rule:

**Epic P9.1 — Per-frame allocation & query gating (performance, lowest risk). ✅ Done** — see the results section at the top of this document.

**Epic P9.2 — One input layer. ✅ Done** — see the results section at the top of this document, and ADR-P9-01 below.

**Epic P9.3 — Free camera orbit. ✅ Done** — see the results section near the top of this document. Scope was tightened at review to explicitly exclude inertia/damping (deferred to a later milestone) and keep this a pure camera-mathematics fix.

**Epic P9.4 — Blender-parity navigation features. ✅ Done** — see the results section near the top of this document. Navigation gizmo/view pie menu deliberately deferred to P9.5/skipped; "orbit around selection" satisfied via Frame Selected rather than a separate persistent-mode toggle.

**Epic P9.5 — View-cube, navigation gizmo, transform gizmo.** Larger UI/rendering surface area; scope after P9.1–P9.4 land, since a gizmo needs a stable camera/input layer under it.

**Epic P9.6 — Targeted file decomposition.** Only `crates/organisms/src/systems.rs` and `crates/app/src/app.rs` clearly qualify by this project's own prior precedent (Phase 7's `render.rs` decomposition was justified by size *and* mixed responsibility, not size alone) — audit each for real seams before splitting either.

**Epic P9.7 — UI polish pass.** Deferred until P9.1–P9.5 land; polishing spacing/animation/transitions on top of an input/camera layer that's about to change would mean redoing some of it.

**Epic P9.8 — `MANUAL_TESTING.md`.** Should be authored once P9.1–P9.5's actual feature set is known — a QA checklist written against features that don't exist yet would need immediate revision.

## 4. What this pass did not do, and why

Full flame-graph/cache-miss/branch-prediction profiling (as the brief's Goal 3 describes) requires OS-level profiling tools not available in this execution environment; the FPS probe plus direct code reading of the actual per-frame call graph is what's real and available here, and is what §1's findings are based on — every claim in §1 traces to a specific file:line, not a profiler trace. If deeper profiling tooling is available in your own environment, Epic P9.1's before/after measurement is the place to attach it.

This document is the "stop after roadmap, before implementation" checkpoint the phase brief asked for. Epics P9.1 through P9.8 are ready to be picked up in order; P9.1 has the best risk/reward ratio and no architectural dependencies on anything else in this list.

---

## ADR-P9-01 — One Canonical Input Layer

**Status: approved (Option 2) and implemented.** See the P9.2 results section at the top of this document for what was built.

**Context.** §2 confirmed three independent places currently interpret raw viewport input: an egui-routed path (`crates/ui/src/plugins/viewport.rs` → `crates/app/src/render.rs`), a winit-routed path (`crates/app/src/events.rs`, raw `WindowEvent::KeyboardInput` for WASD/arrows and `+`/`-` zoom), and shortcut-driven `MenuAction`s (`crates/ui/src/shortcuts.rs`) for camera reset/zoom. Zoom alone has three independent call sites. This isn't a style inconsistency — it means a future change to, say, zoom sensitivity has to be made in three places and can silently drift out of sync, exactly the risk the phase brief's "no duplicated gesture handling" goal names.

**Decision to make (two viable shapes, need a choice before implementing):**

1. **Route everything through the egui path.** `viewport.rs`'s `CanvasInteraction` already captures drag/scroll/zoom gestures when the pointer is over the viewport. Camera-relevant raw keyboard (WASD/fly movement, `+`/`-` zoom) would move from `events.rs`'s winit handler into the same per-frame egui-input read the viewport already does, gated on `!ctx.wants_keyboard_input()` (the same guard other shortcuts already use, so typing in a text field doesn't move the camera). `MenuAction`-driven camera actions (reset, zoom-in/out) would call the same underlying `Camera3d`/controller methods the gesture path calls, rather than duplicating the math.
2. **Introduce a dedicated `ViewportInput` intermediate layer** that both the egui and winit paths feed into (a small struct of "this frame's requested camera deltas"), consumed once per frame by a single camera-update call. More ceremony, but cleanly separates "what input happened" from "what the camera does with it" — relevant if a future 3D-mouse/VR/touch adapter (named in the original brief) needs to feed the same layer without going through egui or winit's `WindowEvent` at all.

**Recommendation:** option 2 — it's the shape that actually satisfies "future devices integrate by adding adapters, not new interaction logic" from the original brief, since option 1 still hard-codes "egui is the input source" into the camera-update call site. Option 1 is less work but doesn't fully solve the stated problem.

**Consequences either way:** `events.rs`'s raw WASD/zoom keyboard handling is deleted, not kept as a fallback — per "no duplicated gesture handling," a fallback path is exactly what caused this problem. Any behavior change here is a real interaction-model change (not just refactoring), so it needs the same before/after manual verification as a rendering change: confirm orbit/pan/zoom/fly all still work identically post-consolidation, not just that the code compiles.

**This is the explicit stopping point** — implementation of P9.2 (and, once its own pitch-clamp redesign is scoped, P9.3) should not proceed until the shape above is confirmed.
