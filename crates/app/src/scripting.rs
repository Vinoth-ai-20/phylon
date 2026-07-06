use crate::app::PhylonApp;

/// Applies one `plugins::ScriptCommand` to a running `PhylonApp`,
/// dispatching to the same shared intervention methods
/// (`crate::interventions`) that `events.rs`'s live menu handler and
/// `app::replay`'s playback driver already use.
fn apply_script_command(app: &mut PhylonApp, command: &plugins::ScriptCommand) {
    match command {
        plugins::ScriptCommand::ReseedEcosystem => app.apply_reseed_ecosystem(),
        plugins::ScriptCommand::SpawnPreset { name, x, y } => {
            app.apply_spawn_preset(name, common::Vec2::new(*x, *y))
        }
        plugins::ScriptCommand::SpawnProtoFish { x, y } => {
            app.apply_spawn_proto_fish(common::Vec2::new(*x, *y))
        }
        plugins::ScriptCommand::SpawnManualHazard { x, y } => {
            let tick = app.current_tick();
            app.apply_spawn_manual_hazard(common::Vec2::new(*x, *y), tick)
        }
    }
}

/// # Scenario / Scripted Intervention Runner
///
/// ## 1. What Happens
/// Reads and runs a `.rhai` script (see `plugins::PluginEngine`), exposing
/// the current live population as a read-only `population` context
/// variable, then applies every command the script requested, in order.
///
/// ## 2. Why It Happens
/// Scenario authoring and scripted mid-run interventions are the same
/// underlying operation ("run this script against the current world
/// state") at two different call sites: `main.rs` calls this once at
/// startup for `research.scenario_path`, and periodically during the
/// headless tick loop for `research.periodic_script_path`.
///
/// ## 3. How It Happens
/// `plugins::PluginEngine` never touches `bevy_ecs` directly (see that
/// crate's doc comment) — this function is the bridge, the same role
/// `app::batch`/`app::analytics_bridge` play for `research`/`analytics`.
pub(crate) fn run_script_file(
    app: &mut PhylonApp,
    path: &std::path::Path,
) -> Result<usize, plugins::PluginError> {
    let mut genome_query = app.world.ecs.query::<&genetics::Genome>();
    let population = genome_query.iter(&app.world.ecs).count() as i64;

    let engine = plugins::PluginEngine::new();
    let script = std::fs::read_to_string(path)?;
    let commands = engine
        .run_script_with_context(&script, &[("population", rhai::Dynamic::from(population))])?;

    let command_count = commands.len();
    for command in &commands {
        apply_script_command(app, command);
    }
    Ok(command_count)
}
