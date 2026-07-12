# Phase 9 — Workbench UX, Performance & Optimization Roadmap

**Status: Phase 9 complete — P9.1 through P9.8 all implemented and verified.** Everything in this document is either a direct measurement or a code-cited finding — no claim here is a guess or an assumption carried over from prior phases' documentation.

## P9.8 — `MANUAL_TESTING.md`: implemented

Authored `MANUAL_TESTING.md` at the repo root (matching `README.md`/`CONTRIBUTING.md`'s existing top-level placement) — a checklist a human tester runs against a real build, covering exactly the interactive surface every P9.x milestone disclosed it couldn't verify itself: no automated input-injection tooling exists in this environment for the live `winit`/`egui` window, so every milestone's own verification fell back to unit tests plus a launch/stability smoke test. This document is that missing interactive pass, written down rather than left as a standing gap.

Sections: setup, Orbit-mode camera (drag/pan/zoom/select/follow/context-menu — the original Phase 7 W0b selection/follow behavior this session's first task was to verify), Camera Navigation (Frame Selected/All, presets, bookmarks, ortho toggle — P9.2–P9.4), Fly mode, Viewport Gizmos (P9.5 — axis triad, nav cube, origin/pivot indicators, selection box, scene info overlay), Performance (the FPS/frame-time claims P9.1 fixed), and a general simulation-controls sanity check. Cross-referenced from both `README.md`'s Contributing section and `CONTRIBUTING.md` itself, so it's discoverable from the two places a contributor would already be looking.

**Verification:** documentation only, no code changed — reviewed against `docs/reference/controls.md`'s authoritative keybinding table for accuracy rather than re-deriving bindings from memory.

---

## P9.7 — UI Polish: implemented, verified

**Scope, confirmed with the user before starting:** a separately-approved 13-milestone "UI Architecture Refinement" plan exists (design tokens, component catalog, shortcut-system fix, sidebar/status-bar rework, Metrics palette, Neural Viewer zoom/pan, docking/window management, accessibility pass, ~5-7 weeks) — but the Phase 9 roadmap's own P9.7 line item only ever named a light "polish spacing/animation/transitions" pass. Asked the user directly rather than guessing which was meant; confirmed P9.7 stays scoped to the light pass, with the 13-milestone plan treated as a separate, later initiative.

With that scope confirmed, P9.7 became a consistency pass over the UI surfaces P9.1–P9.6 themselves added, checked against conventions this codebase had already established elsewhere:

- **`gizmos.rs`'s floating overlays used hand-picked color literals** (`Color32::from_rgba_unmultiplied(20, 20, 24, …)` for both panel backgrounds, `(255, 255, 255, …)` for the origin marker and selection box) instead of the existing `theme::` token system — `render.rs`'s `toast_colors` had already established the "base theme color + explicit alpha" pattern for exactly this situation (semi-transparent floating surfaces). Added `gizmo_panel_fill(alpha)` routing through `theme::CHROME_BG`, and routed the origin-marker/selection-box colors through `theme::TEXT_PRIMARY` the same way. The X/Y/Z axis-triad colors were deliberately left as literals — red/green/blue-for-XYZ is a fixed, universal 3D-tool convention (Blender, Unity, Unreal all use it), not a brand color, so tokenizing it would be a false consistency.
- **The orthographic/perspective toggle in the P9.4 Camera menu used raw Unicode glyphs** (`◻`/`◇`) instead of the Remix Icon font every other menu entry in this file uses (e.g. `CAMERA_LINE` on the submenu itself) — replaced with `egui_remixicon::icons::{SQUARE_LINE, SHAPES_LINE}`, so the toggle's icon renders consistently with the icon font/weight/baseline of every other menu icon instead of falling back to whatever Unicode glyph support the OS font happens to have.

No spacing/animation changes were made — P9.4's `FrameAnimation` easing and the Camera submenu's existing spacing/shortcut-hint formatting were already internally consistent with the rest of `menu.rs` on inspection, so there was nothing there to fix.

**Verification:** `fmt`/`clippy --workspace --all-targets -D warnings`/`build --workspace`/`test --workspace` all clean (no test count change — this pass touched no logic, only color/icon sourcing), `cargo doc --no-deps --document-private-items --workspace` clean, and a windowed release smoke test (no panics beyond the pre-existing `B0003` warning).

---

## P9.6 — Architecture & File Decomposition: implemented, verified

Re-audited the roadmap's own two size-flagged files (`crates/app/src/app.rs` at 1,796 lines, `crates/organisms/src/systems.rs` at 1,823 lines) directly against current code rather than trusting the earlier line counts, per this phase's own "measure, don't assume" discipline — both had grown further since the original audit. Per this project's own established precedent (Phase 7's `render.rs` decomposition, justified by mixed responsibility, not size alone), each file was read in full before splitting anything, looking for genuine seams rather than cutting at an arbitrary line count.

**`app.rs` (1,796 → 579 lines)** bundled four distinct responsibilities that had accreted into one file over several phases:

- **GPU/surface bring-up** — `GpuContext`, `GpuCore`, `request_gpu_core`, and `PhylonApp::{init_gpu, init_gpu_headless, resize}` — extracted verbatim (no logic changed) to `crates/app/src/gpu_init.rs` (332 lines). This is a self-contained concern: acquiring a `wgpu::Device`/`Queue`, the four compute pipelines, and (windowed only) a swapchain surface plus egui state. It shares no logic with ECS/resource wiring or genome seeding.
- **Starter-species genome/CPPN seeding** — `RegulatorySeedWeights`, `seed_regulatory_cppn`, `seed_brain_cppn`, `seed_ecosystem`, plus their existing test module — extracted verbatim to `crates/app/src/species_seed.rs` (909 lines). This is pure data/genome-construction logic with no dependency on GPU state or the ECS resource-wiring `PhylonApp::new` does; `crates/app/src/interventions.rs`'s call sites (sandbox preset spawning) were updated to reference the new module path.
- **ECS resource wiring, lifecycle, and entity picking** (`PhylonApp::new`, `save_preferences`, `current_tick`, `pick_entity`) remain in `app.rs`, alongside the `PhylonApp` struct definition itself and `GpuContext`'s re-export — this is the composition root's actual job and doesn't decompose further without inventing an artificial boundary.

**`organisms/systems.rs` (1,823 → 1,425 lines)** bundled brain-wiring with body growth — the file's own doc comments already named this as a historical accident (`wire_brain_for_completed_organism` was originally `growth_system`'s own inline "Phase 1" block before Phase 7, W5a extracted it as a named function, but never moved to its own file). Extracted `should_wire_synapse`, `assign_hidden_node_regions`, `wire_brain_for_completed_organism`, and their 4 directly-associated pure-function unit tests to `crates/organisms/src/brain_wiring.rs` (416 lines, private module). The two integration tests that exercise brain-wiring through the shared `spawn_growth_entity`/`run_growth_to_completion` test harness stayed in `systems.rs`, since splitting them would have meant duplicating that harness rather than following a real seam.

**Not split further:** `systems.rs`'s remaining ~1,425 lines are `decode_next_segment`/`spawn_grown_segment`/`growth_system`/`producer_growth_system` (one cohesive body-growth concern) plus their test module (~755 lines) — no other file in the codebase has a colocated test module this large, but splitting tests into a mirrored file isn't an existing pattern in this codebase, and inventing one here would be scope creep beyond "split where a clear seam exists." `pick_entity` stayed in `app.rs` for the same reason — a single self-contained ~100-line method doesn't justify its own file.

**Verification:** `fmt`/`clippy --workspace --all-targets -D warnings`/`build --workspace`/`test --workspace` all clean, with identical test counts per crate before and after (no test was lost or silently dropped in the extraction) — plus `cargo doc --no-deps --document-private-items --workspace` clean, and a windowed release smoke test (log confirms `phylon::gpu_init` — the renamed module — logging `GPU surface initialised` at real startup, not just at compile time; no panics beyond the pre-existing, harmless `B0003` despawn-race warning).

---

## P9.5 — Viewport Gizmos: implemented, verified

Built strictly on top of the now-frozen camera math (ADR-P9-02) and the new ADR-P9-03 rule the user imposed before this milestone started ("camera interaction and viewport visualization become separate responsibilities" — gizmos only issue commands to existing camera/navigation APIs, never touch orbit math). New module: `crates/ui/src/plugins/gizmos.rs`, one entry point `render_gizmos`, called once per frame from `render.rs` right after `render_timed_effects` and before the Command Palette overlay.

Implemented, in the user's requested priority order:

1. **Navigation cube** (Top/Bottom/Front/Back/Left/Right) — a simplified 6-button egui overlay, top-right, rather than a true rendered 3D cube widget. Highlights whichever preset the camera's current forward vector is closest to (dot-product match against `PRESET_DIRECTIONS`); clicking pushes the existing `MenuAction::SetCameraPreset`. **Disclosed simplification, not silently presented as the real thing** — a genuine 3D-rendered view-cube widget was out of scope for this milestone's egui-overlay technique.
2. **XYZ axis triad** — fixed bottom-left corner, X/Y/Z drawn via `camera.right()`/`up()`/`forward()` projected as flat 2D directions (depth-ignoring), sorted back-to-front so the nearer axis draws on top.
3. **World origin indicator** — a small crosshair at world `(0, 0, 0)`, drawn only when it projects inside the viewport.
4. **Camera focus/pivot indicator** — a diamond at `OrbitController::focus`, Orbit mode only (Fly has no pivot concept, matching every other Orbit-only feature in this codebase).
5. **Selection bounding box** — a wireframe AABB (8 corners, 12 edges) around every `ParticleNode` sharing the selected entity's `organism_id`, using the same organism-walk pattern `MenuAction::FrameSelected` already established rather than a new query shape.
6. **Measurement gizmos** — not built as new work. The pre-existing `MarqueeMode::Measure` tool already covers this need; no gap was found that justified a second implementation.
7. **Clipping-plane manipulator** — deferred, per the user's own "optional later" scoping.

Plus the requested "beyond Blender" scientific-context overlay, `render_scene_info_overlay`: coordinate convention (Z-up), active projection (Perspective/Orthographic), active nav mode (Orbit/Fly), clip-plane status, and a **world-scale readout** (`Scale: {2×half-height}u across`, derived from Orbit's real focus distance and `fov_y`, or a clearly-labeled 100-unit reference depth in Fly mode since Fly has no pivot to measure from).

Every function in the module only reads `Camera3d`/`WorkbenchState`/ECS state and either paints through the existing, frozen `Camera3d::world_to_screen` projection or pushes a pre-existing `MenuAction` — nothing touches `OrbitController`/`FlyController` directly, and no new orientation math was added anywhere.

**Verification:** `fmt`/`clippy --workspace --all-targets -D warnings`/`build --workspace`/`test --workspace` (all pre-existing tests pass unmodified; no new automated tests were added since this module is pure egui painting with no non-trivial pure-function logic beyond `world_scale_label`, which was verified by hand-tracing against the existing orthographic/perspective test cases rather than a new test) all clean, plus a windowed release smoke test (no panics beyond the pre-existing, harmless `B0003` despawn-race warning).

**Disclosed limitation, same as every prior P9.x milestone:** no automated input-injection tooling in this environment to interactively click the navigation cube or watch gizmos track a live camera drag — verified by code-path reasoning (every gizmo's projection math is the same `world_to_screen` already exercised by P9.4's tests) plus the launch/stability smoke test, not a real interactive pass. If you can drive real input, confirming the navigation cube's highlight tracks the current view and that clicking each face actually lands on the expected preset is the one thing worth a manual check.

---

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

**Epic P9.5 — View-cube, navigation gizmo, transform gizmo. ✅ Done** — see the results section near the top of this document, and ADR-P9-03 below. Navigation cube shipped as a simplified egui button overlay, disclosed as such rather than a true rendered 3D widget; measurement gizmo satisfied by the pre-existing `MarqueeMode::Measure` tool (no new work needed); clipping-plane manipulator deferred per the user's own "optional later" scoping.

**Epic P9.6 — Targeted file decomposition. ✅ Done** — see the results section near the top of this document. `app.rs` split into `gpu_init.rs` (GPU/surface bring-up) and `species_seed.rs` (starter-species genome seeding), 1,796 → 579 lines; `organisms/systems.rs` split off `brain_wiring.rs`, 1,823 → 1,425 lines. Both remaining files' size reflects one cohesive concern each (ECS wiring/lifecycle, and body-growth systems + their test suite respectively), not an unaddressed mixed-responsibility problem.

**Epic P9.7 — UI polish pass. ✅ Done** — see the results section near the top of this document. Scope confirmed with the user as a light consistency pass (not the separately-approved 13-milestone UI Architecture Refinement plan): tokenized `gizmos.rs`'s hand-picked panel/marker colors through the existing `theme::` system, and swapped the Camera menu's ad-hoc Unicode toggle glyphs for the Remix Icon font every other menu entry already uses.

**Epic P9.8 — `MANUAL_TESTING.md`. ✅ Done** — see the results section near the top of this document. Authored last, once every other milestone's real feature set (not a guessed one) was known.

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
