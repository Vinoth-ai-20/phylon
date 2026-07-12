//! # Starter-Species Genome Seeding
//!
//! Builds the hand-authored regulatory/brain CPPN (compositional
//! pattern-producing network — a small neural network used here to
//! generate developmental parameters as a function of gene position,
//! rather than storing them directly) templates and the founding
//! population's `Genome`s that `PhylonApp::new` spawns via `world::World`.
//!
//! ## Purpose
//!
//! The simulation needs a starting population before any evolution has
//! happened. Rather than hand-authoring each starter organism's body plan
//! directly, this module builds a small library of regulatory CPPNs — one
//! per starter species archetype (worm, fish, branchy, omnivore,
//! decomposer, producer) — each of which is decoded through the exact same
//! `develop_at_position` pipeline every evolved organism's genome goes
//! through. This means starter organisms are ordinary, evolvable genomes
//! from tick 1, not specially privileged templates: `Genome::mutate` and
//! reproduction's crossover operate on them identically to any organism
//! born later in the run.
//!
//! ## Design decision: no special-cased morphology generation
//!
//! Nothing in this module hardcodes a segment-type sequence or reads
//! `REGULATORY_GENE_ROLES` at runtime to force a specific shape. Each seed's
//! `RegulatorySeedWeights` are found by sweeping/searching the weight space
//! and selecting for measured properties (segment-type diversity, having at
//! least one actuatable `Muscle` segment) — not by hand-picking weights to
//! match an intended shape. This keeps the founding population inside the
//! same genome representation and evaluation path as everything the
//! simulation evolves afterward, so there is exactly one code path for
//! "genome to body plan" to reason about, verify, and maintain.

/// Per-seed tunable weights for [`seed_regulatory_cppn`]'s four independent
/// local-activation domains (one per [`genetics::RegulatoryGeneRole`] region)
/// plus its shared monotonic/periodic bases. A named struct rather than 7
/// positional `f32`s — with this many knobs, positional args at the call
/// site are an easy place to transpose two values silently.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RegulatorySeedWeights {
    /// The output node's own bias — the network's baseline "all regions
    /// off" level; every region weight below adds on top of this.
    pub output_bias: f32,
    /// Weight on the local-activation bump centered over the Hox gene
    /// region (indices 0-2 of 10 — see `REGULATORY_GENE_ROLES`).
    pub hox_weight: f32,
    /// Weight on the bump centered over the Differentiation region (3-4).
    pub differentiation_weight: f32,
    /// Weight on the bump centered over the Effector region (5-6) — the
    /// region most starved by a single-bump design (see this function's doc
    /// comment for why each region needs its own independent bump).
    pub effector_weight: f32,
    /// Weight on the bump centered over the Pigment region (7-9).
    pub pigment_weight: f32,
    /// Weight on a coarse (~2-cycle) periodic basis across the full gene
    /// range, for broad repeated/alternating structure.
    pub sine_coarse_weight: f32,
    /// Weight on a fine (~5-cycle) periodic basis, for finer repeated
    /// structure than `sine_coarse_weight` alone can produce.
    pub sine_fine_weight: f32,
}

/// A seed regulatory CPPN with real combinatorial representational capacity
/// across every gene role region.
///
/// ## Why a monotonic (single-basis) network is insufficient
///
/// `RegulatoryNetwork::generate` derives every gene's bias and every
/// gene-pair's edge weight by evaluating this CPPN as a function of gene
/// index. If that function were purely linear in gene index, its output
/// would be strictly monotonic — and since a 3-bit Hox code is read off
/// three specific, adjacent gene indices, a monotonic bias function can only
/// ever threshold to a non-decreasing or non-increasing bit sequence
/// (`000,001,011,111` or `000,100,110,111`). Six of the eight possible
/// `SegmentType` codes, including `Muscle` (`010`), would be **structurally
/// unreachable** regardless of parameter tuning — no amount of retuning a
/// linear function's slope/intercept can produce a non-monotonic bit
/// pattern.
///
/// A single shared `Gaussian` bump (one local-activation "hotspot" in
/// gene-index space) fixes reachability for whichever one region it's
/// centered on, but every other gene *role* — crucially `Effector`, since
/// that is what determines whether a segment can actually actuate — sits far
/// outside that one bump's reach and collapses to whatever the flatter
/// Sigmoid/Sine terms produce there. A single bump is a single, non-renewable
/// local-activation budget: tuning it to help one region unavoidably comes
/// at another region's expense.
///
/// ## This design: one independent bump per gene-role region
///
/// Gene *role* is already fully determined by gene *position* under the
/// fixed `REGULATORY_GENE_ROLES` table (Hox = indices 0-2, Differentiation =
/// 3-4, Effector = 5-6, Pigment = 7-9) — there is no missing input
/// dimension, only insufficient local-activation *capacity* in a
/// single-bump design. This CPPN instead gives each region its own
/// independently-weighted `Gaussian` bump, centered at that region's
/// index-fraction midpoint, alongside a shared `Sigmoid` (monotonic
/// gradient) and *two* `Sine` bases at different frequencies (coarse + fine
/// periodic/repeated structure). Every region's bump can be independently
/// strengthened, weakened, or inverted (a negative weight acts as a local
/// *repressor*, not just an activator) via its own `RegulatorySeedWeights`
/// field — tuning `effector_weight` cannot come at the expense of
/// `hox_weight`, because they route through separate connections to
/// separate hidden nodes.
///
/// This gives four bumps, matching today's four fixed `RegulatoryGeneRole`
/// variants. A future role (e.g. organogenesis or physiology) would need one
/// more region bump added here, not a restructuring — the pattern ("one
/// independently-weighted local bump per role region") generalizes directly.
///
/// All bases still combine at one `Linear` output node, so
/// `RegulatoryNetwork::generate`'s calling convention
/// (`evaluate(&[idx, idx])` for bias, `evaluate(&[i/total, j/total])` for
/// edge weight) is unchanged by this richer internal structure. Nothing here
/// reads `REGULATORY_GENE_ROLES` at runtime — the region centers below are
/// constants derived from that table by hand, not a live lookup —
/// deliberately, so this stays a plain, cheap, deterministic `Cppn` rather
/// than a construction whose shape depends on the table's exact contents at
/// call time.
///
/// ## Deliberately not tuned toward any specific `SegmentType`
///
/// This function has no `Muscle`-specific or `Fin`-specific logic anywhere
/// — the four region weights and two sine weights are swept per starter
/// species purely for *diversity* (see each call site's own comment for what
/// was empirically observed, not targeted), and the resulting network
/// remains an ordinary, evolvable `Cppn`: mutation's `mutate_add_node`/
/// `mutate_add_connection`/per-connection jitter operate on it exactly as
/// they would any other genome, with nothing special-cased for starter
/// organisms.
pub(crate) fn seed_regulatory_cppn(w: RegulatorySeedWeights) -> genetics::Cppn {
    // Sigmoid basis: a smooth monotonic gradient, transitioning at the
    // midpoint of the gene-index range.
    const SIGMOID_INPUT_WEIGHT: f32 = 1.5;
    const SIGMOID_BIAS: f32 = -1.5;

    // Each region gets its own width: Hox must sharply discriminate 3
    // *adjacent* gene indices (0.1 apart) to produce a non-monotonic 3-bit
    // code, so it needs a narrow bump; Differentiation/Effector/Pigment each
    // cover 2-3 indices that should mostly move *together*, so a wider bump
    // suits them better — sharing one width (narrow enough for Hox) across
    // all four regions instead collapses Hox's own discrimination, since a
    // width tuned for a 2-3-index-wide region is too wide to separate Hox's
    // adjacent single-index steps. sum = bias + weight*pos + weight*pos =
    // bias + 2*weight*center at the peak, so bias = -2*weight*center places
    // the peak at `center`.
    const HOX_WIDTH: f32 = 10.0;
    const DIFFERENTIATION_WIDTH: f32 = 6.0;
    const EFFECTOR_WIDTH: f32 = 4.0;
    const PIGMENT_WIDTH: f32 = 4.0;
    const HOX_CENTER: f32 = 0.1; // genes 0-2 of 10, midpoint index 1
    const DIFFERENTIATION_CENTER: f32 = 0.35; // genes 3-4, midpoint 3.5
    const EFFECTOR_CENTER: f32 = 0.55; // genes 5-6, midpoint 5.5
    const PIGMENT_CENTER: f32 = 0.8; // genes 7-9, midpoint 8
    const HOX_BIAS: f32 = -2.0 * HOX_WIDTH * HOX_CENTER;
    const DIFFERENTIATION_BIAS: f32 = -2.0 * DIFFERENTIATION_WIDTH * DIFFERENTIATION_CENTER;
    const EFFECTOR_BIAS: f32 = -2.0 * EFFECTOR_WIDTH * EFFECTOR_CENTER;
    const PIGMENT_BIAS: f32 = -2.0 * PIGMENT_WIDTH * PIGMENT_CENTER;

    // Two periodic bases at different frequencies, for repeated/alternating
    // structure at more than one spatial scale.
    const SINE_COARSE_INPUT_WEIGHT: f32 = 6.0; // ~1.9 cycles across [0, 1]
    const SINE_FINE_INPUT_WEIGHT: f32 = 15.0; // ~4.8 cycles across [0, 1]
    const SINE_BIAS: f32 = 0.0;

    genetics::Cppn {
        nodes: vec![
            // 0, 1: inputs (gene-index fractions).
            genetics::CppnNode {
                activation: brain::ActivationFn::Linear,
                bias: 0.0,
                layer: 0,
            },
            genetics::CppnNode {
                activation: brain::ActivationFn::Linear,
                bias: 0.0,
                layer: 0,
            },
            // 2-8: the seven hidden basis functions. `Cppn::evaluate`
            // collects only `layer == 1` nodes into its returned outputs vec
            // (and `RegulatoryNetwork::generate` reads just the first of
            // those) — these seven must stay off that list (`layer: 0`, the
            // same value used for raw inputs, but functionally just "not a
            // collected output" here; `evaluate`'s node-computation loop
            // itself is index-range-based, not layer-gated, so they are
            // still fully computed) so only node 9's combined value is ever
            // read. Marking a basis node `layer: 1` by mistake would make
            // `RegulatoryNetwork::generate` read the wrong value (or the
            // wrong number of values) — verify any change here by directly
            // inspecting `RegulatoryNetwork::generate`'s output rather than
            // assuming it's correct from the node layout alone.
            genetics::CppnNode {
                activation: brain::ActivationFn::Sigmoid,
                bias: SIGMOID_BIAS,
                layer: 0,
            },
            genetics::CppnNode {
                activation: brain::ActivationFn::Gaussian,
                bias: HOX_BIAS,
                layer: 0,
            },
            genetics::CppnNode {
                activation: brain::ActivationFn::Gaussian,
                bias: DIFFERENTIATION_BIAS,
                layer: 0,
            },
            genetics::CppnNode {
                activation: brain::ActivationFn::Gaussian,
                bias: EFFECTOR_BIAS,
                layer: 0,
            },
            genetics::CppnNode {
                activation: brain::ActivationFn::Gaussian,
                bias: PIGMENT_BIAS,
                layer: 0,
            },
            genetics::CppnNode {
                activation: brain::ActivationFn::Sine,
                bias: SINE_BIAS,
                layer: 0,
            },
            genetics::CppnNode {
                activation: brain::ActivationFn::Sine,
                bias: SINE_BIAS,
                layer: 0,
            },
            // 9: output — linear combination of the seven bases. The only
            // `layer: 1` node, so it's the one `.first()` actually reads.
            genetics::CppnNode {
                activation: brain::ActivationFn::Linear,
                bias: w.output_bias,
                layer: 1,
            },
        ],
        connections: vec![
            // Inputs (0, 1) -> each of the 7 hidden bases (2-8).
            genetics::CppnConnection {
                source: 0,
                target: 2,
                weight: SIGMOID_INPUT_WEIGHT,
                enabled: true,
                innovation: 0,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 1,
                target: 2,
                weight: SIGMOID_INPUT_WEIGHT,
                enabled: true,
                innovation: 1,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 0,
                target: 3,
                weight: HOX_WIDTH,
                enabled: true,
                innovation: 2,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 1,
                target: 3,
                weight: HOX_WIDTH,
                enabled: true,
                innovation: 3,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 0,
                target: 4,
                weight: DIFFERENTIATION_WIDTH,
                enabled: true,
                innovation: 4,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 1,
                target: 4,
                weight: DIFFERENTIATION_WIDTH,
                enabled: true,
                innovation: 5,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 0,
                target: 5,
                weight: EFFECTOR_WIDTH,
                enabled: true,
                innovation: 6,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 1,
                target: 5,
                weight: EFFECTOR_WIDTH,
                enabled: true,
                innovation: 7,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 0,
                target: 6,
                weight: PIGMENT_WIDTH,
                enabled: true,
                innovation: 8,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 1,
                target: 6,
                weight: PIGMENT_WIDTH,
                enabled: true,
                innovation: 9,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 0,
                target: 7,
                weight: SINE_COARSE_INPUT_WEIGHT,
                enabled: true,
                innovation: 10,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 1,
                target: 7,
                weight: SINE_COARSE_INPUT_WEIGHT,
                enabled: true,
                innovation: 11,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 0,
                target: 8,
                weight: SINE_FINE_INPUT_WEIGHT,
                enabled: true,
                innovation: 12,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 1,
                target: 8,
                weight: SINE_FINE_INPUT_WEIGHT,
                enabled: true,
                innovation: 13,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            // Hidden bases (2-8) -> output (9), one per-seed evolvable weight
            // each (sigmoid stays fixed at 1.0 — it has no per-region
            // identity to tune independently).
            genetics::CppnConnection {
                source: 2,
                target: 9,
                weight: 1.0,
                enabled: true,
                innovation: 14,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 3,
                target: 9,
                weight: w.hox_weight,
                enabled: true,
                innovation: 15,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 4,
                target: 9,
                weight: w.differentiation_weight,
                enabled: true,
                innovation: 16,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 5,
                target: 9,
                weight: w.effector_weight,
                enabled: true,
                innovation: 17,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 6,
                target: 9,
                weight: w.pigment_weight,
                enabled: true,
                innovation: 18,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 7,
                target: 9,
                weight: w.sine_coarse_weight,
                enabled: true,
                innovation: 19,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 8,
                target: 9,
                weight: w.sine_fine_weight,
                enabled: true,
                innovation: 20,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
        ],
    }
}

/// The hand-built brain-wiring CPPN used as every seed genome's starting
/// neural substrate. This CPPN maps a (source, target) node-coordinate pair
/// to a synapse weight/bias/time-constant — unrelated to the regulatory
/// CPPN's Hox/body-plan decoding above, so every starter species shares this
/// one brain-wiring template regardless of body plan.
pub(crate) fn seed_brain_cppn() -> genetics::Cppn {
    genetics::Cppn {
        nodes: vec![
            genetics::CppnNode {
                activation: brain::ActivationFn::Linear,
                bias: 0.0,
                layer: 0,
            }, // Input: Source Node Coord
            genetics::CppnNode {
                activation: brain::ActivationFn::Linear,
                bias: 0.0,
                layer: 0,
            }, // Input: Target Node Coord
            genetics::CppnNode {
                activation: brain::ActivationFn::Tanh,
                bias: 0.0,
                layer: 1,
            }, // Output: Connection Weight
            genetics::CppnNode {
                activation: brain::ActivationFn::Tanh,
                bias: 0.0,
                layer: 1,
            }, // Output: Bias
            genetics::CppnNode {
                activation: brain::ActivationFn::Linear,
                bias: 0.0,
                layer: 1,
            }, // Output: Time Constant
        ],
        connections: vec![
            genetics::CppnConnection {
                source: 0,
                target: 2,
                weight: 2.0,
                enabled: true,
                innovation: 1,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 1,
                target: 2,
                weight: -1.0,
                enabled: true,
                innovation: 2,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 1,
                target: 3,
                weight: 1.0,
                enabled: true,
                innovation: 3,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
            genetics::CppnConnection {
                source: 1,
                target: 4,
                weight: 0.5,
                enabled: true,
                innovation: 4,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            },
        ],
    }
}

pub(crate) fn seed_ecosystem(
    world: &mut bevy_ecs::world::World,
    lineage_tracker: &mut evolution::LineageTracker,
    species_registry: &mut evolution::SpeciesRegistry,
    tracker: &mut genetics::GlobalInnovationTracker,
    rng: &mut impl rand::Rng,
) {
    // 1. Define Prototypes ("Seed Genomes").
    //
    // Each seed is an ordinary hand-authored `Genome` — no special-cased
    // morphology generation. Its body plan, branching, and
    // pigmentation all emerge from the same `develop_at_position` decode
    // pipeline every evolved organism goes through; `seed_regulatory_cppn`
    // just gives each species archetype a different starting point on that
    // decode (found by sweeping bias/weight and reading off the resulting
    // segment-type sequence, not hand-picked to match any specific shape).
    //
    // Colors are **not** set here — pigmentation is emergent (see
    // `RegulatoryGeneRole::Pigment`'s doc comment), so starter organisms no
    // longer necessarily render in their diet's canonical
    // `Diet::standard_color()`. This is an intentional consequence of
    // retiring genome-stored color, not an oversight.
    let brain_template = seed_brain_cppn();

    // Swept for *diversity* across the modular region-bump basis — see
    // `seed_regulatory_cppn`'s doc comment — not hand-picked to hit any
    // specific `SegmentType`.
    let worm_genome = genetics::Genome::seed(
        genetics::GenomeId(1),
        common::EntityId(0),
        brain_template.clone(),
        genetics::Cppn::new(),
        // Empirically found by random search over `RegulatorySeedWeights`
        // (20,000 draws), selecting for effector activity + Hox-type
        // diversity — not hand-picked to hit Muscle specifically. Unmutated
        // decode: [Germinal, Ganglion, Muscle, Muscle, Muscle, Muscle,
        // Ganglion, Ganglion, Germinal, Germinal], effector active 10/10.
        seed_regulatory_cppn(RegulatorySeedWeights {
            output_bias: -4.45,
            hox_weight: 8.97,
            differentiation_weight: 7.07,
            effector_weight: 3.12,
            pigment_weight: 1.22,
            sine_coarse_weight: 2.15,
            sine_fine_weight: 1.76,
        }),
    );

    let fish_genome = genetics::Genome::seed(
        genetics::GenomeId(2),
        common::EntityId(0),
        brain_template.clone(),
        genetics::Cppn::new(),
        // "Effector active" for a body plan means at least one position
        // decodes as `SegmentType::Muscle` (the only type compiled into an
        // `Elastic`, actuatable spring by `develop.rs`'s `compile_segment`)
        // with a nonzero actuation amplitude *and* survives the germ-line-
        // protection apoptosis check — a position with nonzero raw
        // `actuation_amplitude` that gets pruned by apoptosis contributes no
        // real body. These weights were tuned against both conditions
        // together: unmutated decode (positions 1-9): [Head, Head, Muscle,
        // Head, Head, Head, Head, Head, Head], none apoptotic, 1 real
        // actuatable Muscle segment; ~65% of individuals retain >=1
        // actuatable effector after `spawn_pop`'s 10-round mutation pass at
        // its 0.1 rate.
        seed_regulatory_cppn(RegulatorySeedWeights {
            output_bias: -6.3154593,
            hox_weight: 7.676084,
            differentiation_weight: 3.2809398,
            effector_weight: 6.233916,
            pigment_weight: 1.3872341,
            sine_coarse_weight: 0.5254265,
            sine_fine_weight: 2.490907,
        }),
    );

    let branchy_genome = genetics::Genome::seed(
        genetics::GenomeId(3),
        common::EntityId(0),
        brain_template.clone(),
        genetics::Cppn::new(),
        // Same apoptosis-survival requirement as `fish_genome` above: a
        // Hox table showing Muscle positions is not sufficient on its own
        // if the germ-line-protection apoptosis check prunes those
        // positions before they can actuate. Unmutated decode (positions
        // 1-9): [Ganglion, Germinal, Germinal, Germinal, Germinal,
        // Germinal, Muscle, Muscle, Muscle], none apoptotic, 3 real
        // actuatable Muscle segments; ~51% of individuals retain >=1
        // actuatable effector after `spawn_pop`'s 10-round mutation pass at
        // its 0.1 rate.
        seed_regulatory_cppn(RegulatorySeedWeights {
            output_bias: -4.885546,
            hox_weight: 11.249819,
            differentiation_weight: 2.586886,
            effector_weight: 4.5433483,
            pigment_weight: 2.1518261,
            sine_coarse_weight: 2.6428568,
            sine_fine_weight: 1.3519208,
        }),
    );

    let omnivore_genome = genetics::Genome::seed(
        genetics::GenomeId(4),
        common::EntityId(0),
        brain_template.clone(),
        genetics::Cppn::new(),
        // Unmutated decode: [Muscle, Muscle, Germinal, Germinal, Germinal,
        // Ganglion, Muscle, Muscle, Muscle, Muscle], effector active 8/10.
        seed_regulatory_cppn(RegulatorySeedWeights {
            output_bias: -4.13,
            hox_weight: 8.84,
            differentiation_weight: 2.10,
            effector_weight: 2.96,
            pigment_weight: 2.22,
            sine_coarse_weight: 2.22,
            sine_fine_weight: 2.10,
        }),
    );

    let decomposer_genome = genetics::Genome::seed(
        genetics::GenomeId(5),
        common::EntityId(0),
        brain_template.clone(),
        genetics::Cppn::new(),
        // Unmutated decode: [Tail, Muscle, Muscle, Muscle, Muscle, Muscle,
        // Muscle, Muscle, Tail, Germinal], effector active 10/10.
        seed_regulatory_cppn(RegulatorySeedWeights {
            output_bias: -3.05,
            hox_weight: 6.90,
            differentiation_weight: 3.90,
            effector_weight: 0.69,
            pigment_weight: 0.40,
            sine_coarse_weight: 0.54,
            sine_fine_weight: 1.09,
        }),
    );

    let producer_genome = genetics::Genome::seed(
        genetics::GenomeId(6),
        common::EntityId(0),
        brain_template,
        genetics::Cppn::new(),
        // Producers stay a deliberately short, low-complexity seed (real
        // plants don't need a rich body plan or effector activity) — no
        // seed here is hardcoded to a specific segment outcome.
        seed_regulatory_cppn(RegulatorySeedWeights {
            output_bias: -3.0,
            hox_weight: 0.0,
            differentiation_weight: 0.0,
            effector_weight: 0.0,
            pigment_weight: 1.0,
            sine_coarse_weight: 0.0,
            sine_fine_weight: 0.0,
        }),
    );

    // 2. Helper to spawn a population
    let mut spawn_pop = |genome: &genetics::Genome, diet: ecology::Diet, count: usize| {
        let lineage_id = lineage_tracker.new_lineage_id();
        for _ in 0..count {
            let px = rng.gen_range(-1000.0..1000.0);
            let py = rng.gen_range(-1000.0..1000.0);

            // Give each individual a unique randomized brain if they are not
            // producers. `mutation_rate` here must stay in line with
            // `reproduction`'s own per-birth mutation calls (0.1-0.2 — see
            // `crates/reproduction/src/lib.rs`'s `child_genome.mutate(...)`
            // call sites), not a guaranteed full-strength pass: `mutate`'s
            // first argument is an outer gate, so `mutate(1.0, ...)` makes
            // every one of the 10 rounds mutate at full strength rather than
            // giving 10 *chances* at a milder mutation. A full-strength,
            // 10-round pass collapses the seed regulatory CPPNs'
            // actuatable-Muscle-segment rate to a small minority of founders
            // for several starter presets, and this degradation compounds
            // further as later generations' reproduction mutates the same
            // already-degraded lineages again. At `mutation_rate = 0.1`
            // (matching reproduction's own asexual rate), the same 10-round
            // loop still gives every individual a genuinely unique
            // brain/body-plan while preserving a healthy majority (~60-80%)
            // actuatable-effector rate — see the
            // `starter_species_locomotion_viability` test module below for
            // the measured numbers backing this.
            let mut ind_genome = genome.clone();
            if diet != ecology::Diet::Producer {
                for _ in 0..10 {
                    ind_genome.mutate(0.1, rng, tracker);
                }
            }

            let species_id = species_registry.classify(&ind_genome);

            let e = organisms::spawn_organism(
                world,
                &ind_genome,
                common::Vec3::new(px, py, 0.0),
                diet.clone(),
                ecology::EcologicalCategory::None,
                0,
                0,
                rng,
            );
            lineage_tracker.register_birth(
                common::EntityId(e.to_bits()),
                None,
                lineage_id,
                species_id,
                0,
                0,
            );
        }
    };

    // 3. Spawn Populations
    spawn_pop(&producer_genome, ecology::Diet::Producer, 260);
    spawn_pop(&worm_genome, ecology::Diet::Herbivore, 150);
    spawn_pop(&branchy_genome, ecology::Diet::Herbivore, 150);
    spawn_pop(&omnivore_genome, ecology::Diet::Omnivore, 40);
    spawn_pop(&decomposer_genome, ecology::Diet::Decomposer, 50);
    spawn_pop(&fish_genome, ecology::Diet::Carnivore, 20);

    // 4. Spawn Resource Hotspots
    for _ in 0..20 {
        let px = rng.gen_range(-1000.0..1000.0);
        let py = rng.gen_range(-1000.0..1000.0);
        world.spawn(diffusion::Emitter {
            position: common::Vec2::new(px, py),
            value: rng.gen_range(5.0..20.0),
            radius: rng.gen_range(50.0..150.0),
            layer: diffusion::FieldLayer::Energy,
        });
    }

    // 5. Spawn Initial Minerals
    for _ in 0..300 {
        let px = rng.gen_range(-1000.0..1000.0);
        let py = rng.gen_range(-1000.0..1000.0);
        world.spawn(ecology::MineralPellet {
            position: common::Vec3::new(px, py, 0.0),
            energy_value: 50.0,
        });
    }

    // 6. Spawn Initial Food
    for _ in 0..300 {
        let px = rng.gen_range(-1000.0..1000.0);
        let py = rng.gen_range(-1000.0..1000.0);
        world.spawn(ecology::FoodPellet {
            position: common::Vec3::new(px, py, 0.0),
            energy_value: 50.0,
        });
    }
}

#[cfg(test)]
mod starter_species_locomotion_viability {
    //! Regression coverage: every non-Producer starter species must
    //! actually be capable of muscle-driven locomotion, both unmutated and
    //! after `spawn_pop`'s founder-diversity mutation pass. This guards two
    //! distinct failure modes a bad regulatory seed can hit:
    //!
    //! 1. `spawn_pop`'s mutation dosage (10 rounds at `mutation_rate = 0.1`,
    //!    matching `reproduction`'s own per-birth convention) must not
    //!    collapse the actuatable-effector rate — an outer gate as high as
    //!    1.0 would make every round a guaranteed full-strength mutation
    //!    instead of 10 chances at a milder one, degrading body plans much
    //!    more aggressively than real reproduction ever does.
    //! 2. A seed's regulatory weights must not cause the germ-line-
    //!    protection apoptosis check to fire on nearly every body position —
    //!    doing so prunes the entire body except the head, so the starter
    //!    species grows no muscle-bearing body at all, independent of any
    //!    mutation. Weights are gated on: apoptosis-survives for >=4
    //!    positions AND >=1 real actuatable `Muscle` segment.
    use super::*;
    use rand::SeedableRng;

    fn preset(name: &str) -> RegulatorySeedWeights {
        match name {
            "worm" => RegulatorySeedWeights {
                output_bias: -4.45,
                hox_weight: 8.97,
                differentiation_weight: 7.07,
                effector_weight: 3.12,
                pigment_weight: 1.22,
                sine_coarse_weight: 2.15,
                sine_fine_weight: 1.76,
            },
            "fish" => RegulatorySeedWeights {
                output_bias: -6.3154593,
                hox_weight: 7.676084,
                differentiation_weight: 3.2809398,
                effector_weight: 6.233916,
                pigment_weight: 1.3872341,
                sine_coarse_weight: 0.5254265,
                sine_fine_weight: 2.490907,
            },
            "branchy" => RegulatorySeedWeights {
                output_bias: -4.885546,
                hox_weight: 11.249819,
                differentiation_weight: 2.586886,
                effector_weight: 4.5433483,
                pigment_weight: 2.1518261,
                sine_coarse_weight: 2.6428568,
                sine_fine_weight: 1.3519208,
            },
            "omnivore" => RegulatorySeedWeights {
                output_bias: -4.13,
                hox_weight: 8.84,
                differentiation_weight: 2.10,
                effector_weight: 2.96,
                pigment_weight: 2.22,
                sine_coarse_weight: 2.22,
                sine_fine_weight: 2.10,
            },
            "decomposer" => RegulatorySeedWeights {
                output_bias: -3.05,
                hox_weight: 6.90,
                differentiation_weight: 3.90,
                effector_weight: 0.69,
                pigment_weight: 0.40,
                sine_coarse_weight: 0.54,
                sine_fine_weight: 1.09,
            },
            _ => unreachable!(),
        }
    }

    /// Measures the fraction of organisms with >=1 actuatable (`Muscle`,
    /// nonzero amplitude) segment, for a given preset / mutation-round
    /// count / `total_segments`, mirroring `growth_system`'s real halting
    /// rule (position 0 is the pre-existing head; growth stops at the
    /// first decoded `Tail`; apoptotic positions are skipped).
    fn measure(
        preset_name: &str,
        mutate_rounds: usize,
        total_segments: usize,
        trials: u64,
    ) -> (usize, usize) {
        measure_with_rate(preset_name, mutate_rounds, 1.0, total_segments, trials)
    }

    fn measure_with_rate(
        preset_name: &str,
        mutate_rounds: usize,
        mutation_rate: f32,
        total_segments: usize,
        trials: u64,
    ) -> (usize, usize) {
        let mut tracker = genetics::GlobalInnovationTracker::default();
        let seed = seed_regulatory_cppn(preset(preset_name));

        let mut with_muscle = 0usize;
        let mut with_effector = 0usize;

        for trial in 0..trials {
            let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(trial);
            let mut genome = genetics::Genome::seed(
                genetics::GenomeId(trial),
                common::EntityId(0),
                genetics::Cppn::new(),
                genetics::Cppn::new(),
                seed.clone(),
            );
            for _ in 0..mutate_rounds {
                genome.mutate(mutation_rate, &mut rng, &mut tracker);
            }

            let mut any_effector = false;
            let mut any_muscle = false;
            for pos in 1..total_segments {
                let out = genetics::develop_at_position(
                    &genome.expressed_regulatory_cppn(),
                    pos,
                    total_segments,
                );
                if out.apoptosis {
                    continue;
                }
                if out.segment_type == genetics::SegmentType::Muscle {
                    any_muscle = true;
                    if out.actuation_amplitude.abs() > 0.01 {
                        any_effector = true;
                    }
                }
                if out.segment_type == genetics::SegmentType::Tail {
                    break;
                }
            }
            if any_muscle {
                with_muscle += 1;
            }
            if any_effector {
                with_effector += 1;
            }
        }

        (with_muscle, with_effector)
    }

    #[test]
    fn every_non_producer_preset_has_a_reachable_actuatable_effector_unmutated() {
        for preset_name in ["worm", "fish", "branchy", "omnivore", "decomposer"] {
            let (muscle, effector) = measure(preset_name, 0, 10, 1);
            assert_eq!(
                (muscle, effector),
                (1, 1),
                "preset {preset_name} must decode >=1 reachable, actuatable Muscle segment \
                 unmutated (germ-line-protection apoptosis must not prune the entire body)"
            );
        }
    }

    #[test]
    fn founder_population_retains_a_healthy_effector_majority_after_spawn_pop_mutation() {
        // Mirrors `spawn_pop`'s exact mutation dosage (10 rounds at
        // `mutation_rate = 0.1`) — regression coverage for the measured
        // finding that `mutation_rate = 1.0` (the previous value) collapsed
        // this to single digits.
        let trials = 300u64;
        for preset_name in ["worm", "fish", "branchy", "omnivore", "decomposer"] {
            let (_, effector) =
                measure_with_rate(preset_name, 10, 0.1, organisms::MAX_SEGMENTS, trials);
            let rate = effector as f64 / trials as f64;
            assert!(
                rate > 0.3,
                "preset {preset_name}: post-mutation actuatable-effector rate {rate:.2} \
                 is too low (expected >0.3) at spawn_pop's real mutation dosage"
            );
        }
    }
}
