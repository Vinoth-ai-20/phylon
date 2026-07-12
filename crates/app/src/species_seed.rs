//! Starter-species genome/CPPN seeding (Phase 9, P9.6 file decomposition —
//! extracted from `app.rs`, which bundled ECS/resource wiring, GPU
//! bring-up, entity picking, and this seeding logic in one file). Builds
//! the hand-authored regulatory/brain CPPN templates and the founding
//! population's `Genome`s that `PhylonApp::new` spawns via `world::World`
//! — see `seed_ecosystem`'s own doc comment for why none of this is
//! special-cased morphology generation (ADR-P3-02).

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
    /// region SX-2a's first architecture starved (see this function's doc
    /// comment's "second problem" section).
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
/// (Phase 5, SX-2a — see `PHASE5_SX_ROADMAP.md` §11's full architectural
/// analysis, ADR-P5-06 and ADR-P5-07).
///
/// **First problem this replaces (ADR-P5-06):** the very first seed was a
/// single `Linear` output node with one incoming connection — since
/// `RegulatoryNetwork::generate` derives every gene's bias and every
/// gene-pair's edge weight from a *linear* function of gene index, its
/// output was strictly monotonic in gene index. Since a 3-bit Hox code is
/// read off three specific, adjacent gene indices, a monotonic bias function
/// can only ever threshold to a non-decreasing or non-increasing bit
/// sequence (`000,001,011,111` or `000,100,110,111`) — six of the eight
/// possible `SegmentType` codes, including `Muscle` (`010`), were
/// **structurally unreachable**, for any choice of the old `(bias, weight)`
/// parameters. Measured directly (§11): the unmutated "mostly Muscle body"
/// seed decoded `Germinal` at 100% of positions, and even the real
/// spawn-time mutation regime never once produced a `Muscle` segment across
/// 30 independent trials.
///
/// **Second problem this replaces (ADR-P5-07):** the first fix added a
/// single `Sigmoid` + `Gaussian` + `Sine` basis trio, with the `Gaussian`
/// bump's *one* fixed center tuned to land on the Hox region (gene-index
/// fraction ≈0.1) so `Muscle` became reachable. That single bump was the
/// whole fix's local-activation budget — every other gene *role* (crucially
/// `Effector`, at index fraction ≈0.55) sat far outside the bump's reach and
/// collapsed to whatever the leftover Sigmoid+Sine terms gave, combined with
/// the strongly negative `output_bias` needed to suppress off-peak Hox bits.
/// Measured directly in a real headless run (§11): **363 of 364** sampled
/// non-Producer organisms had zero actuatable effector springs, even though
/// the isolated per-seed measurement (which mutates the *entire* CPPN,
/// relocating the bump over generations) showed 31.2% `Muscle` reachability.
/// The founding population never benefits from that drift — it uses the
/// seed unmutated, where the one bump structurally cannot reach `Effector`.
///
/// **The fix is modular, not another single retuned bump.** Gene *role* is
/// already fully determined by gene *position* under the current fixed
/// `REGULATORY_GENE_ROLES` table (Hox = 0-2, Differentiation = 3-4, Effector
/// = 5-6, Pigment = 7-9) — there's no missing input dimension, only
/// insufficient local-activation *capacity*. So this CPPN gives each region
/// its own independently-weighted `Gaussian` bump, centered at that region's
/// index-fraction midpoint, alongside the existing shared `Sigmoid`
/// (monotonic gradient) and *two* `Sine` bases at different frequencies
/// (coarse + fine periodic/repeated structure, rather than one). Every
/// region's bump can be independently strengthened, weakened, or inverted
/// (a negative weight is a local *repressor*, not just an activator) via its
/// own `RegulatorySeedWeights` field, without starving any other region —
/// this is what makes the fix "modular regulation, one evolvable genome"
/// rather than a minimal patch: tuning `effector_weight` can no longer come
/// at the expense of `hox_weight`, because they're separate connections.
///
/// This scope's four bumps match today's four fixed `RegulatoryGeneRole`
/// variants; a future role (organogenesis, physiology — explicitly listed as
/// future compatibility targets) would need one more region bump added here,
/// the same way this fix added four to the first version's one — not a
/// restructuring, since the pattern ("one independently-weighted local bump
/// per role region") generalizes directly.
///
/// All bases still combine at one `Linear` output node, so
/// `RegulatoryNetwork::generate`'s existing calling convention
/// (`evaluate(&[idx, idx])` for bias, `evaluate(&[i/total, j/total])` for
/// edge weight) is completely unchanged — this is a richer function being
/// queried the same way, not a change to how genes/edges are derived, and
/// nothing here reads `REGULATORY_GENE_ROLES` at runtime (the region centers
/// below are constants derived from that table by hand, not a live lookup) —
/// deliberately, so this stays a plain, cheap, deterministic `Cppn` rather
/// than a construction that depends on the table's exact contents at
/// call-time.
///
/// **Deliberately not tuned toward any specific `SegmentType`.** This
/// function has no `Muscle`-specific or `Fin`-specific logic anywhere — the
/// four region weights and two sine weights are swept per starter species
/// purely for *diversity* (see each call site's own comment for what was
/// empirically observed, not targeted), and the resulting network remains an
/// ordinary, evolvable `Cppn` — mutation's existing `mutate_add_node`/
/// `mutate_add_connection`/per-connection jitter operate on it exactly as
/// they would any other genome, with nothing special-cased for starter
/// organisms (ADR-P3-02).
pub(crate) fn seed_regulatory_cppn(w: RegulatorySeedWeights) -> genetics::Cppn {
    // Sigmoid basis: a smooth monotonic gradient, transitioning at the
    // midpoint of the gene-index range.
    const SIGMOID_INPUT_WEIGHT: f32 = 1.5;
    const SIGMOID_BIAS: f32 = -1.5;

    // Each region gets its own width: Hox must sharply discriminate 3
    // *adjacent* gene indices (0.1 apart) to produce a non-monotonic 3-bit
    // code, so it needs a narrow bump; Differentiation/Effector/Pigment each
    // cover 2-3 indices that should mostly move *together*, so a wider bump
    // (which was tried shared at width 4.0 for all four and measured to
    // collapse Hox discrimination — see §11's ADR-P5-07 entry) suits them
    // better. sum = bias + weight*pos + weight*pos = bias + 2*weight*center
    // at the peak, so bias = -2*weight*center places the peak at `center`.
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
            // read. Getting this wrong (marking a basis node `layer: 1`) was
            // this milestone's first implementation's own first bug (§11) —
            // caught by directly inspecting `RegulatoryNetwork::generate`'s
            // output, not assumed fixed.
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

/// The hand-built brain-wiring CPPN previously baked into `new_hox_driven`
/// (retired, Phase 3 M4) — unrelated to Hox/body-plan decoding, so carried
/// over unchanged as every seed genome's starting neural substrate.
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
    // 1. Define Prototypes ("Seed Genomes" — Phase 3 M4, replacing the
    // retired `new_hox_driven`/`HoxSequence` template mechanism).
    //
    // Each seed is an ordinary hand-authored `Genome` — no special-cased
    // morphology generation (ADR-P3-02). Its body plan, branching, and
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

    // Phase 5, SX-2a (ADR-P5-07): swept for *diversity* across the modular
    // region-bump basis — see `seed_regulatory_cppn`'s doc comment — not
    // hand-picked to hit any specific `SegmentType`. Measured diversity
    // across all six, including effector-activation rate, is recorded in
    // `PHASE5_SX_ROADMAP.md` §11.
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
        // Phase 9, Goal 2 root-cause audit (re-tuned; see
        // `phase9_movement_root_cause_diagnostic` below): the previous
        // weights here decoded [Tail, Torso, Torso, Head, Torso, Torso,
        // Torso, Tail, Tail, Tail] — the "effector active 10/10" this
        // comment used to claim only ever measured raw
        // `actuation_amplitude != 0` at every position (every position
        // always produces *some* amplitude value, per
        // `develop_at_position`), never whether that position actually
        // decoded as `SegmentType::Muscle` (the only type that becomes an
        // `Elastic`, actuatable spring — see `develop.rs`/
        // `compile_segment`). The real bug this decode had: `Tail` at
        // position 0 (i.e. `growth_system`'s first real segment, position
        // 1, was still `Torso`, harmless) but every position had
        // `apoptosis = true` — DEF-002's germ-line-protection pruning fired
        // everywhere, so a real fish organism grew *zero* body past its
        // head, regardless of Hox/Muscle content. Re-tuned unmutated
        // decode (positions 1-9): [Head, Head, Muscle, Head, Head, Head,
        // Head, Head, Head], none apoptotic, 1 real actuatable Muscle
        // segment; ~65% of individuals retain >=1 actuatable effector after
        // `spawn_pop`'s 10-round mutation pass at the corrected 0.1 rate.
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
        // Phase 9, Goal 2 root-cause audit (re-tuned; see
        // `phase9_movement_root_cause_diagnostic` below): the previous
        // weights here had the exact same DEF-002 apoptosis defect as
        // `fish_genome` above — every position except the two `Germinal`
        // ends had `apoptosis = true`, so a real branchy organism also grew
        // zero body past its head despite the Hox table showing 2 Muscle
        // positions. Re-tuned unmutated decode (positions 1-9): [Ganglion,
        // Germinal, Germinal, Germinal, Germinal, Germinal, Muscle, Muscle,
        // Muscle], none apoptotic, 3 real actuatable Muscle segments; ~51%
        // of individuals retain >=1 actuatable effector after `spawn_pop`'s
        // 10-round mutation pass at the corrected 0.1 rate.
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
            // call sites), not a guaranteed full-strength pass: measured
            // directly (Phase 9 Goal 2 root-cause audit — see
            // `phase9_movement_root_cause_diagnostic` below), the previous
            // `mutate(1.0, ...)` x10 — an outer gate of 1.0 means every one
            // of the 10 rounds mutates at full strength, not 10 *chances* at
            // a milder mutation — collapsed the seed regulatory CPPNs'
            // actuatable-Muscle-segment rate from 100% down to ~11-23% for
            // 3 of 5 starter presets (worm/omnivore/decomposer), which is
            // this session's own real headless `PHYLON_MOTION_DIAGNOSTIC=1`
            // observation of 0 actuatable effectors across every sampled
            // organism, compounded further by per-generation reproduction
            // mutating the same already-degraded lineages again. At
            // `mutation_rate = 0.1` (matching reproduction's own asexual
            // rate), the same 10-round loop still gives every individual a
            // genuinely unique brain/body-plan while preserving a healthy
            // majority (~60-80%) actuatable-effector rate.
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
    //! Regression coverage for Phase 9 Goal 2's root-cause finding: every
    //! non-Producer starter species must actually be capable of
    //! muscle-driven locomotion, both unmutated and after `spawn_pop`'s
    //! founder-diversity mutation pass. Two independent, measured defects
    //! were found and fixed here (not guessed — see each call site's own
    //! comment in `seed_ecosystem` for the full measurement):
    //!
    //! 1. `spawn_pop` mutated every founder genome 10 times at
    //!    `mutation_rate = 1.0` (a guaranteed full-strength pass each
    //!    round) before ever spawning it — 100x more aggressive than
    //!    `reproduction`'s own per-birth convention (0.1-0.2, one call).
    //!    Measured effect: collapsed the actuatable-effector rate from
    //!    100% to single digits for otherwise-healthy presets. Fixed by
    //!    matching reproduction's own 0.1 rate.
    //! 2. `fish_genome`/`branchy_genome`'s regulatory seed weights caused
    //!    DEF-002's germ-line-protection apoptosis check to fire on nearly
    //!    every body position, pruning the entire body except the head —
    //!    these two starter species grew no muscle-bearing body at all,
    //!    independent of any mutation. Fixed by re-tuning their weights
    //!    (search anchored near the originals, gated on: apoptosis-survives
    //!    for >=4 positions AND >=1 real actuatable `Muscle` segment).
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
                 unmutated (DEF-002 apoptosis must not prune the entire body)"
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
