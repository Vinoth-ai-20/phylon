use crate::cppn::{Cppn, CppnConnection, CppnNode, GlobalInnovationTracker};
use crate::hox::HoxSequence;
use crate::types::{GenomeId, Ploidy};
use bevy_ecs::prelude::Component;
use common::EntityId;
use serde::{Deserialize, Serialize};

/// The genome of an organism, containing independent CPPNs for body morphology and neural wiring.
#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Genome {
    /// Schema version for serialization compatibility (currently 2).
    pub schema_version: u32,
    /// Unique identifier for this genome sequence.
    pub id: GenomeId,
    /// The ID of the organism that created this genome (for lineage tracking).
    pub origin: EntityId,
    /// Ploidy level (haploid or diploid).
    pub ploidy: Ploidy,
    /// The CPPN responsible for neural wiring topology.
    pub brain_cppn: Cppn,
    /// The CPPN responsible for L-System body morphology growth.
    pub morph_cppn: Cppn,
    /// Optional explicit Hox body-plan sequence.
    ///
    /// When `Some`, the growth system reads the body plan directly from this
    /// sequence rather than querying the morph CPPN.
    pub hox: Option<HoxSequence>,
}

impl Genome {
    /// Creates a minimal genome.
    pub fn new_minimal(id: GenomeId, origin: EntityId) -> Self {
        Self {
            schema_version: 3,
            id,
            origin,
            ploidy: Ploidy::Haploid,
            brain_cppn: Cppn::new(),
            morph_cppn: Cppn::new(),
            hox: None,
        }
    }

    /// Creates a deterministic genome with a pre-defined Hox sequence.
    pub fn new_hox_driven(id: GenomeId, origin: EntityId, hox: HoxSequence) -> Self {
        Self {
            schema_version: 3,
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
                        weight: 0.5,
                        enabled: true,
                        innovation: 1,
                    },
                    CppnConnection {
                        source: 1,
                        target: 2,
                        weight: -0.25,
                        enabled: true,
                        innovation: 2,
                    },
                    CppnConnection {
                        source: 1,
                        target: 3,
                        weight: 0.25,
                        enabled: true,
                        innovation: 3,
                    },
                    CppnConnection {
                        source: 1,
                        target: 4,
                        weight: 0.1,
                        enabled: true,
                        innovation: 4,
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
                    CppnNode {
                        activation: brain::ActivationFn::Tanh,
                        bias: 0.0,
                        layer: 1,
                    }, // Output 3: amplitude
                ],
                connections: vec![
                    CppnConnection {
                        source: 0,
                        target: 2,
                        weight: 1.0,
                        enabled: true,
                        innovation: 5,
                    },
                    CppnConnection {
                        source: 0,
                        target: 3,
                        weight: 1.0,
                        enabled: true,
                        innovation: 6,
                    },
                    CppnConnection {
                        source: 0,
                        target: 4,
                        weight: 1.0,
                        enabled: true,
                        innovation: 7,
                    },
                    CppnConnection {
                        source: 0,
                        target: 5,
                        weight: 1.0,
                        enabled: true,
                        innovation: 8,
                    },
                ],
            },
            hox: Some(hox),
        }
    }

    /// Performs a simple crossover with another genome.
    pub fn crossover<R: rand::Rng>(&self, _other: &Genome, new_id: GenomeId, _rng: &mut R) -> Self {
        Self {
            schema_version: self.schema_version,
            id: new_id,
            origin: self.origin,
            ploidy: self.ploidy,
            brain_cppn: self.brain_cppn.clone(),
            morph_cppn: self.morph_cppn.clone(),
            hox: self.hox.clone(),
        }
    }

    /// Mutates the genome in place.
    pub fn mutate<R: rand::Rng>(
        &mut self,
        mutation_rate: f32,
        rng: &mut R,
        tracker: &mut GlobalInnovationTracker,
    ) {
        if rng.gen::<f32>() < mutation_rate {
            // Mutate Brain CPPN
            if rng.gen::<f32>() < 0.05 {
                self.brain_cppn
                    .mutate_add_node(&mut tracker.next_innovation);
            }
            if rng.gen::<f32>() < 0.10 {
                self.brain_cppn
                    .mutate_add_connection(&mut tracker.next_innovation);
            }
            for conn in &mut self.brain_cppn.connections {
                if rng.gen::<f32>() < 0.2 {
                    conn.weight += rng.gen_range(-1.0..1.0);
                }
            }

            // Mutate Morph CPPN
            if rng.gen::<f32>() < 0.05 {
                self.morph_cppn
                    .mutate_add_node(&mut tracker.next_innovation);
            }
            if rng.gen::<f32>() < 0.10 {
                self.morph_cppn
                    .mutate_add_connection(&mut tracker.next_innovation);
            }
            for conn in &mut self.morph_cppn.connections {
                if rng.gen::<f32>() < 0.2 {
                    conn.weight += rng.gen_range(-1.0..1.0);
                }
            }
        }
    }
}
