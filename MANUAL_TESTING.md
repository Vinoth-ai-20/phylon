# Manual Testing Checklist

This project has no automated input-injection tooling (no way to synthesize
mouse drags/clicks or keyboard events against the live `winit`/`egui` window
in this environment) — every Phase 9 milestone that touched viewport
interaction, the camera, or gizmos disclosed this same limitation and
verified itself with unit tests plus a launch/stability smoke test instead of
a real interactive pass. This document is that missing interactive pass,
written down as a checklist a human tester runs against a real build.

Each item is phrased as an action and an expected result. Check off items as
you verify them; if an item fails, note the build (`git rev-parse HEAD`) and
open an issue rather than silently reverting the underlying change.

## Setup

1. `cargo build -p app --release`
2. Launch `target/release/phylon.exe` (or `cargo run -p app --release`) from
   the repo root, so it finds `data/default.ron`.
3. Confirm the window opens, a population is visible in the viewport, and
   the status bar's tick counter is advancing.
4. `git status` afterward and `git checkout -- data/runs.db
   data/preferences.ron` if either was modified by the session, unless you
   intend to keep those changes.

## Viewport Camera — Orbit Mode (default)

- [ ] Middle-drag: the view orbits smoothly around the current pivot; no
      jump, snap, or sudden flip anywhere, including when dragging past the
      top or bottom of the sphere (continuous pass-over-the-pole, per
      ADR-P9-02's quaternion-composition fix — the pre-P9.3 behavior would
      have hard-clamped here instead).
- [ ] Left-drag: the view pans (pivot translates); orbiting afterward still
      revolves around the new pivot, not the old one.
- [ ] Mouse wheel / trackpad pinch: zooms in/out smoothly, centered on the
      current pivot.
- [ ] `W`/`A`/`S`/`D` and arrow keys: pan the view (Orbit mode's keyboard
      pan), not fly-style movement.
- [ ] `+`/`=` and `-`: zoom in/out.
- [ ] Left-clicking an organism selects it (Inspector panel updates) without
      moving the camera or engaging camera-follow.
- [ ] Double-clicking an organism focuses the camera on it once (a single
      framing action), and does not continuously follow it afterward.
- [ ] Toggling **Follow** (toolbar or Inspector) visibly tracks the selected
      organism every tick, and the toggle's active state is visually
      distinguishable from its inactive state.
- [ ] Right-clicking an organism opens a context menu with working
      **Inspect** and **Track** actions.

## Camera Navigation (Phase 9)

- [ ] `.` (period) — **Frame Selected**: with an organism selected, the
      camera eases smoothly (~250ms, no snap, no spin — yaw/pitch must not
      change) to frame just that organism.
- [ ] `Home` — **Frame All**: camera eases smoothly to frame the entire
      current population, not a fixed/guessed distance — verify this looks
      different at tick 10 (small population) vs. tick 2000+ (larger, more
      spread out).
- [ ] `Num 0` or `Ctrl+R` — **Reset camera**: hard-resets to the literal
      default view (no easing — this is intentionally instant, unlike Frame
      Selected/All).
- [ ] `1` / `Ctrl+1`, `3` / `Ctrl+3`, `7` / `Ctrl+7` — the six preset views
      (Front/Back, Right/Left, Top/Bottom): each re-orients the view around
      the *existing* pivot without re-framing distance.
- [ ] View → Camera menu: every action above is also reachable here, and the
      **Toggle Perspective / Orthographic** and **Toggle Camera Mode**
      entries show the current icon/state correctly (not a fixed icon
      regardless of state).
- [ ] Toggling orthographic: the view's apparent scale doesn't jump
      jarringly at the moment of toggling; parallel edges actually render
      parallel (no vanishing point) in orthographic mode.
- [ ] `Tab` — toggles Orbit ↔ Fly camera mode; the mode shown in the Camera
      menu and the scene info overlay (bottom area of the nav-cube overlay,
      top-right) agree with each other and with actual behavior.
- [ ] Saving a camera bookmark while in Orbit mode, then restoring it, comes
      back in Orbit mode (not force-switched to Fly) — the P9.4 bookmark fix.

## Viewport Camera — Fly Mode

- [ ] `Tab` to enter Fly mode.
- [ ] `W`/`A`/`S`/`D` and arrow keys: move forward/back/strafe through the
      scene (not pan, unlike Orbit mode).
- [ ] Middle-drag: looks around (changes orientation) rather than orbiting
      around a pivot.
- [ ] Fly mode's own pitch clamp still prevents flipping upside-down
      (deliberately untouched by the P9.3 orbit-pitch unbounding).
- [ ] `Tab` back to Orbit mode: the view returns to a coherent orbit state
      (a real pivot, not a degenerate/frozen one).

## Viewport Gizmos (P9.5)

- [ ] Bottom-left axis triad is visible, shows three labeled/colored
      axes (X red, Y green, Z blue by convention), and each axis's on-screen
      direction visibly rotates to match camera orientation as you orbit.
- [ ] Top-right navigation cube (6 buttons: Top/Bottom/Front/Back/Left/
      Right) highlights whichever face is closest to the current view
      direction, and updates live as you orbit.
- [ ] Clicking each of the 6 navigation-cube buttons snaps to the expected
      preset view (cross-check against the keyboard shortcuts above — both
      paths should land on the same view).
- [ ] World origin crosshair is visible when world `(0,0,0)` is in view, and
      disappears when it's panned/orbited out of view.
- [ ] Orbit-mode pivot indicator (small diamond) appears at the current
      orbit focus point; it disappears in Fly mode (no pivot concept there).
- [ ] Selecting an organism draws a wireframe bounding box around its full
      body (all segments), not just its head node.
- [ ] Scene info overlay (below the nav cube) correctly reads: `Z-up`,
      current projection (Perspective/Orthographic), current nav mode
      (Orbit/Fly), clip-plane status, and a world-scale line — all four
      update live as you change the corresponding state.

## Performance

- [ ] At the default `target_organism_count` (1000) and default window size,
      the simulation sustains a visibly smooth frame rate (check the status
      bar's FPS/TPS readout) rather than the ~5 FPS baseline Phase 9 started
      from.
- [ ] Frame rate does not visibly degrade over a long run (30+ minutes) —
      watch for a slow downward drift, which would indicate an unbounded
      per-frame allocation Phase 9's scratch-buffer reuse (P9.1) was meant
      to eliminate.

## General Simulation Controls (sanity check, not new in Phase 9)

- [ ] `Space` — play/pause.
- [ ] `↑`/`↓` — speed up/slow down; the multiplier shown in the status bar
      updates accordingly.
- [ ] `→` — steps exactly one tick while paused.
- [ ] `Ctrl+S` / `Ctrl+O` — save/load state round-trips without errors.
- [ ] `Ctrl+Shift+S` — screenshot capture produces a real image file.
- [ ] `X` — deletes the selected entity.
- [ ] `Ctrl+Z`/menu Reset Simulation — returns to a fresh, valid population.

## Known limitations of this checklist

- This list covers Phase 9's own scope (camera, input, gizmos, performance).
  It is not a full regression suite for genetics/ecology/physiology — see
  the existing `cargo test --workspace` suite for that.
- "Smooth" and "jarring" above are necessarily subjective; if in doubt,
  compare frame-by-frame against a screen recording rather than relying on
  live impression alone.
- The Remix Icon font (used throughout the menu, including the P9.7-fixed
  orthographic toggle) requires the bundled font to have loaded correctly —
  if any menu icon renders as a tofu/box glyph, that's a font-loading
  regression, not a UI logic bug.
