# Camera & Viewport (3D Engine)

Phylon's viewport is a real 3D scene, navigated with a single canonical camera model. This document describes the current architecture; for the keybindings themselves, see [Controls](../reference/controls.md).

## `Camera3d` — the single source of truth

`ui::camera::Camera3d` (`position: Vec3`, `orientation: Quat`, `fov_y`, `near`, `far`) is the *only* place camera projection math lives. Before this was consolidated, the same projection-matrix derivation and screen↔world unprojection logic existed independently in half a dozen places (the renderer, the debug overlay, picking code, a field-overlay shader) and had drifted out of sync with each other. `Camera3d` exposes exactly two operations:

- `view_proj(aspect)` — the view-projection matrix for rendering.
- `screen_to_ray(screen_pos, viewport_size)` — an inverse-projected ray for picking.
- `world_to_screen(world_pos, viewport_size)` — the forward projection, used for on-screen gizmos, labels, and selection handles.

Any new code that needs to reason about the camera should call one of these, not re-derive a projection matrix locally.

`Camera3d` also carries `ortho_half_height: Option<f32>` (Phase 9, P9.4) — `None` (the default) means perspective; `Some(half_height)` means orthographic, with all three methods above branching accordingly (orthographic rays are genuinely parallel, not a perspective approximation). This is a projection-mode field only — it carries no orientation/orbit meaning, and toggling it never touches `yaw`/`pitch`/`focus`/`distance`.

## Controllers — frozen as of P9.3

Two controllers write into `Camera3d` — they hold input state and interaction logic; they never duplicate its projection math:

- **`OrbitController`** (default) — orbits freely around a pivot point (`focus`), with pan and dolly-zoom. Orientation is built from a genuine quaternion composition (`yaw` around world `Z`, then `pitch` around local `X`), not a from-forward-vector reconstruction — this has no degenerate point anywhere on the sphere, unlike the pre-P9.3 implementation, which used a fixed reference-up vector and consequently had to hard-clamp pitch short of the horizon to avoid a real "camera feels locked" bug. `pitch` is unbounded; orbit continues smoothly over the full sphere.
- **`FlyController`** (opt-in) — free-fly navigation, decoupled from any pivot. Unchanged by P9.3 — it still clamps pitch a few degrees short of straight up/down to keep its own basis non-degenerate.

Both are plain input-to-state translators: given mouse/keyboard deltas, they update `Camera3d`'s `position`/`orientation` and nothing else. **P9.3 was the last milestone permitted to change either controller's orientation/orbit mathematics** — later navigation work (P9.4's Frame Selected/Frame All/preset views/orthographic toggle included) only ever reads or writes their existing public fields (`focus`, `distance`, `yaw`, `pitch`) from the outside, or adds narrowly-scoped, additive fields elsewhere (`Camera3d::ortho_half_height`); it does not add new orientation logic to either type.

## Smooth camera transitions (P9.4)

`ui::frame_animation::FrameAnimation` drives a 250ms eased (smoothstep) transition of `OrbitController::focus`/`distance` only — `yaw`/`pitch` are left alone, so Frame Selected/Frame All re-center and re-distance without ever spinning the view. It lives outside `camera.rs` entirely (a `WorkbenchState` field, ticked once per rendered frame from `render.rs`) specifically so it never needed to touch the now-frozen controller types — it only ever writes their existing public `focus`/`distance` fields from the outside, the same way any other external caller would.

## Input — one canonical layer (ADR-P9-01)

Before Phase 9, two independent code paths each interpreted raw platform input and mutated the camera directly: an egui-routed path (mouse drag/scroll) and a separate winit-routed path (raw WASD/arrow keys), with zoom specifically triggered from three different call sites. This is now consolidated into `ui::viewport_input`:

- **`ViewportInput`** — a plain, platform-agnostic struct describing "what interaction happened this frame" (pan/rotate/zoom deltas, key-driven pan/fly-move axes, whether to detach camera-follow). It knows nothing about egui or winit.
- **Adapters** translate a raw input source into a `ViewportInput`: `ViewportInput::from_canvas_interaction` for the egui-routed mouse path; `app::events`'s keyboard handler builds one inline for the winit-routed WASD/arrow path. A future 3D-mouse, VR, touch, or synthetic/replay-driven input source is another adapter, not a third parallel camera-mutation path.
- **`apply_to_camera`** is the single `ViewportController` — the only function anywhere that reads a `ViewportInput` and mutates `OrbitController`/`FlyController`. Every adapter converges here.

Discrete, one-shot camera commands (Home/reset, menu-driven zoom-in/out, frame-selected, toggle camera mode) stay on `ui::types::MenuAction` — they were never the duplicated-gesture problem this layer fixes, since each already had exactly one dispatch path. `ViewportInput` is specifically for continuous, per-frame interaction (orbit/pan/zoom/fly).

## Camera bookmarks (P9.4)

`CameraBookmark` records `orbit_focus: Option<Vec3>` alongside its position/orientation snapshot — `Some(focus)` if `Orbit` mode was active at save time, `None` if `Fly` was. Restoring a bookmark reconstructs whichever mode it was saved in; a bookmark saved while orbiting no longer force-switches you to Fly mode on restore (a real, previously-disclosed bug, now fixed).

## Preset views (P9.4)

Six axis-aligned views (Top/Bottom/Front/Back/Left/Right) each just set `OrbitController::yaw`/`pitch` to a fixed value, leaving `focus`/`distance` untouched — a preset view only ever changes the viewing angle, matching Blender's own preset-view behavior of re-orienting around the existing pivot rather than re-framing it.

## Picking and selection

Picking is ray-based: `screen_to_ray` produces a world-space ray from a screen-space click, tested against organism capsule geometry (`rendering::picking::ray_capsule_hit`). Box-select and lasso-select build on the same `world_to_screen` projection, testing organism head positions against a screen-space rectangle or polygon.

## Rendering pipeline

Organisms render as GPU-instanced, oriented capsule meshes (a hemisphere-capped cylinder per particle-node-to-particle-node bone), shaded with a physically-based (Cook-Torrance) lighting model, replacing an earlier 2D SDF-metaball renderer. This was a deliberate, disclosed visual-identity change — evaluated alternatives (raymarched SDF, a rasterize-and-post-blend hybrid) were rejected or deferred with stated reasons, not silently dropped.

Field overlays (diffusion layers, clip planes) render as camera-facing or world-aligned quads, driven by the same `Camera3d` projection via an inverse-view-projection unproject in the field-overlay shader — so a clip plane or diffusion-field slice stays correctly registered with the 3D scene regardless of camera orientation.

## Spatial indexing

Organism/entity spatial queries (foraging broad-phase, sensing, GPU physics broad-phase) use `spatial::Octree` (replacing an earlier 2D `Quadtree`) on the CPU side, and a fixed-size 3D spatial hash on the GPU physics side (see [Simulation Model](simulation_model.md)) — the two are separate implementations, not a shared abstraction, since a 2D-generalized-to-3D data structure and a GPU-resident hash table have different enough constraints that forcing a shared interface would have made both worse.

## What is still 2D

Chemical diffusion fields are deliberately 2D world-space planes, not volumetric — see [Simulation Model](simulation_model.md) for why. The 3D engine work is a genuine architectural migration (camera, physics, rendering, growth orientation, vision are all real 3D), not yet paired with volumetric environmental fields; organisms' own vertical (Z) position is used, but the world they inhabit is still effectively a populated 3D space over a mostly-flat diffusion substrate.
