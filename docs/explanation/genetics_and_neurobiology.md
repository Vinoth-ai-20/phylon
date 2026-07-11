# Genetics & Neurobiology

Phylon implements a neuro-evolutionary pipeline combining an emergent, decoded body plan with an evolving neural network. Every organism's genome (`genetics::Genome`) is exactly **three independently evolvable [CPPNs](https://en.wikipedia.org/wiki/Compositional_pattern-producing_network)** (Compositional Pattern-Producing Networks) plus nothing else structural:

- `brain_cppn` — generates the CTRNN brain's synapse weights.
- `regulatory_cppn` — generates a small recurrent Gene Regulatory Network (GRN) that decodes the body plan.
- `morph_cppn` — crossed, mutated, and included in the speciation-distance calculation, but **not currently read by anything at growth time**. This is disclosed, current technical debt, not a bug: body-plan decoding moved entirely onto `regulatory_cppn` early in the project's history, and `morph_cppn` was never removed.

There is no `HoxSequence`, `HoxGene`, or literal gene-sequence type anywhere in the current codebase — an earlier design (a hand-authored ordered list of body segments) was fully retired in favor of the regulatory-network decode described below. Any code sample, tutorial, or external note referencing `Genome::new_hox_driven`, `HoxSequence`, or `HoxGene` describes a retired API.

## How a body plan is decoded

`genetics::develop_at_position(regulatory_cppn, segment_index, total_segments)` is the single function every body position — the head, and every subsequent grown segment — is decoded through. There is no special-cased "first segment" path.

1. `regulatory_cppn` generates the weights/biases of a small recurrent runtime network (`RegulatoryNetwork`), the same "evolvable generator → simulated runtime structure" pattern `brain_cppn` uses to produce a `Brain`.
2. The network is run for a fixed number of developmental steps (never "to convergence" — determinism requires a fixed step count).
3. Ten regulatory genes, in four fixed semantic roles, are read off the final state:
   - **Hox** (3 genes) — thresholded at 0.5 into a 3-bit combinatorial code, decoded into one of 8 `SegmentType` variants (`Head`, `Torso`, `Muscle`, `Tail`, `Fin`, `Vascular`, `Ganglion`, `Germinal`) via a fixed lookup table.
   - **Differentiation** (2 genes) — gene 0 decides whether this position sprouts a bilateral fin/limb pair; gene 1 is an apoptosis signal (a `Germinal`-typed position is unconditionally immune, mirroring real germ-line protection).
   - **Effector** (2 genes) — muscle actuation amplitude and phase. Computed at every position regardless of segment type, but only ever *used* if the segment decoded as `Muscle` (the only type mapped to a physically-actuated `Elastic` spring).
   - **Pigment** (3 genes) — R/G/B skin color. Fully emergent per segment, never stored as RGB on the genome — this is why an evolved population's coloring can drift away from the diet-based default palette over generations.

This same decode function is used to grow starter ("seed") species — there is no special-cased morphology generator for them. A starter genome is just an ordinary `Genome` whose `regulatory_cppn` was hand-tuned (via `app::seed_regulatory_cppn(RegulatorySeedWeights{ .. })`) rather than evolved, so it develops through the identical pipeline as any evolved descendant.

**A measured, disclosed fragility in this design:** the regulatory network's decode is sensitive to its seed weights and to mutation — an earlier, overly aggressive founder-mutation dosage was found (via direct headless measurement, not assumption) to collapse the fraction of individuals with any actuatable muscle segment from 100% down to near zero for several starter species. This was root-caused and fixed by matching the founder mutation dosage to the same rate used by ordinary reproduction; see [Architecture Decisions](../roadmap/decisions.md) for the full history. The takeaway for anyone tuning seed weights or mutation rates: **measure the resulting population's actuatable-effector rate directly** (a real headless run, not a single isolated evaluation) before trusting a seed or mutation-rate change.

## Continuous-Time Recurrent Neural Networks (CTRNN)

The brain is a CTRNN, wired by querying `brain_cppn` for every candidate pair of nodes (a NEAT-style indirect encoding, avoiding "broken brain" syndrome when the body plan mutates independently of the brain).

- The sensory input vector is currently **15 values**: 3 scalar channels (Olfaction, ATP/energy, Age) plus a 9-value vision representation (a binned azimuth × elevation cone around the organism's body-fixed forward/dorsal frame — see [Camera & Viewport](camera_and_viewport.md) for the 3D orientation frame this reuses). Earlier revisions of this document (and some still-unmigrated unit-test fixtures) describe a 9-input vector with a 3-value vision representation — that was superseded when vision was rebuilt for full 3D orientation.
- Node states are integrated on the GPU via a WGSL compute shader, clamped to `[-10.0, 10.0]` to prevent runaway positive feedback from producing `NaN`s or extreme saturation.
- Outputs drive the contraction/expansion of `Elastic` muscle springs, which is the *only* path from brain activity to physical locomotion — a segment that never decoded as `Muscle` has no actuatable spring at all, regardless of what its brain outputs.

## Regional brains (real, but currently dormant)

`brain::Brain` carries a `node_regions: Vec<RegionId>` field (`RegionId::Central` by default, or `RegionId::Ganglion(usize)`), and synapse wiring uses a region-aware compatibility threshold. This machinery is fully implemented and wired, but as of this writing **no real genome has been observed to decode a `Ganglion` segment**, so every brain's regions are uniformly `Central` in practice — the feature is real architecture, not yet a real behavior. Don't describe regional brains as an active behavior without re-measuring.

## Reaction-diffusion morphogens (real, both halves implemented)

Body-position decoding can be influenced by two additive signals beyond genome + position:

- **Intra-organism**: `organisms::morphogen_field::MorphogenLevel`, seeded at a growing tip and relaxed/decayed along the Body Graph's own edges (the same graph-relaxation pattern used for nutrient transport and hormone diffusion — an organism's body is a small graph of ~15 nodes, not a continuous tissue mesh, so this is architecturally a graph problem, not a spatial-PDE problem).
- **Inter-organism / environmental**: a fifth GPU world-space diffusion layer (`diffusion::FieldLayer::Morphogen`), sampled at a growing segment's position — meaning nearby developing organisms can, in principle, influence each other's decode.

Both fold into the same additive "external signal" channel `develop_at_position_with_life_stage` already used for life-stage transitions — no new `genetics`-crate parameter was needed. As a disclosed limitation, no organism has yet been observed receiving a *meaningfully nonzero* cross-organism environmental signal in practice; the plumbing is real and tested, but its population-level effect hasn't been characterized.
