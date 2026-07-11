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
| Reset camera | `Ctrl+R` |
| Frame all / reset zoom | `Home` or `Num 0` |
| Zoom in | `+` / `=` |
| Zoom out | `-` |
| Focus on selection | `F` |
| Tab between UI regions | `Tab` |

See [Camera & Viewport](../explanation/camera_and_viewport.md) for the underlying `Camera3d`/`OrbitController`/`FlyController` model these bindings drive, and `crates/ui/src/shortcuts.rs` for the authoritative, current binding list (this table is a convenience summary, not the source of truth — re-check it there if in doubt).
