use crate::types::*;

/// Renders the Research Dashboard — lists every experiment report found
/// under `data/experiments/` and a simple cross-experiment comparison.
///
/// Completes the "Compare" stage of the research loop the Phase 2 roadmap
/// found otherwise empty: `research::ExperimentReport`/`app::batch::run_batch`
/// already existed, but a report was only ever persisted as Markdown prose
/// (`report.md`) — nothing structured to read back. This milestone added
/// `ExperimentReport::save_to_ron`/`load_from_ron` (mirroring
/// `ExperimentManifest`'s existing pair) and wired `run_batch` to write
/// `report.ron` alongside the Markdown, which is what this panel reads.
#[allow(clippy::too_many_arguments)]
pub fn research_dashboard_ui(
    _ctx: &egui::Context,
    ui: &mut egui::Ui,
    _state: &mut crate::WorkbenchState,
    _world: &mut world::World,
    _actions: &mut [MenuAction],
) {
    let reports = discover_experiment_reports(std::path::Path::new("data/experiments"));

    if reports.is_empty() {
        crate::widgets::empty_state(
            ui,
            "No experiment reports found under data/experiments/. Run a headless batch \
             (set research.batch_seeds in config) to produce some.",
        );
        return;
    }

    ui.label(egui::RichText::new(format!("{} experiment report(s)", reports.len())).strong());
    ui.add_space(crate::theme::SPACE_SM);

    // Cross-experiment comparison — the same mean/min/max statistics
    // `research::render_batch_summary_markdown` computes, surfaced as a live
    // UI readout instead of prose a researcher would otherwise have to open
    // `data/experiments/batch-summary.md` to see.
    let populations: Vec<u32> = reports.iter().map(|r| r.final_population).collect();
    let mean = populations.iter().map(|&p| p as f64).sum::<f64>() / populations.len() as f64;
    let min = populations.iter().min().copied().unwrap_or(0);
    let max = populations.iter().max().copied().unwrap_or(0);
    ui.label(format!(
        "Final population across {} run(s) — mean {:.1}, min {}, max {}",
        reports.len(),
        mean,
        min,
        max
    ));
    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("research_dashboard_grid")
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Experiment").strong());
                    ui.label(egui::RichText::new("Seed").strong());
                    ui.label(egui::RichText::new("Ticks Run").strong());
                    ui.label(egui::RichText::new("Final Population").strong());
                    ui.label(egui::RichText::new("Species Count").strong());
                    ui.end_row();

                    for report in &reports {
                        ui.label(&report.manifest.id);
                        ui.label(report.manifest.rng_seed.to_string());
                        ui.label(report.ticks_run.to_string());
                        ui.label(report.final_population.to_string());
                        ui.label(report.final_species_count.to_string());
                        ui.end_row();
                    }
                });
        });
}

/// Scans `dir` for experiment subdirectories containing a `report.ron`,
/// deserializing each one. Missing or malformed reports are silently
/// skipped (a partially-written or corrupted experiment directory
/// shouldn't crash the panel) — sorted by experiment ID for a stable,
/// deterministic display order, since `std::fs::read_dir` makes no
/// ordering guarantee.
fn discover_experiment_reports(dir: &std::path::Path) -> Vec<research::ExperimentReport> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut reports: Vec<research::ExperimentReport> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            research::ExperimentReport::load_from_ron(&entry.path().join("report.ron")).ok()
        })
        .collect();
    reports.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
    reports
}
