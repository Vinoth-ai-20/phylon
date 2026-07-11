//! Runtime Behavior Validation (Phase 9, Goal 3) — measures, rather than
//! assumes, whether the movement-pipeline fix (Goal 2's `seed_ecosystem`
//! mutation-dosage and apoptosis fixes) actually produces the downstream
//! behaviors those fixes were meant to restore: sustained foraging,
//! reproduction, predator/prey interaction, and long-run physical stability.
//!
//! **Purely observational**, mirroring `motion_diagnostic`'s own precedent
//! exactly: reads existing `ParticleNode`/`Diet`/`ChemicalEconomy`/
//! `PhylonEvent`/`LineageTracker` state and logs it — no new simulation
//! logic, no parallel system.
//!
//! **Opt-in, zero-cost when off:** gated behind `PHYLON_BEHAVIOR_VALIDATION`,
//! checked once at startup via `BehaviorValidationConfig`.
//!
//! **State lives in a `Resource`, not `Local`/`EventReader`'s internal
//! cursor persistence assumption** — see `motion_diagnostic`'s doc comment
//! for why: the live app drives every system via `RunSystemOnce`, which
//! builds a fresh `SystemState` (and thus a fresh event-reader cursor) every
//! tick. This module's own `EventReader<events::PhylonEvent>` parameter
//! still works correctly across ticks despite that reset, for the same
//! reason `crates/app/src/systems.rs`'s `interaction_event_log_system`
//! already relies on: `Events::<PhylonEvent>::update()` runs exactly once
//! per tick (see `simulation.rs`), so a fresh reader each tick only ever
//! sees the one tick's worth of not-yet-aged-out events — cumulative counts
//! are what get carried in `BehaviorValidationState`, a real `Resource`.

use bevy_ecs::prelude::*;

/// Whether behavior validation is active this run, decided once at startup.
#[derive(Resource, Debug, Clone, Copy)]
pub struct BehaviorValidationConfig {
    /// `true` if `PHYLON_BEHAVIOR_VALIDATION` was set when the app started.
    pub enabled: bool,
}

impl BehaviorValidationConfig {
    /// Reads the environment once; call at app startup and insert the
    /// result as a resource, so per-tick checks are a plain bool read.
    pub fn from_env() -> Self {
        Self {
            enabled: std::env::var("PHYLON_BEHAVIOR_VALIDATION").is_ok(),
        }
    }
}

/// Ticks between logged windows.
const WINDOW_TICKS: u64 = 300;

/// Any `ParticleNode` velocity above this magnitude (world units/sec) is
/// treated as an "exploding velocity" anomaly worth flagging — deliberately
/// generous (real organism top speeds observed this session were well under
/// 100 units/sec) so this only fires on genuine instability, not fast-but-
/// legitimate motion.
const MAX_SANE_SPEED: f32 = 5_000.0;

/// Cross-tick state — see this module's doc comment for why this is a
/// `Resource`, not `Local`/an assumed-persistent `EventReader` cursor.
#[derive(Resource, Default)]
pub struct BehaviorValidationState {
    tick: u64,
    births_since_start: u64,
    reproductions_since_start: u64,
    deaths_starvation: u64,
    deaths_predation: u64,
    deaths_disease: u64,
    deaths_senescence: u64,
    deaths_other: u64,
    /// Ticks (not necessarily contiguous) on which a NaN or exploding
    /// velocity was observed anywhere in the population.
    anomaly_ticks_total: u64,
    first_anomaly_logged: bool,
}

/// # Behavior Validation System
///
/// ## 1. What Happens
/// Every tick: tallies `PhylonEvent`s (births/deaths-by-cause/reproductions)
/// into cross-tick totals, and scans every `ParticleNode` for a NaN or
/// exploding velocity (logging the first occurrence immediately, at ERROR,
/// rather than waiting for the next window boundary — a stability failure
/// is worth surfacing the instant it's observed). Every `WINDOW_TICKS`,
/// additionally logs: population per `Diet`, live species count (from
/// `evolution::LineageTracker`), and average/max speed + average ATP among
/// sampled mobile (non-Producer, brain-wired) organisms — the same
/// `ParticleNode`-based speed measurement `motion_diagnostic` already
/// established, extended with `ChemicalEconomy::atp` to correlate energy
/// against motion.
///
/// ## 2. Why It Happens
/// Phase 9 Goal 3 requires measuring, not assuming, that the Goal 2
/// locomotion fix produces real downstream behavior over a long run:
/// sustained foraging/reproduction (population not collapsing to zero),
/// real predator/prey interaction (`DeathCause::Predation` counts rising),
/// hazard avoidance and starvation resistance (`DeathCause::Starvation` not
/// dominating), species divergence (species count `> 1` and changing), and
/// physical stability (no NaN/exploding velocities, stable average speed).
///
/// ## 3. How It Happens
/// See field-level comments on `BehaviorValidationState` and the per-window
/// query below.
#[allow(clippy::too_many_arguments)]
pub(crate) fn behavior_validation_system(
    config: Res<BehaviorValidationConfig>,
    mut state: ResMut<BehaviorValidationState>,
    mut phylon_events: EventReader<events::PhylonEvent>,
    node_query: Query<(Entity, &physics::ParticleNode)>,
    diet_query: Query<&ecology::Diet>,
    mobile_query: Query<
        (
            &physics::ParticleNode,
            &ecology::Diet,
            &metabolism::ChemicalEconomy,
        ),
        With<brain::Brain>,
    >,
    lineage_tracker: Option<Res<evolution::LineageTracker>>,
) {
    if !config.enabled {
        return;
    }
    state.tick += 1;

    // ── Every tick: NaN / exploding-velocity guard ─────────────────────────
    let anomaly = node_query.iter().find_map(|(entity, node)| {
        let pos_bad = !node.position.x.is_finite()
            || !node.position.y.is_finite()
            || !node.position.z.is_finite();
        let vel_bad = !node.velocity.x.is_finite()
            || !node.velocity.y.is_finite()
            || !node.velocity.z.is_finite()
            || node.velocity.length() > MAX_SANE_SPEED;
        (pos_bad || vel_bad).then_some((entity, node.position, node.velocity))
    });
    if let Some((entity, position, velocity)) = anomaly {
        state.anomaly_ticks_total += 1;
        if !state.first_anomaly_logged {
            tracing::error!(
                target: "behavior_validation",
                tick = state.tick,
                entity = ?entity,
                position = ?position,
                velocity = ?velocity,
                "Phase 9 Goal 3: NaN or exploding velocity detected"
            );
            state.first_anomaly_logged = true;
        }
    }

    // ── Every tick: tally events (reader sees only this tick's fresh ones,
    // per this module's doc comment) ───────────────────────────────────────
    for event in phylon_events.read() {
        match event {
            events::PhylonEvent::OrganismBorn { .. } => state.births_since_start += 1,
            events::PhylonEvent::ReproductionEvent { .. } => state.reproductions_since_start += 1,
            events::PhylonEvent::OrganismDied { cause, .. } => match cause {
                events::DeathCause::Starvation => state.deaths_starvation += 1,
                events::DeathCause::Predation => state.deaths_predation += 1,
                events::DeathCause::Disease => state.deaths_disease += 1,
                events::DeathCause::Senescence => state.deaths_senescence += 1,
                _ => state.deaths_other += 1,
            },
            _ => {}
        }
    }

    if !state.tick.is_multiple_of(WINDOW_TICKS) {
        return;
    }

    // ── Every window: population, species, speed/energy summary ───────────
    let total_population = diet_query.iter().count();
    let mut producers = 0usize;
    let mut herbivores = 0usize;
    let mut carnivores = 0usize;
    let mut omnivores = 0usize;
    let mut decomposers = 0usize;
    for diet in diet_query.iter() {
        match diet {
            ecology::Diet::Producer => producers += 1,
            ecology::Diet::Herbivore => herbivores += 1,
            ecology::Diet::Carnivore => carnivores += 1,
            ecology::Diet::Omnivore => omnivores += 1,
            ecology::Diet::Decomposer => decomposers += 1,
        }
    }

    let species_count = lineage_tracker
        .as_ref()
        .map(|tracker| {
            let mut species: Vec<u64> = tracker
                .active_records()
                .map(|record| record.species.0)
                .collect();
            species.sort_unstable();
            species.dedup();
            species.len()
        })
        .unwrap_or(0);

    let mobile_samples: Vec<_> = mobile_query
        .iter()
        .filter(|(_, diet, _)| **diet != ecology::Diet::Producer)
        .collect();
    let mobile_count = mobile_samples.len();
    let (avg_speed, max_speed, avg_atp) = if mobile_count > 0 {
        let sum_speed: f32 = mobile_samples
            .iter()
            .map(|(node, _, _)| node.velocity.length())
            .sum();
        let max_speed = mobile_samples
            .iter()
            .map(|(node, _, _)| node.velocity.length())
            .fold(0.0f32, f32::max);
        let sum_atp: f32 = mobile_samples.iter().map(|(_, _, chem)| chem.atp).sum();
        (
            sum_speed / mobile_count as f32,
            max_speed,
            sum_atp / mobile_count as f32,
        )
    } else {
        (0.0, 0.0, 0.0)
    };

    tracing::info!(
        target: "behavior_validation",
        tick = state.tick,
        total_population,
        producers,
        herbivores,
        carnivores,
        omnivores,
        decomposers,
        species_count,
        mobile_count,
        avg_speed,
        max_speed,
        avg_atp,
        births_since_start = state.births_since_start,
        reproductions_since_start = state.reproductions_since_start,
        deaths_starvation = state.deaths_starvation,
        deaths_predation = state.deaths_predation,
        deaths_disease = state.deaths_disease,
        deaths_senescence = state.deaths_senescence,
        deaths_other = state.deaths_other,
        anomaly_ticks_total = state.anomaly_ticks_total,
        "Phase 9 Goal 3 behavior validation window"
    );
}
