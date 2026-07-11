# Camera & Viewport (3D Engine)

Phylon's viewport is a real 3D scene, navigated with a single canonical camera model. This document describes the current architecture; for the keybindings themselves, see [Controls](../reference/controls.md).

## `Camera3d` — the single source of truth

`ui::camera::Camera3d` (`position: Vec3`, `orientation: Quat`, `fov_y`, `near`, `far`) is the *only* place camera projection math lives. Before this was consolidated, the same projection-matrix derivation and screen↔world unprojection logic existed independently in half a dozen places (the renderer, the debug overlay, picking code, a field-overlay shader) and had drifted out of sync with each other. `Camera3d` exposes exactly two operations:

- `view_proj(aspect)` — the view-projection matrix for rendering.
- `screen_to_ray(screen_pos, viewport_size)` — an inverse-projected ray for picking.
- `world_to_screen(world_pos, viewport_size)` — the forward projection, used for on-screen gizmos, labels, and selection handles.

Any new code that needs to reason about the camera should call one of these, not re-derive a projection matrix locally.

## Controllers

Two controllers write into `Camera3d` — they hold input state and interaction logic; they never duplicate its projection math:

- **`OrbitController`** (default) — orbits around a pivot point, with pan and dolly-zoom.
- **`FlyController`** (opt-in) — free-fly navigation, decoupled from any pivot.

Both are plain input-to-state translators: given mouse/keyboard deltas, they update `Camera3d`'s `position`/`orientation` and nothing else.

## Picking and selection

Picking is ray-based: `screen_to_ray` produces a world-space ray from a screen-space click, tested against organism capsule geometry (`rendering::picking::ray_capsule_hit`). Box-select and lasso-select build on the same `world_to_screen` projection, testing organism head positions against a screen-space rectangle or polygon.

## Rendering pipeline

Organisms render as GPU-instanced, oriented capsule meshes (a hemisphere-capped cylinder per particle-node-to-particle-node bone), shaded with a physically-based (Cook-Torrance) lighting model, replacing an earlier 2D SDF-metaball renderer. This was a deliberate, disclosed visual-identity change — evaluated alternatives (raymarched SDF, a rasterize-and-post-blend hybrid) were rejected or deferred with stated reasons, not silently dropped.

Field overlays (diffusion layers, clip planes) render as camera-facing or world-aligned quads, driven by the same `Camera3d` projection via an inverse-view-projection unproject in the field-overlay shader — so a clip plane or diffusion-field slice stays correctly registered with the 3D scene regardless of camera orientation.

## Spatial indexing

Organism/entity spatial queries (foraging broad-phase, sensing, GPU physics broad-phase) use `spatial::Octree` (replacing an earlier 2D `Quadtree`) on the CPU side, and a fixed-size 3D spatial hash on the GPU physics side (see [Simulation Model](simulation_model.md)) — the two are separate implementations, not a shared abstraction, since a 2D-generalized-to-3D data structure and a GPU-resident hash table have different enough constraints that forcing a shared interface would have made both worse.

## What is still 2D

Chemical diffusion fields are deliberately 2D world-space planes, not volumetric — see [Simulation Model](simulation_model.md) for why. The 3D engine work is a genuine architectural migration (camera, physics, rendering, growth orientation, vision are all real 3D), not yet paired with volumetric environmental fields; organisms' own vertical (Z) position is used, but the world they inhabit is still effectively a populated 3D space over a mostly-flat diffusion substrate.
