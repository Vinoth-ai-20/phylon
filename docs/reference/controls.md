# Controls

Keyboard and mouse bindings for the live application. The single active shortcut system is `ui::shortcuts::ShortcutManager` — if a binding below doesn't fire, check that file, not any other code path (an earlier, separate hardcoded shortcut handler existed and silently shadowed several of these; it has since been removed).

## File & Session

| Action | Binding |
|---|---|
| Save state | `Ctrl+S` |
| Load state | `Ctrl+O` |
| Import genome | `Ctrl+Shift+I` |
| Export genome | `Ctrl+Shift+E` |
| Take screenshot | `Ctrl+Shift+S` |
| Toggle recording | `Ctrl+Shift+R` |

## Simulation Playback

| Action | Binding |
|---|---|
| Play / Pause | `Space` |
| Step forward one tick | `→` |
| Speed up | `↑` |
| Slow down | `↓` |

## Panels & Workspace

| Action | Binding |
|---|---|
| Toggle Metrics panel | `Ctrl+M` |
| Toggle Event Log panel | `Ctrl+L` |
| Toggle Sidebar | `Ctrl+B` |
| Command Palette | `Ctrl+Shift+P` |
| Global Search | `Ctrl+F` |

## Selection & Viewport

| Action | Binding |
|---|---|
| Select all | `Ctrl+A` |
| Deselect | `Esc` |
| Spawn (sandbox tool) | `Ctrl+P` |
| Toggle whether the selected entity is fixed in place | `F` |
| Delete selected entity | `X` |
| Toggle Orbit / Fly camera mode | `Tab` |

## Camera Navigation (Phase 9)

| Action | Binding |
|---|---|
| Orbit | Middle-drag (Orbit mode) |
| Pan | Left-drag (Orbit mode) |
| Look around | Middle-drag (Fly mode) |
| Fly move | `W`/`A`/`S`/`D` or arrow keys (Fly mode) |
| Pan (Orbit mode, keyboard) | `W`/`A`/`S`/`D` or arrow keys |
| Zoom in | `+` / `=` |
| Zoom out | `-` |
| Frame Selected (smooth) | `.` (period) |
| Frame All (smooth, fits the real current population) | `Home` |
| Reset camera (hard reset to the literal default view) | `Num 0` or `Ctrl+R` |
| Preset view: Front / Back | `1` / `Ctrl+1` |
| Preset view: Right / Left | `3` / `Ctrl+3` |
| Preset view: Top / Bottom | `7` / `Ctrl+7` |
| Toggle perspective / orthographic | View menu → Camera (no default keybinding) |

All of the above are also reachable from the **View → Camera** menu. See [Camera & Viewport](../explanation/camera_and_viewport.md) for the underlying `Camera3d`/`OrbitController`/`FlyController`/`ViewportInput` model these bindings drive, and `crates/ui/src/shortcuts.rs` for the authoritative, current binding list (this table is a convenience summary, not the source of truth — re-check it there if in doubt).
