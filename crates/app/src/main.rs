//! # Phylon Application
//!
//! The main binary entry point for the Phylon simulation.
//!
//! ## Responsibilities
//!
//! 1. Parse CLI arguments and locate the config file.
//! 2. Initialise structured logging via `tracing_subscriber`.
//! 3. Load `PhylonConfig` from `data/default.ron` (falls back to defaults).
//! 4. If `research.batch_seeds` is non-empty, run [`batch::run_batch`] and exit
//!    (see that function's doc comment) — otherwise:
//! 5. Create a `winit` `EventLoop` and application window (or, if
//!    `research.headless` is set, a headless GPU context and a manual tick
//!    loop instead — see the `is_headless` branch below).
//! 6. Initialise a `wgpu` surface on the window.
//! 7. Run the event loop, calling `PhylonApp::update_simulation` each tick
//!    (the per-tick system order lives in `simulation::update_simulation`;
//!    Phase 6, Epic A removed the `SimulationScheduler` this step used to
//!    construct but never advance).
//!
//! ## Architecture note
//!
//! The `app` crate is the **composition root** — the only crate permitted to
//! depend on everything. All other crates are decoupled from each other via
//! the dependency rules in `docs/02_crate_dependency_graph.md`.

pub mod analytics_bridge;
pub mod app;
pub mod batch;
pub mod behavior_validation;
pub mod capture;
pub mod events;
/// GPU/surface bring-up — extracted from `app.rs` (Phase 9, P9.6).
pub mod gpu_init;
pub mod interventions;
pub mod learning_bridge;
pub mod motion_diagnostic;
pub mod preferences;
pub mod render;
pub mod replay;
pub mod scripting;
pub mod simulation;
/// Starter-species genome/CPPN seeding — extracted from `app.rs` (Phase 9, P9.6).
pub mod species_seed;
pub mod systems;

use anyhow::{Context, Result};
use app::PhylonApp;
use config::PhylonConfig;
use std::path::Path;
use tracing::info;
use winit::event_loop::{ControlFlow, EventLoop};

fn main() -> Result<()> {
    // Initialise structured logging.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new(
                    "info,wgpu_core=warn,wgpu_hal=warn,egui_wgpu=error",
                )
            }),
        )
        .init();

    info!("Phylon v{} starting", env!("CARGO_PKG_VERSION"));

    // Load configuration.
    let config_path = Path::new("data/default.ron");
    let sim_config =
        PhylonConfig::load(Some(config_path)).context("failed to load configuration")?;
    info!(
        tick_rate = sim_config.simulation.tick_rate,
        rng_seed = sim_config.simulation.rng_seed,
        "Configuration loaded"
    );

    if let Some(replay_path) = &sim_config.research.replay_path {
        info!(path = %replay_path, "Running replay playback");
        let bundle =
            storage::replay::ReplayBundle::load_from_file(std::path::Path::new(replay_path))
                .context("failed to load replay bundle")?;
        let target_tick = if sim_config.research.max_ticks > 0 {
            sim_config.research.max_ticks
        } else {
            bundle.log.last_event_tick() + 1
        };
        let _app = replay::run_replay(
            &sim_config,
            &bundle,
            target_tick,
            sim_config.research.replay_speed,
            sim_config.research.replay_realtime_pacing,
        );
        info!(ticks = target_tick, "Replay playback completed");
        return Ok(());
    }

    if !sim_config.research.batch_seeds.is_empty() {
        info!(
            seeds = ?sim_config.research.batch_seeds,
            "Running headless batch"
        );
        let batch_config = research::BatchRunConfig {
            base_description: format!("Batch run: {}", sim_config.research.experiment_id),
            seeds: sim_config.research.batch_seeds.clone(),
            max_ticks: sim_config.research.max_ticks,
        };
        let reports = batch::run_batch(&sim_config, &batch_config);
        info!(runs = reports.len(), "Batch run completed");
        return Ok(());
    }

    let is_headless = sim_config.research.headless;
    let realtime_lock = sim_config.research.realtime_lock;
    let max_ticks = sim_config.research.max_ticks;
    let tick_rate = sim_config.simulation.tick_rate;

    let mut app = PhylonApp::new(sim_config.clone());

    if let Some(scenario_path) = &sim_config.research.scenario_path {
        info!(path = %scenario_path, "Running scenario script");
        match scripting::run_script_file(&mut app, std::path::Path::new(scenario_path)) {
            Ok(count) => info!(commands = count, "Scenario script completed"),
            Err(e) => tracing::error!("scenario script failed: {e}"),
        }
    }

    // Initialize tokio runtime for background tasks and networking
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to create tokio runtime")?;

    // Enter the tokio runtime context so we can spawn tasks or run servers
    let _guard = rt.enter();

    let (marl_tx, mut marl_rx) = tokio::sync::mpsc::channel(10);

    // Start Network server
    if let Some(port) = sim_config.research.network_port {
        let server = network::NetworkServer::new(format!("0.0.0.0:{}", port), marl_tx);
        rt.spawn(async move {
            if let Err(e) = server.start().await {
                tracing::error!("Network server failed: {}", e);
            }
        });
    }

    if is_headless {
        info!("Running in headless mode");
        app.init_gpu_headless()
            .context("failed to initialize headless GPU context")?;

        let tick_duration = std::time::Duration::from_secs_f64(1.0 / tick_rate as f64);
        let mut tick_count = 0;

        if sim_config.research.network_port.is_some() {
            info!("Running in RL environment mode");
            let mut steps_remaining = 0;

            loop {
                if steps_remaining == 0 {
                    match rt.block_on(marl_rx.recv()) {
                        Some(req) => match req.command {
                            network::MarlCommand::Step { ticks } => {
                                steps_remaining = ticks;
                                let _ = req.reply.send(network::MarlResponse::Ok);
                            }
                            network::MarlCommand::GetState => {
                                let observables = learning_bridge::get_state(&mut app);
                                let _ =
                                    req.reply.send(network::MarlResponse::State { observables });
                            }
                            network::MarlCommand::SetActions { actions } => {
                                learning_bridge::set_actions(&mut app, &actions);
                                let _ = req.reply.send(network::MarlResponse::Ok);
                            }
                            network::MarlCommand::Reset => {
                                learning_bridge::reset(&mut app);
                                let _ = req.reply.send(network::MarlResponse::Ok);
                            }
                            network::MarlCommand::SetDifficulty { level } => {
                                learning_bridge::set_difficulty(&mut app, level);
                                let _ = req.reply.send(network::MarlResponse::Ok);
                            }
                        },
                        None => break,
                    }
                }

                if steps_remaining > 0 {
                    app.update_simulation();
                    steps_remaining -= 1;
                    tick_count += 1;

                    if max_ticks > 0 && tick_count >= max_ticks {
                        break;
                    }
                }
            }
        } else {
            let periodic_script_path = sim_config.research.periodic_script_path.clone();
            let periodic_script_interval = sim_config.research.periodic_script_interval_ticks;

            while max_ticks == 0 || tick_count < max_ticks {
                let start = std::time::Instant::now();

                app.update_simulation();
                tick_count += 1;

                if let Some(path) = &periodic_script_path {
                    if periodic_script_interval > 0
                        && tick_count.is_multiple_of(periodic_script_interval)
                    {
                        if let Err(e) =
                            scripting::run_script_file(&mut app, std::path::Path::new(path))
                        {
                            tracing::error!("periodic script failed: {e}");
                        }
                    }
                }

                if realtime_lock {
                    let elapsed = start.elapsed();
                    if elapsed < tick_duration {
                        std::thread::sleep(tick_duration - elapsed);
                    }
                }
            }
        }

        info!(
            "Headless run completed (reached max_ticks = {})",
            tick_count
        );
    } else {
        // Build and run the winit event loop.
        let event_loop = EventLoop::new().context("failed to create event loop")?;
        event_loop.set_control_flow(ControlFlow::Poll);

        event_loop.run_app(&mut app).context("event loop error")?;
    }

    info!("Phylon shutdown complete");
    Ok(())
}
