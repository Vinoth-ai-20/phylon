use anyhow::Result;
use common::Vec2;
use phylon_config::PhylonConfig;
use physics::{Acceleration, Mass, Position, Radius, Velocity};
use rand::Rng;
use scheduler::SimulationScheduler;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};
use world::PhylonWorld;

fn main() -> Result<()> {
    // Initialize tracing
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).init();

    info!("Starting phylon-research headless orchestrator");

    // In a real CLI, we would parse arguments using clap.
    // For now, we'll read a path from the first argument, or fallback to default.ron
    let args: Vec<String> = std::env::args().collect();
    let config_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("data/default.ron")
    };

    let config = PhylonConfig::load(Some(&config_path)).unwrap_or_else(|e| {
        error!("Failed to load config, using defaults: {}", e);
        PhylonConfig::default()
    });

    let run_id = format!(
        "res_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );
    info!("Initializing headless run: {}", run_id);

    let mut world = PhylonWorld::new(config.simulation.world_chunk_size as f32);
    let mut scheduler = SimulationScheduler::new(config.simulation.tick_rate);
    let mut stats = analytics::SimulationStats::new(1000);

    // Start db writer
    let mut db_writer = storage::db::DbWriter::new(&config.research.database_path).unwrap();

    // Spawn starter organisms
    let mut rng = rand::thread_rng();
    let spawn_range = 400.0;
    for _ in 0..100 {
        let mut genome = genetics::Genome::default();
        let num_weights = brain::TOTAL_NEURONS * brain::TOTAL_NEURONS;
        genome.brain_weights = (0..num_weights).map(|_| rng.gen_range(-1.0..1.0)).collect();

        world.spawn((
            organisms::Organism,
            organisms::Age(0),
            organisms::Energy(100.0),
            organisms::Health::default(),
            genome.clone(),
            reproduction::ReproductionCooldown(0),
            Position(Vec2::new(
                rng.gen_range(-spawn_range..spawn_range),
                rng.gen_range(-spawn_range..spawn_range),
            )),
            Velocity(Vec2::new(
                rng.gen_range(-10.0..10.0),
                rng.gen_range(-10.0..10.0),
            )),
            Acceleration(Vec2::ZERO),
            physics::Heading(rng.gen_range(-std::f32::consts::PI..std::f32::consts::PI)),
            Mass(1.0),
            Radius(genome.size),
            sensing::Observation::new(),
            brain::Intention::new(),
        ));
    }

    // Determine target ticks (could be passed in via CLI, default 10,000 for this example)
    let max_ticks = 10000;

    info!("Starting headless loop for {} ticks...", max_ticks);
    let start_time = std::time::Instant::now();

    while scheduler.current_tick.0 < max_ticks {
        scheduler.tick_loop(&mut world);

        // Process analytics
        stats.process_events(&world.last_events, scheduler.current_tick);
        stats.update_metrics(&world, scheduler.current_tick);

        db_writer
            .write_event(storage::db::DbEvent::Metrics {
                tick: scheduler.current_tick.0,
                population: stats.current_population as u32,
                avg_energy: 100.0,
                total_food: 0,
            })
            .unwrap();

        // Save periodic binary snapshots
        if scheduler.current_tick.0 > 0
            && scheduler
                .current_tick
                .0
                .is_multiple_of(config.research.snapshot_interval_ticks)
        {
            let path = format!("{}_tick_{}.bin", run_id, scheduler.current_tick.0);
            match storage::snapshot::save_world(&world, &path) {
                Ok(_) => info!("Saved snapshot to {}", path),
                Err(e) => error!("Failed to save snapshot: {}", e),
            }
        }
    }

    let elapsed = start_time.elapsed();
    info!("Experiment {} finished in {:.2?}", run_id, elapsed);

    // Give DB writer a small moment to drain the channel before exit
    std::thread::sleep(std::time::Duration::from_millis(500));

    Ok(())
}
