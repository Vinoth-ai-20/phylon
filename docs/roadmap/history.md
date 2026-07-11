# Project History

A condensed, phase-by-phase account of what was actually built, extracted from the project's own development record. This is a summary for orientation — the durable architectural decisions behind each phase are in [Architecture Decisions](decisions.md); still-open items are in [Backlog](backlog.md). Day-by-day implementation logs, milestone checklists, and approval narratives from the original phase documents are intentionally not reproduced here.

Two numbering schemes exist in this project's history and should not be conflated: a **main engine/biology track** (Phase 3 through Phase 9, referenced below) and an independent **UI-feature track** ("UI Phase 1", "UI Phase 2") that ran concurrently and is folded into the timeline below at the point it happened.

## Original build-out (pre-Phase-3)

The foundational simulation was built as a 15-epic roadmap: determinism foundation (seeded RNG, fixed timestep), ECS core, physics, chemical diffusion, genetics/CPPN engine, behavior/CTRNN brains, ecology, GPU compute pipeline, rendering, UI shell, analytics, storage, research/batch tooling, scripting, and a reinforcement-learning bridge. Several originally-envisioned directions were explicitly cancelled rather than deferred: an infinite chunked world (replaced by a fixed, bounded arena), dynamic FFI plugin loading, and direct Python/`pyo3` or `burn`/`candle` ML-framework bridges (replaced by an embedded `rhai` scripting engine and a framework-agnostic WebSocket MARL protocol, respectively).

## UI Phase 1

The first UI design-system pass: typography/spacing/color/icon/radius tokens, a shared widget library (`chrome_bar`, `kv_row`, `status_chip`, empty/error states), the Docked/Floating/Closed panel model, and the Neural Viewer's zoom/pan foundation.

## Phase 3 — Evo-Devo

Retired an earlier literal body-plan-sequence design (`HoxSequence`/`HoxGene`) in favor of the current model: a third evolvable CPPN (`regulatory_cppn`) generating a small recurrent Gene Regulatory Network, decoded per body position into a combinatorial 3-bit Hox code. Segment vocabulary grew from 5 to 8 types (adding Vascular, Ganglion, Germinal). Added germ-line-protected apoptosis, emergent per-segment pigmentation, and extended speciation distance to include the regulatory CPPN term.

## Phase 4 — Physiology & Life-Cycle

Made the developmental Body Graph a persistent ECS component (surviving an organism's whole life, not just a growth-time scratch structure), enabling injury/regeneration and re-differentiation. Added per-segment chemical-economy pools with intra-organism transport, hormone diffusion, and disease spread along the same graph; a two-stage life cycle (Juvenile/Adult) with full brain reconstruction at the transition; and a first real event-bus consumer. Two follow-on proposals were scoped but not yet built at this point: regional brains and reaction-diffusion morphogens (both were implemented later, in Phase 6). A 3D-readiness audit (design-only, no code) found the developmental graph's position-as-index representation was already dimension-agnostic by construction — this is a large part of why the later Phase 8 3D migration was tractable.

## UI Phase 2

Lineage/Species Explorer, Research Dashboard, Replay Browser, a shared cross-panel selection model, camera bookmarks, Command Palette, Minimap, and Focus Mode.

## Phase 5 — Locomotion & UI Audit ("SX")

A direct, measurement-first investigation into why organisms appeared static found the actual cause was structural, not behavioral: the original single-connection seed regulatory CPPN could only produce monotonic Hox-gene bit patterns, making the `Muscle` segment type structurally unreachable for most starter species. Fixed with a modular design — one independently-weighted local-activation bump per regulatory-gene role — which substantially, but not completely, improved actuatable-muscle rates. Also formalized a five-tier signal-priority system for viewport rendering (selection > health > death/reproduction > behavior > cosmetic) and folded several standalone physiology panels into the Inspector as collapsible sections.

## Phase 6 — Research Platform

Removed the never-advanced `SimulationScheduler` from the app's actual tick-driving path (a hand-written per-tick function had been doing the real work all along) and closed three `fastrand::`-instead-of-seeded-RNG determinism leaks. Implemented both of Phase 4's deferred proposals for real: regional-brain wiring metadata (`RegionId`, still dormant in practice — no genome has been observed to decode a `Ganglion` segment) and reaction-diffusion morphogens (a real intra-organism graph-relaxation signal plus a fifth GPU world-space diffusion layer for inter-organism/environmental coupling). Substantially expanded what a save file actually captures — eleven more component types now round-trip through save/load, closing a gap where reloading a simulation had silently stopped organisms from aging, metabolizing, or being diseased. Added real user-preferences persistence and did a significant dead-code removal pass (unused menu actions, a duplicate chrome-bar system, a decorative animation that had violated the project's own "no unearned motion" rule).

## Phase 7 — Workbench

Unified several previously-organic UI conventions into explicit, binding rules: a single selection/follow mutation pathway, a single Recent Items service, a four-tier event-communication model (silent / local visual feedback / session notification / persistent research record), and a single generic node-link graph-canvas primitive shared by the Neural and GRN viewers (which keep their own domain-specific layout and coloring — the canvas itself has zero domain knowledge). Replaced an ad hoc `render.rs` with a decomposed builder-function structure. Confirmed, by direct measurement, that the `gpu` crate is compute-only (no rendering code at all) — a clean boundary that Phase 8 preserved.

## Phase 8 — Native 3D Engine

Migrated the engine from 2D to 3D across camera, rendering, physics, growth orientation, and vision, while deliberately leaving chemical diffusion as 2D world-space planes (a measured tradeoff, not an oversight). Consolidated six duplicated camera-projection implementations into a single `Camera3d`, added orbit and fly controllers, and replaced the 2D SDF-metaball renderer with mesh-based capsule instancing and physically-based shading (an explicit, sign-off-gated visual-identity change). Replaced the dense-grid physics broad-phase with a spatial hash (avoiding a ~128× memory blowup at 3D resolution), gave organisms a body-fixed forward/dorsal orientation frame for bilateral symmetry and 3D vision, and bumped the storage schema to include full 3D state (a disclosed breaking change — pre-migration save files do not load).

## Phase 9 — Behavior Validation & Camera Polish (in progress)

Goal 1 (Blender-quality viewport navigation) has not yet been started. Goal 2 (restore organism locomotion) traced the full genome-to-physics pipeline and found two concrete, measured root causes rather than assuming one: a founder-population mutation dosage 100x more aggressive than ordinary reproduction's own rate, and two starter species whose regulatory-network seed weights caused near-total developmental apoptosis, leaving them with no body beyond the head. Both were fixed and verified with a real headless run showing genuine, sustained muscle-driven locomotion. Goal 3 (behavior validation over a long run) confirmed foraging, predation, reproduction, and physical stability all hold up over 15,000 ticks, and surfaced one more real, previously-undiagnosed bug in the same audit style: a population cap (`EcologyConfig::max_organisms`) that had never been connected to the intended population-size config, silently blocking all reproduction once the founder population exceeded it. Also confirmed, directly and for the first time, that the GPU physics pipeline is not currently bit-exact across separate runs of the same seed — see [Determinism](../explanation/determinism.md) — and that no speciation occurred at all over the 15,000-tick validation run, both tracked as open items in [Backlog](backlog.md).
