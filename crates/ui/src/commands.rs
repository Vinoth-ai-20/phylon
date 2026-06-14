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
