//! Positional decode of a [`RegulatoryNetwork`] into concrete developmental
//! outputs (Phase 3, M4).
//!
//! Every body position — the head node spawned directly by
//! `organisms::spawning::spawn_organism`, and every subsequent segment
//! spawned by `organisms::growth_system` — is decoded through the exact same
//! [`develop_at_position`] function. There is no special-cased "first
//! segment" path: per ADR-P3-02, segment identity (and, as of this
//! milestone, pigmentation) is always a decode of the regulatory network at
//! a position, never a direct lookup or a template-specific branch.

use crate::cppn::Cppn;
use crate::morphogen::external_inputs_for_position;
use crate::regulatory::{RegulatoryGeneRole, RegulatoryNetwork, REGULATORY_GENE_ROLES};
use crate::types::SegmentType;

/// Number of developmental steps run before gene states are read — fixed,
/// per ADR-P3-05 ("fixed step count, never iterate to convergence").
pub const DEVELOPMENT_STEPS: usize = 8;

/// Fixed decode order from a 3-bit Hox combinatorial code to a
/// [`SegmentType`]. Wrapped via modulo (see [`decode_segment_type`]) since
/// the code space (8) exceeds today's variant count (5) — comfortable room
/// for Phase 3 M5's broadened vocabulary to occupy the remaining codes
/// without changing this decode's shape.
const SEGMENT_TYPES_BY_CODE: [SegmentType; 5] = [
    SegmentType::Head,
    SegmentType::Torso,
    SegmentType::Muscle,
    SegmentType::Tail,
    SegmentType::Fin,
];

/// All developmental outputs decoded at one body-axis position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DevelopmentalOutputs {
    /// This position's anatomical category.
    pub segment_type: SegmentType,
    /// Whether this position sprouts a bilateral fin/limb pair (only
    /// meaningful for `Torso`/`Muscle` — see `organisms::growth_system`).
    pub branches: bool,
    /// Muscle actuation amplitude, `0.0` if not applicable at this segment.
    pub actuation_amplitude: f32,
    /// Muscle actuation phase, in radians.
    pub actuation_phase: f32,
    /// This position's emergent skin pigmentation, `[R, G, B]` in `[0, 1]`.
    pub pigment: [f32; 3],
}

/// Decodes a [`SegmentType`] from the Hox-designated gene states as a
/// combinatorial binary code (each gene thresholded at `0.5`), wrapped into
/// `SegmentType`'s fixed variant count via modulo — see ADR-P3-02. Order of
/// `hox_states` must match [`REGULATORY_GENE_ROLES`]'s Hox-designated
/// ordering.
pub fn decode_segment_type(hox_states: &[f32]) -> SegmentType {
    let code = hox_states
        .iter()
        .fold(0usize, |acc, &state| (acc << 1) | usize::from(state > 0.5));
    SEGMENT_TYPES_BY_CODE[code % SEGMENT_TYPES_BY_CODE.len()]
}

/// Runs development at one body-axis position and decodes every output this
/// milestone defines. `regulatory_cppn` should already be the
/// dominance-expressed CPPN (see `Genome::expressed_regulatory_cppn`), not a
/// raw diploid allele.
pub fn develop_at_position(
    regulatory_cppn: &Cppn,
    segment_index: usize,
    total_segments: usize,
) -> DevelopmentalOutputs {
    let gene_count = REGULATORY_GENE_ROLES.len();
    let mut network = RegulatoryNetwork::generate(regulatory_cppn, gene_count);
    let inputs = external_inputs_for_position(segment_index, total_segments, gene_count);
    network.develop(DEVELOPMENT_STEPS, &inputs);

    let states_for = |role: RegulatoryGeneRole| -> Vec<f32> {
        network
            .nodes
            .iter()
            .zip(REGULATORY_GENE_ROLES.iter())
            .filter(|(_, &r)| r == role)
            .map(|(n, _)| n.state)
            .collect()
    };

    let hox_states = states_for(RegulatoryGeneRole::Hox);
    let differentiation_states = states_for(RegulatoryGeneRole::Differentiation);
    let effector_states = states_for(RegulatoryGeneRole::Effector);
    let pigment_states = states_for(RegulatoryGeneRole::Pigment);

    let segment_type = decode_segment_type(&hox_states);
    let branches = differentiation_states.first().copied().unwrap_or(0.0) > 0.5;
    let actuation_amplitude = effector_states.first().copied().unwrap_or(0.0) * 2.0;
    let actuation_phase = effector_states.get(1).copied().unwrap_or(0.0) * std::f32::consts::TAU;
    let pigment = [
        pigment_states.first().copied().unwrap_or(0.5),
        pigment_states.get(1).copied().unwrap_or(0.5),
        pigment_states.get(2).copied().unwrap_or(0.5),
    ];

    DevelopmentalOutputs {
        segment_type,
        branches,
        actuation_amplitude,
        actuation_phase,
        pigment,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_segment_type_is_deterministic_and_in_range() {
        let a = decode_segment_type(&[0.9, 0.1, 0.9]);
        let b = decode_segment_type(&[0.9, 0.1, 0.9]);
        assert_eq!(a, b);
    }

    #[test]
    fn decode_segment_type_covers_the_full_code_space_without_panicking() {
        for hi in [0.0_f32, 1.0] {
            for mid in [0.0_f32, 1.0] {
                for lo in [0.0_f32, 1.0] {
                    let _ = decode_segment_type(&[hi, mid, lo]);
                }
            }
        }
    }

    #[test]
    fn develop_at_position_is_deterministic_for_the_same_position() {
        let cppn = Cppn::new();
        let a = develop_at_position(&cppn, 3, 10);
        let b = develop_at_position(&cppn, 3, 10);
        assert_eq!(a, b);
    }

    #[test]
    fn develop_at_position_pigment_channels_are_normalized() {
        let cppn = Cppn::new();
        let outputs = develop_at_position(&cppn, 0, 5);
        for channel in outputs.pigment {
            assert!((0.0..=1.0).contains(&channel));
        }
    }

    #[test]
    fn develop_at_position_yields_no_special_case_for_the_first_index() {
        // Position 0 (the head node, per `organisms::spawning`) runs through
        // exactly the same decode as every later position — this test just
        // confirms it doesn't panic or behave differently in kind (not
        // value) from an interior position.
        let cppn = Cppn::new();
        let head = develop_at_position(&cppn, 0, 8);
        let mid = develop_at_position(&cppn, 4, 8);
        assert!((0.0..=1.0).contains(&head.pigment[0]));
        assert!((0.0..=1.0).contains(&mid.pigment[0]));
    }
}
