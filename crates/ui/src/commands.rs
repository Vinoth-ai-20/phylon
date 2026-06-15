use common::EntityId;
use std::path::PathBuf;

#[derive(Clone, Debug, Default)]
pub struct DietFilter {
    pub herbivore: bool,
    pub carnivore: bool,
    pub scavenger: bool,
}

pub enum AppCommand {
    ResetWorld,
    LoadSnapshot(PathBuf),
    SaveSnapshot(PathBuf),
    StepOneTick,
    ResetCamera,
    ZoomIn,
    ZoomOut,
    WheelZoom {
        mouse_position: [f32; 2],
        delta: f32,
    },
    PanCamera([f32; 2]),
    ClickWorld([f32; 2]),
    DoubleClickWorld([f32; 2]),
    TrackEntity(EntityId),
    ToggleFullscreen,
    SelectByDiet(DietFilter),
    SelectBySpecies(Vec<organisms::SpeciesId>),
    InvertSelection,
    QueryAllEntityIds,
    QuerySpeciesList,
    SeekReplayToTick(u64),
    SeekToPreviousSpeciationEvent,
    SeekToNextSpeciationEvent,
    RunExperiment(research::Experiment),
    StageExperiment(research::Experiment),
    StopExperiment,
    RunScript(PathBuf),
    RunScriptLine(String),
    RunDbQuery(String),
    ExportLineageTree(PathBuf),
    UndoGodMode(crate::state::GodModeAction),
    RedoGodMode(crate::state::GodModeAction),
    Quit,
}
