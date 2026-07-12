# Phylon Architecture Principles

This document is not a roadmap. Roadmaps (`docs/roadmap/*.md`) describe what's
planned or in progress and change often. This document describes what should stay
true regardless of which roadmap is current. It exists so a decision made in one
phase doesn't get quietly re-litigated in a later one just because the person making
it wasn't there for the first discussion.

These principles were made explicit after the Mote comparative investigation
(`docs/research/MOTE_ENGINEERING_LESSONS.md`, `docs/research/MOTE_VS_PHYLON_DECISION_MATRIX.md`)
surfaced how much of Phylon's architecture was already correct *by convention* —
consistently applied across nine development phases — but never written down as a
standing rule a new contributor (or a later version of an existing one) could check
a proposal against without re-deriving the reasoning from scratch.

## The principles

1. **CPU is the authoritative simulation state.** `bevy_ecs::World` holds the one
   source of truth for what entities exist and what components they carry. GPU
   pipelines receive flat, typed snapshots gathered from that state each tick — they
   never hold state that outlives a tick unless a documented, deliberate exception says
   otherwise (e.g. persistent buffer *capacity*, which grows and is reused, is not the
   same as persistent *entity data*, which is re-derived every tick).

2. **GPU is a massively parallel accelerator, not the source of truth.** Every GPU
   compute pipeline in this workspace is stateless with respect to entity semantics —
   it knows about physics nodes, springs, CTRNN nodes, synapses, and diffusion layers
   as flat structs, never about what species an entity is or what components it has.
   That knowledge lives CPU-side. This is what makes it possible to reason about
   correctness by reading CPU-side ECS state, without also having to track a second,
   independently-mutating copy of the world living in GPU memory.

3. **Biology takes priority over visual fidelity.** When a rendering technique and a
   simulation requirement conflict, the simulation wins. Phylon's organism rendering
   has changed twice already (2D SDF-metaball → mesh-instanced PBR) as the underlying
   simulation went from 2D to 3D — the renderer follows what's actually being
   simulated, not the other way around.

4. **Determinism is preferred over peak throughput, unless a feature explicitly opts
   out.** A given `PhylonConfig` (including its RNG seed) should produce the same
   simulation trajectory on repeated runs. Every stochastic system draws from
   `common::SimRng`, never an unseeded RNG. GPU compute is exempt from bit-exact
   cross-run determinism today (work-item scheduling order isn't guaranteed
   cross-driver) — this is a disclosed, known gap, not a silent one, and any change
   that would widen it (e.g. moving more authoritative state onto the GPU) needs to
   name that tradeoff explicitly rather than accept it by default.

5. **Measure before optimizing.** No performance claim ships without a before/after
   number. "This should be faster" is not evidence; a benchmark, a profiler trace, or
   a disclosed FPS measurement is. If a change can't be measured in this environment,
   the write-up says so honestly rather than asserting a number that wasn't taken.

6. **Every optimization must be benchmarked, including the decision not to pursue
   one.** When a proposal is deferred because the evidence doesn't yet justify it (see
   `docs/roadmap/PHYLON_NEXT_GENERATION.md`'s "Deferred / Not Recommended Now"
   section for a working example), the deferral itself names the specific measurement
   that would change the answer — not just "later."

7. **Documentation follows the code, not the other way around.** Source comments
   explain the architecture as it exists today, in timeless, implementation-focused
   language — not narrated development history ("Phase 5 added X"). Implementation
   history belongs in Git history and `docs/roadmap/history.md`/`decisions.md`, never
   in source comments a future reader has to already know the project's history to
   parse.

8. **Every major architectural change requires a recorded decision.** A change that
   alters a load-bearing tradeoff (data layout, determinism boundary, rendering
   philosophy, genome representation, crate dependency direction) gets an entry in
   `docs/roadmap/decisions.md` explaining what was chosen and why — not just what
   changed. A change that doesn't touch a load-bearing tradeoff doesn't need one;
   this principle is about durable architectural commitments, not a process tax on
   every PR.

9. **Scientific correctness takes precedence over gameplay convenience.** Phylon is a
   research tool, not a sandbox game, even where a feature (embodiment, diegetic UI
   elements) is inspired by one. Features that make the simulation more fun to poke at
   are welcome exactly insofar as they don't compromise the ability to trust an
   experiment's results.

10. **Backward compatibility is secondary to research correctness when schema
    evolution requires breaking changes.** Save-file (`schema_version`) and genome
    (`GENOME_SCHEMA_VERSION`) formats bump without a migration path when a field's
    meaning changes — a schema that silently reinterprets old data incorrectly is a
    worse failure mode than a save file that cleanly refuses to load. This is already
    Phylon's practice (see `docs/roadmap/decisions.md`); this principle exists so it
    isn't accidentally reversed under pressure to preserve an old save file.

## Evaluating a new feature or architectural proposal

Before committing to a nontrivial feature or architectural change, answer these six
questions honestly:

1. Does this improve scientific realism?
2. Does this improve scalability?
3. Does this improve maintainability?
4. Does this improve reproducibility?
5. Does this increase architectural complexity?
6. Is there measured evidence that this is needed?

(Question 5 is inverted relative to the others — a "yes" here counts *against* the
proposal, since complexity is a cost, not a benefit, being weighed against the other
five.)

**If fewer than four of the six answers come out favorably (yes to 1–4/6, no to 5),
the feature should probably wait** — either for more evidence, a narrower scope, or a
different approach that scores better. This isn't a hard gate enforced by tooling; it's
a discipline for whoever is proposing the change to apply to themselves honestly before
asking someone else to review it, the same way `docs/roadmap/PHYLON_NEXT_GENERATION.md`
applied "would I recommend this if Phylon were released as an open-source research
platform, independent of any comparison that inspired it?" to its own recommendations
before finalizing them.

This exists to keep the project from becoming a collection of individually-interesting
ideas that collectively erode the properties (determinism, biological fidelity,
maintainability) the principles above exist to protect.

## Relationship to other documents

- `docs/roadmap/decisions.md` — the historical record of *specific* decisions made and
  why. This document is the durable, general rules those decisions were each an
  instance of.
- `docs/roadmap/backlog.md` — known open gaps, distinct from either the principles
  (which don't change often) or a specific roadmap (which does).
- `docs/explanation/architecture.md` — the descriptive account of how the system is
  built today. This document is prescriptive: what should stay true as it keeps
  changing.
- `CONTRIBUTING.md` — points here for the "why" behind its enforcement rules.
