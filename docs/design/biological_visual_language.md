# Biological Visual Language

**Status:** canonical specification. Required reading before any Phase 5 (`PHASE5_SX_ROADMAP.md`) milestone that touches a visual — nothing in Epic 1 (Simulation Readability) or beyond may invent an ad hoc encoding not defined here. If a milestone needs a state this document doesn't cover, this document is amended first, in its own small change, before the milestone's implementation.

## Why this document exists

Phase 5's own audit (`PHASE5_SX_ROADMAP.md` §1.2) found that Phylon's confirmed, dominant readability problem is not motion, it's communication: no consistent mapping from simulation state to appearance exists. Building that mapping one milestone at a time, each inventing its own color/glyph choices, would recreate the exact problem Phase 1's own design-token work solved for chrome/typography — driftable, inconsistent, ad hoc decisions. This document is that same discipline applied to *biological* state instead of *UI* state: **every simulation state gets exactly one canonical visual representation, decided once, here, and reused everywhere.**

## Encoding vocabulary

Every state below is assigned encodings from this fixed set — never a bespoke effect:

| Channel | What it means here |
|---|---|
| **Color** | A hue shift, from `theme.rs` tokens only — never a new literal |
| **Outline** | A ring/border around the organism's head (or a segment), reusing the existing `EcologicalCategory` ring and `render_highlight`'s selection-outline machinery (`app/src/render.rs`) |
| **Glyph** | A small icon above/near the organism (via `egui_remixicon`, matching every panel's existing icon usage) |
| **Motion** | An animation driven by a real numeric value (never a decorative loop) |
| **Opacity** | Alpha blending on the organism's rendered body |
| **Scale** | Size change, driven by a real value (e.g. growth progress) |
| **Particles** | A `events::TimedEffects` burst (floating text or a short-lived glyph), for *moments*, not ongoing state |
| **Label** | Plain text, always a last resort (highest cognitive cost per `docs/design/typography.md`'s own information-density guidance) |

**Rule: never color alone.** Every state pairs color with at least one non-color channel (outline shape/dash pattern, glyph, motion, or scale), so the visual language holds up under the colorblind simulations `docs/design/accessibility.md` already runs for the diet palette.

## Priority tiers

| Tier | Meaning | Default visibility |
|---|---|---|
| **Primary** | Core ecological storytelling — the Phase 5 brief's own success criteria (hunting/fleeing/feeding/reproducing/diseased/dying) | Always visible, population-wide, no toggle needed |
| **Secondary** | Real but momentary or slower-forming signals | Always visible, but transient (`TimedEffects`) or contextual (selected/tracked entity only) |
| **Tertiary** | Research/debug detail | Off by default; opt-in via an explicit toggle (matching P4-V2's existing `PhysiologyOverlayLayer` pattern) |

**Stacking rule:** because every state is assigned to a *different* channel wherever possible, most combinations (e.g. Fleeing + Diseased) simply coexist — a glyph and a ring don't compete. Where two states could plausibly claim the same channel (Health's opacity and Age's desaturation both touch color/alpha), an explicit precedence is stated in that state's own entry below, always survival-relevance-first.

### Numeric priority hierarchy (canonical from SX-1e onward)

The Primary/Secondary/Tertiary tiers above describe *default visibility*; they don't by themselves say what happens when two *visible* signals occupy the same screen position. Every entry below Behavior onward is written before this hierarchy existed and still uses the older Primary/Secondary/Tertiary language only — this table is the authoritative resolution rule that now governs *paint order* (which signal is allowed to visually sit on top of which), checked at every milestone from SX-1e onward, applying uniformly regardless of which tier an individual entry names itself:

| Priority | States | Paints |
|---|---|---|
| **1 (highest)** | Selection, Hover, Pinned organism | Last — always on top of everything else |
| **2** | Death, Predation, Critical health | After 3-5 |
| **3** | Disease, Reproduction, Development | After 4-5 |
| **4** | Behavior, Physiology, Ecological status | After 5 |
| **5 (lowest)** | Species identity, Pigmentation, cosmetic information | First — the base layer everything else can sit on top of |

**Enforcement, not aspiration — two real violations found and fixed at SX-1e:** re-auditing the existing draw order (not assumed correct) found two concrete Priority-1-below-Priority-4/5 violations: (1) in `crates/app/src/render.rs`'s GPU submission order, `debug_instances` (Health/Disease/Category badges — Priority 2/3/5) drew *after* the selection/hover highlight (Priority 1), meaning a low-health ring could visually obscure a selection outline; reordered so debug instances submit first, highlight last. (2) in `crates/ui/src/render.rs`'s egui overlay, `render_timed_effects` (Death/Reproduction bursts — Priority 2/3) drew *before* `render_behavior_glyphs`/`render_physiology_overlay` (Priority 4), meaning a Behavior glyph could paint over a same-position death burst; reordered so timed effects paint last among the biological overlays. Both are pure call-order fixes — no new drawing logic, no new rendering system, per the standing "reuse existing infrastructure" rule. Command Palette and Toasts (pure UI chrome, not biological signals) are unaffected and continue to paint above everything, since they're not part of this hierarchy at all.

**What "paints last" does NOT mean:** it does not mean lower-priority signals stop rendering or get hidden — every signal still draws in full. It means that where two signals' screen footprints genuinely overlap, the higher-priority one's pixels win in that overlap region, so it stays legible; everywhere else, both remain independently visible exactly as designed (e.g. Health's disk and Disease's offset badge, per the Disease entry below, are positioned specifically so they rarely need this rule to resolve anything between them).

---

## State specifications

Each entry: simulation state → biological meaning → viewport → inspector → chart → event log → accessibility → animation → priority → interaction (does it change interaction, e.g. can you click it).

### Behavior (Hunting / Fleeing / Foraging / Idle / Mating / Sleeping)

- **Meaning:** what an organism is actively doing this tick, from `behavior::BehaviorState`'s 6 variants.
- **Viewport:** a small glyph above the head — orange chevron (`ARROW_UP_S_LINE`, Hunting), red zigzag (`ALERT_LINE`, Fleeing), green leaf (`LEAF_LINE`, Foraging), pink heart (`HEART_LINE`, Mating), blue "Zzz" (`ZZZ_LINE`, Sleeping), none (Idle — **absence is the encoding for the most common state**, so the viewport isn't cluttered with a badge on every idle organism). Amended for SX-1b (this document's own rule: cover a needed state here before implementing it) — the original spec only enumerated 4 of `BehaviorState`'s 6 variants.
- **Inspector:** already shown live (`inspector.rs`'s Behavior section) — unchanged, this document doesn't touch it.
- **Chart:** none — behavior state is too high-frequency/per-organism for a time-series chart; population-level behavior *counts* could be a future Metrics addition, not specified here.
- **Event log:** none — too frequent to log without flooding (matches `NarrationLog`'s existing predation-only-for-deaths precedent).
- **Accessibility:** glyph shape carries the meaning, color is reinforcement only.
- **Animation:** none decorative — the glyph itself is static; only its presence/absence changes, driven directly by `BehaviorState`.
- **Priority:** Primary / **4** (numeric hierarchy, see above — paints beneath Death/Disease/Health).
- **Interaction:** no change — clicking the organism still selects it, same as today.

### Health

- **Meaning:** `metabolism::Health.current / .max`.
- **Viewport:** **Opacity** of the organism's rendered body scales with health fraction (below ~40%: also tints the existing `EcologicalCategory` ring amber; below ~15%: red) — reuses `theme::WARN`/`BAD`.
- **Inspector:** already shown (Physiology section) — unchanged.
- **Chart:** a future population-average-health line is plausible but not specified here (would duplicate existing per-diet population counts' story without new insight — see Engineering Rule against decorative/duplicate charts).
- **Event log:** none (too frequent).
- **Accessibility:** opacity + ring color are two channels; ring uses `WARN`/`BAD`, already colorblind-checked tokens.
- **Animation:** none — opacity is a direct, non-animated function of the current value each frame.
- **Priority:** Primary / **2** (numeric hierarchy, see above).
- **Interaction:** unchanged.

### Disease

- **Meaning:** `ecology::disease::Infection.state`/`SegmentInfection.severity`, with 5 distinguishable viewport states derived honestly from the real enum (not invented): **Healthy** (no `Infection` component), **Incubating**, **Infectious**, **Critical** (`Infectious` + aggregated segment severity or `Health` fraction past SX-1c's own `< 0.15` threshold — an intensification of `Infectious`, not a new state the simulation tracks separately), **Recovered** (permanently immune — see note below, this is not an ongoing "healing" process despite reading as "Recovering" at a glance).
- **Viewport (amended, SX-1d — corrected from this entry's original draft):** the original draft specified a "segmented/dashed ring" — re-audited at implementation time and found `crates/rendering/src/debug_quad.wgsl` (the shared primitive every population-wide overlay, including Health's SX-1c ring, actually draws through) only rasterizes a filled disk with a crisp radius cutoff; it has no annulus (hollow-center) or angular dash-pattern capability. Adding one would mean new shader work, out of scope for this milestone's budget. **Corrected spec:** a small filled-disk badge, offset up-and-left from the head position (not concentric with Health's centered disk — an explicit, documented interaction: this keeps the two always independently readable instead of alpha-blending into a muddy combined color when both apply to the same organism, since both are opaque-ish filled disks on the same shared primitive). Color: `ecology::Diet::Decomposer.standard_color()` (a live call, not a copied literal — corrected from an earlier draft's reference to a `theme::CHART_DECOMPOSER` token that was never actually added to `theme.rs`), scaled toward `theme::BAD` as severity approaches `Critical`. Size/alpha scale with the organism's segments' averaged `SegmentInfection.severity` (walked via `DevelopmentalGraph`, the same pattern `render_physiology_overlay`/Health's ring already established). **No animation** (corrected — the original draft specified a slow ticks-driven pulse; removed per explicit instruction: every SX-1d-and-later visual is fully static per current value, matching SX-1b/1c's own no-decorative-animation precedent applied strictly). `Incubating` gets the faintest, smallest, palest version of the badge (biologically asymptomatic, so deliberately the least prominent — but not literally invisible, since the brief for this milestone asks that it be distinguishable at a glance without opening a panel). `Recovered` gets a small, solid (not severity-scaled) `theme::GOOD` dot — a permanent "survived and immune" marker, distinct in color and constancy from the escalating Incubating→Infectious→Critical sequence. Per-segment severity detail remains the existing, unchanged P4-V2 opt-in overlay (Tertiary).
- **Inspector:** already shown (Ecology section shows Diet only today — this document requires adding `Infection` state to Inspector, closing a gap; see roadmap SX-4-family).
- **Chart:** none new (population infection *count* could extend Metrics' Demographics chart as an additional series — plausible future work, not specified here).
- **Event log:** onset already logged for the interesting case (spontaneous/transmitted infection) via this phase's own P4-V1 "Infected!" `TimedEffects` burst — canonical, unchanged.
- **Accessibility:** position offset + color + size/alpha, three channels — never color alone.
- **Animation:** none (corrected, see above).
- **Priority:** Primary (organism-wide badge) / Tertiary (per-segment severity detail, existing P4-V2 overlay) / **3** (numeric hierarchy, see above — paints beneath Death/Health, above Behavior/Physiology).
- **Interaction with Health (SX-1c):** documented above — offset position, not concentric, so the two never blend into an ambiguous combined color; Health's ring stays centered on the head (survival-relevance-first, per this document's own stacking rule), Disease's badge sits offset beside it.
- **Interaction:** unchanged (clicking still selects, same as today).

### Death (all causes — expanded SX-1e, was Predation-only)

- **Meaning:** an organism has died, from any `events::DeathCause` (`Predation`, `Starvation`, `Disease`, `Senescence`, `Injury`, `Environment`, `GodMode`, `Unknown`) — not just predation.
- **Viewport:** every cause now gets a `TimedEffects` floating-text burst at the death position, via one shared, exhaustive `death_effect_text_and_color` mapping (`crates/app/src/systems.rs`) — before SX-1e, only `Predation` ("Eaten!") triggered one; Starvation/Senescence/Disease deaths produced a correctly-caused `PhylonEvent` but no viewport signal at all (the gap `PHASE5_SX_ROADMAP.md` §2.6 flagged). Colors are never new literals: `Disease` reuses the *exact* purple `ecology::Diet::Decomposer.standard_color()` gives the Disease badge above (SX-1d), so a disease death visually reads as the same biological family as the badge it followed, not a coincidence; `Starvation`/`Environment` share `theme::WARN`; `Predation`/`Injury` share `theme::BAD`; `Senescence`/`GodMode` share the neutral `theme::ACCENT` (neither is a biological failure — one is a natural end, one is an experimenter action); `Unknown` gets a muted grey, deliberately not `BAD`, since an unclassified cause isn't confirmed adverse. `GodMode`/`Injury`/`Environment` aren't constructed by any code path yet, but are matched exhaustively anyway — a future system producing them needs no rendering change, only a new arm in that one function.
- **Priority-hierarchy interaction (new, SX-1e):** Death is Priority 2 — see the Numeric priority hierarchy section above. Enforced by reordering `render_timed_effects` to paint after `render_behavior_glyphs`/`render_physiology_overlay` (Priority 4), so a same-position Behavior glyph or Physiology ring never obscures a death burst.
- **Inspector:** N/A (the organism no longer exists — see roadmap SX-4a's "explicit death state" gap).
- **Chart:** none.
- **Event log:** still only `Predation` is logged to `NarrationLog` (unchanged this milestone — logging every death would flood it; a real, separate future decision, not silently expanded here).
- **Accessibility:** text-based burst, color reinforces cause but text is the primary channel.
- **Animation:** the existing fade-over-last-20-ticks in `render_timed_effects` — canonical, unchanged; no new animation added for the newly-covered causes.
- **Priority:** Secondary (Primary/Secondary/Tertiary sense) / **2** (numeric sense, see above).
- **Interaction:** none (entity is gone).

### Reproduction

- **Meaning:** a birth (`events::PhylonEvent::OrganismBorn`/`ReproductionEvent`).
- **Viewport:** already implemented — P4-V1's "Born!" `TimedEffects` burst (green). **Addition specified here:** pair it with a brief expanding cyan ring (particle channel) at the birth position, distinguishing "arrival" from "departure" (predation/death use text only; birth gets text + a expanding-ring motif) — a new, small extension of the existing framework, not a new system.
- **Inspector:** the new organism's Identity section already shows `BirthTick`/`ParentEntity` — unchanged.
- **Chart:** population charts already reflect births indirectly via count changes — no new chart.
- **Event log:** existing generation-milestone logging (every 5th generation) stays as is — this document doesn't require logging every birth (would flood the log, an intentional existing restraint).
- **Accessibility:** text + shape (expanding ring), not color-dependent.
- **Animation:** ring expands over a fixed short duration (reuse `TimedEffects`' tick-based expiry) — real event-triggered, not looping.
- **Priority:** Secondary (momentary).
- **Interaction:** the new organism becomes selectable immediately, same as today.

### Development / Growth

- **Meaning:** `organisms::growth_system` spawning a new Body Graph segment.
- **Viewport:** a new segment fades/scales in from 0 to full size over a short fixed number of ticks (reusing the `TimedEffects` tick-based timing pattern, generalized from "text with alpha" to "geometry with scale factor") rather than popping into existence at full size instantly.
- **Inspector:** a Development section (currently absent — a real gap per the SX-4 roadmap family) should show live `GrowthState` progress (segments grown / ceiling) while growing, and re-entrant growth's `LifeStage` when applicable (P4-L1).
- **Chart:** none.
- **Event log:** none (too frequent during normal growth).
- **Accessibility:** shape/scale-based, no color dependency.
- **Animation:** the fade/scale-in itself, tied directly to real segment-spawn events — never applied to an already-existing segment.
- **Priority:** Secondary.
- **Interaction:** unchanged.

### Age

- **Meaning:** `metabolism::Age.ticks / .max_lifespan`.
- **Viewport:** slow desaturation of the organism's emergent pigment color as this fraction approaches 1.0 — a background, slow-forming signal, not an alert.
- **Inspector:** already shown (Identity section) — unchanged.
- **Chart:** none new.
- **Event log:** senescence deaths already get their own `DeathCause` (P4-L2) — no additional logging required by this document.
- **Accessibility:** desaturation is a saturation change, not a hue change — distinguishable from Health's opacity/ring by being a *color* effect (muted, not transparent) rather than an *alpha* effect, so the two remain visually distinct even for an old AND unhealthy organism simultaneously.
- **Animation:** none decorative — a direct, continuous function of age fraction.
- **Priority:** Secondary. **Precedence rule:** if Health's opacity/ring is active (organism below ~40% health) at the same time as strong age-desaturation, Health's signal is drawn on top / takes visual precedence, since survival-relevant state outranks a slow background signal.
- **Interaction:** unchanged.

### Stress

- **Meaning:** `brain::Neuromodulators.noradrenaline` (already documented, in its own doc comment, as "arousal/stress signal ... high when energy reserves are low") — reused, not reinterpreted.
- **Viewport:** a subtle high-frequency positional jitter on the organism's rendered body, amplitude scaled by `noradrenaline` — real data driving real motion, per the Engineering Rules' "every animation must correspond to a biological process."
- **Inspector:** Hormone Viewer (P4-R3) already shows this value numerically — unchanged.
- **Chart:** none new.
- **Event log:** none.
- **Accessibility:** motion-based; a `prefers_reduced_motion`-style user setting (if/when Phylon adds one) should be able to suppress this specific jitter without affecting other encodings — noted as a requirement for whichever milestone implements it, not implemented by this document.
- **Animation:** the jitter itself — bounded, small amplitude, directly proportional to a real value, never present at `noradrenaline == 0`.
- **Priority:** Tertiary (subtle, easy to miss by design — stress is a contributing factor, not a headline state like Health/Disease).
- **Interaction:** unchanged.

### Mutation

- **Meaning:** a new organism's genome distance from its parent (`genetics::Genome::distance`, Phase 3 M7) exceeds a threshold — a "notable" mutation, not every routine one.
- **Viewport:** a `TimedEffects` burst at birth, reusing `theme::LOG_MUTATION`'s existing purple token (already defined for exactly this category in `event_log.rs`, just not yet connected to a viewport burst).
- **Inspector:** **Correction (Phase 7, W4d)** — this entry previously claimed `MutationCount`/`MutationHistory` were hardcoded "Not Available." Re-checked directly against `inspector.rs`: `MutationCount` already shows a live value (`genome.mutation_count.to_string()`, per that code's own "Phase 5, SX-4b" comment) — not hardcoded. However, it's a running mutate()-call counter, not the "distance from parent" metric this entry actually specifies — showing it doesn't satisfy this entry's stated encoding, a genuine metric mismatch, not just a stale doc. `MutationHistory` doesn't exist and was deliberately not added (a full per-event history is out of scope) — dropped from this entry rather than left as an aspirational claim.
- **Chart:** none new.
- **Event log:** already has a `LOG_MUTATION` category defined in the design system; this document specifies it should actually be used (currently no system publishes to it under this category — a real, existing gap this document surfaces).
- **Accessibility:** text-based burst.
- **Animation:** the existing `TimedEffects` fade, reused.
- **Priority:** Secondary (momentary, threshold-gated so it doesn't fire on every birth).
- **Interaction:** unchanged.

### Speciation

- **Meaning:** `evolution::SpeciesRegistry::classify` assigns a genuinely new `SpeciesId` for the first time.
- **Viewport:** population-wide, opt-in (Tertiary) species-glyph badges (a shape, not just a color, per the accessibility rule — e.g. a small distinct outline shape per species cluster, not just a hue) shown only when a "Show Species" toggle is active (mirrors P4-V2's `PhysiologyOverlayLayer` toggle pattern exactly — reuse, don't duplicate). At the moment a *new* species is first detected: a one-time, population-visible `TimedEffects` announcement plus a `NarrationLog` entry (both currently missing — see roadmap SX-3b).
- **Inspector:** Identity's `SpeciesId` is already live — unchanged.
- **Chart:** a species-over-time view is specified as SX-3b in the roadmap, not duplicated here.
- **Event log:** new "Speciation" category — reuse the existing `event_log.rs` filter-chip pattern, add one chip, not a parallel filtering mechanism.
- **Accessibility:** shape-coded badges, not color-only.
- **Animation:** the one-time announcement only; ongoing species badges are static.
- **Priority:** Secondary (announcement) / Tertiary (ongoing badges).
- **Interaction:** unchanged.

### Communication

- **Meaning:** `diffusion::SignalEmitter`'s active output strength.
- **Viewport:** a brief radiating ring (particle channel) at the emitting organism, triggered only when strength crosses a fixed threshold from below — an explicit edge-trigger, resolving the deferral P4-V1's own execution log recorded ("continuous phenomena don't map onto a discrete flash") by defining the discrete event as *crossing the threshold*, not the continuous value itself.
- **Inspector:** Behavior or a future Communication sub-section could show the live raw signal strength — not specified as a new Inspector section by this document (low priority relative to the other gaps already listed).
- **Chart:** none.
- **Event log:** none (too frequent even with thresholding, expected).
- **Accessibility:** shape/motion-based (expanding ring), not color-dependent.
- **Animation:** the ring expansion, `TimedEffects`-driven, real-threshold-triggered.
- **Priority:** Tertiary.
- **Interaction:** unchanged.

### Hormones

- **Meaning:** `brain::Neuromodulators`/`HormoneLevel` (P4-F4).
- **Viewport:** **already implemented, canonical as-is** — P4-V2's Hormone `PhysiologyOverlayLayer`, opt-in, per-segment ring colored by dominant channel.
- **Inspector:** Hormone Viewer (P4-R3), already implemented, canonical.
- **Chart:** none new specified.
- **Event log:** none.
- **Accessibility:** already ring + numeric value (Hormone Viewer's table) — two channels.
- **Animation:** none beyond the existing relaxation-driven value changes each tick.
- **Priority:** Tertiary (opt-in).
- **Interaction:** unchanged.

### Immune response

- **Meaning:** `ecology::disease::SegmentInfection`/`SegmentImmunity` (P4-F5).
- **Viewport:** the per-segment detail is P4-V2's existing Immune overlay, canonical, opt-in (Tertiary) — the organism-wide summary is Disease's own Primary-tier ring (above); this entry exists to make explicit that Immune response's *detailed* per-segment view and Disease's *headline* view are deliberately two tiers of the same underlying data, not two competing encodings.
- **Inspector:** Immune Viewer (P4-R4), canonical.
- **Chart/event log:** see Disease.
- **Accessibility:** see Disease/Hormones.
- **Animation:** none beyond existing per-tick severity changes.
- **Priority:** Tertiary (detail) — see Disease for the Primary tier.
- **Interaction:** unchanged.

### Circulation

- **Meaning:** `metabolism::ChemicalEconomy`, moved by `organisms::transport_system` (P4-F3).
- **Viewport:** **already implemented, canonical as-is** — P4-V2's Circulation overlay, opt-in, per-segment ring by ATP fraction. Disclosed limitation (from P4-R2/V2's own execution log, restated here as canonical, not re-litigated): shows current levels, not animated flow — a future milestone may add particle-trail flow animation along Body Graph edges, which would become this entry's new canonical viewport encoding when it lands, superseding the ring.
- **Inspector/Chart/event log:** see Physiology Viewer (P4-R1)/Circulation Viewer (P4-R2), canonical, unchanged.
- **Priority:** Tertiary (opt-in).
- **Interaction:** unchanged.

### Neural activity

- **Meaning:** live `Brain` CTRNN node states/outputs.
- **Viewport:** none population-wide (this is a research/debug-adjacent detail, not core ecological storytelling per the Phase 5 success criteria) — Tertiary, selection-only: a subtle glyph pulse-rate on the *selected* organism only, tied to output variance over the last second (real data, not decorative), so a researcher watching one organism can visually correlate "is this brain doing anything interesting" without population-wide clutter.
- **Inspector:** **Correction (Phase 7, W4d)** — this entry previously claimed the Neural section's `CTRNNState`/`BrainOutputs` fields were hardcoded "Not Available," required before this entry's viewport glyph could be considered complete. Re-checked directly against `inspector.rs`: `BrainInputs`/`BrainOutputs`/`NeuronActivity`/`SynapseActivity` already show real live per-tick values (a 6-value preview), per that code's own "Phase 5, SX-4b" comment — this gap has already been closed. Only the viewport pulse-glyph itself (below) remains unimplemented; no pulse/output-variance rendering code exists yet.
- **Chart:** none.
- **Event log:** none.
- **Accessibility:** motion-based, single organism only (low population-wide accessibility risk).
- **Animation:** pulse rate directly proportional to measured output variance.
- **Priority:** Tertiary.
- **Interaction:** unchanged.

### Environmental hazards

- **Meaning:** `ecology::catastrophe`'s hazard field.
- **Viewport:** **already implemented, canonical as-is** — the existing world-space hazard field overlay. **Addition specified here:** pair each new hazard's spawn with a `TimedEffects` burst at its origin (currently only `NarrationLog` gets a "Hazard" text entry — the viewport itself has no moment-of-spawn cue beyond the field gradually appearing), for consistency with every other event category in this document.
- **Inspector:** N/A (environmental, not per-organism).
- **Chart:** none new.
- **Event log:** already logged — canonical, unchanged.
- **Accessibility:** the field itself is a gradient (already a scale/intensity encoding, not color-only, per its existing heatmap-style rendering).
- **Animation:** the burst above; the field's own animation is already real (diffusion-driven), unchanged.
- **Priority:** Primary (the field itself, already always-visible) / Secondary (the new spawn burst).
- **Interaction:** unchanged.

### Physiology (general `ChemicalEconomy`, not circulation specifically)

- **Meaning:** the organism-level (head) `ChemicalEconomy` — glucose/ATP/O2/CO2 overall.
- **Viewport:** **folds into Health's encoding, deliberately, rather than a separate one** — ATP depletion is the proximate cause of starvation death, and Health's opacity/ring already communicates "this organism is in survival danger" at the right level of abstraction for population-wide, always-visible signaling. A separate physiology-specific population-wide encoding would compete with Health for the same "how urgent is this" question without adding new information at a glance.
- **Inspector:** Physiology Viewer (P4-R1), canonical, opt-in detail.
- **Chart/event log:** unchanged.
- **Priority:** folds into Health (Primary); detailed view is Tertiary (P4-R1).
- **Interaction:** unchanged.

### Research selection

- **Meaning:** `WorkbenchState.selected_entity`.
- **Viewport:** **already implemented, canonical as-is** — the pulsing white outline (`render.rs`'s `render_highlight`), distinct from the static green hover outline and from every biological-state ring above (white is reserved exclusively for selection, never reused for any biological meaning, so it can never be confused with one).
- **Inspector:** the entire Inspector *is* this state's detail view — unchanged.
- **Priority:** Primary (for the one selected entity).
- **Interaction:** this is the interaction mechanism itself.

### Debug-only state

- **Meaning:** structural/segment-type debug coloring (`rendering/src/debug.rs`), the `EcologicalCategory` ring's own base implementation, and any future purely-technical overlay.
- **Viewport:** stays in Debug Mode/`rendering` only — **explicitly and permanently out of scope for the biological visual language**, so debug information can never be mistaken for a biological signal. This is a deliberate, permanent boundary, not a gap to close.
- **Priority:** N/A (different visual register entirely, by design).
- **Interaction:** unchanged.

### Experimental / externally-controlled state

- **Meaning:** `brain::Brain.external_override` is `Some` (an RL agent or scripted intervention is driving this organism's actions, not its own evolved brain).
- **Viewport:** a small, distinct "override" glyph (e.g. a circuit/wrench icon), always shown whenever this is active, regardless of any other toggle — a scientific-integrity signal: a researcher must never mistake externally-controlled behavior for emergent behavior when reading the viewport.
- **Inspector:** Behavior section should state this plainly (not currently shown — a gap this document surfaces).
- **Chart/event log:** none.
- **Accessibility:** glyph-based, always paired with an Inspector text confirmation.
- **Animation:** none — static glyph, present exactly when the override is active.
- **Priority:** Primary whenever active (rare, but never allowed to be missed).
- **Interaction:** unchanged.

---

## Summary table (quick reference)

| State | Primary encoding | Priority | Visibility |
|---|---|---|---|
| Behavior | Glyph (absent = Idle) | Primary | Always |
| Health | Opacity + ring color | Primary | Always |
| Disease (organism-wide) | Offset filled-disk badge, Decomposer purple | Primary | Always |
| Predation | `TimedEffects` text | Secondary | Momentary |
| Reproduction | `TimedEffects` text + ring | Secondary | Momentary |
| Development/Growth | Scale-in animation | Secondary | Momentary |
| Age | Desaturation | Secondary | Always (subtle) |
| Stress | Motion jitter | Tertiary | Always (subtle) |
| Mutation | `TimedEffects` text | Secondary | Momentary (thresholded) |
| Speciation | Badge + one-time announcement | Secondary/Tertiary | Momentary / opt-in |
| Communication | Radiating ring | Tertiary | Momentary (thresholded) |
| Hormones | Segment ring (existing) | Tertiary | Opt-in |
| Immune response (detail) | Segment ring (existing) | Tertiary | Opt-in |
| Circulation | Segment ring (existing) | Tertiary | Opt-in |
| Neural activity | Glyph pulse (selected only) | Tertiary | Contextual |
| Environmental hazards | Field (existing) + burst | Primary/Secondary | Always / momentary |
| Physiology (general) | Folds into Health | Primary | Always |
| Research selection | Pulsing white outline (existing) | Primary | Contextual (selected) |
| Debug-only | Out of scope, permanently | N/A | Debug mode only |
| Experimental/override | Glyph, always | Primary | Always when active |

## What this document does not do

It does not implement any of the above. Per the corrected Phase 5 milestone order, `PHASE5_SX_ROADMAP.md`'s Epic 1 (SX-1b onward) implements against this specification, one milestone at a time, with its own re-audit/verification discipline. Any milestone that finds this specification wrong or incomplete for a state it's implementing amends this document first, in its own small change, before continuing — the same rule this document opens with.
