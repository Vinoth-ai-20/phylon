use crate::app::PhylonApp;

/// # Headless Batch Run Orchestrator
///
/// ## 1. What Happens
/// Runs one headless experiment per seed in `batch.seeds`, each a fresh
/// `PhylonApp` built from `sim_config` with `simulation.rng_seed` and
/// `research.experiment_id` overridden per seed. After each run completes
/// (`batch.max_ticks` ticks), writes that run's `ExperimentReport` as
/// Markdown to `data/experiments/<id>/report.md` (for a human to read) and as
/// RON to `data/experiments/<id>/report.ron` (for the UI's Research
/// Dashboard — see `ui::plugins::research_dashboard` — to read back
/// structured data instead of parsing prose), then writes one aggregate
/// `data/experiments/batch-summary.md` covering the whole batch.
///
/// ## 2. Why It Happens
/// A single headless run (see `main.rs`) answers "what happens with this
/// exact seed?" Research usually needs "what happens *across* seeds?" —
/// batch running is the minimal structure that answers that without a
/// hand-rolled shell loop around the binary, and ties every run back to a
/// real, persisted `ExperimentManifest` (see `research::ExperimentManifest`'s
/// doc comment for why that matters).
///
/// ## 3. How It Happens
/// Each seed gets a fully independent `PhylonApp` (own ECS `World`, own GPU
/// context) run to completion sequentially, not in parallel — headless GPU
/// contexts and `bevy_ecs::World`s aren't `Send` across the kind of
/// lightweight parallelism that would be worth the complexity here, and
/// sequential batch runs are simpler to reason about and debug. Final
/// population is counted via the `genetics::Genome` component (present on
/// exactly one node per organism, see `organisms::spawn_organism`); final
/// species count reads `evolution::SpeciesRegistry::species_count`
/// directly.
pub fn run_batch(
    sim_config: &config::PhylonConfig,
    batch: &research::BatchRunConfig,
) -> Vec<research::ExperimentReport> {
    let mut reports = Vec::with_capacity(batch.seeds.len());

    for &seed in &batch.seeds {
        let mut cfg = sim_config.clone();
        cfg.simulation.rng_seed = seed;
        cfg.research.experiment_id = format!("{}-seed{}", cfg.research.experiment_id, seed);

        let mut app = PhylonApp::new(cfg);
        if let Err(e) = app.init_gpu_headless() {
            tracing::error!("batch run seed {seed} failed to init headless GPU: {e}");
            continue;
        }

        let mut tick_count = 0u64;
        while tick_count < batch.max_ticks {
            app.update_simulation();
            tick_count += 1;
        }

        let mut genome_query = app.world.ecs.query::<&genetics::Genome>();
        let final_population = genome_query.iter(&app.world.ecs).count() as u32;
        let final_species_count = app
            .world
            .ecs
            .get_resource::<evolution::SpeciesRegistry>()
            .map(|r| r.species_count())
            .unwrap_or(0);

        let report = research::ExperimentReport {
            manifest: app.experiment_manifest.clone(),
            ticks_run: tick_count,
            final_population,
            final_species_count,
        };

        let report_dir = std::path::Path::new("data/experiments").join(&report.manifest.id);
        let report_md_path = report_dir.join("report.md");
        if let Some(parent) = report_md_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&report_md_path, report.to_markdown()) {
            tracing::warn!("failed to write experiment report markdown for seed {seed}: {e}");
        }
        if let Err(e) = report.save_to_ron(&report_dir.join("report.ron")) {
            tracing::warn!("failed to write experiment report RON for seed {seed}: {e}");
        }

        tracing::info!(
            seed,
            ticks_run = report.ticks_run,
            final_population = report.final_population,
            final_species_count = report.final_species_count,
            "batch run completed"
        );

        reports.push(report);
    }

    let summary_path = std::path::Path::new("data/experiments/batch-summary.md");
    if let Some(parent) = summary_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(
        summary_path,
        research::render_batch_summary_markdown(&reports),
    ) {
        tracing::warn!("failed to write batch summary report: {e}");
    }

    reports
}
