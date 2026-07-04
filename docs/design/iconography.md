# Iconography

## Family

Every icon in Phylon comes from **Remix Icon**'s line set, via the `egui-remixicon` crate — a genuine existing strength confirmed during the audit: stroke width and visual weight already match across the entire UI because there's only ever been one icon source. This document governs sizing and meaning, not family (the family is already correct and unchanging).

## Size tokens

Icon sizes were previously ad hoc literals (11, 12, 13, 14, 16, 18, 20, 36, 64px scattered across `toolbar.rs`, `sidebar.rs`, `render.rs`, `dialogs.rs`) with no naming. Tokenized:

| Token | Size | Use |
|---|---|---|
| `ICON_SM` | 14px | Inline with body text (chrome-bar Close/Detach glyphs) |
| `ICON_MD` | 16px | Toolbar buttons (step, restart, screenshot) |
| `ICON_LG` | 20px | Activity bar / sidebar tab icons |
| `ICON_XL` | 40px+ | Splash screen only — explicitly exempted from the standard scale, since it's a different context (a one-time launch screen, not workbench chrome) |

## Semantic gaps found and fixed

- **World-boundary toggle** used `CROP_LINE` (`toolbar.rs`) — a crop icon conventionally means "trim this image," not "show world bounds." Replaced with `RECTANGLE_LINE` (a literal bounding-box glyph) in Milestone 13, and the toggle itself moved from the always-visible toolbar into the View menu (alongside the Colormap selector) as a labeled checkbox, since neither is a per-frame control that needs constant screen real estate.
- **Sidebar tab icons** (Inspector `SEARCH_LINE`, Genetics `TEST_TUBE_LINE`, Ecology `EARTH_LINE`, Environment `CLOUD_LINE`, Analytics `LINE_CHART_LINE`, Sandbox `TOOLS_LINE`, Tuning `EQUALIZER_LINE`, Settings `SETTINGS_LINE`) are each reasonably literal already and unchanged — the *discoverability* gap (see [`components.md`](components.md)'s `labeled_icon_tab`) was the lack of a persistent label, not the icon choices themselves. Their **display labels** did need renaming in Milestone 13, though: "Analytics" → **"Snapshot"** (it's a single point-in-time readout of current counts, not the time-series charts — the old name invited confusion with the separate Metrics dashboard) and "Tuning" → **"Simulation Parameters"** (the panel is entirely speed/thickness/atmosphere sliders — "Tuning" undersold what it actually configures). The `SidebarTab` enum variant names themselves are unchanged; only the user-facing strings in `tab_label()`/`NAV_TABS` moved.
- **"Structural" render mode** (the debug wireframe view, surfaced in the View menu checkbox, the Simulation Parameters panel, and the status bar's System tooltip) renamed to **"Wireframe"** throughout — "Structural" described the code path, not what a user sees on screen.
- **Screenshot/Recording** (`SCREENSHOT_LINE`, `RECORD_CIRCLE_LINE`/`RECORD_CIRCLE_FILL`) are correctly literal and unchanged.

## Rule

An icon is never the sole carrier of meaning for an action that isn't near-universally recognized (play/pause, close, search are fine icon-only; "toggle world boundary" or "toggle wireframe" are not) — either a persistent label or a tooltip is mandatory. This is why `labeled_icon_tab` (see [`components.md`](components.md)) defaults to icon+label rather than icon-only.
