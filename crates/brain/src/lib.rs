//! # Phylon Brain
//!
//! Neural substrate for organisms: NEAT topology evolution (via `genetics`),
//! CTRNN dynamics, Hebbian plasticity, and neuromodulator channels.
//!
//! The brain crate defines the data structures and evaluation interfaces for
//! neural networks; it is deliberately independent of `burn`, `metabolism`,
//! and every other simulation crate to keep compilation fast and dependency
//! direction one-way. The systems that actually *drive* plasticity each
//! tick — reading metabolic state, applying [`Brain::apply_hebbian_update`]
//! — live in `organisms` (see `organisms::neuromodulator_system` and
//! `organisms::hebbian_plasticity_system`), since they need to bridge
//! `brain` with `metabolism`.
//!
//! CTRNN numerical integration itself runs on the GPU (`crates/gpu/src/brain.wgsl`);
//! this crate only defines the data layout that gets uploaded/read back, plus
//! the CPU-only Hebbian/pruning/winner-take-all logic — see
//! [`Brain::apply_hebbian_update`], [`Brain::prune_weak_synapses`], and
//! [`Brain::get_outputs`]'s doc comments for why each lives where it does.
//!
//! ## Not yet implemented
//!
//! Synaptic delay mapping and glial support effects are named in the
//! original spec but have no code here yet.

#![warn(missing_docs)]
#![warn(clippy::all)]

use serde::{Deserialize, Serialize};

/// A unique identifier for a neural brain instance.
///
/// Distinct from [`common::EntityId`] because brains persist in the lineage
/// record for cross-generation comparison studies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BrainId(pub u64);

/// Activation function types available to neural nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivationFn {
    /// Standard sigmoid: `1 / (1 + exp(-x))`.
    Sigmoid,
    /// Hyperbolic tangent.
    Tanh,
    /// Rectified linear unit: `max(0, x)`.
    ReLU,
    /// Leaky ReLU with slope 0.01 for negative inputs.
    LeakyReLU,
    /// Sinusoidal activation (useful for rhythmic/oscillatory behaviours).
    Sine,
    /// Gaussian activation `exp(-x^2)` (useful for bilateral symmetry in CPPNs).
    Gaussian,
    /// Absolute value `|x|`.
    Abs,
    /// Linear / Identity `x`.
    Linear,
    /// Step function: `0` if `x < 0`, else `1`.
    Step,
}

/// # Continuous-Time Recurrent Neural Node
///
/// ## 1. What Happens
/// The `CtrnnNode` represents a single artificial neuron with an internal state potential,
/// a time constant ($\tau$), an activation function, and indices tracking incoming synapses.
///
/// ## 2. Why It Happens
/// Unlike standard feed-forward networks, biological brains operate continuously in time.
/// CTRNNs emulate this by treating nodes as leaky integrators (differential equations) rather
/// than discrete step-functions. This naturally supports rhythmic pattern generation
/// (like walking gaits) and short-term memory.
///
/// ## 3. How It Happens
/// Nodes are strictly `#[repr(C)]` PODs (Plain Old Data) allowing direct upload to WGPU
/// buffers. The `first_synapse` and `synapse_count` fields allow a GPU shader to iterate
/// only over the incoming connections without pointer traversal.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Serialize, Deserialize)]
pub struct CtrnnNode {
    /// The current state/activation potential of the node.
    pub state: f32,
    /// The time constant (tau) dictating how fast the state updates.
    pub time_constant: f32,
    /// Bias added before activation.
    pub bias: f32,
    /// Activation function index (mapped from ActivationFn enum).
    pub activation: u32,
    /// Start index of synapses targeting this node.
    pub first_synapse: u32,
    /// Number of synapses targeting this node.
    pub synapse_count: u32,
}

/// # Neural Synapse
///
/// ## 1. What Happens
/// The `CtrnnSynapse` represents a directed, weighted connection between two `CtrnnNode`s.
///
/// ## 2. Why It Happens
/// Synapses define the topology and strength of information flow in the brain. Over
/// generations, evolution (NEAT) modifies these weights, adds new synapses, or splices
/// new nodes into existing synapses to grow complex cognitive architectures.
///
/// ## 3. How It Happens
/// Stored as a flat `Vec<CtrnnSynapse>` in the `Brain`, sorted by `target` index. The GPU
/// uses the target's `first_synapse` offset to quickly aggregate incoming signals:
/// $I_j = \sum (w_{ij} \cdot \sigma(state_i))$.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Serialize, Deserialize)]
pub struct CtrnnSynapse {
    /// Source node index.
    pub source: u32,
    /// Target node index.
    pub target: u32,
    /// Connection weight.
    pub weight: f32,
    /// Padding for 16-byte alignment.
    pub _padding: u32,
}

/// # Neural Region Identity (Phase 6, Epic C, Milestone N1a)
///
/// ## 1. What Happens
/// Identifies which anatomical cluster a `Brain` node belongs to — pure
/// CPU-side wiring metadata, one entry per `Brain::nodes` element (see
/// `Brain::node_regions`).
///
/// ## 2. Why It Happens
/// `PHASE4_EPIC1_NEURAL_ROADMAP.md`'s audit found brain wiring today is
/// purely index-driven (`growth_system` queries the CPPN with each node's
/// *normalized array index*, not its body position or anatomy) — there is
/// no way to express "these nodes belong to this Ganglion" at all. This is
/// the first, additive step: the field exists and defaults uniformly, with
/// no behavior change yet. Region-bound *wiring* (N1b) and real Ganglion
/// anchoring (N1c) are later, separate milestones.
///
/// ## 3. How It Happens
/// `RegionId::Central` is every node's default — `Brain::new` fills
/// `node_regions` with it uniformly, so no existing organism's brain
/// topology changes as a result of this type existing. `Ganglion(usize)`
/// names a specific developmental-graph position (see
/// `organisms::developmental_graph::DevelopmentalGraph`) once N1c starts
/// actually assigning it — unused by any wiring logic until then.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RegionId {
    /// No anatomical anchor — every node's default today.
    #[default]
    Central,
    /// Anchored to the `SegmentType::Ganglion` segment at this
    /// developmental-graph position (see
    /// `organisms::developmental_graph::DevelopmentalNode::position`).
    /// Not yet assigned by any wiring logic — reserved for N1c.
    Ganglion(usize),
}

/// # Organism Brain Substrate
///
/// ## 1. What Happens
/// The `Brain` component encapsulates the complete neural network (nodes and synapses)
/// driving an organism's behavior.
///
/// ## 2. Why It Happens
/// To achieve agency, the organism needs a control mechanism mapping sensory inputs
/// to muscular actuation. The `Brain` acts as the mapping layer. We encapsulate it into
/// flat arrays so it can be cloned cheaply during reproduction or sent to the GPU.
///
/// ## 3. How It Happens
/// The brain reads sensory data via `set_inputs`. During the `BrainEvaluation` pipeline,
/// the integration equations are evaluated (either on CPU or GPU). Afterwards, `get_outputs`
/// extracts the post-activation values to drive the organism's physics springs (muscles).
#[derive(bevy_ecs::prelude::Component, Debug, Clone, Serialize, Deserialize)]
pub struct Brain {
    /// Unique identifier for this brain.
    pub id: BrainId,
    /// Nodes in the network.
    pub nodes: Vec<CtrnnNode>,
    /// Synapses connecting the nodes.
    pub synapses: Vec<CtrnnSynapse>,
    /// Number of input nodes.
    pub input_count: usize,
    /// Number of output nodes.
    pub output_count: usize,
    /// Winner-take-all action gating: when true, [`Brain::get_outputs`]
    /// zeroes every output except the single largest-magnitude one — only
    /// one action "wins" per tick, instead of blending all outputs.
    pub winner_take_all: bool,
    /// Fixed-brain ablation mode: when false, [`Brain::apply_hebbian_update`]
    /// is a no-op for this brain — its synapse weights never adapt within
    /// its lifetime, letting a control group run alongside plastic siblings
    /// with everything else held equal.
    pub plasticity_enabled: bool,
    /// When `Some`, [`Brain::get_outputs`] returns this directly instead of
    /// reading node states — the hook external RL control
    /// (`learning::ExternalAgent`, see `app::learning_bridge`) uses to
    /// inject an action vector for this tick without disabling the CTRNN's
    /// own internal dynamics (they keep integrating; only the read-out is
    /// intercepted, so control can be handed back to the evolved brain at
    /// any tick with no discontinuity in its internal state).
    ///
    /// `#[serde(skip)]`: this is transient per-tick control state, not
    /// evolved/genetic data — skipping it means adding this field needed no
    /// `storage::SchemaVersion` bump, unlike `winner_take_all`/
    /// `plasticity_enabled` in Epic 8.
    #[serde(skip)]
    pub external_override: Option<Vec<f32>>,
    /// Phase 6, Epic C (N1a): which anatomical region each node in `nodes`
    /// belongs to, same length and index alignment as `nodes` — parallel,
    /// not embedded in `CtrnnNode` itself, since `CtrnnNode` is a
    /// `#[repr(C)]`/`Pod` GPU upload type and region is purely a CPU-side
    /// wiring-time concept (ADR-N1-01: no GPU buffer/shader change). Always
    /// `RegionId::Central` for every node until N1c starts assigning
    /// `Ganglion` regions. `nodes` is never reordered/resized after
    /// `Brain::new` (`prune_weak_synapses`/`reindex_synapses` only ever
    /// mutate `synapses`), so this stays index-aligned with no
    /// synchronization logic needed.
    pub node_regions: Vec<RegionId>,
}

impl Brain {
    /// Extracts the output values from the current node states.
    /// In the new architecture, the integration happens on the GPU,
    /// so this simply reads the post-activation output states.
    ///
    /// When [`Brain::winner_take_all`] is set, only the single
    /// largest-magnitude output survives (every other output is zeroed) —
    /// action gating instead of blended outputs.
    ///
    /// When [`Brain::external_override`] is `Some`, it's returned directly
    /// and neither `winner_take_all` gating nor the usual node-state
    /// readout applies — external control replaces the brain's output
    /// wholesale for this tick.
    pub fn get_outputs(&self) -> Vec<f32> {
        if let Some(override_actions) = &self.external_override {
            return override_actions.clone();
        }

        if self.nodes.is_empty() {
            return Vec::new();
        }

        let mut outputs = Vec::with_capacity(self.output_count);
        let start_idx = self.nodes.len().saturating_sub(self.output_count);
        for i in start_idx..self.nodes.len() {
            outputs.push(Self::apply_activation(
                self.nodes[i].state + self.nodes[i].bias,
                self.nodes[i].activation,
            ));
        }

        if self.winner_take_all {
            Self::gate_winner_take_all(&mut outputs);
        }

        outputs
    }

    /// Zeroes every output except the single largest-magnitude one.
    fn gate_winner_take_all(outputs: &mut [f32]) {
        let Some(winner) = outputs
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
            .map(|(i, _)| i)
        else {
            return;
        };
        for (i, o) in outputs.iter_mut().enumerate() {
            if i != winner {
                *o = 0.0;
            }
        }
    }

    /// Sets the input node states from sensor values.
    /// This happens on CPU before uploading the nodes to the GPU.
    pub fn set_inputs(&mut self, inputs: &[f32]) {
        for (i, &input_val) in inputs.iter().enumerate() {
            if i < self.input_count && i < self.nodes.len() {
                self.nodes[i].state = input_val;
            }
        }
    }

    /// Applies the mathematical activation function mapped to the given activation ID.
    pub fn apply_activation(x: f32, act_id: u32) -> f32 {
        match act_id {
            0 => 1.0 / (1.0 + (-x).exp()), // Sigmoid
            1 => x.tanh(),                 // Tanh
            2 => x.max(0.0),               // ReLU
            3 => {
                if x > 0.0 {
                    x
                } else {
                    0.01 * x
                }
            } // LeakyReLU
            4 => x.sin(),                  // Sine
            5 => (-x * x).exp(),           // Gaussian
            6 => x.abs(),                  // Abs
            7 => x,                        // Linear
            8 => {
                if x > 0.0 {
                    1.0
                } else {
                    0.0
                }
            } // Step
            _ => x,
        }
    }

    /// Creates a new functional CTRNN brain and properly sorts synapses for GPU compatibility.
    pub fn new(
        id: BrainId,
        mut nodes: Vec<CtrnnNode>,
        mut synapses: Vec<CtrnnSynapse>,
        input_count: usize,
        output_count: usize,
    ) -> Self {
        Self::reindex_synapses(&mut nodes, &mut synapses);
        let node_regions = vec![RegionId::Central; nodes.len()];

        Self {
            id,
            nodes,
            synapses,
            input_count,
            output_count,
            winner_take_all: false,
            plasticity_enabled: true,
            external_override: None,
            node_regions,
        }
    }

    /// Sets or clears this tick's external action override (see that
    /// field's doc comment) — `Some(actions)` to inject external control,
    /// `None` to return to the brain's own CTRNN-computed output.
    pub fn set_external_action_override(&mut self, actions: Option<Vec<f32>>) {
        self.external_override = actions;
    }

    /// Enables winner-take-all output gating (see that field's doc comment).
    /// Builder-style, so existing `Brain::new` call sites are unaffected.
    pub fn with_winner_take_all(mut self, enabled: bool) -> Self {
        self.winner_take_all = enabled;
        self
    }

    /// Sets whether this brain's synapses adapt via Hebbian plasticity (see
    /// [`Brain::plasticity_enabled`]'s doc comment for the ablation use
    /// case). Builder-style, so existing `Brain::new` call sites are
    /// unaffected.
    pub fn with_plasticity_enabled(mut self, enabled: bool) -> Self {
        self.plasticity_enabled = enabled;
        self
    }

    /// Sorts `synapses` by target node and rebuilds each node's
    /// `first_synapse`/`synapse_count` GPU-gather offsets — shared by
    /// [`Brain::new`] and [`Brain::prune_weak_synapses`], since removing
    /// synapses invalidates the same offsets a fresh brain needs computed.
    fn reindex_synapses(nodes: &mut [CtrnnNode], synapses: &mut [CtrnnSynapse]) {
        synapses.sort_by_key(|s| s.target);

        for node in nodes.iter_mut() {
            node.first_synapse = 0;
            node.synapse_count = 0;
        }

        if synapses.is_empty() {
            return;
        }

        let mut current_target = synapses[0].target as usize;
        let mut current_start = 0;
        let mut current_count = 0;

        for (i, syn) in synapses.iter().enumerate() {
            if syn.target as usize != current_target {
                if current_target < nodes.len() {
                    nodes[current_target].first_synapse = current_start;
                    nodes[current_target].synapse_count = current_count;
                }
                current_target = syn.target as usize;
                current_start = i as u32;
                current_count = 1;
            } else {
                current_count += 1;
            }
        }
        // Tail
        if current_target < nodes.len() {
            nodes[current_target].first_synapse = current_start;
            nodes[current_target].synapse_count = current_count;
        }
    }

    /// Removes synapses whose `|weight|` has decayed below `threshold`, then
    /// rebuilds GPU-gather offsets via `reindex_synapses` so the brain stays
    /// internally consistent after pruning.
    pub fn prune_weak_synapses(&mut self, threshold: f32) {
        self.synapses.retain(|s| s.weight.abs() >= threshold);
        Self::reindex_synapses(&mut self.nodes, &mut self.synapses);
    }

    /// Applies one Hebbian-plasticity step to every synapse's weight, using
    /// this tick's already-integrated CTRNN node states as the pre/post
    /// synaptic activity signal — "neurons that fire together, wire
    /// together," with an Oja-style weight-decay term so weights settle
    /// instead of growing without bound. A no-op when
    /// [`Brain::plasticity_enabled`] is false or `hebbian_rate` is `0.0`.
    ///
    /// Deliberately CPU-only rather than folded into the `brain.wgsl` CTRNN
    /// integration kernel: synapse weights are CPU-authoritative and
    /// re-uploaded fresh to the GPU every tick (see
    /// `crates/app/src/simulation.rs`), so mutating them on the GPU would
    /// need a second read-back round-trip for no benefit — this runs after
    /// this tick's GPU-integrated states are already back on the CPU (see
    /// `organisms::hebbian_plasticity_system`).
    pub fn apply_hebbian_update(&mut self, hebbian_rate: f32, weight_decay: f32, max_weight: f32) {
        if !self.plasticity_enabled || hebbian_rate == 0.0 {
            return;
        }

        let Brain {
            nodes, synapses, ..
        } = self;
        for syn in synapses.iter_mut() {
            let (Some(pre_node), Some(post_node)) = (
                nodes.get(syn.source as usize),
                nodes.get(syn.target as usize),
            ) else {
                continue;
            };
            let pre = Self::apply_activation(pre_node.state + pre_node.bias, pre_node.activation);
            let post =
                Self::apply_activation(post_node.state + post_node.bias, post_node.activation);
            let delta = hebbian_rate * (pre * post - weight_decay * syn.weight);
            syn.weight = (syn.weight + delta).clamp(-max_weight, max_weight);
        }
    }
}

/// Global tunables for Hebbian plasticity and synapse pruning — see
/// `organisms::hebbian_plasticity_system` for how each field is applied.
#[derive(bevy_ecs::prelude::Resource, Debug, Clone)]
pub struct PlasticityConfig {
    /// Base Hebbian learning rate applied to every plastic brain's synapses
    /// each tick, before [`Neuromodulators::dopamine`] scaling.
    pub hebbian_rate: f32,
    /// Oja-style weight-decay coefficient in [`Brain::apply_hebbian_update`]
    /// that keeps weights bounded instead of growing without limit.
    pub weight_decay: f32,
    /// Absolute clamp applied to every synapse weight after a Hebbian update.
    pub max_weight: f32,
    /// Synapses with `|weight|` below this are pruned every
    /// `prune_interval_ticks`.
    pub prune_threshold: f32,
    /// How often (in simulation ticks) pruning runs.
    pub prune_interval_ticks: u64,
    /// How much [`Neuromodulators::dopamine`] scales the base Hebbian rate:
    /// `effective_rate = hebbian_rate * (1.0 + dopamine_gain * dopamine)`.
    pub dopamine_gain: f32,
}

impl Default for PlasticityConfig {
    fn default() -> Self {
        Self {
            hebbian_rate: 0.01,
            weight_decay: 0.01,
            max_weight: 8.0,
            prune_threshold: 0.02,
            prune_interval_ticks: 600, // ~10s at the default 60 Hz tick rate
            dopamine_gain: 1.0,
        }
    }
}

/// # Neuromodulator Channels
///
/// Per-organism analogues of dopamine/serotonin/noradrenaline, updated each
/// tick from metabolic state (see `organisms::neuromodulator_system`) and
/// consumed to scale the effective Hebbian learning rate — a reward/stress
/// signal gating *how much* plasticity happens, not the plasticity rule
/// itself.
#[derive(bevy_ecs::prelude::Component, Debug, Clone, Copy)]
pub struct Neuromodulators {
    /// Reward signal: an exponential moving average of positive ATP deltas
    /// (energy gained this tick, normalized against `max_atp`), in `[0, 1]`.
    pub dopamine: f32,
    /// Satiety/stability signal: current ATP as a fraction of max ATP, in
    /// `[0, 1]`.
    pub serotonin: f32,
    /// Arousal/stress signal: `1.0 - serotonin`, high when energy reserves
    /// are low, in `[0, 1]`.
    pub noradrenaline: f32,
    last_atp: f32,
}

impl Neuromodulators {
    /// Creates a fresh, neutral neuromodulator state seeded from an
    /// organism's starting ATP, so the first tick's dopamine delta isn't
    /// computed against a bogus `0.0` baseline.
    pub fn new(initial_atp: f32) -> Self {
        Self {
            dopamine: 0.0,
            serotonin: 0.0,
            noradrenaline: 0.0,
            last_atp: initial_atp,
        }
    }

    /// Updates all three channels from this tick's ATP reading. Called once
    /// per organism per tick by `organisms::neuromodulator_system`.
    pub fn update(&mut self, atp: f32, max_atp: f32) {
        let capacity = max_atp.max(1e-6);
        let normalized_delta = ((atp - self.last_atp) / capacity).clamp(0.0, 1.0);
        self.dopamine = (self.dopamine * 0.9 + normalized_delta * 0.1).clamp(0.0, 1.0);
        self.serotonin = (atp / capacity).clamp(0.0, 1.0);
        self.noradrenaline = 1.0 - self.serotonin;
        self.last_atp = atp;
    }
}

/// # Per-Segment Hormone Level
///
/// A body segment's own local reading of the same three channels
/// [`Neuromodulators`] tracks — Phase 4, `PHASE4_ROADMAP.md` milestone
/// P4-F4. The organism's head carries the authoritative [`Neuromodulators`]
/// (driven directly by its own metabolic state); every other segment
/// carries a `HormoneLevel` instead, which `organisms::endocrine_diffusion_system`
/// relaxes toward its structural parent's level each tick (head's
/// `Neuromodulators` for a segment attached directly to the head, or an
/// upstream segment's own already-updated `HormoneLevel` otherwise) — an
/// unbroadcast, non-conserved diffusion (the source keeps its own level;
/// only the receiving side moves), unlike P4-F3's mass-conserving
/// `ChemicalEconomy` transport.
#[derive(
    bevy_ecs::prelude::Component, Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize,
)]
pub struct HormoneLevel {
    /// Local reading of the organism-wide dopamine channel, in `[0, 1]`.
    pub dopamine: f32,
    /// Local reading of the organism-wide serotonin channel, in `[0, 1]`.
    pub serotonin: f32,
    /// Local reading of the organism-wide noradrenaline channel, in `[0, 1]`.
    pub noradrenaline: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brain_id_equality() {
        assert_eq!(BrainId(1), BrainId(1));
    }

    #[test]
    fn activation_fn_is_copy() {
        let a = ActivationFn::Sigmoid;
        let _a2 = a;
    }

    fn two_node_brain(weight: f32) -> Brain {
        Brain::new(
            BrainId(0),
            vec![
                CtrnnNode {
                    state: 1.0,
                    time_constant: 1.0,
                    bias: 0.0,
                    activation: 7, // Linear
                    first_synapse: 0,
                    synapse_count: 0,
                },
                CtrnnNode {
                    state: 1.0,
                    time_constant: 1.0,
                    bias: 0.0,
                    activation: 7, // Linear
                    first_synapse: 0,
                    synapse_count: 0,
                },
            ],
            vec![CtrnnSynapse {
                source: 0,
                target: 1,
                weight,
                _padding: 0,
            }],
            1,
            1,
        )
    }

    #[test]
    fn new_defaults_to_plastic_and_not_winner_take_all() {
        let brain = two_node_brain(0.5);
        assert!(brain.plasticity_enabled);
        assert!(!brain.winner_take_all);
        assert!(brain.external_override.is_none());
    }

    #[test]
    fn external_override_replaces_computed_outputs() {
        let mut brain = two_node_brain(0.5);
        let normal_outputs = brain.get_outputs();
        assert_ne!(normal_outputs, vec![42.0]);

        brain.set_external_action_override(Some(vec![42.0]));
        assert_eq!(brain.get_outputs(), vec![42.0]);

        brain.set_external_action_override(None);
        assert_eq!(brain.get_outputs(), normal_outputs);
    }

    #[test]
    fn external_override_bypasses_winner_take_all() {
        let mut brain = two_node_brain(0.5).with_winner_take_all(true);
        brain.set_external_action_override(Some(vec![1.0, 2.0]));
        // Winner-take-all would zero one of these; override bypasses it.
        assert_eq!(brain.get_outputs(), vec![1.0, 2.0]);
    }

    #[test]
    fn winner_take_all_zeroes_all_but_largest_magnitude_output() {
        let brain = Brain::new(
            BrainId(0),
            vec![
                CtrnnNode {
                    state: 0.0,
                    time_constant: 1.0,
                    bias: 0.2,
                    activation: 7,
                    first_synapse: 0,
                    synapse_count: 0,
                },
                CtrnnNode {
                    state: 0.0,
                    time_constant: 1.0,
                    bias: -0.9,
                    activation: 7,
                    first_synapse: 0,
                    synapse_count: 0,
                },
            ],
            vec![],
            0,
            2,
        )
        .with_winner_take_all(true);
        let outputs = brain.get_outputs();
        assert_eq!(outputs[0], 0.0);
        assert_eq!(outputs[1], -0.9);
    }

    /// Phase 6, Epic C (N1a): `Brain::new` must default every node's region
    /// to `RegionId::Central`, one entry per node, with no wiring-behavior
    /// change — this is purely additive infrastructure per ADR-N1-01.
    #[test]
    fn brain_new_defaults_every_node_region_to_central() {
        let brain = two_node_brain(0.5);
        assert_eq!(brain.node_regions.len(), brain.nodes.len());
        assert!(brain
            .node_regions
            .iter()
            .all(|region| *region == RegionId::Central));
    }

    /// `RegionId::default()` (the derive N1a relies on for future
    /// convenience constructors) must agree with what `Brain::new` actually
    /// assigns — both should mean "no anatomical anchor."
    #[test]
    fn region_id_default_is_central() {
        assert_eq!(RegionId::default(), RegionId::Central);
    }

    #[test]
    fn hebbian_update_is_noop_when_plasticity_disabled() {
        let mut brain = two_node_brain(0.5).with_plasticity_enabled(false);
        brain.apply_hebbian_update(0.1, 0.01, 8.0);
        assert_eq!(brain.synapses[0].weight, 0.5);
    }

    #[test]
    fn hebbian_update_moves_weight_toward_correlated_activity() {
        let mut brain = two_node_brain(0.0);
        // Both nodes are active (state 1.0, linear activation), so
        // pre*post > 0 — the weight should grow from zero.
        brain.apply_hebbian_update(0.1, 0.0, 8.0);
        assert!(brain.synapses[0].weight > 0.0);
    }

    #[test]
    fn hebbian_update_respects_max_weight_clamp() {
        let mut brain = two_node_brain(7.99);
        for _ in 0..1000 {
            brain.apply_hebbian_update(0.5, 0.0, 8.0);
        }
        assert!(brain.synapses[0].weight <= 8.0);
    }

    #[test]
    fn prune_weak_synapses_removes_below_threshold_and_reindexes() {
        let mut brain = two_node_brain(0.01);
        brain.prune_weak_synapses(0.1);
        assert!(brain.synapses.is_empty());
        assert_eq!(brain.nodes[1].synapse_count, 0);
    }

    #[test]
    fn prune_weak_synapses_keeps_strong_connections() {
        let mut brain = two_node_brain(5.0);
        brain.prune_weak_synapses(0.1);
        assert_eq!(brain.synapses.len(), 1);
        assert_eq!(brain.nodes[1].synapse_count, 1);
    }

    #[test]
    fn neuromodulators_dopamine_rises_on_atp_gain_and_serotonin_tracks_satiety() {
        let mut neuro = Neuromodulators::new(50.0);
        neuro.update(60.0, 100.0);
        assert!(neuro.dopamine > 0.0);
        assert_eq!(neuro.serotonin, 0.6);
        assert!((neuro.noradrenaline - 0.4).abs() < 1e-6);
    }

    #[test]
    fn neuromodulators_dopamine_stays_zero_on_atp_loss() {
        let mut neuro = Neuromodulators::new(50.0);
        neuro.update(40.0, 100.0);
        assert_eq!(neuro.dopamine, 0.0);
    }
}
