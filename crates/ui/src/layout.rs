use egui_tiles::{Behavior, TileId, UiResponse};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pane {
    Analytics,
    Research,
    BrainInspector,
    EntityInspector,
    GenomeInspector,
}

pub struct TreeBehavior<'a> {
    pub ui_state: &'a mut crate::state::UiState,
    pub stats: &'a analytics::SimulationStats,
    pub tick: common::Tick,
    pub script_path: &'a mut String,
    pub load_script: &'a mut bool,
}

impl<'a> Behavior<Pane> for TreeBehavior<'a> {
    fn pane_ui(&mut self, ui: &mut egui::Ui, _tile_id: TileId, pane: &mut Pane) -> UiResponse {
        match pane {
            Pane::Analytics => {
                ui.label(format!("Tick: {}", self.tick.0));
                ui.label(format!("Population: {}", self.stats.current_population));
                ui.separator();
                ui.label("Deaths by Cause:");
                ui.label(format!("- Starvation: {}", self.stats.deaths_by_starvation));
                ui.label(format!("- Predation: {}", self.stats.deaths_by_predation));
                ui.label(format!("- Old Age: {}", self.stats.deaths_by_age));

                ui.separator();
                ui.label("Population History");

                let points: egui_plot::PlotPoints = self
                    .stats
                    .population_history
                    .iter()
                    .map(|(t, p)| [*t, *p])
                    .collect();

                let line = egui_plot::Line::new(points);
                egui_plot::Plot::new("population_plot")
                    .view_aspect(2.0)
                    .show(ui, |plot_ui| plot_ui.line(line));

                let selected = self.ui_state.selected_entities.clone();
                crate::panels::analytics::render_lineage_tree(ui, self.ui_state, &selected);
            }
            Pane::Research => {
                ui.horizontal(|ui| {
                    ui.label("Script:");
                    ui.text_edit_singleline(self.script_path);
                });
                if ui.button("Load & Run").clicked() {
                    *self.load_script = true;
                }
            }
            Pane::BrainInspector => {
                crate::panels::brain_inspector::render_brain_inspector(ui, self.tick);
            }
            Pane::EntityInspector => {
                ui.label("Entity Inspector (Not Implemented)");
            }
            Pane::GenomeInspector => {
                ui.label("Genome Inspector (Not Implemented)");
            }
        }
        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        match pane {
            Pane::Analytics => "Analytics".into(),
            Pane::Research => "Research & Plugins".into(),
            Pane::BrainInspector => "Brain Inspector".into(),
            Pane::EntityInspector => "Entity Inspector".into(),
            Pane::GenomeInspector => "Genome Inspector".into(),
        }
    }
}
