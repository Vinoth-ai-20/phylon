//! # Runtime Motion Diagnostic
//!
//! Measures, rather than assumes, whether organisms actually move and by
//! how much — an opt-in, purely observational instrument for investigating
//! locomotion-fidelity questions (e.g. "why do organisms appear static") by
//! logging real per-organism movement and effector-actuation numbers,
//! rather than guessing whether the cause is a physics bug, a neural-output
//! problem, or something else entirely (e.g. every effector spring on the
//! sampled population having zero actuation amplitude, which looks
//! identical to a physics bug from the outside but has a completely
//! different fix).
//!
//! **Purely observational.** This module reads existing `Brain`/`MotorSystem`/
//! `Spring`/`ParticleNode` state and logs it — it does not duplicate or
//! reimplement any of `behavior`/`brain`/`physics`'s own logic, so there is
//! no risk of this diagnostic and the real systems drifting apart on what
//! "actuation" or "movement" means.
//!
//! **Opt-in, zero-cost when off:** gated behind the `PHYLON_MOTION_DIAGNOSTIC`
//! environment variable, checked once at startup (not per-tick) via
//! `MotionDiagnosticConfig`. When unset, `motion_diagnostic_system` returns
//! immediately after one cheap resource read.
//!
//! **Why state lives in a `Resource`, not `Local`:** the live app drives
//! every system via `bevy_ecs::system::RunSystemOnce::run_system_once`
//! (see `crate::simulation`'s module doc), which constructs a fresh,
//! ephemeral `SystemState` on every call — a `Local<T>` parameter is reset
//! to its default every single tick, not persisted across ticks the way it
//! would be under a real `bevy_ecs::schedule::Schedule`. A tick counter or
//! per-organism accumulator stored as `Local` would therefore never
//! advance past its initial value across ticks, since each
//! `run_system_once` call sees a fresh default rather than the previous
//! tick's value. `MotionDiagnosticState` below avoids this by living in the
//! ECS `World` as a `Resource`, which *does* persist across separate
//! `run_system_once` calls.

use bevy_ecs::prelude::*;

/// Whether the motion diagnostic is active this run, decided once at
/// startup from the `PHYLON_MOTION_DIAGNOSTIC` environment variable (any
/// value, including empty, counts as set) — not re-read per tick.
#[derive(Resource, Debug, Clone, Copy)]
pub struct MotionDiagnosticConfig {
    /// `true` if `PHYLON_MOTION_DIAGNOSTIC` was set when the app started.
    pub enabled: bool,
}

impl MotionDiagnosticConfig {
    /// Reads the environment once; call at app startup and insert the
    /// result as a resource, so per-tick checks are a plain bool read.
    pub fn from_env() -> Self {
        Self {
            enabled: std::env::var("PHYLON_MOTION_DIAGNOSTIC").is_ok(),
        }
    }
}

/// Ticks between logged windows — once per second at the default 60Hz tick
/// rate, coarse enough to keep the diagnostic's logging overhead negligible
/// while still resolving movement trends over a human-readable timescale.
const WINDOW_TICKS: u64 = 60;

/// How many organisms (with a wired `Brain` + `MotorSystem`) to sample per
/// window — small and fixed, so this remains cheap regardless of population
/// size.
const SAMPLE_COUNT: usize = 5;

/// Per-sampled-organism accumulator, reset at the end of every logged
/// window.
#[derive(Default, Clone)]
pub struct SampleAccumulator {
    /// Position at the start of this window, for net (start-to-end)
    /// displacement — distinguishes "traveled somewhere" from "wiggled in
    /// place."
    window_start_position: Option<common::Vec3>,
    /// Sum of `|position_t - position_{t-1}|` across every tick in the
    /// window — total path length traveled, distinct from net displacement.
    total_path_length: f32,
    /// Highest instantaneous speed (`velocity.length()`) observed this
    /// window.
    max_speed: f32,
    /// Last known position, for computing this tick's path-length delta.
    last_position: Option<common::Vec3>,
}

/// Cross-tick state for the motion diagnostic — see this module's doc
/// comment for why this is a `Resource` and not `Local` system-params.
#[derive(Resource, Default)]
pub struct MotionDiagnosticState {
    tick: u64,
    accumulators: std::collections::HashMap<Entity, SampleAccumulator>,
}

/// Logs real per-organism movement and effector-actuation numbers once per
/// window (see `WINDOW_TICKS`), for up to `SAMPLE_COUNT` organisms.
///
/// For each sampled organism, logs: total path length traveled, net
/// displacement, max instantaneous speed, the organism's live `Brain`
/// output vector, and how many of its effector springs currently have
/// positive vs. negative `actuation_amplitude` — `muscle_actuation.wgsl`'s
/// actuation logic treats these two signs asymmetrically, so a population
/// skewed heavily toward one sign is itself diagnostically meaningful, not
/// only the raw effector count.
///
/// Before treating "organisms appear static" as any specific kind of bug,
/// this system produces the numbers that distinguish the real possibilities
/// (no actuatable effectors at all, effectors present but not actuating,
/// actuating but too weakly to visibly move, or actually moving but hard to
/// perceive visually) instead of guessing.
///
/// Accumulates per-organism path length/speed every tick via
/// `MotionDiagnosticState` (reset per window, persisted across the whole
/// run via the ECS `Resource` mechanism — see this module's doc comment),
/// then logs one `tracing::info!` line per sampled organism at each window
/// boundary. Sampling is by entity insertion order (first `SAMPLE_COUNT`
/// encountered each window) — not statistically randomized, but sufficient
/// to distinguish "no organism moves" from "organisms move a measurable
/// amount."
pub(crate) fn motion_diagnostic_system(
    config: Res<MotionDiagnosticConfig>,
    mut state: ResMut<MotionDiagnosticState>,
    query: Query<(
        Entity,
        &physics::ParticleNode,
        &brain::Brain,
        &behavior::MotorSystem,
        &ecology::Diet,
    )>,
    spring_query: Query<&physics::Spring>,
) {
    if !config.enabled {
        return;
    }
    state.tick += 1;

    // Sample non-Producer organisms preferentially — `Diet::Producer`
    // seeds are deliberately short, static, effector-less bodies (see
    // `species_seed.rs`'s producer seed genome comment), so sampling the
    // first N entities encountered (which skew Producer, spawned in bulk)
    // would measure "do plants move" rather than the question this
    // diagnostic actually exists to answer.
    let sample: Vec<_> = query
        .iter()
        .filter(|(_, _, _, _, diet)| **diet != ecology::Diet::Producer)
        .take(SAMPLE_COUNT)
        .map(|(e, _, _, _, _)| e)
        .collect();

    // One-time-per-window population summary: how many non-Producer,
    // brain-wired organisms exist right now, and what fraction of them have
    // zero actuatable (Elastic/Rotational) effector springs at all —
    // distinguishes "no effectors" as a sampling artifact (a few sampled
    // organisms happen to have none) from a population-wide body-plan
    // phenomenon (most/all organisms have none).
    if state.tick.is_multiple_of(WINDOW_TICKS) {
        let mobile_diet_total = query
            .iter()
            .filter(|(_, _, _, _, diet)| **diet != ecology::Diet::Producer)
            .count();
        let mobile_diet_zero_effectors = query
            .iter()
            .filter(|(_, _, _, motor, diet)| {
                **diet != ecology::Diet::Producer && motor.effectors.is_empty()
            })
            .count();
        tracing::info!(
            target: "motion_diagnostic",
            tick = state.tick,
            mobile_diet_total,
            mobile_diet_zero_effectors,
            "motion_diagnostic population effector summary"
        );
    }

    // Accumulate every tick for whichever entities are currently sampled.
    for (entity, node, _brain, _motor, _diet) in query.iter().filter(|(e, ..)| sample.contains(e)) {
        let acc = state.accumulators.entry(entity).or_default();
        if acc.window_start_position.is_none() {
            acc.window_start_position = Some(node.position);
        }
        if let Some(last) = acc.last_position {
            acc.total_path_length += (node.position - last).length();
        }
        acc.last_position = Some(node.position);
        acc.max_speed = acc.max_speed.max(node.velocity.length());
    }

    if !state.tick.is_multiple_of(WINDOW_TICKS) {
        return;
    }

    for (entity, node, brain, motor, _diet) in query.iter().filter(|(e, ..)| sample.contains(e)) {
        let Some(acc) = state.accumulators.get(&entity) else {
            continue;
        };
        let net_displacement = acc
            .window_start_position
            .map(|start| (node.position - start).length())
            .unwrap_or(0.0);

        let (positive_amplitude_count, negative_amplitude_count) = motor
            .effectors
            .iter()
            .filter_map(|&spring_entity| spring_query.get(spring_entity).ok())
            .fold((0u32, 0u32), |(pos, neg), spring| {
                if spring.actuation_amplitude > 0.0 {
                    (pos + 1, neg)
                } else if spring.actuation_amplitude < 0.0 {
                    (pos, neg + 1)
                } else {
                    (pos, neg)
                }
            });

        tracing::info!(
            target: "motion_diagnostic",
            tick = state.tick,
            entity = ?entity,
            total_path_length = acc.total_path_length,
            net_displacement = net_displacement,
            max_speed = acc.max_speed,
            brain_outputs = ?brain.get_outputs(),
            effector_count = motor.effectors.len(),
            positive_amplitude_effectors = positive_amplitude_count,
            negative_amplitude_effectors = negative_amplitude_count,
            "motion_diagnostic sample"
        );
    }

    state.accumulators.clear();
}
