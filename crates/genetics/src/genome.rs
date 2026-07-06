use crate::cppn;
use crate::cppn::{Cppn, CppnConnection, CppnNode, GlobalInnovationTracker, DEFAULT_MUTATION_RATE};
use crate::hox::HoxSequence;
use crate::types::{GenomeId, Ploidy};
use common::EntityId;
use serde::{Deserialize, Serialize};

/// Current `Genome::schema_version`. Bumped from 3 to 4 by the addition of
/// `regulatory_cppn` (Phase 3, M1 — see `PHASE3_ROADMAP.md`'s ADR-P3-01).
/// No migration path exists from schema 3 or earlier, matching the
/// project's established policy (bump and document the break; see
/// `IMPLEMENTATION_STATUS.md`'s ADR-010).
pub const GENOME_SCHEMA_VERSION: u32 = 4;

/// A diploid genome's second allele set — present only when
/// `Genome::ploidy` is [`Ploidy::Diploid`]. Mirrors `Genome::brain_cppn`/
/// `morph_cppn`/`regulatory_cppn` exactly, so the alleles can be
/// compared/blended gene-for-gene (matched by CPPN connection innovation
/// number, the same scheme [`Cppn::crossover`] already uses).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiploidAlleles {
    /// The second allele's brain-wiring CPPN.
    pub brain_cppn: Cppn,
    /// The second allele's body-morphology CPPN.
    pub morph_cppn: Cppn,
    /// The second allele's regulatory-network-generating CPPN (Phase 3, M1).
    pub regulatory_cppn: Cppn,
}

/// The genome of an organism, containing independent CPPNs for body morphology and neural wiring.
#[derive(bevy_ecs::prelude::Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Genome {
    /// Schema version for serialization compatibility (currently
    /// [`GENOME_SCHEMA_VERSION`]).
    pub schema_version: u32,
    /// Unique identifier for this genome sequence.
    pub id: GenomeId,
    /// The ID of the organism that created this genome (for lineage tracking).
    pub origin: EntityId,
    /// Ploidy level (haploid or diploid).
    pub ploidy: Ploidy,
    /// The CPPN responsible for neural wiring topology.
    ///
    /// For a [`Ploidy::Diploid`] genome, this is the *first* allele — see
    /// [`Genome::expressed_brain_cppn`] for the dominance-resolved phenotype
    /// actually used by growth/brain evaluation. Direct field access (as UI
    /// panels currently do, for display) always sees this first allele only.
    pub brain_cppn: Cppn,
    /// The CPPN responsible for L-System body morphology growth.
    ///
    /// Same first-allele-only caveat as `brain_cppn` — see
    /// [`Genome::expressed_morph_cppn`].
    pub morph_cppn: Cppn,
    /// The CPPN generating a `regulatory::RegulatoryNetwork`'s weights
    /// (Phase 3, M1 — see `PHASE3_ROADMAP.md`'s ADR-P3-01). Not yet wired
    /// to growth/crossover/mutation/speciation-distance — those are
    /// separate, later milestones (M2, M4, M7 respectively).
    ///
    /// Same first-allele-only caveat as `brain_cppn` — see
    /// [`Genome::expressed_regulatory_cppn`].
    pub regulatory_cppn: Cppn,
    /// The second allele set, present only when `ploidy` is
    /// [`Ploidy::Diploid`]. `None` for haploid genomes (the common case
    /// today) — this field was added in schema version 3, so bincode
    /// snapshots saved under schema version 2 cannot be deserialized
    /// against the current `Genome` layout (no migration path exists yet;
    /// see `docs` or the implementation roadmap's Epic 7 note).
    pub second_allele: Option<DiploidAlleles>,
    /// Optional explicit Hox body-plan sequence.
    ///
    /// When `Some`, the growth system reads the body plan directly from this
    /// sequence rather than querying the morph CPPN.
    pub hox: Option<HoxSequence>,
}

impl Genome {
    /// Creates a minimal haploid genome.
    pub fn new_minimal(id: GenomeId, origin: EntityId) -> Self {
        Self {
            schema_version: GENOME_SCHEMA_VERSION,
            id,
            origin,
            ploidy: Ploidy::Haploid,
            brain_cppn: Cppn::new(),
            morph_cppn: Cppn::new(),
            regulatory_cppn: Cppn::new(),
            second_allele: None,
            hox: None,
        }
    }

    /// Creates a diploid genome by pairing two independently-constructed
    /// haploid genomes' CPPNs as its two alleles — `self`'s fields (`id`,
    /// `origin`, `hox`) are kept; only `allele_a`'s and `allele_b`'s CPPNs
    /// are used.
    ///
    /// Each allele tuple is `(brain_cppn, morph_cppn, regulatory_cppn)` —
    /// extended in Phase 3, M1 to carry the third CPPN so a diploid genome
    /// is diploid at every gene locus, not just the original two.
    ///
    /// This is the only way to construct a [`Ploidy::Diploid`] genome today;
    /// `reproduction::reproduction_system`'s sexual-mating path still
    /// produces haploid children via [`Genome::crossover`] (blending, not
    /// preserving both parents' full genomes) — deciding whether mating
    /// should itself produce diploid offspring is a separate product
    /// decision, out of scope for activating the type itself.
    pub fn new_diploid(
        id: GenomeId,
        origin: EntityId,
        hox: Option<HoxSequence>,
        allele_a: (Cppn, Cppn, Cppn),
        allele_b: (Cppn, Cppn, Cppn),
    ) -> Self {
        Self {
            schema_version: GENOME_SCHEMA_VERSION,
            id,
            origin,
            ploidy: Ploidy::Diploid,
            brain_cppn: allele_a.0,
            morph_cppn: allele_a.1,
            regulatory_cppn: allele_a.2,
            second_allele: Some(DiploidAlleles {
                brain_cppn: allele_b.0,
                morph_cppn: allele_b.1,
                regulatory_cppn: allele_b.2,
            }),
            hox,
        }
    }

    /// The brain CPPN actually used for neural evaluation.
    ///
    /// For a haploid genome (or a diploid genome missing its second
    /// allele — shouldn't happen via the public constructors, but handled
    /// gracefully), this is just `brain_cppn`. For a diploid genome, this
    /// is the *expressed* CPPN: per-connection dominance resolved by
    /// matching innovation numbers (the same scheme [`Cppn::crossover`]
    /// uses) and keeping whichever allele's connection has the larger
    /// `|weight|` — a stronger effect dominating a weaker one, standing in
    /// for classical dominant/recessive expression without requiring an
    /// arbitrary, RNG-dependent dominance assignment.
    pub fn expressed_brain_cppn(&self) -> std::borrow::Cow<'_, Cppn> {
        match &self.second_allele {
            Some(alleles) => {
                std::borrow::Cow::Owned(express_diploid(&self.brain_cppn, &alleles.brain_cppn))
            }
            None => std::borrow::Cow::Borrowed(&self.brain_cppn),
        }
    }

    /// The morphology CPPN actually used for procedural growth — see
    /// [`Genome::expressed_brain_cppn`] for the dominance rule.
    pub fn expressed_morph_cppn(&self) -> std::borrow::Cow<'_, Cppn> {
        match &self.second_allele {
            Some(alleles) => {
                std::borrow::Cow::Owned(express_diploid(&self.morph_cppn, &alleles.morph_cppn))
            }
            None => std::borrow::Cow::Borrowed(&self.morph_cppn),
        }
    }

    /// The regulatory-network-generating CPPN actually used once wired to
    /// growth (Phase 3, M4+) — see [`Genome::expressed_brain_cppn`] for the
    /// dominance rule. Not yet called anywhere in this milestone; provided
    /// now so diploid genomes have a well-defined answer for this locus as
    /// soon as something needs it, rather than a gap discovered later.
    pub fn expressed_regulatory_cppn(&self) -> std::borrow::Cow<'_, Cppn> {
        match &self.second_allele {
            Some(alleles) => std::borrow::Cow::Owned(express_diploid(
                &self.regulatory_cppn,
                &alleles.regulatory_cppn,
            )),
            None => std::borrow::Cow::Borrowed(&self.regulatory_cppn),
        }
    }

    /// NEAT-style genetic-distance between two genomes' expressed phenotypes
    /// — the sum of the brain and morphology CPPNs' compatibility distances
    /// (see [`Cppn::compatibility_distance`]). Diploid genomes are compared
    /// on their expressed (dominance-resolved) CPPNs, so distance reflects
    /// phenotype, not raw allele storage. Used by `evolution::SpeciesRegistry`
    /// to cluster organisms into species without a hardcoded `SpeciesId`.
    pub fn distance(&self, other: &Genome) -> f32 {
        let brain_d = self.expressed_brain_cppn().compatibility_distance(
            &other.expressed_brain_cppn(),
            cppn::EXCESS_COEFFICIENT,
            cppn::DISJOINT_COEFFICIENT,
            cppn::WEIGHT_DIFF_COEFFICIENT,
        );
        let morph_d = self.expressed_morph_cppn().compatibility_distance(
            &other.expressed_morph_cppn(),
            cppn::EXCESS_COEFFICIENT,
            cppn::DISJOINT_COEFFICIENT,
            cppn::WEIGHT_DIFF_COEFFICIENT,
        );
        brain_d + morph_d
    }

    /// Creates a deterministic genome with a pre-defined Hox sequence.
    pub fn new_hox_driven(id: GenomeId, origin: EntityId, hox: HoxSequence) -> Self {
        Self {
            schema_version: GENOME_SCHEMA_VERSION,
            id,
            origin,
            ploidy: Ploidy::Haploid,
            brain_cppn: Cppn {
                nodes: vec![
                    CppnNode {
                        activation: brain::ActivationFn::Linear,
                        bias: 0.0,
                        layer: 0,
                    }, // Input: Source Node Coord
                    CppnNode {
                        activation: brain::ActivationFn::Linear,
                        bias: 0.0,
                        layer: 0,
                    }, // Input: Target Node Coord
                    CppnNode {
                        activation: brain::ActivationFn::Tanh,
                        bias: 0.0,
                        layer: 1,
                    }, // Output: Connection Weight
                    CppnNode {
                        activation: brain::ActivationFn::Tanh,
                        bias: 0.0,
                        layer: 1,
                    }, // Output: Bias
                    CppnNode {
                        activation: brain::ActivationFn::Linear,
                        bias: 0.0,
                        layer: 1,
                    }, // Output: Time Constant
                ],
                connections: vec![
                    CppnConnection {
                        source: 0,
                        target: 2,
                        weight: 2.0,
                        enabled: true,
                        innovation: 1,
                        mutation_rate: DEFAULT_MUTATION_RATE,
                    },
                    CppnConnection {
                        source: 1,
                        target: 2,
                        weight: -1.0,
                        enabled: true,
                        innovation: 2,
                        mutation_rate: DEFAULT_MUTATION_RATE,
                    },
                    CppnConnection {
                        source: 1,
                        target: 3,
                        weight: 1.0,
                        enabled: true,
                        innovation: 3,
                        mutation_rate: DEFAULT_MUTATION_RATE,
                    },
                    CppnConnection {
                        source: 1,
                        target: 4,
                        weight: 0.5,
                        enabled: true,
                        innovation: 4,
                        mutation_rate: DEFAULT_MUTATION_RATE,
                    },
                ],
            },
            morph_cppn: Cppn {
                nodes: vec![
                    CppnNode {
                        activation: brain::ActivationFn::Linear,
                        bias: 0.0,
                        layer: 0,
                    }, // Input 0: segment_idx / MAX
                    CppnNode {
                        activation: brain::ActivationFn::Linear,
                        bias: 0.0,
                        layer: 0,
                    }, // Input 1: parent_type_as_float
                    CppnNode {
                        activation: brain::ActivationFn::Tanh,
                        bias: 0.0,
                        layer: 1,
                    }, // Output 0: type/stop
                    CppnNode {
                        activation: brain::ActivationFn::Tanh,
                        bias: 0.0,
                        layer: 1,
                    }, // Output 1: branching
                    CppnNode {
                        activation: brain::ActivationFn::Tanh,
                        bias: 0.0,
                        layer: 1,
                    }, // Output 2: phase
                ],
                connections: vec![
                    CppnConnection {
                        source: 0,
                        target: 2,
                        weight: 1.0,
                        enabled: true,
                        innovation: 5,
                        mutation_rate: DEFAULT_MUTATION_RATE,
                    },
                    CppnConnection {
                        source: 0,
                        target: 3,
                        weight: 1.0,
                        enabled: true,
                        innovation: 6,
                        mutation_rate: DEFAULT_MUTATION_RATE,
                    },
                    CppnConnection {
                        source: 0,
                        target: 4,
                        weight: 1.0,
                        enabled: true,
                        innovation: 7,
                        mutation_rate: DEFAULT_MUTATION_RATE,
                    },
                ],
            },
            // Empty for this test-fixture constructor — `new_hox_driven`'s
            // whole point is an explicit, non-regulatory Hox sequence
            // (`hox: Some(hox)` below), so there's nothing meaningful to
            // template here yet. Phase 3 M4 (which replaces direct Hox
            // lookup with regulatory-network-decoded identity) will need to
            // revisit whether this constructor still makes sense at all.
            regulatory_cppn: Cppn::new(),
            second_allele: None,
            hox: Some(hox),
        }
    }

    /// Performs NEAT-style crossover with another genome, mixing genes from
    /// both parents rather than cloning either one outright.
    ///
    /// If `self` is diploid and carries a second allele, the child's second
    /// allele is produced by crossing `self`'s second allele against
    /// `other`'s (if `other` has one too — see [`Genome::new_diploid`]'s
    /// doc comment for why mating doesn't otherwise produce diploid
    /// children on its own).
    pub fn crossover<R: rand::Rng>(&self, other: &Genome, new_id: GenomeId, rng: &mut R) -> Self {
        let hox = match (&self.hox, &other.hox) {
            (Some(a), Some(b)) if a.genes.len() == b.genes.len() => Some(HoxSequence {
                genes: a
                    .genes
                    .iter()
                    .zip(b.genes.iter())
                    .map(|(ga, gb)| {
                        if rng.gen_bool(0.5) {
                            ga.clone()
                        } else {
                            gb.clone()
                        }
                    })
                    .collect(),
                color: if rng.gen_bool(0.5) { a.color } else { b.color },
            }),
            (Some(_), Some(_)) => {
                if rng.gen_bool(0.5) {
                    self.hox.clone()
                } else {
                    other.hox.clone()
                }
            }
            _ => self.hox.clone(),
        };

        let second_allele =
            self.second_allele
                .as_ref()
                .map(|self_allele| match &other.second_allele {
                    Some(other_allele) => DiploidAlleles {
                        brain_cppn: self_allele
                            .brain_cppn
                            .crossover(&other_allele.brain_cppn, rng),
                        morph_cppn: self_allele
                            .morph_cppn
                            .crossover(&other_allele.morph_cppn, rng),
                        regulatory_cppn: self_allele
                            .regulatory_cppn
                            .crossover(&other_allele.regulatory_cppn, rng),
                    },
                    None => self_allele.clone(),
                });

        Self {
            schema_version: self.schema_version,
            id: new_id,
            origin: self.origin,
            ploidy: self.ploidy,
            brain_cppn: self.brain_cppn.crossover(&other.brain_cppn, rng),
            morph_cppn: self.morph_cppn.crossover(&other.morph_cppn, rng),
            regulatory_cppn: self.regulatory_cppn.crossover(&other.regulatory_cppn, rng),
            second_allele,
            hox,
        }
    }

    /// Mutates the genome in place.
    ///
    /// For a diploid genome, the second allele is mutated independently —
    /// each allele copy accumulates its own mutations, exactly as separate
    /// chromosome copies would, rather than always mutating in lockstep.
    /// As of Phase 3 M2, `regulatory_cppn` mutates under the same pass gate
    /// as `brain_cppn`/`morph_cppn` — see `mutate_cppn_trio`.
    pub fn mutate<R: rand::Rng>(
        &mut self,
        mutation_rate: f32,
        rng: &mut R,
        tracker: &mut GlobalInnovationTracker,
    ) {
        mutate_cppn_trio(
            &mut self.brain_cppn,
            &mut self.morph_cppn,
            &mut self.regulatory_cppn,
            mutation_rate,
            rng,
            tracker,
        );
        if let Some(alleles) = &mut self.second_allele {
            mutate_cppn_trio(
                &mut alleles.brain_cppn,
                &mut alleles.morph_cppn,
                &mut alleles.regulatory_cppn,
                mutation_rate,
                rng,
                tracker,
            );
        }
    }
}

/// Applies the standard mutation roll to one genome's `brain`/`morph`/
/// `regulatory` CPPN triplet — shared between `Genome::mutate`'s primary
/// allele and (for diploid genomes) its second allele, so the two are
/// mutated independently rather than the second allele silently never
/// mutating. Named `_trio` (not `_pair`) since Phase 3 M2 extended this from
/// two CPPNs to three; `regulatory_cppn` uses the identical mutation rates
/// (5% add-node, 10% add-connection, per-connection jitter) as the other
/// two — no separate tuning was judged necessary for this milestone.
/// Maximum per-mutation-pass drift applied to a connection's own
/// `mutation_rate`, and the range it's clamped to — self-adaptation lets
/// evolution itself tune how volatile each locus is, rather than fixing it
/// permanently at [`cppn::DEFAULT_MUTATION_RATE`].
const MUTATION_RATE_DRIFT: f32 = 0.02;
const MUTATION_RATE_RANGE: std::ops::RangeInclusive<f32> = 0.0..=1.0;

/// Jitters a single connection's weight and, independently, its own
/// `mutation_rate` (self-adaptive drift, clamped to
/// [`MUTATION_RATE_RANGE`]) — each locus's volatility is itself heritable
/// and can evolve, rather than being a fixed global constant.
fn mutate_connection<R: rand::Rng>(conn: &mut CppnConnection, rng: &mut R) {
    if rng.gen::<f32>() < conn.mutation_rate {
        conn.weight += rng.gen_range(-1.0..1.0);
    }
    let drift = rng.gen_range(-MUTATION_RATE_DRIFT..MUTATION_RATE_DRIFT);
    conn.mutation_rate = (conn.mutation_rate + drift)
        .clamp(*MUTATION_RATE_RANGE.start(), *MUTATION_RATE_RANGE.end());
}

fn mutate_cppn_trio<R: rand::Rng>(
    brain_cppn: &mut Cppn,
    morph_cppn: &mut Cppn,
    regulatory_cppn: &mut Cppn,
    mutation_rate: f32,
    rng: &mut R,
    tracker: &mut GlobalInnovationTracker,
) {
    if rng.gen::<f32>() < mutation_rate {
        // Mutate Brain CPPN
        if rng.gen::<f32>() < 0.05 {
            brain_cppn.mutate_add_node(&mut tracker.next_innovation, rng);
        }
        if rng.gen::<f32>() < 0.10 {
            brain_cppn.mutate_add_connection(&mut tracker.next_innovation, rng);
        }
        for conn in &mut brain_cppn.connections {
            mutate_connection(conn, rng);
        }

        // Mutate Morph CPPN
        if rng.gen::<f32>() < 0.05 {
            morph_cppn.mutate_add_node(&mut tracker.next_innovation, rng);
        }
        if rng.gen::<f32>() < 0.10 {
            morph_cppn.mutate_add_connection(&mut tracker.next_innovation, rng);
        }
        for conn in &mut morph_cppn.connections {
            mutate_connection(conn, rng);
        }

        // Mutate Regulatory CPPN (Phase 3, M2) — same rates as brain/morph
        // above; appended after them (rather than interleaved) so the
        // pre-existing brain/morph mutation draw sequence for a given seed
        // is disturbed as little as possible by this addition.
        if rng.gen::<f32>() < 0.05 {
            regulatory_cppn.mutate_add_node(&mut tracker.next_innovation, rng);
        }
        if rng.gen::<f32>() < 0.10 {
            regulatory_cppn.mutate_add_connection(&mut tracker.next_innovation, rng);
        }
        for conn in &mut regulatory_cppn.connections {
            mutate_connection(conn, rng);
        }
    }
}

/// Resolves a diploid pair of CPPNs (matched by connection innovation
/// number, the same scheme [`Cppn::crossover`] uses) into a single
/// "expressed" CPPN, per [`Genome::expressed_brain_cppn`]'s doc comment:
/// whichever allele's connection has the larger `|weight|` dominates.
/// Nodes come from `primary` (the first allele) — if the two alleles have
/// different node counts, `primary`'s node list is authoritative and any
/// `secondary` connection referencing an out-of-range index is dropped,
/// mirroring `Cppn::crossover`'s existing node-count-mismatch handling.
fn express_diploid(primary: &Cppn, secondary: &Cppn) -> Cppn {
    let node_count = primary.nodes.len();
    let secondary_by_innovation: std::collections::HashMap<usize, &CppnConnection> = secondary
        .connections
        .iter()
        .map(|c| (c.innovation, c))
        .collect();

    let connections = primary
        .connections
        .iter()
        .map(|c| match secondary_by_innovation.get(&c.innovation) {
            Some(other_c) if other_c.weight.abs() > c.weight.abs() => (*other_c).clone(),
            _ => c.clone(),
        })
        .filter(|c| c.source < node_count && c.target < node_count)
        .collect();

    Cppn {
        nodes: primary.nodes.clone(),
        connections,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn sample_cppn(weight: f32) -> Cppn {
        Cppn {
            nodes: vec![
                CppnNode {
                    activation: brain::ActivationFn::Linear,
                    bias: 0.0,
                    layer: 0,
                },
                CppnNode {
                    activation: brain::ActivationFn::Tanh,
                    bias: 0.0,
                    layer: 1,
                },
            ],
            connections: vec![CppnConnection {
                source: 0,
                target: 1,
                weight,
                enabled: true,
                innovation: 0,
                mutation_rate: DEFAULT_MUTATION_RATE,
            }],
        }
    }

    #[test]
    fn new_minimal_has_empty_regulatory_cppn() {
        let g = Genome::new_minimal(GenomeId(1), EntityId(0));
        assert_eq!(g.regulatory_cppn.nodes.len(), 0);
        assert_eq!(g.regulatory_cppn.connections.len(), 0);
    }

    #[test]
    fn crossover_combines_regulatory_cppn_from_both_parents() {
        // Phase 3 M2: real NEAT-style crossover now applies to this field,
        // matching brain_cppn/morph_cppn (superseding M1's "carried over
        // unchanged" placeholder). For a single-connection CPPN,
        // `Cppn::crossover`'s per-gene coin flip means the result always
        // equals exactly one parent's weight, never a blend — so across
        // enough seeds, both parents' values should each appear at least
        // once, proving real combination (not always picking one side).
        let mut a = Genome::new_minimal(GenomeId(1), EntityId(0));
        a.regulatory_cppn = sample_cppn(0.7);
        let mut b = Genome::new_minimal(GenomeId(2), EntityId(0));
        b.regulatory_cppn = sample_cppn(-0.3);

        let mut saw_a = false;
        let mut saw_b = false;
        for seed in 0..30 {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let child = a.crossover(&b, GenomeId(3), &mut rng);
            let w = child.regulatory_cppn.connections[0].weight;
            if w == 0.7 {
                saw_a = true;
            }
            if w == -0.3 {
                saw_b = true;
            }
        }
        assert!(
            saw_a && saw_b,
            "expected crossover to draw this gene from each parent at least once across 30 seeds"
        );
    }

    #[test]
    fn mutate_changes_regulatory_cppn() {
        // Phase 3 M2: real mutation now applies to this field (superseding
        // M1's "unchanged" placeholder), using the same rates as
        // brain_cppn/morph_cppn — see `mutate_cppn_trio`.
        let mut g = Genome::new_minimal(GenomeId(1), EntityId(0));
        g.regulatory_cppn = sample_cppn(0.42);
        let before = g.regulatory_cppn.clone();
        let mut rng = ChaCha8Rng::seed_from_u64(2);
        let mut tracker = GlobalInnovationTracker::default();
        for _ in 0..50 {
            g.mutate(1.0, &mut rng, &mut tracker);
        }
        assert_ne!(g.regulatory_cppn, before);
    }

    #[test]
    fn haploid_expressed_cppn_borrows_primary_directly() {
        let g = Genome::new_minimal(GenomeId(1), EntityId(0));
        assert!(matches!(
            g.expressed_brain_cppn(),
            std::borrow::Cow::Borrowed(_)
        ));
    }

    #[test]
    fn diploid_expression_prefers_larger_magnitude_weight() {
        let expressed = express_diploid(&sample_cppn(0.2), &sample_cppn(-0.9));
        assert_eq!(expressed.connections[0].weight, -0.9);

        // And the reverse: primary dominates when it's larger.
        let expressed2 = express_diploid(&sample_cppn(1.5), &sample_cppn(0.3));
        assert_eq!(expressed2.connections[0].weight, 1.5);
    }

    #[test]
    fn new_diploid_produces_diploid_ploidy_with_second_allele() {
        let g = Genome::new_diploid(
            GenomeId(1),
            EntityId(0),
            None,
            (sample_cppn(0.5), Cppn::new(), Cppn::new()),
            (sample_cppn(-0.9), Cppn::new(), Cppn::new()),
        );
        assert_eq!(g.ploidy, Ploidy::Diploid);
        assert!(g.second_allele.is_some());
        // -0.9 has the larger magnitude, so it dominates the expressed CPPN.
        let expressed = g.expressed_brain_cppn();
        assert_eq!(expressed.connections[0].weight, -0.9);
    }

    #[test]
    fn mutate_affects_both_alleles_independently() {
        let mut g = Genome::new_diploid(
            GenomeId(1),
            EntityId(0),
            None,
            (sample_cppn(0.1), Cppn::new(), Cppn::new()),
            (sample_cppn(0.1), Cppn::new(), Cppn::new()),
        );
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let mut tracker = GlobalInnovationTracker::default();
        for _ in 0..50 {
            g.mutate(1.0, &mut rng, &mut tracker);
        }
        let primary_weight = g.brain_cppn.connections[0].weight;
        let second_weight = g.second_allele.as_ref().unwrap().brain_cppn.connections[0].weight;
        // Both alleles started identical (weight 0.1); since they're
        // mutated independently (not in lockstep) from the same rng
        // stream, they consume different draws and should have diverged.
        assert_ne!(primary_weight, second_weight);
    }

    #[test]
    fn diploid_second_allele_regulatory_cppn_crosses_and_mutates_too() {
        // Phase 3 M2: verifies the diploid second-allele path for
        // `regulatory_cppn` specifically — both `new_diploid`'s 3-tuple
        // signature and `mutate_cppn_trio`'s per-allele call.
        let mut g = Genome::new_diploid(
            GenomeId(1),
            EntityId(0),
            None,
            (Cppn::new(), Cppn::new(), sample_cppn(0.2)),
            (Cppn::new(), Cppn::new(), sample_cppn(0.2)),
        );
        let mut rng = ChaCha8Rng::seed_from_u64(11);
        let mut tracker = GlobalInnovationTracker::default();
        for _ in 0..50 {
            g.mutate(1.0, &mut rng, &mut tracker);
        }
        let primary_weight = g.regulatory_cppn.connections[0].weight;
        let second_weight = g
            .second_allele
            .as_ref()
            .unwrap()
            .regulatory_cppn
            .connections[0]
            .weight;
        assert_ne!(primary_weight, second_weight);
    }

    #[test]
    fn genome_mutate_is_deterministic_for_same_seed() {
        let build = || {
            Genome::new_diploid(
                GenomeId(1),
                EntityId(0),
                None,
                (sample_cppn(0.1), Cppn::new(), Cppn::new()),
                (sample_cppn(0.1), Cppn::new(), Cppn::new()),
            )
        };
        let mut g1 = build();
        let mut g2 = build();
        let mut rng1 = ChaCha8Rng::seed_from_u64(7);
        let mut rng2 = ChaCha8Rng::seed_from_u64(7);
        let mut tracker1 = GlobalInnovationTracker::default();
        let mut tracker2 = GlobalInnovationTracker::default();
        for _ in 0..20 {
            g1.mutate(1.0, &mut rng1, &mut tracker1);
            g2.mutate(1.0, &mut rng2, &mut tracker2);
        }
        assert_eq!(g1, g2);
    }

    #[test]
    fn zero_locus_rate_never_mutates_weight() {
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        let mut conn = CppnConnection {
            source: 0,
            target: 1,
            weight: 1.0,
            enabled: true,
            innovation: 0,
            mutation_rate: 0.0,
        };
        for _ in 0..10_000 {
            // Pin the self-adaptive drift to zero so only the weight-jitter
            // gate (the thing under test) can move `weight`.
            conn.mutation_rate = 0.0;
            mutate_connection(&mut conn, &mut rng);
        }
        assert_eq!(conn.weight, 1.0);
    }

    #[test]
    fn full_locus_rate_always_mutates_weight() {
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        let mut conn = CppnConnection {
            source: 0,
            target: 1,
            weight: 1.0,
            enabled: true,
            innovation: 0,
            mutation_rate: 1.0,
        };
        for _ in 0..10_000 {
            let before = conn.weight;
            conn.mutation_rate = 1.0; // pin: only test the weight-jitter gate
            mutate_connection(&mut conn, &mut rng);
            assert_ne!(conn.weight, before);
        }
    }

    #[test]
    fn distance_is_zero_for_clones_and_positive_for_divergent_genomes() {
        let g1 = Genome::new_minimal(GenomeId(1), EntityId(0));
        let g2 = g1.clone();
        assert_eq!(g1.distance(&g2), 0.0);

        let mut g3 = Genome::new_diploid(
            GenomeId(2),
            EntityId(0),
            None,
            (sample_cppn(0.1), Cppn::new(), Cppn::new()),
            (Cppn::new(), Cppn::new(), Cppn::new()),
        );
        g3.second_allele = None;
        let mut rng = ChaCha8Rng::seed_from_u64(5);
        let mut tracker = GlobalInnovationTracker::default();
        let mut g4 = g3.clone();
        for _ in 0..20 {
            g4.mutate(1.0, &mut rng, &mut tracker);
        }
        assert!(g3.distance(&g4) > 0.0);
    }

    #[test]
    fn mutation_rate_self_adapts_within_bounds() {
        let mut rng = ChaCha8Rng::seed_from_u64(3);
        let mut conn = CppnConnection {
            source: 0,
            target: 1,
            weight: 0.0,
            enabled: true,
            innovation: 0,
            mutation_rate: 0.5,
        };
        for _ in 0..10_000 {
            mutate_connection(&mut conn, &mut rng);
            assert!((0.0..=1.0).contains(&conn.mutation_rate));
        }
    }
}
