# Phylon Design System

This is the entry point for Phylon's design system — the permanent, versioned source of truth for every visual and interaction decision in the workbench UI. It exists because a UI/UX audit (scored 5.8/10 against Blender/Unity/JetBrains/MATLAB/ParaView-class tooling) found that Phylon's existing token module (`crates/ui/src/theme.rs`) was real but incomplete, and that several visual inconsistencies (chart colors that contradict the viewport's own diet palette, three different "close button" reds, zero tokens reaching dialogs/toasts) existed specifically because there was nowhere written down to check against. These eight files are that checkpoint. Code implements what's documented here — not the other way around.

## How the other seven files relate

| File | Answers |
|---|---|
| [`typography.md`](typography.md) | What text size/weight for what content? |
| [`colors.md`](colors.md) | What color for what meaning, and where does it come from? |
| [`spacing.md`](spacing.md) | What gap for what relationship between elements? |
| [`layout.md`](layout.md) | What panel goes where, at what ratio, with what docking rules? |
| [`components.md`](components.md) | What reusable widget exists, and what does it look like in every state? |
| [`iconography.md`](iconography.md) | What icon for what action, at what size? |
| [`accessibility.md`](accessibility.md) | Does this hold up for colorblind users, keyboard-only users, and 8-hour sessions? |

## Principles

1. **One token, every consumer.** A color, size, or spacing value used in two places is a token in `theme.rs`, not two literals. `ecology::Diet::standard_color()` is the single source of truth for diet-category color everywhere it appears — the viewport, the status bar, and every chart series.
2. **egui, immediate-mode, no retained widget tree.** Phylon's UI is 100% egui (`egui-wgpu` + `egui-winit`). `bevy_ecs` exists only for simulation state — there is no `bevy_ui` anywhere in this codebase. Every design decision here assumes a function is called fresh every frame, not a persistent scene graph.
3. **Simulation data is never cached into UI state.** `WorkbenchState` (`crates/ui/src/state.rs`) is presentation-only; every panel re-queries `world::World` live each frame. New design work follows this pattern.
4. **A component is documented before it's built.** See [`components.md`](components.md) — Purpose, Variants, States, Tokens, Accessibility, Owner, and Dependencies are filled in before the first line of Rust is written.
5. **Accessibility is load-bearing, not a pass at the end.** The audit's single highest-priority finding was a red(Carnivore)/green(Producer) distinction sitting on top of the most common form of color blindness. See [`accessibility.md`](accessibility.md).

## Status

This design system was authored alongside a 13-milestone implementation roadmap (see the project's UI Architecture Refinement plan). Milestone 0 is this documentation; Milestone 1 implements the tokens documented here into `crates/ui/src/theme.rs`.
