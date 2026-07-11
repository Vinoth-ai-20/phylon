# Phase 4, Epic 4 — Reaction-Diffusion Morphogens: Dedicated Audit & Sub-Roadmap (P4-D1 / P4-D2)

> **Archived historical record.** This document describes the project as of when it was written and is retained only for provenance and source-code cross-references — it is not maintained going forward. For current documentation, see [docs/](../../index.md); for durable decisions and knowledge extracted from this document, see [Architecture Decisions](../decisions.md) and [Project History](../history.md).

## 0. Why This Document Exists

Same process as `PHASE4_EPIC1_NEURAL_ROADMAP.md` (regional brains): `PHASE4_ROADMAP.md`'s **ADR-P4-04** left Epic 4 (reaction-diffusion morphogens) unscoped in the main roadmap specifically because it is GPU-touching and High-complexity, requiring its own dedicated audit before implementation — *"do not skip that step under schedule pressure."* This is that document, for **P4-D1** (reaction-diffusion morphogens) and, at low resolution, **P4-D2** (temporal gene expression), which depends on D1. **No code has been written for D1/D2. This is audit and design only, awaiting approval.**

This epic additionally **reopens two previously-closed decisions**, which this document must handle with the same care `PHASE4_ROADMAP.md` itself used for ADR-P4-01 reversing ADR-P3-04:

- **ADR-P3-03** (`PHASE3_ROADMAP.md`): chose purely analytic, closed-form morphogen gradients over any field-based approach, explicitly naming its own reversal trigger: *"a concrete need for environmentally-coupled or inter-organism developmental signaling."*
- **ADR-P4-02** (`PHASE4_ROADMAP.md`): already decided, in the main roadmap, that this trigger has now been reached, and pre-selected a direction — *"extend (not replace) `diffusion_step.wgsl`'s infrastructure to a per-organism-scoped field,"* explicitly rejecting a CPU-only per-organism PDE approximation at the time, citing "performance risk, no benchmark coverage."

**This document's central finding (see §2) is that new evidence — this session's own P4-F3/P4-F4 work — changes the calculus ADR-P4-02 was written under**, and proposes superseding it. This is flagged prominently and explicitly, per this project's ADR Discipline ("never modify an accepted ADR — supersede explicitly, with reasoning"), not silently substituted.

---

## 1. Current Architecture Audit

### 1.1 Morphogens today: pure, closed-form, position-only

`crates/genetics/src/morphogen.rs` (Phase 3, M3):
- `ap_position(segment_index, total_segments) -> f32` — normalized antero-posterior position.
- `distance_from_head_gradient(segment_index, total_segments) -> f32` — `(-3.0 * ap).exp()`.
- `external_inputs_for_position(segment_index, total_segments, gene_count) -> Vec<f32>` — combines both into one scalar, broadcast identically to every gene input.
- `develop_at_position(regulatory_cppn, position, total)` (called from `organisms::developmental_graph`) takes **only** the genome and an integer position — no field, no tick, no world/ECS state of any kind. Confirmed via source: zero coupling to simulation state exists today. A doc comment in `regulatory.rs` already anticipated "a future morphogen-gradient reading" as the natural extension point — the seam D1 needs to use already exists conceptually, just unwired.

### 1.2 Existing GPU diffusion: world-space only, singleton, no per-organism concept

- `DiffusionComputePipeline` (`crates/gpu/src/diffusion_pipeline.rs`): ping-pong `texture_2d_array` (4 layers: Pheromone/Energy/O2/CO2), 256×256 grid, instantiated **exactly twice** total (main field + a separate hazard field) — both singleton world resources, not one per organism.
- Shader (`diffusion_step.wgsl`): `delta = dt * (D * laplacian - λ * center + emission)`, 5-point Neumann-reflecting Laplacian, clamped `[0, 1000]` — a real, working reaction-diffusion solver, just scoped to the whole world, not any one organism.
- `CpuFieldState`/`CpuHazardFieldState` (`crates/diffusion/src/lib.rs`) mirror this GPU state for CPU-side reads (`metabolism_system`'s O2/CO2 sampling already uses this exact path, confirmed in this session's P4-F2 work).
- **Confirmed: no per-organism-scoped field of any kind exists anywhere in the codebase.** Building one from scratch, as ADR-P4-02 originally proposed, means a wholly new GPU resource — not an extension of an existing per-organism structure, since none exists.

### 1.3 What this session's P4-F3/P4-F4 already proved, that ADR-P4-02 didn't have available

ADR-P4-02 was written as part of the main `PHASE4_ROADMAP.md`, **before** P4-F3 (intra-body transport) and P4-F4 (endocrine diffusion) were implemented. Both are now real, tested, shipped code (this session) demonstrating a **cheap, deterministic, CPU-only, graph-based relaxation pattern**, scoped per-organism, along the persistent Body Graph's parent/child edges:

- `organisms::transport_system`: mass-conserving relaxation of `ChemicalEconomy` pools (glucose/o2/atp/co2) along Body Graph edges, `O(segments)` per organism, no GPU involvement, fully deterministic (parent-before-child insertion order), unit-tested including multi-hop propagation within a single tick.
- `organisms::endocrine_diffusion_system`: one-directional (broadcast, not conserved) relaxation of `HormoneLevel` along the same edges — proving the same graph-walk shape generalizes to a second, physically-different signaling model.

**This is directly relevant evidence ADR-P4-02 didn't have**: a body in this simulation is a small graph (`MAX_SEGMENTS = 15`-ish nodes), not a continuous 2D tissue mesh. A morphogen signal propagating "through the body" is architecturally a graph-relaxation problem — exactly what P4-F3/F4 already solved cheaply — not a 2D spatial PDE problem, which is what a GPU texture field is built for. **A per-organism GPU texture would simulate diffusion at a spatial resolution the anatomy itself doesn't have** (there is no tissue between body-graph nodes to diffuse across — only the discrete graph edges).

### 1.4 `simulate_growth_timeline` and the determinism guarantee

`simulate_growth_timeline(regulatory_cppn: &Cppn) -> DevelopmentalGraph` (Phase 3, M13) calls `develop_at_position(regulatory_cppn, position, total)` per position — genome and position only, matching ADR-P3-03/P3-09's "pure function" guarantee exactly. **If `develop_at_position` gains a real, stateful field-reading parameter, this function can no longer reproduce what a live run actually decoded** — the same class of break ADR-P4-01 already named and handled (by keeping `simulate_growth_timeline` as the historical/analytic-only tool, and the persistent `DevelopmentalGraph` component as the live source of truth). This document adopts the identical resolution — see §4.

---

## 2. Design Tension & Decision

### ADR-D1-01: Intra-organism morphogen signaling is graph-based, not a per-organism GPU field — supersedes ADR-P4-02's GPU-field direction for the intra-organism case

**Status:** Proposed (ratified on approval of this document).

**Supersedes:** ADR-P4-02, specifically its "extend `diffusion_step.wgsl`'s infrastructure to a per-organism-scoped field" direction — **not** its underlying finding that ADR-P3-03's reversal trigger has been reached (that finding stands; only the *mechanism* changes).

**Decision:** Split "reaction-diffusion morphogens" into the two cases ADR-P3-03's own trigger language actually named separately — *"environmentally-coupled **or** inter-organism"* — and solve them with different, already-precedented mechanisms:

1. **Intra-organism** developmental signaling (a growing organism's own body influencing its own later segments' decode) — a graph-relaxation system over the persistent Body Graph, reusing the exact `transport_system`/`endocrine_diffusion_system` architecture (§3, D1a). No GPU involvement.
2. **Inter-organism / environmental** developmental coupling (the trigger ADR-P3-03 actually named) — a genuinely new channel on the **existing** world-space diffusion field (adding a 5th texture-array layer), which organisms can read from and emit into during development. This is real GPU work, but a much smaller increment than a new per-organism resource, since it reuses infrastructure that already exists (§3, D1b).

**Reasoning:** §1.3's evidence — P4-F3/F4 already proved the graph-walk pattern works, is cheap, and is architecturally matched to this project's graph-shaped (not mesh-shaped) anatomy. Building a wholly new per-organism GPU texture field for a within-body signal that a ~15-node graph walk already handles well would be over-engineering relative to what the anatomy can even represent, and reintroduces exactly the GPU risk ADR-P4-04 asked this document to scrutinize rather than inherit uncritically.

**Consequence:** DEF-006 ("true diffusible morphogen-gradient fields, distinct from CPPN-driven Hox") is satisfied by D1a for the intra-organism case without new GPU state. The one genuinely new GPU surface (D1b's 5th layer) is narrowly scoped and independently reviewable.

---

## 3. Milestone Breakdown

| Milestone | Goal | Depends on | Risk | Effort (days) |
|---|---|---|---|---|
| **D1a** | Intra-organism morphogen field: a new small per-organism concentration map (one `f32` per Body Graph position, same shape as `ChemicalEconomy`'s per-segment pools), relaxed each tick by a new graph-walk system mirroring `transport_system` exactly. `genetics::develop_at_position` gains a new parameter (a concrete `&[f32]` or `Option<f32>` reading, **not** a field/world reference — `genetics` stays pure and parameter-driven; `organisms` owns the stateful field and passes concrete numbers in, preserving the crate-dependency direction). All existing call sites (`growth_system`, `spawning`, `simulate_growth_timeline`) updated to pass a value (real field reading, or `None`/baseline for the pure-replay path). | P4-F1 (persistent graph), P4-F3 (proves the pattern) | Medium (signature change touches several call sites; no GPU) | 3 |
| **D1b** | Inter-organism/environmental coupling: add a 5th layer to the existing world-space diffusion texture array (a "Morphogen" channel), emitted into by developing organisms (e.g., proportional to local growth activity) and sampled by nearby developing organisms and future in-progress growth decisions — the actual mechanism ADR-P3-03's reversal trigger named. Requires updating `diffusion_step.wgsl`'s layer count and every hardcoded "4 layers" assumption (`DiffusionComputePipeline::new`, `CpuFieldState`'s `FieldLayer` enum, any bind-group layout referencing layer count). | D1a (establishes the CPU-side read/write pattern first) | High (the genuinely GPU-touching part — shader layer count, bind groups, existing hazard-field precedent to follow) | 4 |
| **D1c** | `simulate_growth_timeline` reconciliation: update its doc comment to explicitly scope it as "genome + zero-field-input" analytic replay (a lower-bound/reference reconstruction, not a live-run-identical one, once D1a/D1b introduce real field state) — same resolution pattern ADR-P4-01 already used for the persistent-graph/transient-graph split. Add a comparison test that **quantifies** (not just asserts equal/unequal) how far a live run's decode diverges from the pure replay for a fixture genome with nonzero field input, so future regressions are caught by magnitude, not just a binary pass/fail. | D1a, D1b | Low | 1 |

**Total D1 effort estimate:** ~8 days — larger than any single F-tier milestone, reflecting genuinely new representation work (as ADR-P4-04 anticipated), but meaningfully smaller and lower-risk than a naive "build a full per-organism GPU field" reading of the original milestone table row would have implied, because of ADR-D1-01's split.

### 3.1 Testing requirements (per milestone)

- **D1a:** same-seed-same-output determinism test (matching every F-tier milestone this session); a test proving `develop_at_position`'s new parameter actually changes decode output for a nonzero field reading vs. `None`/baseline; a regression test confirming `simulate_growth_timeline` (called with baseline/no field) still matches its own pre-D1 fixture output exactly, so this milestone doesn't silently change Phase 3's existing guarantee before D1c formally re-scopes it.
- **D1b:** a test on the CPU-side (`CpuFieldState`) confirming the new 5th layer round-trips independently of the other 4 (no cross-channel bleed); GPU validation per this project's own standing rule for GPU-touching work ("not just `cargo test`" — matching `IMPLEMENTATION_STATUS.md`'s Phase 8 Verification Matrix precedent already cited by the main roadmap for GPU work).
- **D1c:** the quantified-divergence comparison test described above.

### 3.2 Non-goals for D1 (explicitly out of scope)

- Any change to the existing Pheromone/Energy/O2/CO2 layers' own semantics (D1b only *adds* a layer).
- Temporal/tick-windowed gene expression (that's D2, see below).
- Any visualization of the new field (R-tier/instrumentation work, gated behind D1 landing).
- Reconsidering P4-F5's DEF-009 disposition ("activate opportunistically once D1 lands") — that remains a separate, later decision for whoever picks up F5's follow-on, not pre-empted here.

---

## 4. P4-D2 — Temporal Gene Expression (scope deferred)

Same principle as the N1 sub-roadmap's treatment of N2: D2 should be re-audited once D1a/D1b's actual landed shape is known, not speculatively designed now. At low resolution: "activation windows, checkpoints" plausibly extends `DevelopmentalOutputs`/`develop_at_position`'s now-field-aware signature (D1a) with a tick-since-growth-start parameter, gating which regulatory genes are even eligible to fire — but this should be confirmed against D1's real implementation, not assumed here. **No effort/risk estimate is given for D2 in this document**, matching this project's own stated discipline.

---

## 5. Risk Assessment

| Risk | Mitigation |
|---|---|
| Building an unneeded per-organism GPU field (over-engineering relative to graph-shaped anatomy) | Avoided by ADR-D1-01's split — intra-organism case handled by a proven cheap graph-walk instead |
| D1b's shader layer-count change breaks the 3 existing layers | Explicit non-goal boundary (§3.2) + dedicated round-trip test isolating the new layer |
| `develop_at_position`'s signature change ripples across call sites inconsistently | D1a's own testing requirement includes a regression test on `simulate_growth_timeline`'s pre-D1 output specifically |
| D2 designed speculatively against a not-yet-real D1 shape | §4 explicitly defers D2's detailed scoping to a re-audit once D1 lands |

## 6. Verification Plan

D1a: standard `cargo build/clippy -D warnings/fmt --check/test/doc -D warnings --workspace`, no GPU validation needed (no GPU code touched). D1b: the same, **plus** GPU validation per this project's standing rule for genuinely GPU-touching work (matching `IMPLEMENTATION_STATUS.md`'s Phase 8 Verification Matrix, already cited by `PHASE4_ROADMAP.md` §7 for this exact class of change). D1c: standard verification only.

## 7. Executive Summary

The main finding of this audit: ADR-P4-02's original "per-organism GPU field" direction, decided before P4-F3/F4 existed as working precedent, is no longer the best-supported design once that precedent is accounted for. This document proposes **ADR-D1-01**, splitting the milestone into a cheap graph-based intra-organism mechanism (reusing P4-F3/F4's proven shape, no new GPU state) and a narrowly-scoped inter-organism/environmental mechanism (one new layer on the existing world-space field) — satisfying DEF-006 and ADR-P3-03's named reversal trigger at meaningfully lower risk than a naive reading of the original milestone row.

**This document is a proposal, not an authorization to implement.** D1a/D1b/D1c — and in particular, ADR-D1-01's supersession of ADR-P4-02 — await explicit approval before any code is written.
