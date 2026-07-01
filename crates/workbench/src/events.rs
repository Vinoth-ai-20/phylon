use bevy::prelude::*;

/// Commands sent from the UI toolbar to control the simulation state.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub enum SimulationControlEvent {
    /// Start or resume the simulation.
    Play,
    /// Pause the simulation.
    Pause,
    /// Reset the simulation back to initial state.
    Reset,
    /// Step exactly one tick forward (only works if paused).
    StepOneTick,
    /// Adjust the relative simulation speed (e.g. 1.0 = normal, 2.0 = fast).
    SetSpeed(f32),
}

/// Commands sent from the UI toolbar to control the camera.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraControlEvent {
    /// Reset the camera position to origin and zoom to default.
    ResetCamera,
    /// Toggle spectator mode (follow selected entity).
    ToggleSpectator,
}

/// Commands sent from the UI toolbar to change overlays.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayChangedEvent {
    /// Cycle to the next available overlay.
    NextOverlay,
}
