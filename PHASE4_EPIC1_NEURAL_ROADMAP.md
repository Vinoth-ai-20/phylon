# Phase 4, Epic 1 — Regional Brain Representation: Dedicated Audit & Sub-Roadmap (P4-N1 / P4-N2)

## 0. Why This Document Exists

`PHASE4_ROADMAP.md`'s **ADR-P4-04** deliberately left Epic 1 (regional brains, ganglia, CNS) unscoped in the main roadmap: *"Epic 1 is explicitly not pre-specified to milestone-level detail in this document. It is scoped as its own dedicated audit-and-roadmap, produced when this epic is reached, following the same process this document itself follows for Phase 4 as a whole."* This is that document, for **P4-N1** (regional brain representation) and, at lower resolution, **P4-N2** (axon guidance/pruning), which explicitly depends on N1.

Per the roadmap's own risk table: *"Regional brains (Epic 1) and reaction-diffusion morphogens (Epic 4) are both GPU-touching, High-complexity, 'own sub-roadmap' items — risk of underestimating them if rushed... do not skip that step under schedule pressure."* This document is that step. **No code has been written for N1/N2. This is audit and design only, awaiting approval before implementation, exactly as `PHASE4_ROADMAP.md` itself required before Phase 4's own milestones began.**

---

## 1. Current Architecture Audit

### 1.1 `brain::Brain` is a flat, unregioned array

- `CtrnnNode` (`crates/brain/src/lib.rs:79-92`, `#[repr(C)]`, GPU-`Pod`): `state, time_constant, bias, activation, first_synapse, synapse_count` — no region/position/segment field.
- `CtrnnSynapse` (`lib.rs:110-119`): `source, target, weight, _padding` — plain index pair into the same flat node array.
- `Brain` (`lib.rs:137-171`): `id, nodes: Vec<CtrnnNode>, synapses: Vec<CtrnnSynapse>, input_count, output_count, winner_take_all, plasticity_enabled, external_override`. No region/ganglion/cluster field anywhere in the struct.
- `Brain::reindex_synapses` (`lib.rs:315-349`) sorts `synapses` by `target` and computes contiguous `[first_synapse, first_synapse+synapse_count)` ranges — purely index-based bookkeeping, no hierarchy.
- A full-file search for "region"/"ganglion"/"cluster" returns zero hits. There is no regional concept today, confirmed from source, not assumed.

### 1.2 Brain wiring today is index-pair-driven, not position-driven

In `growth_system`'s brain-wiring branch (`crates/organisms/src/systems.rs`, `is_finished` block):

- `input_count = 9` (hardcoded: 6 sensors + Signal + Hazard + Pacemaker), `hidden_count = 4` (hardcoded), `output_count = effectors.len() + 1`.
- Per-node CPPN evaluation feeds `[i / total_nodes, i / total_nodes]` (the node's own normalized **index**, twice) into `expressed_brain_cppn` to get `bias`/`time_constant`. No body position, no segment identity, no anatomy.
- Per-synapse CPPN evaluation feeds `[i / total_nodes, j / total_nodes]` for every `(i, j)` with `j` in the hidden/output range, thresholding `weight.abs() > 0.01` to decide whether the synapse exists at all. This is **all-pairs-via-CPPN over node indices** — the entire "regional" gap the roadmap's own audit names.
- The one piece of anatomy that reaches the brain at all is `state.effectors: Vec<Entity>` (springs/muscles in growth order) — Braitenberg left/right fin detection walks this list ordinally, with no left/right or region semantics beyond "first fin found, second fin found."

### 1.3 `SegmentType::Ganglion` is a physics-only placeholder today

`Ganglion` is a real `SegmentType` variant (`crates/genetics/src/types.rs`), with its own doc comment already stating *"neural-centralization behavior remains an enum-only placeholder."* `compile_segment` (`crates/organisms/src/developmental_graph.rs`) gives it the same stiffness/constraint profile as `Torso` — it is physically indistinguishable from ordinary tissue except its numeric type tag. It has **zero neural special-casing** anywhere in `growth_system` or the `brain` crate today. `genetics::DevelopmentalOutputs` (segment_type/branches/actuation_amplitude/actuation_phase/pigment/apoptosis) has no brain-related field — a Ganglion segment carries no data that could inform brain wiring even if the wiring code wanted to look for it.

### 1.4 What P4-F1's persistent Body Graph already gives Epic 1 — and what it doesn't

Phase 4's P4-F1 (this same session, done) made `DevelopmentalGraph` a persistent component with `DevelopmentalNode { role: SegmentType, parent, is_branch, position, entity: Option<Entity> }`, plus `root()`/`children_of()`/`node_at_position()` queries. This is real, useful plumbing: **N1 already has a live, queryable map from body position/role to a real ECS entity** — the anchor data structure regional wiring needs to answer "which body position is this neural cluster near."

What P4-F1 does **not** provide, confirmed by this audit: any bridge from a `DevelopmentalGraph` index/entity to a `Brain` node index or range. `Brain` and `DevelopmentalGraph` today have no relationship to each other at all — the brain is wired once, flat, at growth completion, with no reference to the graph that was just built. **This is the genuinely new plumbing N1 needs to add** — P4-F1 is a necessary precondition (hence its listing as N1's dependency in `PHASE4_ROADMAP.md`'s own dependency graph), not a sufficient one.

### 1.5 The GPU path — where the real risk lives

`crates/gpu/src/brain_pipeline.rs` defines `GpuCtrnnNode`/`GpuCtrnnSynapse` as byte-identical layouts to the CPU types. `crates/app/src/simulation.rs`'s GPU CTRNN evaluation section runs a single flat query over **every organism's** `Brain` component, concatenating all nodes/synapses into one global buffer per tick, with per-organism `start_node`/`start_syn` offsets folded into each synapse's `source`/`target`, and a `brain_offsets: Vec<(Entity, u32, usize)>` list used to split the readback back out per-organism. There is one level of indirection (organism), not two (organism, then region) — introducing a second level would mean reworking `brain.wgsl`'s single-level gather logic and this flattening loop. **This is the concrete shape of the "GPU-touching, high risk" concern** ADR-P4-04 flagged, and it is avoidable — see §2's design decision.

---

## 2. Design Tension & Decision

The central question this document exists to answer: **does "regional brain" require a new GPU data shape, or can it be built as a CPU-side wiring-time concept on top of the existing flat GPU buffer?**

### Option A — Region as a GPU-visible structure (rejected)

Add a region/cluster field to `GpuCtrnnNode`, give the GPU shader region-aware dispatch or a second level of buffer indirection. **Rejected**: this is exactly the expensive, risky path ADR-P4-04 warned against. It would require `brain.wgsl` changes, new bind-group layouts, and revalidation of the whole GPU brain pipeline — a large, high-risk undertaking with no clear payoff, since nothing about *evaluating* a CTRNN node's dynamics ($\dot{y} = \frac{1}{\tau}(-y + \sum w\sigma(y+\theta) + I)$) actually needs to know which region a node belongs to. Region only matters for **deciding which synapses exist in the first place** — a wiring-time, not runtime, question.

### Option B — Region as a CPU-side wiring-time concept (chosen)

Keep `CtrnnNode`/`CtrnnSynapse`/the GPU buffer layout **completely unchanged**. Add region information as new, GPU-irrelevant metadata on the CPU-side `Brain` struct — a `Vec<RegionId>` parallel to `nodes` (one entry per node) — that is consulted **only** by `growth_system`'s brain-wiring code, at the moment it decides which `(i, j)` pairs to query the CPPN for and how to weight the result. Once wiring is finalized for a tick, the resulting flat `nodes`/`synapses` arrays are exactly the same shape they are today, and flow through the existing GPU dispatch path completely unmodified.

**Decision: Option B.** This directly resolves ADR-P4-04's stated risk — region becomes a Medium-complexity, CPU-only concept for its first milestone, with the higher-risk GPU question deferred indefinitely (and only revisited if a future milestone finds a concrete reason region needs to affect runtime *evaluation*, not just wiring — no such reason is apparent today).

### ADR-N1-01: Region is CPU-side wiring metadata, not a GPU buffer change

**Status:** Proposed (part of this sub-roadmap; ratified on approval of this document, same as `PHASE4_ROADMAP.md`'s own ADRs).

**Decision:** `Brain` gains a `node_regions: Vec<RegionId>` (or equivalent) field, consulted only by CPU-side wiring logic in `growth_system`. `CtrnnNode`, `CtrnnSynapse`, `GpuCtrnnNode`, `GpuCtrnnSynapse`, and `brain.wgsl` are **not modified** by N1.

**Reasoning:** See §2 above — regional wiring is a decision about which synapses exist, not about how an existing synapse's weight is integrated at runtime. Containing "region" to wiring-time avoids the GPU risk ADR-P4-04 named, without giving up anything the roadmap's stated goal ("organ-anchored node ownership, region-bound wiring") actually needs.

**Consequence:** If a future milestone needs region-aware *runtime* behavior (e.g., a Ganglion's nodes literally running at a different rate, gated by local physiology from Epic 2), that would need its own ADR and its own GPU-risk assessment at that time — not pre-authorized by this one.

---

## 3. Milestone Breakdown

Three milestones for N1, kept small and independently revertable per this project's established discipline; N2 is deliberately left at low resolution (§4), since it depends on N1 landing first and deserves its own re-audit at that time, not speculative pre-design now.

| Milestone | Goal | Depends on | Risk | Effort (days) |
|---|---|---|---|---|
| **N1a** | Region plumbing: add `RegionId`/`node_regions` to `Brain`, define `RegionId` (e.g. `Central` default, `Ganglion(graph_position: usize)` for a Ganglion-anchored cluster). No wiring behavior change — every node defaults to `Central`, so existing organisms' brains are bit-for-bit unaffected. Pure additive infrastructure, same "infra before behavior" pattern as ADR-P4-01/P4-05. | P4-F1 (persistent graph) | Low | 2 |
| **N1b** | Region-bound wiring: `growth_system`'s synapse-wiring loop consults `node_regions` — same-region `(i, j)` pairs are queried and wired as today; cross-region pairs are wired more sparsely (e.g. only through designated "hub" nodes near a Ganglion), replacing pure all-pairs-via-index-CPPN with a rule that actually reflects anatomy. This is the real behavior change. | N1a | Medium-High | 3 |
| **N1c** | Ganglion becomes a real neural anchor: at brain-wiring time, `DevelopmentalGraph::children_of`/`node_at_position` (already built by P4-F1) is used to find which decoded body positions are `SegmentType::Ganglion`, and hidden nodes are preferentially assigned `RegionId::Ganglion(position)` for the nearest one (by body-graph distance, not Euclidean) rather than `Central` — giving the existing-but-inert `Ganglion` segment type its first real behavioral consequence. | N1a, N1b | Medium | 2 |

**Total N1 effort estimate:** ~7 days across 3 milestones — comparable to P4-F1's own effort (3 days) plus P4-F2/F3 combined, reflecting genuinely new representation work, not a parameterization.

### 3.1 Testing requirements (per milestone)

- **N1a:** unit tests confirming `Brain::new`'s (or a new constructor's) default `RegionId::Central` assignment; confirming existing brain-wiring tests in `organisms::systems::tests` are byte-for-byte unaffected (a regression guard, not a new behavior test).
- **N1b:** same-seed-same-output determinism test (matching every other Phase 4 F-tier milestone's discipline); a test proving cross-region synapse density is measurably lower than intra-region density for a fixture genome with an explicit Ganglion segment; a test proving an organism with **no** Ganglion segments produces the same topology as before N1b (graceful degradation to the old all-Central behavior).
- **N1c:** a test proving a hidden node's assigned region matches the nearest Ganglion by body-graph distance (not accidentally by raw node index); a test with two Ganglion segments confirming nodes split between them correctly by graph distance.

### 3.2 Non-goals for N1 (explicitly out of scope)

- Any GPU/`brain.wgsl` change (per ADR-N1-01).
- Any change to `input_count`/`hidden_count`'s current hardcoded values (4 hidden nodes total may turn out to be too few once regions exist — that is a follow-on tuning question, not part of N1's own scope).
- Axon guidance, neuron migration, developmental pruning — that is N2, see below.
- Any visualization of regions (that is P4-R-tier/N-adjacent instrumentation work, gated behind N1 landing, not part of it).

---

## 4. P4-N2 — Axon Guidance / Neuron Migration / Pruning (scope deferred)

Per `PHASE4_ROADMAP.md`'s own dependency graph, N2 depends on N1. Per this document's own principle (ADR-P4-04's "don't skip the audit"), **N2 should get its own re-audit once N1a-c actually exist in code** — designing N2's exact mechanics against a regional-brain representation that doesn't exist yet would be speculative, not evidence-based. What can be said now, at low resolution:

- "Axon guidance"/"neuron migration" most naturally map onto **which region a new hidden node is assigned to as growth proceeds** (a re-entrant version of N1c's assignment rule, relevant once P4-L1 makes growth re-entrant for life stages) — i.e., N2 may turn out to be a smaller extension of N1c than a wholly new mechanism, but this should be confirmed by re-auditing N1's actual landed shape, not assumed here.
- "Developmental pruning" has a plausible existing hook: `brain::PlasticityConfig`'s existing `prune_threshold`/`prune_interval_ticks` (used by `hebbian_plasticity_system`) already prunes weak synapses at runtime — N2 may extend this existing mechanism to be region-aware, rather than inventing a separate pruning pipeline.
- **N2 is not scheduled with an effort/risk estimate here.** Producing one would violate the same discipline this document itself exists to uphold.

---

## 5. Risk Assessment

| Risk | Mitigation |
|---|---|
| GPU buffer/shader rework | Avoided entirely by ADR-N1-01 — region is CPU-only wiring metadata |
| N1b changes brain topology, could regress existing locomotion/fitness | Same-seed determinism tests + explicit no-Ganglion-organism regression test (graceful degradation) |
| `hidden_count = 4` may be too coarse once regions exist (not enough nodes to meaningfully split across regions) | Flagged as an explicit non-goal/follow-on in §3.2 — do not silently expand N1's scope to fix this pre-emptively |
| N2 designed speculatively against a not-yet-real N1 shape | §4 explicitly defers N2's own detailed scoping to a re-audit once N1 lands |

## 6. Verification Plan

Same discipline as every other Phase 4 milestone in `PHASE4_ROADMAP.md`: `cargo build/clippy -D warnings/fmt --check/test --workspace`, plus `cargo doc -D warnings`, per milestone (N1a, N1b, N1c each independently). No GPU-specific validation is needed given ADR-N1-01 (no GPU code is touched) — a meaningful simplification versus the "GPU validation, not just `cargo test`" requirement `PHASE4_ROADMAP.md`'s §7 flagged for genuine GPU-touching work.

## 7. Executive Summary

Regional brains do not require the GPU-risk this epic was flagged for, provided region is scoped as CPU-side wiring metadata (ADR-N1-01) rather than a runtime GPU concept — this is this document's main finding. Three small, ordered milestones (N1a infra → N1b wiring behavior → N1c Ganglion anchoring) implement this at a total effort comparable to a single mid-sized F-tier milestone pair. N2 is intentionally left unscoped pending N1's actual landing, per the same "don't skip the audit" principle this whole document exists to demonstrate.

**This document is a proposal, not an authorization to implement.** Per this session's established process, N1a/N1b/N1c await explicit approval before any code is written, exactly as `PHASE4_ROADMAP.md` itself did before Phase 4 began.
