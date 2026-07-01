//! # Phylon Application
//!
//! The main binary entry point for the Phylon simulation.

pub mod camera;
pub mod metrics_plot;
pub mod plugins;
pub mod render;
pub mod selection;
pub mod systems;
pub mod ui;

use anyhow::{Context, Result};
use config::PhylonConfig;
use std::path::Path;
use tracing::info;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::prelude::*;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};

use plugins::{AppState, PhylonPlugins};

#[derive(Resource)]
pub struct PositionReceiver(pub crossbeam_channel::Receiver<Vec<u8>>);

#[derive(Resource)]
pub struct NodeEntitiesReceiver(pub crossbeam_channel::Receiver<Vec<Entity>>);

#[derive(Resource)]
pub struct BrainDataReceiver(pub crossbeam_channel::Receiver<Vec<u8>>);

#[derive(Resource)]
pub struct DiffusionDataReceiver(pub crossbeam_channel::Receiver<Vec<f32>>);

#[derive(Resource, Default)]
pub struct ActiveOverlay(pub Option<diffusion::FieldLayer>);

fn main() -> Result<()> {
    // Initialise structured logging to stdout and behavior.jsonl
    let file_appender = tracing_appender::rolling::never("logs", "behavior.jsonl");
    let (non_blocking, _guard_tracing) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_writer(non_blocking)
                .with_target(true)
                .with_span_events(FmtSpan::NONE),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
        .init();

    info!("Phylon v{} starting", env!("CARGO_PKG_VERSION"));

    // Load configuration.
    let config_path = Path::new("data/default.ron");
    let sim_config = PhylonConfig::load(Some(config_path)).unwrap_or_default();

    println!(
        "Configuration loaded: tick_rate={}, rng_seed={}",
        sim_config.simulation.tick_rate, sim_config.simulation.rng_seed
    );

    // Initialize tokio runtime for background tasks and networking
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to create tokio runtime")?;

    let _guard = rt.enter();

    let (marl_tx, _marl_rx) = tokio::sync::mpsc::channel(10);
    let (gpu_pos_tx, gpu_pos_rx) = crossbeam_channel::unbounded();
    let (gpu_node_entities_tx, gpu_node_entities_rx) = crossbeam_channel::unbounded();
    let (brain_data_tx, brain_data_rx) = crossbeam_channel::unbounded();
    let (diffusion_data_tx, diffusion_data_rx) = crossbeam_channel::unbounded();

    // Start Network server
    if let Some(port) = sim_config.research.network_port {
        let server = network::NetworkServer::new(format!("0.0.0.0:{}", port), marl_tx);
        rt.spawn(async move {
            if let Err(e) = server.start().await {
                tracing::error!("Network server failed: {}", e);
            }
        });
    }

    // --- BEVY BOOTSTRAPPING ---

    let asset_path = if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let mut path = std::path::PathBuf::from(manifest_dir);
        while !path.join("Cargo.lock").exists() && path.parent().is_some() {
            path = path.parent().unwrap().to_path_buf();
        }
        path.join("assets").to_string_lossy().into_owned()
    } else {
        "assets".to_string()
    };

    App::new()
        .add_plugins(
            DefaultPlugins
                .build()
                .disable::<bevy::log::LogPlugin>()
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Phylon".into(),
                        resolution: WindowResolution::new(1280, 720),
                        present_mode: PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: asset_path,
                    ..default()
                }),
        )
        .init_state::<AppState>()
        .insert_resource(PositionReceiver(gpu_pos_rx))
        .insert_resource(NodeEntitiesReceiver(gpu_node_entities_rx))
        .insert_resource(BrainDataReceiver(brain_data_rx))
        .insert_resource(DiffusionDataReceiver(diffusion_data_rx))
        .insert_resource(ActiveOverlay(Some(diffusion::FieldLayer::Energy)))
        .add_plugins(PhylonPlugins {
            gpu_pos_tx,
            gpu_node_entities_tx,
            brain_data_tx,
            diffusion_data_tx,
        })
        .run();

    info!("Phylon shutdown complete");
    Ok(())
}
