# Phylon Next-Generation Engineering Roadmap

This is **not** a roadmap copied from Mote. It is Phylon's own prioritized engineering
strategy, informed by a structured comparison against Mote's publicly-demonstrated
architecture (see [MOTE_ENGINEERING_LESSONS.md](../research/MOTE_ENGINEERING_LESSONS.md)
and [MOTE_VS_PHYLON_DECISION_MATRIX.md](../research/MOTE_VS_PHYLON_DECISION_MATRIX.md)),
but every proposal below is justified on Phylon's own merits, evidence, and stated
research goals — not on "Mote does this."

Priority is ranked by **measured or reasonably-evidenced impact**, not by how closely
an idea resembles something Mote demonstrated. Several genuinely Mote-inspired ideas
rank low or are explicitly deferred because Phylon's own evidence doesn't yet justify
them; several Phylon-internal cleanups surfaced *by* the comparison rank ahead of some
Mote-inspired ideas because they're lower-risk and higher-certainty.

This document does not authorize implementation. Per the standing instruction that
produced it: **implementation begins only after this roadmap is reviewed and each
milestone is explicitly prioritized/approved.**

---

## Phase 10 — Million-Organism Initiative (objective, not yet started)

Everything below this point in the document is optimization framed around removing
specific, measured bottlenecks. That framing is correct but incomplete on its own — a
list of optimizations with no named target can drift indefinitely without ever
declaring victory or defeat. This section exists to give the "Path to Massive-Scale
Simulation" section below a concrete objective to be judged against, rather than an
open-ended direction.

**This is a target for a future phase, not a commitment being made now.** Nothing in
this roadmap's priority list above depends on it, and none of it is scoped or costed
yet — it is a success-criteria definition, so that if and when Phylon's own research
roadmap decides to pursue large-scale simulation, there's already an agreed, specific
definition of "done" rather than a vague "make it faster."

### Success metrics

- **1,000,000 concurrent organisms**, not 1,000,000 physics particles — i.e. full
  organisms with grown bodies, wired brains, and metabolic state, matching what "an
  organism" means everywhere else in this codebase.
- **60 FPS sustained**, not a one-time burst — measured the same way P9.1's own FPS
  probe methodology worked (real windowed run, steady-state population, not a
  cherry-picked frame).
- **Deterministic** — the existing `SimRng`/fixed-tick-order guarantee holds at this
  scale, not relaxed to get there. If a scale-driven change would widen the
  already-disclosed GPU-determinism gap (see `docs/explanation/determinism.md`), that
  tradeoff must be named and justified explicitly, per Architecture Principle 4 in
  `docs/architecture/ARCHITECTURE_PRINCIPLES.md` — not accepted silently as the cost of
  scale.
- **GPU utilization >90%** during the simulation-heavy portion of a tick — a proxy for
  "the bottleneck has actually moved off the CPU," which is the inverse of Phylon's
  current, measured state (CPU-side ECS gather is the confirmed limiter today, not GPU
  throughput).
- **VRAM bounded** — a stated, documented ceiling that doesn't grow unboundedly with
  population (this is already true today for the fixed-size spatial-hash buffers;
  needs to stay true for whatever new state a million-organism design would add).
- **Headless cluster support** — the existing headless/batch path (`app::batch`,
  `init_gpu_headless`) extended to actually run at this scale without a windowed
  surface, on hardware sized for it.
- **Reproducible research** — a run at this scale produces the same
  `ExperimentReport`/snapshot artifacts, with the same save/replay guarantees, as a
  1,000-organism run today. Scale should not become a reason to weaken what a
  researcher can trust about a completed experiment.

### How this reframes existing work

Every future proposal aimed at scale should be stated against one of these metrics
directly, not as a vague direction:

- Not *"optimize rendering"* — instead: *"reduce CPU-side render-instance gather cost
  by 40% at 100,000 organisms"* (a specific, falsifiable claim, checkable against the
  GPU-utilization and FPS metrics above).
- Not *"improve memory"* — instead: *"reduce per-organism GPU-resident state from N
  bytes to M bytes"* (checkable against the VRAM-bounded metric).
- Not *"scale the ECS"* — instead: a specific number, at a specific population, with a
  before/after measurement, per Architecture Principle 5 ("measure before
  optimizing").

The items in "Worth investigating" under Path to Massive-Scale Simulation below are
exactly the open questions that stand between Phylon's current, measured state and
this objective. None of them are scoped as committed work yet — they're the specific,
falsifiable questions that would need answering, in order, before any part of this
initiative could be responsibly costed.

---

## Priority 1 — `pyo3` In-Process Python/NumPy Research Binding

**Problem**: Phylon's existing research/RL workflow (the `network` WebSocket bridge) is
out-of-process and message-passing — every RL step pays a JSON-serialization + TCP
round-trip. This is a real, structural latency cost for ML-training throughput that a
researcher iterating on policies would feel directly.

**Evidence**: Two independent investigations (covering different domains — engine/GPU
and UI/research-workflow) converged on this exact recommendation independently. Mote's
demoed research workflow (CONFIRMED, timestamped, live Jupyter demo) is an in-process
embedding — `import engine; obs = engine.step()` — with zero IPC overhead. Phylon's
`learning` crate doc comment already names this exact extension point as anticipated
("multiple backends (`burn`, external Python via `pyo3`, etc.) can implement the policy
trait without coupling the rest of the simulation") — this is not a new idea, it's
already-planned work this investigation independently re-derived and now recommends
prioritizing.

**Current Phylon state**: A real, working, tested headless GPU path
(`PhylonApp::init_gpu_headless`), a real, working, tested out-of-process WebSocket RL
bridge (`network`/`learning` crates), framework-agnostic `ObservationVector`/
`ActionVector` types already defined and already used by that bridge, and a proven
headless-run pattern (`app::batch::run_batch`) this binding would reuse rather than
duplicate.

**Recommendation**: Build a `pyo3`-based Python extension module wrapping the existing
headless path, exposing `ObservationVector`/`ActionVector` directly as NumPy-compatible
arrays, with the Python-visible API modeled on `batch.rs`'s existing step/observe/act
pattern. Keep the existing WebSocket bridge — it remains the right tool for a trainer
running on a separate machine/process; this is additive, not a replacement.

**Dependencies**: None blocking — all underlying pieces (headless init, observation/
action types, the policy-trait abstraction) already exist and are stable.

**Risk**: Low-to-medium. Must be built strictly on top of `PhylonApp::update_simulation()`
— the same call `batch.rs` already uses — rather than introducing a second simulation-
stepping path, to avoid any determinism divergence between the Python-driven path and
the existing headless/batch path. Build tooling (`maturin`/`setuptools-rust`) is a new
CI/packaging surface, not a simulation-correctness risk.

**Expected FPS gain**: Not applicable — this is a research-workflow throughput change,
not a simulation frame-rate change.

**Expected memory improvement**: Not applicable.

**Expected research value**: High. This is the single highest-value recommendation in
this entire investigation. It closes a real, structurally-explainable latency gap
against a demonstrated competitor capability, using infrastructure Phylon already has
and already planned for.

**Expected implementation complexity**: Medium. FFI/binding surface and NumPy interop
work, not new simulation logic — the simulation-stepping and observation/action
plumbing this binding calls into already exists and is already tested.

---

## Priority 2 — Embodiment / Manual-Control Diagnostic Mode

**Problem**: A researcher cannot currently directly experience what an evolved organism's
effector layout is actually capable of — only infer it from telemetry (Inspector panel,
Metrics dashboard). This is a real gap in qualitative validation tooling, distinct from
(and complementary to) Phylon's existing quantitative/telemetry-based tools.

**Evidence**: Mote's "all entities are players" principle (CONFIRMED, explicit design
framing) demonstrates a genuinely useful diagnostic pattern: letting a human directly
drive an entity's actuators. Phylon already has the exact data-layer hook this would
need — `Brain::set_external_action_override`, built for RL policy injection
(`ExternalAgent`/`PolicyProvider` in the `learning` crate) — just not exposed as an
interactive, keyboard/mouse-driven feature.

**Current Phylon state**: Selection/inspect/track/follow interaction model only; no
embodiment mechanic. The override hook this would reuse is real, tested, and already
used by the RL bridge for a different purpose (external policy actions instead of
human input).

**Recommendation**: Add a scoped "possess" mode: selecting an organism and entering a
manual-control state routes keyboard/mouse input through the same action-override point
the RL bridge already uses, temporarily superseding that organism's `Brain` output.
Exit returns full control to the evolved brain. This is explicitly a diagnostic/outreach
tool, not a step toward "gamifying" Phylon — do not expand it into full action-verb
symmetry between player and AI (a much larger undertaking not justified by Phylon's
research-tool identity).

**Dependencies**: None blocking — reuses the existing, stable override mechanism.

**Risk**: Low, if scoped strictly as described. Risk rises sharply only if expanded
into a broader "player as first-class entity" architectural mandate — explicitly not
recommended.

**Expected FPS gain**: None — this is a feature addition, not a performance change.

**Expected memory improvement**: None.

**Expected research value**: Medium. A genuine qualitative-validation complement to
existing telemetry tools ("does this genome's effector layout actually let it turn/
accelerate the way its morphology suggests"), not a new experimental capability that
changes what can be studied, only what can be manually explored.

**Expected implementation complexity**: Medium — mostly UI/input-routing work reusing
an already-built override point, not new brain/actuator logic.

---

## Priority 3 — Diegetic Measure Tool (persistent in-world ruler)

**Problem**: Phylon's Measure tool is a one-shot screen-space marquee drag, not a
persistent object a researcher can leave in the scene for repeated reference.

**Evidence**: Mote's flagship diegetic-UI example — an extendable "tape measure"
membrane that is a literal, collidable physical object in the simulation (CONFIRMED,
28:01) — maps almost exactly onto what Phylon's existing Measure tool conceptually
does, minus persistence.

**Current Phylon state**: `MarqueeMode::Measure` exists as a screen-space drag gesture
producing a one-time distance readout. Phylon's ECS/physics stack already has
everything needed (a `ParticleNode`-like position component, the existing physics
world) to make a measuring object a real, persistent spawned entity.

**Recommendation**: Convert the Measure tool into an optional persistent mode: dragging
in the viewport spawns a lightweight, position-anchored entity (a literal ruler) that
remains in the scene until explicitly removed, rather than only producing a transient
readout. Do not pursue a broader diegetic-UI paradigm shift beyond this single, narrow
fit — Phylon's conventional egui panel/docking investment is coherent and working, and
nothing in the evidence suggests a research tool needs a wholesale UI-philosophy change.

**Dependencies**: None blocking.

**Risk**: Low — a new, simple entity type layered on existing marquee/physics
machinery, with no change to the wider UI architecture.

**Expected FPS gain**: None.

**Expected memory improvement**: Negligible (one lightweight entity per active ruler).

**Expected research value**: Low-medium — a genuine quality-of-life improvement for
spatial measurement tasks (e.g. "how far apart do these colonies typically settle"),
not a research-capability unlock.

**Expected implementation complexity**: Medium — a new lightweight ECS entity type plus
viewport-drag-to-place logic on the existing marquee/physics stack.

---

## Priority 4 — GPU Buffer-Growth Code Consolidation

**Problem**: Phylon's three GPU compute pipelines (physics, brain, diffusion) each
independently reimplement near-identical geometric (2×) buffer-growth logic
(`ensure_capacity`/`ensure_emitter_capacity`). This is a maintainability risk, not a
performance one — a future change to the growth policy in one pipeline could silently
drift from the others.

**Evidence**: Not a Mote-derived idea — surfaced by directly comparing all three
pipeline files against each other while investigating Phylon's memory-management
approach relative to Mote's. Phylon's growth-policy discipline is already sound
(confirmed, all three pipelines use the same doubling-growth-never-shrinks strategy
correctly); the only issue is that the logic is duplicated three times rather than
shared once.

**Current Phylon state**: Three separate, currently-consistent implementations of the
same pattern across `physics_pipeline.rs`, `brain_pipeline.rs`, `diffusion_pipeline.rs`.

**Recommendation**: Extract the shared doubling-growth-buffer pattern into one common
helper or trait, used by all three pipelines. Purely a refactor — no behavior change.

**Dependencies**: None.

**Risk**: Very low — mechanical deduplication of already-tested, already-consistent
logic.

**Expected FPS gain**: None.

**Expected memory improvement**: None (behavior is unchanged; this is a code-structure
change).

**Expected research value**: None directly — a maintainability investment.

**Expected implementation complexity**: Low.

---

## Priority 5 — Prototype: Supplementary Cheap Movement Mode for Background Organisms

**Problem**: If Phylon's own research goals ever require populations substantially
larger than what's currently tested (~1,000+ organisms), per-organism CTRNN evaluation
cost could become a real limiting factor on sustainable population size. This is
currently **unmeasured** — not a confirmed bottleneck, a plausible future one.

**Evidence**: Two independent investigations, covering different domains (physics/
locomotion and behavior/ecology), independently proposed the same idea: an opt-in,
much cheaper gradient-follow movement mode — reusing diffusion fields Phylon's CTRNN
already samples as an olfaction input — for background/non-focal organisms at very
large scale, as a supplement to (never a replacement for) the full evolved-brain
pipeline. This is explicitly speculative, inspired by Mote's apparent scale-vs-fidelity
tradeoff (chemotaxis-only steering, CONFIRMED as the *described* mechanism; whether it
is truly Mote's *only* mechanism is LIKELY, not CONFIRMED — an argument from silence),
not a copy of any confirmed Mote implementation detail.

**Current Phylon state**: The CTRNN pipeline is already GPU-batched across the whole
population in one dispatch — meaning the marginal per-organism cost may not dominate
the way a naive CPU-bound implementation's would. No profiling data exists yet showing
CTRNN evaluation is actually the limiting factor at any tested population size.

**Recommendation**: **Do not build this as a feature yet.** First, profile actual
CTRNN-evaluation cost at population sizes an order of magnitude beyond what's currently
benchmarked (Phylon's own benchmarks currently test up to 10,000 physics nodes — the
brain-pipeline-specific cost at that scale has not been isolated and measured
separately in the evidence available to this investigation). Only if that measurement
shows CTRNN evaluation as a genuine, dominant per-tick cost at a population scale
Phylon's own research roadmap actually targets, prototype a minimal opt-in gradient-
follow mode for a small subset of organisms and measure the actual gain before
committing to it as a real, shipped feature.

**Dependencies**: A profiling pass isolating brain-pipeline cost at large population
sizes, which does not currently exist.

**Risk**: Medium-high if built without the measurement above. Two behavior pathways
existing side-by-side increases surface area for divergent bugs (e.g., ATP-cost/
efficiency accounting needs to apply consistently to both modes), and risks
contaminating exactly the kind of individual-level behavioral-evolution research the
CTRNN system exists to enable, for organisms that get downgraded to the cheap mode.

**Expected FPS gain**: Potentially large at very large population scale (an O(1)
gradient lookup vs. a CTRNN evaluation that scales with node/synapse count) — but
**unquantified without the profiling step above**; do not treat this as a confirmed
number.

**Expected memory improvement**: Likely modest — the diffusion fields this would reuse
already exist; a cheap-mode organism would need less brain-state memory, but brain
state is not currently a named memory bottleneck in any evidence gathered.

**Expected research value**: Medium if pursued and if it works — could let Phylon
study a spectrum from purely-reflexive to fully-evolved-brain agents within one
ecosystem, a genuine ALife research question. This is a research-direction bet, not a
guaranteed payoff, and should not be pursued before the profiling step above.

**Expected implementation complexity**: Medium (if pursued after measurement)  — the
diffusion-sampling and spring-actuation write paths already exist; the new work is a
gradient-to-actuation mapping, a per-organism/per-species toggle, and validating
determinism guarantees hold across both modes.

---

## Priority 6 — Verify / Add Explicit Max-Velocity Integrator Clamp

**Problem**: Mote's creator names an explicit max-velocity clamp as a defensive measure
against physics tunneling (fast-moving particles passing through constraints without
collision). No investigator found an equivalent named guard in Phylon's Rust-side
physics code, but the actual WGSL shader source was not read in this investigation —
this is a genuine coverage gap, not a confirmed absence.

**Evidence**: Creator's own stated lesson (not independently timestamped in the
supplied transcript excerpt, but consistent with standard physics-engine practice).
Phylon's own 3-iteration PBD projection layer may already serve a similar de-facto
role by directly correcting positions toward rest length each substep, independent of
velocity.

**Current Phylon state**: Unknown whether a max-velocity clamp exists in
`physics.wgsl`/`muscle_actuation.wgsl` — not read in this investigation.

**Recommendation**: First, read the actual WGSL shader source to confirm whether a
clamp already exists (likely, given the PBD layer's stabilizing role) or is genuinely
absent. Only if absent, add a simple, low-risk clamp.

**Dependencies**: None.

**Risk**: Near-zero — a pure safety net. The only risk is picking a clamp value low
enough to visibly cap fast/violent organism movement, which would need a small manual
check against real gameplay/simulation footage.

**Expected FPS gain**: None (a correctness/stability safeguard, not a throughput
change).

**Expected memory improvement**: None.

**Expected research value**: Negligible directly; prevents a rare-but-catastrophic
instability class that could otherwise silently corrupt a long-running experiment.

**Expected implementation complexity**: Low (if genuinely needed).

---

## Deferred / Not Recommended Now

These were considered and explicitly are not being prioritized, with reasons — listed
here so they aren't silently forgotten or accidentally re-proposed without this context.

### GPU-Resident State + Dense Compaction (Mote's core architecture)

**Problem this would solve**: O(population) CPU→GPU upload bandwidth cost, paid every
tick regardless of churn — a real cost that scales linearly with population.

**Why it's deferred, not rejected**: This is the one item in this entire investigation
where the honest answer is "not yet, revisit later" rather than "no." Phylon's own
prior, disclosed performance measurements are unambiguous: the confirmed current
bottleneck is CPU-side ECS query/allocation overhead (fixed for a measured ~8% FPS
gain without touching GPU architecture at all), and GPU physics compute itself is
measured at 321–542µs per tick at 1,000–10,000 nodes — comfortably under 4% of a 60Hz
frame budget. A GPU-resident rewrite targets a bottleneck (CPU↔GPU bandwidth at very
large population) that Phylon has not yet reached or benchmarked. Building it now would
be optimizing for a scale not in evidence, directly against this project's own
repeatedly-stated "measure before optimizing" discipline.

**When to revisit**: If/when Phylon's own research roadmap sets a concrete population
target an order of magnitude or more beyond what's currently benchmarked (10,000
nodes), re-measure the actual CPU↔GPU upload cost at that scale first. Only build this
if that measurement shows it's the real limiter — and budget for the real, well-
documented risk this carries (GPU race conditions in pointer/index rewiring during
compaction are a known-hard problem class; even Mote's own team, per the transcript's
own "Questions Remaining" section, hadn't fully resolved this at the time of the talk).

**Expected effort if pursued**: High — a foundational architecture change touching the
ECS-is-authoritative principle, ECS↔GPU data flow, and Phylon's documented determinism
guarantees (GPU work-item scheduling order is not guaranteed cross-driver, making
GPU-resident mutable state a strictly harder determinism problem than the current
"always re-derive from authoritative CPU state" model).

### Mote's Flat Bitflag Genome / Component Model

**Rejected as a genome replacement.** Would be a scientific regression against
Phylon's stated developmental/evolutionary-biology fidelity goals (Hox-style
combinatorial regulation, morphogen gradients, germ-line protection, self-adaptive
mutation, diploidy, NEAT-style speciation) — these are Phylon's actual differentiator
from a project like Mote, not incidental complexity to shed for scale. See the Decision
Matrix for the full reasoning.

### Mote's Analytic/SDF 2D Rendering Technique

**Rejected.** Already tried (the closest analog, 2D SDF-metaball rendering) and
consciously abandoned during Phylon's 3D migration, with stated reasons on record. Not
revisited by this investigation because no new evidence surfaced to justify
re-litigating that decision.

### Mote's 2D Orthographic Camera Model

**Rejected.** Solves a strictly simpler problem than Phylon's actual, genuinely-3D
simulation (real organism morphology, real 3D vision cones, real 3D physics). Adopting
it would be a straightforward regression, not an improvement.

### Decentralized, Splittable-Cell Organism Embodiment

**Rejected.** Mote's most distinctive demonstrated capability, but adopting it would
require rearchitecting Phylon's core organism model away from "one evolved genome
develops one body" toward a fundamentally different cellular-colony paradigm — not an
incremental feature, and one that would directly threaten Phylon's core developmental-
fidelity differentiator for an unclear research payoff.

---

## Path to Massive-Scale Simulation

Phylon's own current, measured scale ceiling is not established beyond ~1,000+
organisms (P9.1's own FPS measurements) and 10,000 physics nodes (the existing
`physics_broad_phase.rs` benchmark's tested range). Mote's publicly-stated scale is
"hundreds of thousands of organisms." This section distinguishes, honestly, what would
actually help close that gap — not by assuming Mote's specific implementation, but by
identifying the research directions the comparison surfaced.

### Already implemented in Phylon (no further action needed for these specifically)

- **Persistent, geometrically-grown GPU buffers** (physics/brain/diffusion pipelines) —
  confirmed by direct code read, arguably a stronger instance of "pre-allocate
  everything" than what's evidenced for Mote itself.
- **Fixed-size, allocated-once spatial-hash buffers** for the physics broad-phase,
  independent of population.
- **CPU-side scratch-buffer reuse** (`RenderInstanceScratch`, `SimTickScratch`) —
  measured, disclosed ~8% FPS improvement from eliminating per-frame/per-tick
  allocation churn.
- **Deferred GPU readback** (dispatch now, resolve next tick) to overlap GPU and CPU
  work.
- **Fixed-timestep-decoupled-from-render-rate** scheduling (a standard pattern, not
  Mote-specific, but confirmed already present).
- **A GPU-batched CTRNN pipeline** — one dispatch integrates the entire population's
  brains, not one dispatch per organism.

### Worth investigating (real, not-yet-answered questions this comparison surfaced)

- **Whether CPU-side ECS query/gather cost re-emerges as the dominant bottleneck at
  10× current tested population** — Phylon's own prior root-cause work identified this
  as the current limiter at ~1,000 organisms; whether the same category of fix (or a
  deeper one, like frame-to-frame change detection — only rebuild what actually
  changed, explicitly named as Phylon's own next disclosed step, unrelated to Mote)
  continues to scale is unmeasured past current population sizes.
- **CTRNN evaluation cost at large population** (see Priority 5 above) — genuinely
  unmeasured, plausible future limiter, worth a dedicated profiling pass before any
  architecture decision.
- **Whether Phylon's diffusion grid and GPU physics spatial hash could share
  infrastructure** to reduce buffer/dispatch count — unproven, possibly incompatible
  cell-size requirements, low priority.
- **GPU-resident population state** (deferred above) — the correct research direction
  *if and only if* CPU↔GPU upload bandwidth is later measured as the real limiter at a
  population scale Phylon's own roadmap actually targets.
- **Dedicated GPU profiling tooling** (RenderDoc, Tracy, or a wgpu-compatible
  equivalent) — Phylon's own prior performance work disclosed it lacked this in its
  build environment; closing this gap would improve the *quality of future measurement*
  itself, which is a prerequisite for making any of the above decisions well.

### Not worth adopting (considered, explicitly rejected as a path to scale)

- **A full GPU-resident dense-buffer-compaction rewrite, undertaken now** — see
  Deferred section above; the real bottleneck at Phylon's current scale is elsewhere,
  and this carries real, well-documented risk (GPU race conditions in index rewiring)
  that even Mote's own team hadn't fully solved.
- **Replacing the CTRNN with chemotaxis-only steering** — would not be "scaling
  Phylon," it would be abandoning the thing that makes Phylon's locomotion research
  meaningful. The correct framing (Priority 5) is a supplementary, opt-in mode for
  non-focal organisms, never a wholesale replacement.
- **Replacing the CPPN/regulatory-network genome with a flat bitflag array** — same
  reasoning: this would trade away Phylon's actual scientific differentiator for a
  scale Phylon has not established it needs, on a comparison basis (Mote) whose own
  genome-evolution capability is, by its creator's own disclosure, not yet built either.

---

## Summary: implementation order, if approved

1. `pyo3` Python/NumPy research binding (Priority 1)
2. Embodiment/manual-control diagnostic mode (Priority 2)
3. Diegetic Measure tool (Priority 3)
4. GPU buffer-growth code consolidation (Priority 4) — can run in parallel with 1–3,
   no dependencies
5. Read `physics.wgsl`/`muscle_actuation.wgsl` to resolve the max-velocity-clamp
   question (Priority 6) — cheap, can happen any time
6. Profile CTRNN-evaluation cost at 10×+ current population scale — a measurement
   task, not a feature; only after this, consider prototyping Priority 5

No implementation begins until this roadmap and its priority order are explicitly
reviewed and approved.

---

## Final Architectural Review

Before finalizing this roadmap, every recommendation above was re-examined against one
test: **if Phylon were released today as an open-source computational biology research
platform, with no Mote comparison ever having happened, would this still be worth
recommending?**

- **Priority 1 (`pyo3` binding)** — passes cleanly. It closes a real, structurally-
  explainable latency gap in Phylon's own ML-research workflow and is already an
  anticipated extension point in Phylon's own source, independent of anything about
  Mote.
- **Priority 2 (embodiment mode)** — passes, but is the more conservative of the two
  "adopt" recommendations. Its value (direct qualitative validation of evolved effector
  layouts) is real and stands alone, but it is a diagnostic convenience, not a research
  capability unlock. Kept at its current (second) priority rather than elevated.
- **Priority 3 (diegetic Measure tool)** — this is the **weakest-justified
  recommendation in the roadmap**, and worth saying so plainly rather than burying it.
  Its independent-of-Mote justification (a persistent spatial reference for long-running
  experiments) is real but modest. If this roadmap needs to be trimmed, this is the
  first item to cut — it was kept because it is low-risk and low-effort, not because it
  is high-value.
- **Priority 4 (buffer-growth consolidation)** — passes trivially; this is ordinary
  engineering hygiene that would be recommended in any code review, Mote or not.
- **Priority 5 (cheap movement mode)** — deliberately gated behind a measurement step
  specifically *because* it did not pass this test on its own: its benefit is currently
  unquantified, and the roadmap says so rather than recommending the feature itself.
- **Priority 6 (velocity clamp)** — passes as ordinary defensive engineering, contingent
  on confirming the gap is real.
- **Every "Deferred/Not Recommended" item** — re-confirmed as correctly excluded. None
  of them were included and then cut for being "too Mote-like" — they were never
  included as recommendations in the first place, only as considered-and-rejected
  entries with reasoning, which is why they appear in their own section rather than the
  numbered priority list.

**Nothing was removed at this stage** — no proposal in the numbered list above existed
solely because Mote does it; each has an independent Phylon-side justification stated
in its own "Evidence"/"Recommendation" text. The one adjustment this review makes is
qualitative, not structural: **Priority 3 is flagged as optional/cuttable**, and
Priority 2 is confirmed as appropriately scoped rather than expanded. The roadmap's
overall shape — one clear highest-value item, a small number of scoped medium-value
items, one explicitly-gated prototype, and a longer list of honestly-deferred or
rejected ideas — is the intended outcome of applying this test, not a compromise of it.
