# Accessibility

## Highest-priority finding (from the original audit) — verified, Milestone 12; fixed, Phase 6 Epic J

Phylon's core visual language leans on 5 hue-coded diet categories, repeated in the viewport, the status bar, and (since Milestone 7) the Metrics charts. Red–green confusion is the single most common form of color blindness (~8% of men), so the diet palette (`ecology::Diet::standard_color()`) was run through a Viénot-matrix Deuteranopia simulation:

| Diet | sRGB (normal vision) | Simulated (deuteranopia) |
| --- | --- | --- |
| Producer | `#4CAF50` (green) | `#71696D` — desaturated grayish-brown |
| Herbivore | `#48CAE4` (blue) | `#796FDC` — blue-purple (stays distinct: blue channel dominates) |
| Carnivore | `#F05454` (red) | `#B5C154` — yellow-olive |
| Omnivore (original, pre-Phase-6) | `#FFB703` (amber) | `#E4E939` — yellow |
| Decomposer | `#9B5DE5` (purple) | `#8488BC` — blue-gray (moderately close to Herbivore, still separable by lightness) |

**Original finding — not the pair the original audit named:** Producer and Carnivore turn out to stay reasonably separable (one desaturated gray-brown, the other yellow-olive), because Producer's green retains a non-trivial blue component that survives the transform. The pair that actually collided was **Carnivore and Omnivore** — red and amber both converged on near-identical yellow-olive tones (`#B5C154` vs `#E4E939`), differing mainly in lightness, which is a weak signal under real-world viewing conditions (small viewport dots, chart lines).

**Fixed, Phase 6 Epic J (Milestone J5).** This was flagged-not-fixed through Milestone 12 and the whole of Phase 5, since changing `Diet::standard_color()` changes the simulation's visual identity outside the `ui` crate boundary, not just a chrome color, and needed its own explicit sign-off. Measured (not guessed) via a throwaway Machado et al. (2009) deuteranopia simulation matrix applied to the real palette (`crates/ecology/examples/deuteranopia_check.rs`, deleted after use, per this project's "measure honestly, then delete" convention): the first hypothesis — shifting Omnivore's hue *toward* orange/red-adjacent tones — was tested and found to make the collision **worse** (converges harder with Carnivore's red under the transform), not better. Shifting instead toward a fully saturated, high-lightness bright yellow (`#FFDE00`, vs. the original amber `#FFB703`) measurably improved simulated-color separation from Carnivore (+43%), Producer (+35%), and Decomposer (+8%), at the cost of a small reduction vs. Herbivore (-7% — still an enormous margin; Herbivore's blue is nowhere near the yellow/red/green cluster this palette's real risk lives in). `Diet::Omnivore`'s `standard_color()` now returns `[1.0, 0.737972, 0.0]` (linear-space, `#FFDE00` sRGB) instead of `[1.0, 0.482, 0.0]` (`#FFB703`).

## Tritanopia risk (Herbivore/Decomposer) — measured, Phase 7 W4c: not a real collision

The Phase 7 audit flagged an **unverified** risk (not confirmed, not dismissed): Herbivore and Decomposer's `standard_color()` values share a near-identical linear-space blue channel (0.776 vs 0.789), a plausible tritanopia (blue-yellow deficiency) collision — but explicitly not measured at the time it was flagged, per this project's "measure before changing" discipline (the same rule that shaped the Deuteranopia fix above).

**Measured, not assumed.** Applied a Machado, Oliveira & Fernandes (2009) tritanopia simulation matrix directly to `standard_color()`'s real linear-space RGB values (correctly *not* re-decoding them as sRGB — the function's own doc comment states they're already linear), then compared all 10 diet-color pairs by CIE Lab distance (ΔE), both under normal vision and under the simulated deficiency:

| Pair | Normal ΔE | Tritanopia ΔE |
| --- | --- | --- |
| Herbivore vs. Decomposer (the flagged pair) | 90.5 | **69.5** |
| Producer vs. Herbivore | 68.7 | 23.5 (largest relative drop, still separable) |
| Omnivore vs. Decomposer | 163.7 | 47.5 |
| (all other pairs) | 91–164 | 48–139 |

**Finding: not a real collision.** Herbivore/Decomposer's shared blue channel does not produce a tritanopia collision — their simulated-tritanopia distance (69.5) stays far above the range where two of this palette's genuinely-confusable pairs would sit (for comparison, Phase 6's real, fixed Deuteranopia collision was between two colors converging to `#B5C154`/`#E4E939`, a difference small enough to read as "the same yellow-olive" at a glance). No pair in this palette collapses under simulated tritanopia the way Carnivore/Omnivore did under deuteranopia — the closest pair (Producer/Herbivore) still separates by a wide margin. **No color change is justified by this measurement** — `Diet::standard_color()` is unchanged. This closes the flagged-but-unverified risk as measured-and-dismissed, the same honest outcome Phase 6's own Deuteranopia check reached for the Producer/Carnivore pair it *originally* suspected before measuring the real collision elsewhere.

## Minimum text size

`SIZE_SMALL` was raised from 11px to 12px (see [`typography.md`](typography.md)) — 11px is below a comfortable floor for an 8-hour continuous research session. `SIZE_MICRO` (11px) is retained only for the status bar's system zone, a deliberate, narrow exception, not a floor being ignored elsewhere.

## Focus visibility — implemented, Milestone 12

`FOCUS_RING` (see [`colors.md`](colors.md)) is applied in `theme::apply_style` to `style.visuals.widgets.active.{bg_stroke,fg_stroke}`. egui renders a keyboard-focused widget using its `active` `WidgetVisuals` (the same state used while a widget is being clicked/dragged — see `egui::style::Widgets::style`), so this one call site covers every focusable control app-wide rather than needing a per-widget fix. Focus order follows egui's default tab order, which already matches visual/logical layout order in every panel (nothing overrides it).

## Disabled state — implemented, Milestone 12

`DISABLED_FG` now replaces the ad hoc `egui::Color32::GRAY` literal that was scattered across nearly every plugin file for muted/secondary/hint text (timestamps, empty-state hints, "Not Available" values, disabled-looking icons) — previously `DISABLED_FG` was defined in `theme.rs` but never referenced anywhere, so two near-identical grays existed with no enforced relationship. `DISABLED_BG` has no current call site — no control in the workbench today renders a custom disabled background (egui's own disabled-state dimming applies automatically via `Ui::add_enabled`/`Ui::disable`, which the codebase doesn't yet use anywhere); it stays defined and documented for the first panel that needs one.

## Selection is never color-alone

Any "this is selected" signal (viewport entity outline, sidebar active tab, list row highlight) pairs color with a second cue — an outline, a weight change, an icon state — never relying on hue alone to carry the meaning. This is both a colorblind-safety rule and a general low-vision accommodation.

## Keyboard navigation

Tab/Shift+Tab moves between controls in one predictable order per panel. Escape always closes the topmost transient UI (menu, tooltip, dialog) without side effects. This is checked as part of every milestone's Definition of Done, not audited once at the end.

## What's explicitly out of scope for now

Screen-reader support is not part of this roadmap — egui does not have mature screen-reader integration as of this writing, and retrofitting it is a substantially larger effort than the 13 milestones here cover. This is noted as a known, deliberate gap rather than a silent omission.
