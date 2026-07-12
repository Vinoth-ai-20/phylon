# Mote vs. Phylon — Decision Matrix

Every subsystem below ends with exactly one decision:

- **KEEP PHYLON** — Phylon's current approach is already equal to or better than what's evidenced for Mote; no change.
- **ADAPT MOTE IDEA** — a specific, scoped idea from Mote is worth building into Phylon, in a Phylon-appropriate form.
- **PROTOTYPE FIRST** — the idea has real merit but an unproven cost/benefit; build a small, cheap experiment before committing.
- **REJECT** — considered and explicitly not worth pursuing, with reasons.
- **UNKNOWN** — the evidence does not support a decision either way; stated honestly rather than guessed.

Confidence ratings (CONFIRMED / HIGH CONFIDENCE / LIKELY / HYPOTHESIS / UNKNOWN) are carried forward from the five independent investigations that produced [MOTE_ENGINEERING_LESSONS.md](MOTE_ENGINEERING_LESSONS.md); see that document for full evidence citations. This matrix is the terse, decision-oriented companion to it.

---

## Entity Storage Model (CPU ECS + per-tick gather vs. GPU-resident dense buffers)

**Decision: KEEP PHYLON**

Phylon's `bevy_ecs`-authoritative model, with GPU pipelines treated as stateless-between-ticks compute accelerators fed by a fresh per-tick gather, is a deliberate, documented architectural choice (cache coherency, `rayon` CPU parallelism, and — the load-bearing reason — determinism, since GPU work-item scheduling order is not guaranteed cross-driver). Mote's GPU-resident model is real (CONFIRMED) but solves a different problem (avoiding CPU↔GPU round-trip at very large scale) that Phylon has not yet measured as its bottleneck. Revisit only if Phylon's own population targets grow past what's currently benchmarked (see Performance/Scale below).

## GPU Memory Compaction

**Decision: REJECT (for now)**

Mote's dedicated GPU compaction pass (CONFIRMED) solves fragmentation in a persistent, GPU-resident population buffer. Phylon has no such buffer — it re-uploads the full live population every tick, and `bevy_ecs`'s own Table storage already does CPU-side dense-row compaction (swap-remove) for free on every despawn. Building a compaction pass would solve a problem Phylon's architecture doesn't have, while importing real, well-documented risk (the Mote transcript's own "Questions Remaining" section discloses unresolved GPU race-condition concerns around pointer rewiring during compaction, even for Mote's own team).

## Diffusion / Chemical Field Representation

**Decision: KEEP PHYLON**

Both systems converge on the same solution independently: a 2D, ping-pong double-buffered diffusing grid. Phylon's is texture-based (`R32Float`, 5 named layers), tested for cross-layer isolation, and deliberately kept 2D even though the rest of the engine is 3D, with a written cost rationale (a volumetric field would cost roughly two orders of magnitude more memory/bandwidth). This is the one subsystem where Phylon and Mote are architecturally closest; there is nothing to adopt.

## Physics Core Model (integration, constraint solving)

**Decision: KEEP PHYLON**

Phylon's hybrid symplectic-Euler-force + 3-iteration Position-Based Dynamics solver is more numerically principled than what's evidenced for Mote (Hooke's-law springs with informal "tear limits"/velocity-clamp instability band-aids, per the creator's own framing). Phylon is not behind here; if anything, ahead on solver sophistication. One small, independent-of-Mote defensive item is worth a look (see below).

## Max-Velocity Clamp (integrator safety)

**Decision: PROTOTYPE FIRST**

Not found as a named, explicit guard in Phylon's CPU physics fallback during this investigation (the GPU WGSL shader source itself was not read — this is a real, disclosed coverage gap, not a confirmed absence). Low effort, near-zero risk, if genuinely missing. Confirm first by reading `physics.wgsl` before adding anything, since the PBD projection layer may already serve a similar de-facto role.

## Collision Broad-Phase / Spatial Partitioning

**Decision: KEEP PHYLON**

Phylon has three independently-justified CPU spatial-index implementations (`UniformGrid`, `SpatialHash`, `Octree`) plus a separate, fixed-size GPU spatial hash for the physics broad-phase — each purpose-built for a different density/query profile, with a written memory-cost rationale for the GPU-side design choice. Mote's equivalent is HYPOTHESIS-only (the transcript never names or diagrams its collision algorithm). Phylon's spatial-partitioning story is more explicit and better-evidenced than Mote's, not behind it.

## Shared Diffusion-Grid / Collision-Hash Infrastructure

**Decision: UNKNOWN**

A Mote-*inspired* (not Mote-*confirmed*) idea: could Phylon's diffusion grid and GPU physics spatial hash share one structure to reduce buffer/dispatch count? Genuinely unproven — diffusion needs smooth, low-frequency sampling; collision broad-phase needs short-range, high-frequency binning; these may need incompatible cell-size assumptions. Flagged honestly as unresolved rather than recommended either way; low priority regardless.

## Unused `SpatialIndex` Trait Abstraction

**Decision: ADAPT MOTE IDEA (indirectly — a Phylon-internal cleanup surfaced by the comparison)**

Not a Mote-derived idea at all, but surfaced by the act of comparing Phylon's spatial code against Mote's simpler model: the shared `SpatialIndex` trait across all three CPU spatial structures has, per the code's own doc comments, no live caller — every call site uses concrete inherent methods directly. Worth removing or actually using during an unrelated pass; not a priority on its own.

## Locomotion / Steering Mechanism

**Decision: KEEP PHYLON**

Phylon's locomotion (an evolved CTRNN — continuous-time recurrent neural network — one output per effector spring, CPPN-generated topology/weights, Hebbian within-lifetime plasticity, neuromodulator gating, Braitenberg baseline wiring) is confirmed, real, and substantially more sophisticated than the *only* mechanism the Mote evidence describes (chemotaxis: moving along a sampled scent-field gradient). **Important honesty note carried into this decision**: "Mote has no neural-network-driven locomotion at all" is rated LIKELY, not CONFIRMED, by two independent investigations — an argument from silence over a ~50-minute talk, reinforced but not proven by a fruitless independent web search. The decision (keep Phylon's system, don't adopt chemotaxis as a replacement) holds regardless of how this uncertainty resolves, since Phylon's system is confirmed sophisticated on its own terms, not merely "better than an unconfirmed alternative."

## Supplementary Cheap Movement Mode for Background Organisms

**Decision: PROTOTYPE FIRST**

Two independent investigations converged on the same idea from different domains (physics/locomotion and behavior/ecology): an opt-in, cheap gradient-follow movement mode — reusing Phylon's existing diffusion fields already sampled as CTRNN olfaction input — for background/non-focal organisms at very large population scale, as a supplement to (never a replacement for) the CTRNN. This is explicitly a LOD-style idea inspired by Mote's likely scale-vs-fidelity tradeoff, not a confirmed Mote technique. Real engineering cost (a second behavior pathway, with real risk of divergent bugs between the two modes' ATP-cost/efficiency accounting) and unproven benefit (Phylon's CTRNN is already GPU-batched, so the marginal per-organism cost may not dominate the way a naive CPU-bound implementation's would). Build a small measurement first — profile actual CTRNN-eval cost at a realistic large population — before committing to this as a real feature.

## Organism Genome / Development Model (CPPN + regulatory network vs. flat bitflag components)

**Decision: KEEP PHYLON**

These solve different problems, not a strictly-better-vs-worse comparison. Phylon's three-CPPN genome driving a Hox-inspired regulatory-network decode is deliberately, documentedly more biologically rigorous (explicitly modeled on real Hox combinatorics; an earlier, simpler, hand-authored `HoxSequence` design was retired specifically because it couldn't be smoothly evolved). Mote's flat bitflag array (HIGH CONFIDENCE, not CONFIRMED — inferred from one code slide) is cheaper, more robust to random mutation, and more legible, but is optimized for a different goal (cheap emergent behavior at massive scale, with the creator's own disclosed current-build status: no genetic algorithm or learned weights yet). Adopting Mote's model wholesale would be a scientific regression against Phylon's stated developmental-biology-fidelity goals.

## Decentralized, Splittable-Cell Organism Embodiment

**Decision: REJECT**

Mote's most distinctive, best-evidenced claim (CONFIRMED, 25:24: splitting a plant leaves both halves independently functional) has no Phylon analog and no easy path to one — Phylon's organisms are graph-structured bodies grown from a single genome/regulatory decode, not decentralized colonies of independently-viable cells. Pursuing this would mean rearchitecting the core organism model away from "one evolved genome develops one body" toward something closer to a cellular-automaton colony model — a fundamentally different simulation paradigm, not an incremental feature. This would directly threaten Phylon's core developmental-biology-fidelity differentiator (Hox-style single-genome regulation) for a capability whose research payoff, for Phylon's stated goals, is unclear. Not recommended.

## Ecology (predation, photosynthesis, decomposition, disease, catastrophe)

**Decision: KEEP PHYLON**

Phylon's food web (producer/herbivore/carnivore/decomposer) parallels Mote's closely in shape, but with more explicit, tested closed-loop resource accounting: two independently-named, deliberately-fixed "carbon leak" bugs (photosynthesis and corpse-decay ends), a disease model with per-transmission-event pathogen mutation, and a fungal decomposition mechanism modeled on real mycelial remote nutrient transport (distinct from simple on-contact eating). Mote's energy-conservation strictness is explicitly UNKNOWN per its own evidence; Phylon's is at least engineered-toward and code-evidenced, if not formally verified end-to-end. No adoption indicated.

## Speciation / Lineage Tracking

**Decision: KEEP PHYLON**

Phylon has a genuine, tested, NEAT-style genetic-distance speciation mechanism and a bounded, tested lineage DAG tracker. The Mote evidence shows only per-entity generation-count tracking, with no species-clustering concept described at all. Not a comparison to import from — Phylon already has real, working capability here that Mote's evidence doesn't show an equivalent of.

## Camera System

**Decision: KEEP PHYLON**

Phylon's completed 3D orbit/fly camera (quaternion-composed, unbounded pitch, additive orthographic mode, six preset views, eased smooth framing, bookmarks, gizmo overlays, enforced single-source-of-truth architecture) is more capable than Mote's inferred 2D orthographic camera on every axis except unbounded analytic-zoom sharpness — which is a rendering-representation property (mesh vs. analytic/SDF), not a camera-model one, and not a like-for-like comparison given Phylon's organisms have real 3D morphology a 2D camera couldn't express regardless. Adopting Mote's camera model would be a straightforward regression.

## Organism Rendering Technique (mesh + PBR vs. analytic/SDF 2D)

**Decision: REJECT**

Phylon already evaluated and consciously moved away from the closest analog to Mote's technique (2D SDF-metaball rendering) during its migration to a genuine 3D engine, a decision explicitly documented with stated reasons (real depth buffering, lighting, and scaling that a screen-space analytic/SDF technique in a 3D scene cannot easily provide). Mote's no-lighting, no-post-processing, per-fragment-analytic-AA approach is well-suited to its own 2D-orthographic, no-lighting design; re-litigating Phylon's already-made, already-documented decision is not something this investigation surfaces new evidence to justify.

## Embodiment / Manual-Control Interaction Mode

**Decision: ADAPT MOTE IDEA**

Mote's "all entities are players" principle (CONFIRMED) is not adopted wholesale — Phylon is a research/observation tool, not a sandbox game, and full action-verb symmetry between player and AI would be a large, unjustified undertaking. But a scoped, low-risk version is worth building: a manual embodiment/possess mode that reuses the *already-existing* `Brain::set_external_action_override` hook (built for RL policy injection) to let a researcher directly drive one selected organism's actuators via keyboard/mouse. This is a genuine qualitative-validation tool ("does this evolved effector layout actually let it turn the way its morphology suggests"), medium effort, low risk if scoped strictly as a diagnostic mode.

## Diegetic (Physically-Simulated) UI Elements

**Decision: ADAPT MOTE IDEA (narrowly)**

Do not adopt Mote's diegetic-UI principle as a paradigm shift — Phylon's conventional egui panel/docking system is coherent, disciplined, and working, and nothing in the evidence suggests a research tool needs Mote's emergent-play affordance to function as a research tool. One narrow, genuinely good fit: turn the existing Measure tool from a one-shot screen-space marquee drag into a persistent, position-anchored in-world ruler entity — Phylon's ECS/physics already has everything needed to make this a real spawned object, and it maps almost exactly onto Mote's own flagship diegetic-UI example (the tape-measure membrane).

## Scientific Visualization (heatmaps, population/state graphing)

**Decision: KEEP PHYLON**

Phylon's heatmap overlay system already covers and exceeds Mote's demoed O2/CO2/scent variables (adding Glucose, ATP, Pheromones, Energy Density), and its Metrics Dashboard has materially more scientific depth (diversity indices, colony-connectivity tracking, event-correlated time series, CSV/JSON/PNG export) than anything the Mote evidence attributes to it. The one design-taste difference (Mote graphs directly in-viewport; Phylon uses a docked panel) is not a demonstrated capability gap either direction.

## Headless Execution / Research Workflow / ML Integration

**Decision: ADAPT MOTE IDEA**

The single highest-confidence, highest-value finding of the entire investigation, independently reached by two investigators: build a `pyo3`-based in-process Python binding around Phylon's already-existing headless path, exposing the already-defined `ObservationVector`/`ActionVector` types as NumPy-compatible arrays. This directly closes Phylon's real latency disadvantage against Mote's demoed in-process NumPy-extraction model (Phylon's current path is out-of-process/networked — a WebSocket+JSON round-trip per RL step — which pays real serialization/IPC cost Mote's in-process model doesn't). This is not a novel idea invented here: it is already an explicitly anticipated extension point in Phylon's own `learning` crate documentation. Medium effort, high research-value, low-to-medium risk if built strictly on top of the existing `PhylonApp::update_simulation()` call.

## Multi-Agent RL Maturity

**Decision: KEEP PHYLON, WITH AN HONESTY CAVEAT**

Phylon's single-agent RL bridge (`network`/`learning`, a real, working, tested WebSocket lock-step protocol) is more mature than Mote's Q&A-discussed-but-undemoed MARL plans. But Phylon's own source explicitly discloses true multi-agent RL (more than one externally-controlled agent at a time) does not exist yet either. The honest framing for any public comparison: Phylon leads on single-agent infrastructure; both projects are, by their own respective disclosures, pre-multi-agent-RL. Do not claim outright MARL superiority.

## Global Force Live-Tuning (gravity, centering as an interactive demo feature)

**Decision: UNKNOWN**

Phylon's `PhysicsConfig` already has equivalent fields; no investigator found evidence of a live UI exposing them the way Mote demos "reshape the world" interactively. Low effort if pursued, but no concrete research need was identified in this investigation to justify prioritizing it — genuinely take-it-or-leave-it, not a gap worth acting on without a specific motivating use case.

## GPU Buffer-Growth Code Duplication (physics/brain/diffusion pipelines)

**Decision: ADAPT MOTE IDEA (indirectly — a Phylon-internal cleanup surfaced by the comparison)**

Not a Mote technique — a maintainability observation made *while* comparing Phylon's memory-management approach against Mote's. All three GPU pipelines independently reimplement near-identical geometric-buffer-growth logic. Low-effort, low-risk, code-quality-only consolidation; not a performance change.

## GPU Profiling Tooling

**Decision: UNKNOWN**

Phylon has working, opportunistic GPU timestamp-query profiling, but its own prior performance-audit work disclosed that deeper flame-graph/cache-miss profiling tooling wasn't available in its build environment. Whether to invest in a dedicated GPU profiler (RenderDoc, Tracy, or similar) is a real, open, worthwhile question this investigation surfaces but does not resolve — it's a tooling/workflow decision, not a code change, and outside the scope of what this investigation can recommend with evidence.

---

## Decision count summary

| Decision | Count |
|---|---|
| KEEP PHYLON | 11 |
| ADAPT MOTE IDEA | 5 |
| PROTOTYPE FIRST | 2 |
| REJECT | 2 |
| UNKNOWN | 4 |

The heavy weighting toward **KEEP PHYLON** is itself a finding, not a foregone conclusion reached by bias: every "keep" decision above traces to a specific, cited piece of Phylon's own documented or code-confirmed architecture that either already matches or exceeds what the Mote evidence could establish. Where Mote genuinely demonstrates something Phylon lacks (in-process Python bindings, embodiment, diegetic UI elements), this matrix says so plainly and scopes an adoption path sized to the actual evidence, not to Mote's scale or ambition in general.
