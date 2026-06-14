use egui_tiles::{Behavior, TileId, UiResponse};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pane {
    Analytics,
    SimulationViewport,
    BrainAndGenome,
    Timeline,
    SystemLogs,
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
                egui::ScrollArea::vertical().show(ui, |ui| {
                    crate::panels::analytics::render_analytics_dashboard(ui, self.stats, self.tick);
                    ui.separator();
                    crate::panels::entity_inspector::render_entity_inspector(
                        ui,
                        &self.ui_state.selected_entities,
                    );
                });
            }
            Pane::SimulationViewport => {
                let rect = ui.available_rect_before_wrap();
                self.ui_state.viewport_rect = Some(rect);
                // No background drawn here, so wgpu surface shows through
            }
            Pane::BrainAndGenome => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    crate::panels::brain_inspector::render_brain_inspector(ui, self.tick);
                    ui.separator();
                    crate::panels::genome_inspector::render_genome_inspector(ui);
                    ui.separator();
                    crate::panels::research::render_research(
                        ui,
                        self.script_path,
                        self.load_script,
                    );
                });
            }
            Pane::Timeline => {
                crate::panels::timeline::render_timeline(
                    ui,
                    self.tick,
                    &mut self.ui_state.simulation_speed,
                    &mut self.ui_state.is_paused,
                );
            }
            Pane::SystemLogs => {
                crate::panels::system_logs::render_system_logs(ui, &self.ui_state.system_logs);
            }
        }
        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        match pane {
            Pane::Analytics => "Analytics Dashboard".into(),
            Pane::SimulationViewport => "Simulation".into(),
            Pane::BrainAndGenome => "Inspectors".into(),
            Pane::Timeline => "Timeline & Replay".into(),
            Pane::SystemLogs => "System Logs".into(),
        }
    }
}
