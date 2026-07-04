# Typography

## Type family

- **IBM Plex Sans** (Regular + SemiBold) — all UI text. Chosen over egui's default (Ubuntu-Light) because IBM Plex is designed for small-size screen legibility and ships tabular figures, which matters in a UI where numbers (tick counts, ATP levels, population counts) update every frame.
- **IBM Plex Mono** (Regular) — anywhere digits line up in a column or update live: status bar counters, Inspector's chemical/genetic values, Metrics axis labels. Monospace digits stop numbers from visually reflowing their neighbors as they change.

Both are embedded via `include_bytes!` in `crates/ui/src/theme.rs::install_fonts()` — no runtime font loading, no risk of a missing-font fallback.

## Scale

The original scale (4 steps: 18/14/13/11) was too compressed — panel titles (14) and body text (13) read as the same size at arm's length. Extended to 6 steps:

| Token | Size | Weight | Family | Use |
|---|---|---|---|---|
| `SIZE_DISPLAY` | 22px | SemiBold | Plex Sans | Dialog/modal titles only (About, Keybinds) — never used in the docked workbench itself |
| `SIZE_HEADING` | 18px | SemiBold | Plex Sans | `ui.heading()`, sidebar panel section headers |
| `SIZE_SUBHEADING` | 15px | SemiBold | Plex Sans | Panel/window chrome titles — bumped from 14 specifically to read as distinct from Body at a glance |
| `SIZE_BODY` | 13px | Regular | Plex Sans | Default text, data-grid rows, buttons — the app's baseline |
| `SIZE_SMALL` | 12px | Regular | Plex Sans | Timestamps, counts, footers — bumped from 11 (11px is below a comfortable floor for an 8-hour session) |
| `SIZE_MICRO` | 11px | Regular | Plex Sans | Status-bar secondary/system zone only — the one place a smaller size is deliberate, not a floor being ignored |

## Numerals

Every value that updates live (tick count, FPS/TPS, population counts, ATP/glucose readouts, camera coordinates) must render through the Monospace family (`.monospace()` in egui, or `TextStyle::Monospace`) so digits are tabular. This was previously applied inconsistently — most numbers used proportional Plex Sans, which visibly shifted neighboring text every time a digit count changed (e.g. population 99→100). This is a hard rule going forward, checked in every milestone's Definition of Done, not a style suggestion.

## Icon sizing

Icon glyphs (Remix Icon, via `egui-remixicon`) are a separate scale from text, since an icon's legibility curve is different from a letterform's — see [`iconography.md`](iconography.md).

## Capitalization

- Menu items and dialog titles: Title Case ("Take Screenshot").
- Toolbar hover text and body copy: sentence case ("Show world boundary").
- Status-bar abbreviations (FPS, TPS, ATP, CO2): always uppercase, no periods.

These three conventions coexist because they serve different reading contexts (a menu is scanned top-to-bottom, hover text is read as a sentence) — the rule is consistency *within* each context, not one convention everywhere.
