use crate::app::PhylonApp;

/// Human-readable one-line description of a recorded `ReplayAction`, for the
/// Replay Browser panel (`ui::MenuAction::OpenReplayBundle`) — static
/// inspection of a bundle's contents, not live playback (see that action's
/// doc comment for why the two are separate).
pub(crate) fn describe_action(action: &storage::replay::ReplayAction) -> String {
    match action {
        storage::replay::ReplayAction::ReseedEcosystem => "Reseed Ecosystem".to_string(),
        storage::replay::ReplayAction::SpawnPreset { name, position } => {
            format!(
                "Spawn Preset \"{name}\" at ({:.0}, {:.0})",
                position.x, position.y
            )
        }
        storage::replay::ReplayAction::SpawnProtoFish { position } => {
            format!("Spawn Proto-Fish at ({:.0}, {:.0})", position.x, position.y)
        }
        storage::replay::ReplayAction::SpawnManualHazard { position } => {
            format!(
                "Spawn Manual Hazard at ({:.0}, {:.0})",
                position.x, position.y
            )
        }
    }
}

/// Applies one recorded `storage::replay::ReplayAction` to a running
/// `PhylonApp`, dispatching to the shared intervention methods in
/// `crate::interventions` — the same code `events.rs`'s live menu-action
/// handler calls, so playback can never silently diverge from what
/// actually happened during recording.
fn apply_replay_action(app: &mut PhylonApp, tick: u64, action: &storage::replay::ReplayAction) {
    match action {
        storage::replay::ReplayAction::ReseedEcosystem => {
            app.apply_reseed_ecosystem();
        }
        storage::replay::ReplayAction::SpawnPreset { name, position } => {
            app.apply_spawn_preset(name, position.clone().into());
        }
        storage::replay::ReplayAction::SpawnProtoFish { position } => {
            app.apply_spawn_proto_fish(position.clone().into());
        }
        storage::replay::ReplayAction::SpawnManualHazard { position } => {
            app.apply_spawn_manual_hazard(position.clone().into(), tick);
        }
    }
}

/// Deterministic replay playback.
///
/// Restores a `PhylonApp` to `bundle.initial_snapshot`'s exact state
/// (including reseeding `common::SimRng` — the single seeded RNG every
/// stochastic system must draw from, see `app.rs`'s module doc — from
/// `bundle.log.seed`, since restoring world state alone is not sufficient
/// for deterministic continuation without also restoring the RNG stream
/// that state was produced from), then steps the simulation forward tick by
/// tick via the same `update_simulation` every interactive/headless/batch
/// run uses, re-applying every recorded intervention at the exact tick it
/// was originally applied.
///
/// This is the whole point of recording *interventions* instead of per-tick
/// state (see `storage::replay::ReplayLog`'s doc comment): everything
/// between interventions is already perfectly reproducible from the seeded
/// `SimRng` alone, so replay only needs to re-inject the non-deterministic
/// *external* inputs at the right moments.
///
/// `speed_multiplier` controls real-time pacing when `realtime_pacing` is
/// requested: sleeping `tick_duration / speed_multiplier` between ticks
/// gives an "Nx speed" viewing experience; when `realtime_pacing` is
/// `false` (the default, matching headless/batch mode), ticks run back to
/// back as fast as possible and `speed_multiplier` is ignored — there's no
/// real-time frame to pace against.
pub(crate) fn run_replay(
    sim_config: &config::PhylonConfig,
    bundle: &storage::replay::ReplayBundle,
    target_tick: u64,
    speed_multiplier: f32,
    realtime_pacing: bool,
) -> PhylonApp {
    let mut app = PhylonApp::new(sim_config.clone());
    if let Err(e) = app.init_gpu_headless() {
        tracing::error!("replay failed to init headless GPU: {e}");
        return app;
    }

    bundle.initial_snapshot.restore_world(&mut app.world.ecs);
    app.world
        .ecs
        .insert_resource(common::SimRng::from_seed(bundle.log.seed));

    let dt = app.world.ecs.resource::<common::TickRate>().dt();
    let tick_duration = std::time::Duration::from_secs_f32(dt.max(f32::EPSILON));
    let paced_duration = tick_duration.div_f32(speed_multiplier.max(f32::EPSILON));

    let mut tick = 0u64;
    while tick < target_tick {
        for action in bundle.log.events_at(tick) {
            apply_replay_action(&mut app, tick, action);
        }

        let start = std::time::Instant::now();
        app.update_simulation();
        tick += 1;

        if realtime_pacing {
            let elapsed = start.elapsed();
            if elapsed < paced_duration {
                std::thread::sleep(paced_duration - elapsed);
            }
        }
    }

    app
}
