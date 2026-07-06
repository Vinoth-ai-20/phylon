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

**Accessibility note:** the Producer/Carnivore pair is green/red — the single most common form of color blindness, and was verified to stay separable under a Deuteranopia simulation. The pair that actually collides post-transform is Carnivore/Omnivore — see [`accessibility.md`](accessibility.md) for the full simulation table and the (currently unlanded) recommended fix; this file records the palette's *semantic* assignment, not its colorblind sign-off.

**Naming note:** unlike the non-diet chart tokens below, `chart_color(diet: &ecology::Diet)` is a function, not five separate constants — `Diet::standard_color()` is authored in the viewport's linear color space, so each call re-derives the on-screen sRGB value rather than caching it as a literal. Earlier drafts of this document referred to `CHART_PRODUCER`/`CHART_HERBIVORE`/etc. as if they were named constants; they aren't, and no such constants should be added — `chart_color()` is what call sites use.

## Chart series — non-diet data

Metrics' Performance/Resources/Environment plots chart data with no `ecology::Diet` counterpart, so these are plain constants in `theme.rs`, not derived from simulation state. Values match what `metrics.rs` drew before tokenization (see `IMPLEMENTATION_STATUS.md`'s Metrics finding) — naming them, not re-picking them.

| Token | Panel | Hex |
|---|---|---|
| `CHART_FPS` | Performance | white |
| `CHART_TPS` | Performance | light green |
| `CHART_MEM` | Performance | light red |
| `CHART_FOOD` | Resources | `#96FFFF` |
| `CHART_MINERALS` | Resources | `#969696` |
| `CHART_CORPSES` | Resources | `#C86464` |
| `CHART_SUNLIGHT` | Environment | yellow |
| `CHART_O2` | Environment | light blue |
| `CHART_CO2` | Environment | gray |
| `CHART_TEMP` | Environment | `#FFA500` |
| `CHART_SHANNON` | Diversity | `#64C8FF` |
| `CHART_SIMPSON` | Diversity | `#FF69B4` |
| `CHART_RICHNESS` | Diversity | `#FFD700` |
| `CHART_TURNOVER` | Diversity | `#9400D3` |
| `CHART_COLONY_DIAMETER` | Colony Connectivity | `#00CED1` |

Each panel's own set is internally distinct (verified by eye per plot); no cross-panel distinctness guarantee is made or needed, since Performance/Resources/Environment/Demographics/Diversity/Colony Connectivity are never rendered overlaid on one shared axis. (Phase 2, M1: Diversity and Colony Connectivity added — `analytics::MetricsState`'s Shannon/Simpson/richness/turnover/colony-diameter history was already being recorded every tick; these two charts are the first UI surface for it.)

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

`GOOD` is also reused (Phase 1, M3) for a handful of "this is currently active/affirmative" cues that previously hardcoded their own `LIGHT_GREEN`: the Inspector's entity-name heading, the status bar's "selected entity" chip and "System: Engine Online" label, and the Event Log's auto-scroll-on toggle. None of these are toasts, but all share the same "affirmative state" meaning `GOOD` already names.

## Destructive / urgent

| Token | Value | Use |
|---|---|---|
| `DANGER` | `rgb(220,80,80)` | Any destructive-action button or urgent-state indicator: Kill Entity (viewport context menu), Quit (splash screen), the toolbar's active-recording dot, and the Event Log's "death" category. Previously four independent near-matching reds (`rgb(220,80,80)`/`rgb(220,100,100)`/`rgb(220,60,60)`) with no reason for the difference except which file wrote them — unified in Phase 1, M3. |

## Playback state

| Token | Value | Use |
|---|---|---|
| `PLAYBACK_LIVE` | `LIGHT_GREEN` | Toolbar and status bar's play/live indicator — previously defined identically in both files independently. |
| `PLAYBACK_PAUSED` | `rgb(255,150,50)` | Toolbar and status bar's paused indicator, same duplication fixed. |

## Event Log category palette

A categorical palette for `event_log.rs`'s per-entry color, analogous in spirit to the Diet chart palette above but scoped to log-entry types, not simulation entities:

| Token | Value | Category |
|---|---|---|
| `LOG_BIRTH` | `rgb(100,220,100)` | birth/spawn events |
| `LOG_HAZARD` | `rgb(255,140,40)` | hazard/catastrophe/fire events |
| `LOG_MUTATION` | `rgb(160,100,255)` | mutation/speciation events |
| `LOG_USER` | `rgb(100,180,255)` | user-initiated/manual events |

Death/extinction events reuse `DANGER` rather than a fifth `LOG_*` token, since both already carried the identical RGB value.

## Chrome-bar specific

| Token | Value | Use |
|---|---|---|
| `CLOSE_RED` | one value | The Close button, everywhere a panel/window can be closed. Previously three implementations (`panel_chrome`, `top_bar_right_ui`, `floating_chrome`) each hardcoded their own red, and two of the three didn't even match each other (`rgb(180,80,80)` vs `rgb(220,80,80)`). |
| `DETACH_BLUE` | one value | The Detach/float button, same everywhere. |
| `MINIMIZE_YELLOW` | `rgb(180,180,60)` | The Minimize-to-title-bar button, the third chrome-bar action. |

## Disabled and focus

| Token | Use |
|---|---|
| `DISABLED_FG` | Muted/secondary/hint text app-wide (timestamps, empty-state hints, "Not Available" values) — replaces the ad hoc `egui::Color32::GRAY`/`DARK_GRAY` literals previously scattered across nearly every plugin file. See [`accessibility.md`](accessibility.md). |
| `DISABLED_BG` | Defined, but has no call site yet — no control in the workbench today renders a custom disabled background (egui's built-in disabled-state dimming applies automatically, and the codebase doesn't yet use `Ui::add_enabled`/`Ui::disable` anywhere). Stays defined and documented for the first panel that needs one. |
| `FOCUS_RING` | The visible focus outline for keyboard navigation, applied once in `theme::apply_style` to `style.visuals.widgets.active.{bg_stroke,fg_stroke}` — covers every focusable control app-wide, since egui renders keyboard focus using the same `active` `WidgetVisuals` state as a click-in-progress. Fixes the previously low-contrast default focus ring against Phylon's near-black chrome. |

## Depth and separation

The audit's core color finding: the viewport (near-black navy, animated with sunlight) and every UI panel (`CHROME_BG = rgb(24,24,28)`) sit within a few luma steps of true black, so there's no "this is the instrument, that is the specimen" cue the way Blender (mid-grey chrome, black viewport) or Unity (light-grey chrome, colored viewport) provide one. `VIEWPORT_FLOOR` exists specifically to give the canvas a fixed, distinguishable tone independent of the animated clear color.
