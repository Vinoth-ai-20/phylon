# Determinism

Phylon's determinism guarantee has two independent halves — a randomness-seeding contract and a floating-point/GPU-execution contract. This document states plainly what's real and enforced today, and what's a known, open gap, rather than repeating an aspirational "bit-exact" claim that hasn't been verified.

## What is real and enforced

**Seeded randomness.** `common::SimRng` (a seeded `ChaCha8Rng` wrapper) is the single source of randomness for every stochastic system — mutation, crossover, spawn placement, hazard timing, and so on. It's inserted once as an ECS resource at startup from a config-provided `rng_seed` and is the only RNG any simulation system is meant to touch; direct use of an unseeded RNG (e.g. `fastrand::`) anywhere in a simulation-affecting code path is a determinism bug, not an accepted shortcut — this exact class of bug has been found and fixed more than once in this project's history.

**Fixed timesteps.** The simulation advances in discrete ticks at a fixed `dt`, never a wall-clock delta. Physics integration, metabolic burn, and every other time-dependent system use this fixed constant.

**Sorted ECS iteration.** Where a system aggregates inputs from multiple entities in a way whose result could depend on iteration order (e.g. multiple organisms interacting with the same resource), inputs are sorted by `EntityId` before processing — this removes `rayon` thread-scheduling order as a source of nondeterminism for those specific aggregations.

**Within a single run, from a fixed starting state, ticks are reproducible.** Two consecutive ticks of the same running process produce consistent, causally-ordered state — this is what the sorted-iteration and fixed-timestep guarantees are actually for.

## What is not (yet) verified — a real, measured gap

**Cross-run bit-exact reproducibility of the GPU physics pipeline is not currently true, and has been measured directly, not assumed.** Two headless runs from the identical `rng_seed`, same binary, same machine, diverge: per-organism floating-point values (e.g. accumulated path length) first differ in the 5th–6th significant digit within a few hundred ticks, and that divergence cascades into different discrete outcomes (population counts, death tallies) within roughly 600 ticks. This was confirmed to pre-date the change under investigation at the time it was found — it is not a regression from a specific recent change, but an existing characteristic of the GPU compute path (most likely floating-point accumulation-order or atomic-operation-order nondeterminism in the physics compute shaders, though the exact mechanism has not yet been isolated).

If you are relying on this project for exact cross-run reproducibility (e.g. comparing two experiment runs value-for-value), **verify it for your specific use case first** — don't assume it from this document or from the "determinism" framing used elsewhere in the project's history. If you isolate the specific shader/operation responsible, that's a well-scoped, valuable contribution — see [Open Items & Backlog](../roadmap/backlog.md).

## Practical implications

- CPU-only, non-GPU-physics-dependent unit tests that assert same-seed-same-output *are* reliable, and this project has many of them (search for `is_deterministic_for_a_given_seed`/`_is_deterministic` test names) — these test individual systems in isolation against synthetic fixtures, which is exactly the boundary where the guarantee is real.
- A real running simulation's population-level trajectory (who lives, who dies, who reproduces) should be treated as reproducible **in distribution**, not bit-for-bit, until the GPU-side gap above is closed.
