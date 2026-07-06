//! # Phylon Evolution
//!
//! Selection pressure, speciation, lineage tracking, fitness metrics, and
//! hybridization barriers.
//!
//! Evolution in Phylon is **emergent** — there is no explicit fitness function.
//! Survival and reproduction pressure exerted by the ecology system acts as
//! the selection gradient.
//!
//! Speciation, by contrast, is explicit: [`SpeciesRegistry`] clusters
//! organisms by [`genetics::Genome::distance`] (NEAT-style genetic
//! compatibility), replacing the placeholder `SpeciesId(0)` every organism
//! used to receive regardless of its genome.
//!
//! ## Not yet implemented
//!
//! Explicit selection/fitness metrics and hybridization barriers (declared
//! in this module's original scope) have no code here yet — only lineage
//! tracking and speciation are implemented so far.

#![warn(missing_docs)]
#![warn(clippy::all)]

use common::EntityId;
use serde::{Deserialize, Serialize};

/// A lineage identifier linking related organisms across generations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LineageId(pub u64);

/// A species cluster identifier assigned by the speciation algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpeciesId(pub u64);

/// # Lineage Trajectory Record
///
/// ## 1. What Happens
/// The `LineageRecord` structurally tracks the demographic lifecycle of a single specific organism,
/// linking it to its ancestral topology (parent), demographic cluster (lineage/species), and temporal bounds (birth/death).
///
/// ## 2. Why It Happens
/// Evolution is emergent, meaning fitness is entirely implicit—organisms survive because they didn't die.
/// To study how genetic configurations correlate with survival, researchers must reconstruct the phylogenetic
/// tree post-simulation. This record is the irreducible quantum of that tree.
///
/// ## 3. How It Happens
/// When an organism is spawned via reproduction, $Entity_{child}$ is linked to $Entity_{parent}$.
/// The fitness metric (Lifespan $L$) can be defined mathematically upon death:
///
/// $$ L = T_{death} - T_{birth} $$
///
/// The collection of all records forms a Directed Acyclic Graph (DAG) representing the evolutionary tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageRecord {
    /// The entity this record belongs to.
    pub entity: EntityId,
    /// The parent entity, if any.
    pub parent_id: Option<EntityId>,
    /// The lineage cluster this organism belongs to.
    pub lineage: LineageId,
    /// The species cluster assigned at last speciation check.
    pub species: SpeciesId,
    /// Generation number (0 for initial population).
    pub generation: u64,
    /// The tick at which this organism was born.
    pub birth_tick: u64,
    /// The tick at which this organism died.
    pub death_tick: Option<u64>,
    /// The cause of death, if applicable.
    pub cause_of_death: Option<String>,
}

/// # In-Memory Phylogeny Tracker
///
/// ## 1. What Happens
/// `LineageTracker` is a central ECS resource that acts as an ephemeral holding buffer for the
/// evolutionary Directed Acyclic Graph (DAG) of the current active population.
///
/// ## 2. Why It Happens
/// Logging every birth and death directly to an SQLite disk database causes extreme I/O bottlenecking
/// during periods of high population turnover (e.g., mass extinction events or invasive species blooms).
/// Maintaining an in-memory hash map allows $O(1)$ updates without blocking the simulation thread.
///
/// ## 3. How It Happens
/// The tracker maintains an active set $A$. When an organism is born, it is inserted into $A$.
/// When it dies, its record in $A$ is mutated to include $T_{death}$. The set $A$ is then partitioned
/// during the `extract_completed_records` phase to flush completed lineages to cold storage.
#[derive(bevy_ecs::system::Resource)]
pub struct LineageTracker {
    next_lineage_id: u64,
    records: std::collections::HashMap<EntityId, LineageRecord>,
}

impl LineageTracker {
    /// Creates a new lineage tracker.
    pub fn new() -> Self {
        Self {
            next_lineage_id: 1,
            records: std::collections::HashMap::new(),
        }
    }

    /// Allocates a new lineage ID for completely new organisms.
    pub fn new_lineage_id(&mut self) -> LineageId {
        let id = LineageId(self.next_lineage_id);
        self.next_lineage_id += 1;
        id
    }

    /// Registers a newly born organism.
    pub fn register_birth(
        &mut self,
        entity: EntityId,
        parent_id: Option<EntityId>,
        lineage: LineageId,
        species: SpeciesId,
        generation: u64,
        birth_tick: u64,
    ) {
        self.records.insert(
            entity,
            LineageRecord {
                entity,
                parent_id,
                lineage,
                species,
                generation,
                birth_tick,
                death_tick: None,
                cause_of_death: None,
            },
        );
    }

    /// Records the death of an organism.
    pub fn register_death(&mut self, entity: EntityId, death_tick: u64, cause: String) {
        if let Some(record) = self.records.get_mut(&entity) {
            record.death_tick = Some(death_tick);
            record.cause_of_death = Some(cause);
        }
    }

    /// Retrieves an active record.
    pub fn get_record(&self, entity: EntityId) -> Option<&LineageRecord> {
        self.records.get(&entity)
    }

    /// Iterates every record for a currently-alive organism
    /// (`death_tick.is_none()` — a record with `death_tick` set is still in
    /// `records` until the next [`LineageTracker::extract_completed_records`]
    /// call, so this filters those out). Used by
    /// `app::analytics_bridge` to compute species/age/generation
    /// distributions without exposing the underlying `HashMap`.
    pub fn active_records(&self) -> impl Iterator<Item = &LineageRecord> {
        self.records.values().filter(|r| r.death_tick.is_none())
    }

    /// # Ephemeral DAG Cold-Storage Extraction
    ///
    /// ## 1. What Happens
    /// The `extract_completed_records` method filters the in-memory active set $A$ for all records
    /// where `death_tick` is populated, removes them from the tracker, and returns them as a batch.
    ///
    /// ## 2. Why It Happens
    /// Memory cannot grow infinitely. To prevent Out-Of-Memory (OOM) panics over a multi-day simulation
    /// run with millions of generations, completed dead lineages must be evicted from the active map
    /// and passed to the asynchronous `storage` crate for permanent SQLite persistence.
    ///
    /// ## 3. How It Happens
    /// The filter operation runs over the active set $A$:
    ///
    /// $$ D = \{ r \in A \mid r.death\_tick \ne \emptyset \} $$
    /// $$ A' = A \setminus D $$
    ///
    /// The extracted set $D$ is returned as an owned `Vec` to be handed over to a background rayon
    /// thread, preventing garbage collection stuttering.
    pub fn extract_completed_records(&mut self) -> Vec<LineageRecord> {
        let completed: Vec<EntityId> = self
            .records
            .iter()
            .filter(|(_, record)| record.death_tick.is_some())
            .map(|(e, _)| *e)
            .collect();

        let mut extracted = Vec::with_capacity(completed.len());
        for e in completed {
            if let Some(record) = self.records.remove(&e) {
                extracted.push(record);
            }
        }
        extracted
    }
}

impl Default for LineageTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Compatibility-distance threshold below which two genomes are considered
/// the same species — a [`genetics::Genome::distance`] value under this
/// classifies as a match. Tuned to the same order of magnitude as the
/// standard NEAT default (3.0), since [`genetics::cppn`]'s
/// `EXCESS_COEFFICIENT`/`DISJOINT_COEFFICIENT`/`WEIGHT_DIFF_COEFFICIENT` are
/// themselves the standard NEAT values.
pub const DEFAULT_COMPATIBILITY_THRESHOLD: f32 = 3.0;

/// Number of [`SpeciesRegistry::classify`] calls between automatic
/// representative refreshes (DEF-022, `IMPLEMENTATION_STATUS.md`). Chosen
/// as a coarse, infrequent cadence — refreshing is a drift correction, not
/// a per-birth operation — not tuned against a specific population size.
pub const REPRESENTATIVE_REFRESH_INTERVAL: u64 = 500;

/// One tracked species: its assigned ID and the representative genome new
/// arrivals are compared against.
struct SpeciesRecord {
    id: SpeciesId,
    representative: genetics::Genome,
    /// The most recently classified member of this species since the last
    /// refresh — becomes the new `representative` at the next refresh
    /// (DEF-022). `None` if no member has joined since the last refresh.
    most_recent_member: Option<genetics::Genome>,
}

/// # Genetic-Distance Speciation Registry
///
/// Classifies organisms into species by NEAT-style genetic distance
/// ([`genetics::Genome::distance`]), replacing the `SpeciesId(0)` placeholder
/// every organism was previously assigned regardless of its genome.
///
/// ## How it works
/// Each species is represented by the genome of whichever organism founded
/// it. A newly spawned organism is compared against every existing
/// species's representative; if any comparison falls under
/// `compatibility_threshold`, the organism joins that species. Otherwise it
/// founds a new one, becoming that species's representative.
///
/// Representatives are never reassigned after a species is founded *except*
/// via the periodic refresh below — this keeps classification at
/// O(species_count) per spawn (not O(population²): distances are only ever
/// computed against one genome per species, never between arbitrary
/// population pairs), which stays cheap even as population grows into the
/// thousands. The tradeoff is the classic NEAT one: a species's
/// representative can grow unrepresentative of its current members as they
/// keep mutating — every [`REPRESENTATIVE_REFRESH_INTERVAL`] classify calls,
/// [`SpeciesRegistry::classify`] promotes each species's most recently
/// classified member to be its new representative (DEF-022,
/// `IMPLEMENTATION_STATUS.md`). This is a coarse "most recent member"
/// refresh, not a true population centroid (which isn't well-defined for a
/// NEAT-style CPPN graph) — a reasonable, simple correction for drift, not
/// a claim of statistical centrality.
#[derive(bevy_ecs::system::Resource)]
pub struct SpeciesRegistry {
    next_species_id: u64,
    compatibility_threshold: f32,
    species: Vec<SpeciesRecord>,
    classify_count: u64,
}

impl SpeciesRegistry {
    /// Creates a new, empty registry with the given compatibility threshold.
    pub fn new(compatibility_threshold: f32) -> Self {
        Self {
            next_species_id: 1,
            compatibility_threshold,
            species: Vec::new(),
            classify_count: 0,
        }
    }

    /// Classifies a genome against existing species, founding a new species
    /// if it matches none within the compatibility threshold. Also records
    /// `genome` as its species's most-recent-member candidate, and — every
    /// [`REPRESENTATIVE_REFRESH_INTERVAL`] calls — refreshes representatives
    /// (see [`SpeciesRegistry::refresh_representatives`]).
    pub fn classify(&mut self, genome: &genetics::Genome) -> SpeciesId {
        let id = self.classify_inner(genome);

        self.classify_count += 1;
        if self
            .classify_count
            .is_multiple_of(REPRESENTATIVE_REFRESH_INTERVAL)
        {
            self.refresh_representatives();
        }

        id
    }

    fn classify_inner(&mut self, genome: &genetics::Genome) -> SpeciesId {
        for record in &mut self.species {
            if record.representative.distance(genome) < self.compatibility_threshold {
                record.most_recent_member = Some(genome.clone());
                return record.id;
            }
        }
        let id = SpeciesId(self.next_species_id);
        self.next_species_id += 1;
        self.species.push(SpeciesRecord {
            id,
            representative: genome.clone(),
            most_recent_member: None,
        });
        id
    }

    /// Promotes each species's most-recently-classified member (since the
    /// last refresh) to be its new representative (DEF-022). Species with
    /// no new member since the last refresh keep their current
    /// representative unchanged. Called automatically by
    /// [`SpeciesRegistry::classify`] every [`REPRESENTATIVE_REFRESH_INTERVAL`]
    /// calls; exposed publicly so tests (and any caller wanting a refresh
    /// on a different cadence) don't need to drive hundreds of `classify`
    /// calls just to trigger one.
    pub fn refresh_representatives(&mut self) {
        for record in &mut self.species {
            if let Some(new_representative) = record.most_recent_member.take() {
                record.representative = new_representative;
            }
        }
    }

    /// The number of distinct species currently tracked.
    pub fn species_count(&self) -> usize {
        self.species.len()
    }
}

impl Default for SpeciesRegistry {
    fn default() -> Self {
        Self::new(DEFAULT_COMPATIBILITY_THRESHOLD)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn lineage_id_equality() {
        assert_eq!(LineageId(1), LineageId(1));
        assert_ne!(LineageId(1), LineageId(2));
    }

    #[test]
    fn identical_genomes_classify_into_the_same_species() {
        let mut registry = SpeciesRegistry::default();
        let g1 = genetics::Genome::new_minimal(genetics::GenomeId(1), common::EntityId(0));
        let g2 = g1.clone();
        assert_eq!(registry.classify(&g1), registry.classify(&g2));
        assert_eq!(registry.species_count(), 1);
    }

    #[test]
    fn divergent_genomes_found_a_new_species() {
        let mut registry = SpeciesRegistry::new(0.5);
        let g1 = genetics::Genome::new_minimal(genetics::GenomeId(1), common::EntityId(0));

        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(9);
        let mut tracker = genetics::GlobalInnovationTracker::default();
        let mut g2 = genetics::Genome::new_diploid(
            genetics::GenomeId(2),
            common::EntityId(0),
            (sample_cppn(), genetics::Cppn::new(), genetics::Cppn::new()),
            (
                genetics::Cppn::new(),
                genetics::Cppn::new(),
                genetics::Cppn::new(),
            ),
        );
        g2.second_allele = None;
        for _ in 0..30 {
            g2.mutate(1.0, &mut rng, &mut tracker);
        }

        let s1 = registry.classify(&g1);
        let s2 = registry.classify(&g2);
        assert_ne!(s1, s2);
        assert_eq!(registry.species_count(), 2);
    }

    fn sample_cppn() -> genetics::Cppn {
        genetics::Cppn {
            nodes: vec![
                genetics::CppnNode {
                    activation: brain::ActivationFn::Linear,
                    bias: 0.0,
                    layer: 0,
                },
                genetics::CppnNode {
                    activation: brain::ActivationFn::Tanh,
                    bias: 0.0,
                    layer: 1,
                },
            ],
            connections: vec![genetics::CppnConnection {
                source: 0,
                target: 1,
                weight: 0.5,
                enabled: true,
                innovation: 0,
                mutation_rate: genetics::cppn::DEFAULT_MUTATION_RATE,
            }],
        }
    }

    #[test]
    fn regulatory_cppn_divergence_alone_founds_a_new_species() {
        // Phase 3 M7: `Genome::distance` now sums a `regulatory_cppn` term
        // too — two genomes identical in brain/morph but divergent enough
        // in `regulatory_cppn` should classify as different species, which
        // was impossible before this milestone (the term didn't exist).
        let mut registry = SpeciesRegistry::new(0.5);
        let mut g1 = genetics::Genome::new_minimal(genetics::GenomeId(1), common::EntityId(0));
        g1.regulatory_cppn = sample_cppn();
        let mut g2 = genetics::Genome::new_minimal(genetics::GenomeId(2), common::EntityId(0));
        g2.regulatory_cppn = genetics::Cppn::new();

        let s1 = registry.classify(&g1);
        let s2 = registry.classify(&g2);
        assert_ne!(s1, s2);
        assert_eq!(registry.species_count(), 2);
    }

    #[test]
    fn refresh_representatives_promotes_most_recent_member() {
        // A wide threshold keeps every genome in one species regardless of
        // how far the representative has drifted from new arrivals — this
        // isolates the refresh mechanism itself from the classify/threshold
        // logic already covered by other tests.
        let mut registry = SpeciesRegistry::new(1000.0);
        let founder = genetics::Genome::new_minimal(genetics::GenomeId(1), common::EntityId(0));
        registry.classify(&founder);

        let mut latest = genetics::Genome::new_minimal(genetics::GenomeId(2), common::EntityId(0));
        latest.regulatory_cppn = sample_cppn();
        registry.classify(&latest);

        registry.refresh_representatives();

        assert_eq!(registry.species[0].representative.id, latest.id);
        assert!(registry.species[0].most_recent_member.is_none());
    }

    #[test]
    fn classify_auto_refreshes_at_the_interval() {
        // Drives exactly `REPRESENTATIVE_REFRESH_INTERVAL` classify calls
        // and confirms `classify` (not a manual `refresh_representatives`
        // call) promoted the last-classified genome to representative —
        // `tests` is a child module of the crate root, so it can read
        // `SpeciesRegistry`/`SpeciesRecord`'s private fields directly for a
        // precise assertion, rather than inferring the refresh indirectly.
        let mut registry = SpeciesRegistry::new(1000.0);
        let founder = genetics::Genome::new_minimal(genetics::GenomeId(1), common::EntityId(0));
        registry.classify(&founder); // classify_count == 1

        // The founder's own classify call already consumed 1 of the
        // interval's counts, so only `INTERVAL - 1` more calls are needed
        // to land exactly on the next multiple of the interval.
        let mut last_genome = founder.clone();
        for i in 0..(REPRESENTATIVE_REFRESH_INTERVAL - 1) {
            last_genome =
                genetics::Genome::new_minimal(genetics::GenomeId(100 + i), common::EntityId(0));
            registry.classify(&last_genome);
        }

        assert_eq!(registry.species_count(), 1);
        assert_eq!(registry.species[0].representative.id, last_genome.id);
        assert!(registry.species[0].most_recent_member.is_none());
    }
}
