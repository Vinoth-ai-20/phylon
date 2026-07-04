# Colors

## The one rule that matters most

**`ecology::Diet::standard_color()` is the single source of truth for diet-category color, everywhere it appears.** Before this design system, it wasn't: the viewport and status bar read it correctly, but `crates/ui/src/plugins/metrics.rs` hand-picked five separate RGB literals for the same five categories, none of which matched (`Herbivore` is canonically blue `#48CAE4`; the Metrics chart drew it pale green `rgb(200,255,150)`). Every chart/legend/swatch token below (`CHART_*`) is a re-export of `standard_color()`, not a new literal, so this class of drift is impossible by construction going forward.

## Diet / chart palette (canonical, from `ecology::Diet::standard_color()`)

| Token | Category | Hex |
|---|---|---|
| `CHART_PRODUCER` | Producer | `#4CAF50` (green) |
| `CHART_HERBIVORE` | Herbivore | `#48CAE4` (blue) |
| `CHART_CARNIVORE` | Carnivore | `#F05454` (red) |
| `CHART_OMNIVORE` | Omnivore | `#FFB703` (amber) |
| `CHART_DECOMPOSER` | Decomposer | `#9B5DE5` (purple) |

**Accessibility note:** the Producer/Carnivore pair is green/red — the single most common form of color blindness. See [`accessibility.md`](accessibility.md) for the verification pass this palette must pass before Milestone 12 closes; this file records the palette's *semantic* assignment, not its colorblind sign-off.

## Chrome / surface

| Token | Value | Use |
|---|---|---|
| `CHROME_BG` | `rgb(24,24,28)` | Panel/window chrome background (was `layout.rs::PANEL_BG`, relocated here so `theme.rs` is the one place a color is defined) |
| `VIEWPORT_FLOOR` | fixed dark tone, distinct from `CHROME_BG` | The viewport's baseline tone, independent of the day/night clear-color animation layered on top — gives the simulation canvas visual separation from the surrounding UI chrome, which today are both near-black and read as one undifferentiated surface |

## Interactive accent

| Token | Use |
|---|---|
| `ACCENT` / `ACCENT_INK` | The one interactive accent color app-wide: active tab underline, focus ring, primary button. Deliberately distinct from every diet color and every semantic color below — an accent that happened to collide with "Carnivore red" would misread as simulation data. |

## Semantic (state, not accent)

| Token | Meaning | Use |
|---|---|---|
| `GOOD` / `GOOD_SOFT` | Success | Toast success state, "shortcut fixed" style confirmations |
| `WARN` / `WARN_SOFT` | Caution | Toast warnings, non-blocking validation |
| `BAD` / `BAD_SOFT` | Error/blocking | Toast errors, destructive-action confirmation |

Promoted from the pre-existing `ToastSeverity` color logic in `render.rs` (which was correct but only ever applied to toasts) — these become the one semantic palette used anywhere the UI needs to say "this is fine / be careful / this failed," including future validation states in dialogs and forms.

## Chrome-bar specific

| Token | Value | Use |
|---|---|---|
| `CLOSE_RED` | one value | The Close button, everywhere a panel/window can be closed. Previously three implementations (`panel_chrome`, `top_bar_right_ui`, `floating_chrome`) each hardcoded their own red, and two of the three didn't even match each other (`rgb(180,80,80)` vs `rgb(220,80,80)`). |
| `DETACH_BLUE` | one value | The Detach/float button, same everywhere. |

## Disabled and focus

| Token | Use |
|---|---|
| `DISABLED_FG` / `DISABLED_BG` | Any control in a disabled state — currently undefined anywhere in the codebase; every disabled control today falls back to whatever egui's built-in dark theme happens to do, unrelated to Phylon's own palette. |
| `FOCUS_RING` | The visible focus outline for keyboard navigation — currently undefined; egui's default focus ring is low-contrast against Phylon's near-black chrome. |

## Depth and separation

The audit's core color finding: the viewport (near-black navy, animated with sunlight) and every UI panel (`CHROME_BG = rgb(24,24,28)`) sit within a few luma steps of true black, so there's no "this is the instrument, that is the specimen" cue the way Blender (mid-grey chrome, black viewport) or Unity (light-grey chrome, colored viewport) provide one. `VIEWPORT_FLOOR` exists specifically to give the canvas a fixed, distinguishable tone independent of the animated clear color.
