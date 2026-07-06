//! Analytic morphogen gradients (Phase 3, M3).
//!
//! See `PHASE3_ROADMAP.md`'s ADR-P3-03: these are closed-form functions of
//! position, not a diffused PDE field (that upgrade stays deferred as
//! DEF-006). A real early-embryo gradient such as Bicoid in *Drosophila* is
//! itself close to an exponential decay from a localized source, so a
//! closed form is a reasonable first model, not a simplification purely for
//! convenience.
//!
//! **Scope of this milestone:** these functions compute [`RegulatoryNetwork`]
//! `external_inputs` from a body-axis position. They are not yet called by
//! `organisms::growth_system` (that wiring, plus reading Hox-designated
//! genes for segment identity, is Phase 3 M4).
//!
//! [`RegulatoryNetwork`]: crate::regulatory::RegulatoryNetwork

/// Steepness of the distance-from-head decay. Chosen so the gradient falls
/// to roughly 5% of its head-value by the tail of a body plan of typical
/// size, rather than tuned against any specific fixture.
const DECAY_RATE: f32 = 3.0;

/// Normalized anterior-posterior axis position: `0.0` at the head segment,
/// `1.0` at the tail segment. A single-segment body plan is defined as
/// entirely "head" (`0.0`), since there is no posterior to distinguish it
/// from.
pub fn ap_position(segment_index: usize, total_segments: usize) -> f32 {
    if total_segments <= 1 {
        return 0.0;
    }
    segment_index.min(total_segments - 1) as f32 / (total_segments - 1) as f32
}

/// Distance-from-head morphogen concentration: `1.0` at the head, decaying
/// exponentially toward the tail — the closed-form analog of a localized
/// source diffusing along the body axis (see this module's doc comment).
pub fn distance_from_head_gradient(segment_index: usize, total_segments: usize) -> f32 {
    let ap = ap_position(segment_index, total_segments);
    (-DECAY_RATE * ap).exp()
}

/// Builds the `external_inputs` slice [`RegulatoryNetwork::step`]/[`develop`]
/// expect: one value per gene, computed from this position's morphogen
/// signals. For this milestone every gene receives the same combined signal
/// (AP position plus the distance-from-head gradient) — which genes read
/// which *specific* morphogen channel is a Phase 3 M4 decoding concern, not
/// this milestone's.
///
/// [`RegulatoryNetwork::step`]: crate::regulatory::RegulatoryNetwork::step
/// [`develop`]: crate::regulatory::RegulatoryNetwork::develop
pub fn external_inputs_for_position(
    segment_index: usize,
    total_segments: usize,
    gene_count: usize,
) -> Vec<f32> {
    let signal = ap_position(segment_index, total_segments)
        + distance_from_head_gradient(segment_index, total_segments);
    vec![signal; gene_count]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regulatory::{RegulatoryNetwork, REGULATORY_GENE_ROLES};

    #[test]
    fn ap_position_head_is_zero_and_tail_is_one() {
        assert_eq!(ap_position(0, 5), 0.0);
        assert_eq!(ap_position(4, 5), 1.0);
    }

    #[test]
    fn ap_position_is_monotonically_increasing() {
        let total = 6;
        let mut previous = ap_position(0, total);
        for i in 1..total {
            let current = ap_position(i, total);
            assert!(current > previous);
            previous = current;
        }
    }

    #[test]
    fn ap_position_handles_degenerate_single_segment_body() {
        assert_eq!(ap_position(0, 1), 0.0);
        assert_eq!(ap_position(0, 0), 0.0);
    }

    #[test]
    fn distance_from_head_gradient_decays_toward_tail() {
        let total = 5;
        let head = distance_from_head_gradient(0, total);
        let mid = distance_from_head_gradient(2, total);
        let tail = distance_from_head_gradient(4, total);
        assert_eq!(head, 1.0);
        assert!(mid < head);
        assert!(tail < mid);
    }

    #[test]
    fn external_inputs_have_one_entry_per_gene() {
        let inputs = external_inputs_for_position(1, 5, REGULATORY_GENE_ROLES.len());
        assert_eq!(inputs.len(), REGULATORY_GENE_ROLES.len());
    }

    #[test]
    fn external_inputs_are_deterministic_for_the_same_position() {
        let a = external_inputs_for_position(2, 6, 4);
        let b = external_inputs_for_position(2, 6, 4);
        assert_eq!(a, b);
    }

    #[test]
    fn different_positions_yield_different_network_states() {
        let cppn = crate::cppn::Cppn::new();
        let gene_count = REGULATORY_GENE_ROLES.len();
        let mut head_net = RegulatoryNetwork::generate(&cppn, gene_count);
        let mut tail_net = head_net.clone();

        let head_inputs = external_inputs_for_position(0, 5, gene_count);
        let tail_inputs = external_inputs_for_position(4, 5, gene_count);
        head_net.develop(3, &head_inputs);
        tail_net.develop(3, &tail_inputs);

        assert_ne!(head_net, tail_net);
    }
}
