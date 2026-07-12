//! # MARL Command Bridge
//!
//! Implements the four `network::MarlCommand` handlers `main.rs`'s headless
//! multi-agent reinforcement learning (MARL) loop dispatches to:
//! [`get_state`] returns a real organism's observation, [`set_actions`]
//! actually drives that organism's behavior, [`reset`] resets the
//! ecosystem, and [`set_difficulty`] scales hazard frequency/severity for
//! curriculum learning.
//!
//! `learning`/`network` deliberately stay decoupled from `sensing`/`brain`/
//! live `bevy_ecs::World` access (see `learning::ExternalAgent`'s doc
//! comment) — `app`, the composition root, is where that decoupling gets
//! bridged into real effects, the same role `app::batch`/
//! `app::analytics_bridge`/`app::scripting` play for `research`/
//! `analytics`/`plugins`.
//!
//! Exactly one organism carries `learning::ExternalAgent` at a time (see
//! [`ensure_agent_assigned`]); [`get_state`] reads its
//! `sensing::SensoryState`, and [`set_actions`] writes its
//! `brain::Brain::external_override` (see that method's doc comment for why
//! this doesn't disable the organism's own CTRNN — continuous-time
//! recurrent neural network, the organism brain model — just intercepts its
//! read-out).

use crate::app::PhylonApp;
use bevy_ecs::prelude::*;

/// Finds the current `learning::ExternalAgent`, auto-assigning the first
/// organism with both `SensoryState` and `Brain` if none exists yet (e.g.
/// right after startup or a reset). Returns `None` if no eligible organism
/// exists at all.
fn ensure_agent_assigned(app: &mut PhylonApp) -> Option<Entity> {
    let mut existing = app
        .world
        .ecs
        .query_filtered::<Entity, With<learning::ExternalAgent>>();
    if let Some(entity) = existing.iter(&app.world.ecs).next() {
        return Some(entity);
    }

    let mut candidates = app
        .world
        .ecs
        .query::<(Entity, &sensing::SensoryState, &brain::Brain)>();
    let candidate = candidates.iter(&app.world.ecs).next().map(|(e, ..)| e);

    if let Some(entity) = candidate {
        app.world
            .ecs
            .entity_mut(entity)
            .insert(learning::ExternalAgent);
    }
    candidate
}

/// Returns the current external agent's observation vector (its
/// `sensing::SensoryState::inputs`, already exactly `learning::ObservationVector`'s
/// shape), or an empty vector if no eligible organism exists.
pub(crate) fn get_state(app: &mut PhylonApp) -> learning::ObservationVector {
    let Some(agent) = ensure_agent_assigned(app) else {
        return Vec::new();
    };
    app.world
        .ecs
        .get::<sensing::SensoryState>(agent)
        .map(|s| s.inputs.clone())
        .unwrap_or_default()
}

/// Injects `actions` as the current external agent's brain output override
/// for this tick (see `brain::Brain::set_external_action_override`).
/// No-ops if no eligible organism exists.
pub(crate) fn set_actions(app: &mut PhylonApp, actions: &learning::ActionVector) {
    let Some(agent) = ensure_agent_assigned(app) else {
        return;
    };
    if let Some(mut brain) = app.world.ecs.get_mut::<brain::Brain>(agent) {
        brain.set_external_action_override(Some(actions.clone()));
    }
}

/// Resets the ecosystem (via the same `apply_reseed_ecosystem` the
/// interactive menu and replay playback use) — the previous agent
/// assignment is despawned along with everything else; the next
/// `get_state`/`set_actions` call re-assigns a fresh one.
pub(crate) fn reset(app: &mut PhylonApp) {
    app.apply_reseed_ecosystem();
}

/// Scales hazard frequency/severity for curriculum learning (see
/// `ecology::catastrophe::CatastropheConfig::set_difficulty`).
pub(crate) fn set_difficulty(app: &mut PhylonApp, level: f32) {
    if let Some(mut config) = app
        .world
        .ecs
        .get_resource_mut::<ecology::catastrophe::CatastropheConfig>()
    {
        config.set_difficulty(level);
    }
}
