use crate::types::SegmentType;
use serde::{Deserialize, Serialize};

/// One gene in the Hox sequence — describes a single axial segment and whether
/// it should sprout a lateral appendage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HoxGene {
    /// The type of this axial segment.
    pub segment: SegmentType,
    /// Branching threshold in `[-1, 1]`.  
    /// A value **> 0.0** means this segment grows a bilateral fin/limb pair.
    /// Torso and Muscle segments are the only ones where branching makes
    /// biological sense; the growth system should ignore this for Head/Tail.
    pub branching_signal: f32,
    /// Actuation amplitude for muscle segments (0.0 for non-muscle).
    pub actuation_amplitude: f32,
    /// Actuation phase offset (radians).
    pub actuation_phase: f32,
}

impl HoxGene {
    /// A plain structural torso gene with no branching.
    pub fn torso() -> Self {
        Self {
            segment: SegmentType::Torso,
            branching_signal: -1.0,
            actuation_amplitude: 0.0,
            actuation_phase: 0.0,
        }
    }

    /// A torso gene that **will** branch into bilateral fins.
    pub fn branching_torso(actuation_amplitude: f32, actuation_phase: f32) -> Self {
        Self {
            segment: SegmentType::Torso,
            branching_signal: 0.5, // > 0 → branch
            actuation_amplitude,
            actuation_phase,
        }
    }

    /// A muscle gene with a given actuation amplitude and phase.
    pub fn muscle(amplitude: f32, phase: f32) -> Self {
        Self {
            segment: SegmentType::Muscle,
            branching_signal: -1.0,
            actuation_amplitude: amplitude,
            actuation_phase: phase,
        }
    }

    /// A tail gene.
    pub fn tail() -> Self {
        Self {
            segment: SegmentType::Tail,
            branching_signal: -1.0,
            actuation_amplitude: 0.0,
            actuation_phase: 0.0,
        }
    }

    /// A head gene.
    pub fn head() -> Self {
        Self {
            segment: SegmentType::Head,
            branching_signal: -1.0,
            actuation_amplitude: 0.0,
            actuation_phase: 0.0,
        }
    }
}

/// The complete axial Hox sequence for an organism's body plan.
///
/// Growth walks this list front-to-back: index 0 is the anteriormost segment
/// (Head), the last index is the posteriormost (Tail).  Each intermediate
/// segment is a Torso, Muscle, or Fin gene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HoxSequence {
    /// The ordered list of segment genes (Head → ... → Tail).
    pub genes: Vec<HoxGene>,
    /// Per-organism skin colour encoded as `[R, G, B]` in `[0, 1]`.
    pub color: [f32; 3],
}

impl HoxSequence {
    /// Construct a sequence from a slice of genes and a colour.
    pub fn new(genes: Vec<HoxGene>, color: [f32; 3]) -> Self {
        Self { genes, color }
    }

    /// A minimal worm-like organism: Head + N Muscle segments + Tail.
    /// No branching.
    pub fn worm(torso_count: usize, color: [f32; 3]) -> Self {
        let mut genes = vec![HoxGene::head()];
        for i in 0..torso_count {
            let phase = i as f32 * std::f32::consts::PI / 2.0;
            // Amplitude kept to ≤6% of segment_length (20 units) to stay in
            // the numerically stable regime for symplectic-Euler + PBD.
            genes.push(HoxGene::muscle(1.2, phase));
        }
        genes.push(HoxGene::tail());
        Self::new(genes, color)
    }

    /// A fish-like organism: Head + some rigid Torso + branching Torso
    /// (fins) + muscle Torso + Tail.
    pub fn fish(torso_count: usize, fin_at: usize, color: [f32; 3]) -> Self {
        let mut genes = vec![HoxGene::head()];
        for i in 0..torso_count {
            if i == fin_at {
                // Fin amplitude 2.5 units ≈ 17% of fin_spread (15 units) —
                // enough to produce visible flapping without physics blow-up.
                genes.push(HoxGene::branching_torso(2.5, 0.0));
            } else {
                let phase = i as f32 * std::f32::consts::PI / 3.0;
                genes.push(HoxGene::muscle(1.2, phase));
            }
        }
        genes.push(HoxGene::tail());
        Self::new(genes, color)
    }

    /// A static, non-actuated plant organism.
    pub fn plant(color: [f32; 3]) -> Self {
        Self::new(vec![HoxGene::head(), HoxGene::torso()], color)
    }
}
