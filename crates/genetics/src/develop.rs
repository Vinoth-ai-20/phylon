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
/// [`SegmentType`]. As of Phase 3 M5, the code space (8, from 3 thresholded
/// Hox genes) exactly matches `SegmentType`'s 8 variants — no modulo wrap is
/// needed any more (`decode_segment_type` no longer collides codes 5-7 back
/// onto 0-2, the way it did with only 5 variants).
const SEGMENT_TYPES_BY_CODE: [SegmentType; 8] = [
    SegmentType::Head,
    SegmentType::Torso,
    SegmentType::Muscle,
    SegmentType::Tail,
    SegmentType::Fin,
    SegmentType::Vascular,
    SegmentType::Ganglion,
    SegmentType::Germinal,
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
    /// Developmental apoptosis (Phase 3, M8; DEF-002): when `true`,
    /// `organisms::growth_system` prunes this position — it is never
    /// spawned, as if it had never formed. Germ-soma separation is folded
    /// directly into this decode rather than a separate flag: a
    /// `SegmentType::Germinal` position is never marked for apoptosis
    /// regardless of the raw differentiation signal, mirroring real
    /// biology's germ-line protection from programmed cell death.
    pub apoptosis: bool,
}

/// Decodes a [`SegmentType`] from the Hox-designated gene states as a
/// combinatorial binary code (each gene thresholded at `0.5`) — see
/// ADR-P3-02. Order of `hox_states` must match [`REGULATORY_GENE_ROLES`]'s
/// Hox-designated ordering. `hox_states.len()` beyond 3 genes would produce
/// a code exceeding `SEGMENT_TYPES_BY_CODE`'s range; the modulo is kept as a
/// defensive bound (not a design feature) rather than assuming callers never
/// pass more than 3 states.
pub fn decode_segment_type(hox_states: &[f32]) -> SegmentType {
    let code = hox_states
        .iter()
        .fold(0usize, |acc, &state| (acc << 1) | usize::from(state > 0.5));
    SEGMENT_TYPES_BY_CODE[code % SEGMENT_TYPES_BY_CODE.len()]
}

/// Applies unconditional germ-line protection (Phase 3 M8, DEF-002) to a raw
/// apoptosis signal: a `SegmentType::Germinal` position is never marked for
/// apoptosis, regardless of `apoptosis_signal` — mirroring real biology's
/// germ-line protection from programmed cell death, applied here at the
/// source so nothing downstream can forget it.
pub fn decode_apoptosis(apoptosis_signal: bool, segment_type: SegmentType) -> bool {
    apoptosis_signal && segment_type != SegmentType::Germinal
}

/// Returns the raw (pre-threshold) Hox-designated gene states at a body
/// position — the same values [`develop_at_position`] thresholds into a
/// combinatorial code internally, exposed here for research instrumentation
/// (Phase 3, M10's HOX Visualizer) that wants to show *how close* a bit is
/// to flipping, not just the final decoded [`SegmentType`]. A small, cheap,
/// pure recomputation (development is already fast enough to run once per
/// displayed position); not folded into `DevelopmentalOutputs` itself since
/// that type is `Copy` and used pervasively by `organisms::growth_system` —
/// adding a `Vec` field there would cost every existing call site a
/// `.clone()` for no growth-time benefit.
pub fn hox_states_at_position(
    regulatory_cppn: &Cppn,
    segment_index: usize,
    total_segments: usize,
) -> Vec<f32> {
    let gene_count = REGULATORY_GENE_ROLES.len();
    let mut network = RegulatoryNetwork::generate(regulatory_cppn, gene_count);
    let inputs = external_inputs_for_position(segment_index, total_segments, gene_count);
    network.develop(DEVELOPMENT_STEPS, &inputs);

    network
        .nodes
        .iter()
        .zip(REGULATORY_GENE_ROLES.iter())
        .filter(|(_, &r)| r == RegulatoryGeneRole::Hox)
        .map(|(n, _)| n.state)
        .collect()
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
    // Phase 3 M8 (DEF-002): the Differentiation role's second gene — unused
    // by any milestone until now — is the apoptosis signal.
    let apoptosis_signal = differentiation_states.get(1).copied().unwrap_or(0.0) > 0.5;
    let apoptosis = decode_apoptosis(apoptosis_signal, segment_type);

    DevelopmentalOutputs {
        segment_type,
        branches,
        actuation_amplitude,
        actuation_phase,
        pigment,
        apoptosis,
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
    fn decode_segment_type_maps_all_8_codes_to_distinct_types() {
        // Phase 3 M5: with 8 `SegmentType` variants matching the 3-bit Hox
        // code's 8 possible values, every code should decode to a unique
        // type — no more collisions from the pre-M5 modulo-5 wrap.
        let mut seen = std::collections::HashSet::new();
        for hi in [0.0_f32, 1.0] {
            for mid in [0.0_f32, 1.0] {
                for lo in [0.0_f32, 1.0] {
                    seen.insert(decode_segment_type(&[hi, mid, lo]));
                }
            }
        }
        assert_eq!(seen.len(), 8);
    }

    #[test]
    fn hox_states_at_position_has_3_entries_and_matches_the_decode() {
        let cppn = Cppn::new();
        let states = hox_states_at_position(&cppn, 2, 10);
        assert_eq!(states.len(), 3);
        assert_eq!(
            decode_segment_type(&states),
            develop_at_position(&cppn, 2, 10).segment_type
        );
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
    fn apoptosis_never_fires_for_a_germinal_position() {
        // Phase 3 M8 (DEF-002): germ-line protection is unconditional — a
        // Germinal position stays protected even when the raw apoptosis
        // signal fires.
        assert!(!decode_apoptosis(true, SegmentType::Germinal));
        assert!(!decode_apoptosis(false, SegmentType::Germinal));
    }

    #[test]
    fn apoptosis_fires_for_non_germinal_positions_when_signaled() {
        assert!(decode_apoptosis(true, SegmentType::Muscle));
        assert!(decode_apoptosis(true, SegmentType::Torso));
        assert!(!decode_apoptosis(false, SegmentType::Muscle));
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
