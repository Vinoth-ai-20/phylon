//! # Phylon Application
//!
//! The main binary entry point for the Phylon simulation.
//!
//! ## Responsibilities
//!
//! 1. Parse CLI arguments and locate the config file.
//! 2. Initialise structured logging via `tracing_subscriber`.
//! 3. Load `PhylonConfig` from `data/default.ron` (falls back to defaults).
//! 4. Create a `winit` `EventLoop` and application window.
//! 5. Initialise a `wgpu` surface on the window.
//! 6. Create a `SimulationScheduler`.
//! 7. Run the event loop — advancing the scheduler on each `AboutToWait` and
//!    presenting a cleared frame on each `RedrawRequested`.
//!
//! ## Architecture note
//!
//! The `app` crate is the **composition root** — the only crate permitted to
//! depend on everything. All other crates are decoupled from each other via
//! the dependency rules in `docs/02_crate_dependency_graph.md`.

pub mod app;
pub mod capture;
pub mod events;
pub mod render;
pub mod simulation;
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

    let is_headless = sim_config.research.headless;
    let realtime_lock = sim_config.research.realtime_lock;
    let max_ticks = sim_config.research.max_ticks;
    let tick_rate = sim_config.simulation.tick_rate;

    let mut app = PhylonApp::new(sim_config.clone());

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
                        Some(req) => {
                            match req.command {
                                network::MarlCommand::Step { ticks } => {
                                    steps_remaining = ticks;
                                    let _ = req.reply.send(network::MarlResponse::Ok);
                                }
                                network::MarlCommand::GetState => {
                                    let observables = vec![]; // Placeholder
                                    let _ = req
                                        .reply
                                        .send(network::MarlResponse::State { observables });
                                }
                                network::MarlCommand::SetActions { actions: _ } => {
                                    let _ = req.reply.send(network::MarlResponse::Ok);
                                }
                                network::MarlCommand::Reset => {
                                    let _ = req.reply.send(network::MarlResponse::Ok);
                                }
                            }
                        }
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
            while max_ticks == 0 || tick_count < max_ticks {
                let start = std::time::Instant::now();

                app.update_simulation();
                tick_count += 1;

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
