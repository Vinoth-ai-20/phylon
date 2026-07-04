//! Keyboard shortcut bindings for the Phylon Workbench.

use crate::types::MenuAction;
use egui::{Key, KeyboardShortcut, Modifiers};

/// Centralised registry of all keyboard shortcuts used in the Workbench.
///
/// Each field binds one logical action to a key combination. The
/// [`ShortcutManager::consume_all`] method polls every shortcut once per
/// frame and pushes the matching [`MenuAction`] when a shortcut fires.
#[derive(Clone, Debug)]
pub struct ShortcutManager {
    /// Save the current simulation state to disk.
    pub save_state: KeyboardShortcut,
    /// Load a simulation state from disk.
    pub load_state: KeyboardShortcut,
    /// Import a genome file.
    pub import_genome: KeyboardShortcut,
    /// Export the selected organism's genome.
    pub export_genome: KeyboardShortcut,

    /// Toggle between play and pause.
    pub play_pause: KeyboardShortcut,
    /// Advance the simulation by a single tick.
    pub step_forward: KeyboardShortcut,
    /// Double the simulation speed.
    pub speed_up: KeyboardShortcut,
    /// Halve the simulation speed.
    pub slow_down: KeyboardShortcut,

    /// Toggle the Metrics panel visibility.
    pub toggle_metrics: KeyboardShortcut,
    /// Toggle the Event Log panel visibility.
    pub toggle_log: KeyboardShortcut,
    /// Toggle the Sidebar panel visibility.
    pub toggle_sidebar: KeyboardShortcut,

    /// Reset the viewport camera to the home position.
    pub reset_camera: KeyboardShortcut,
    /// Select all entities.
    pub select_all: KeyboardShortcut,
    /// Clear the current selection.
    pub deselect: KeyboardShortcut,
    /// Spawn a prototype organism at the cursor.
    pub spawn: KeyboardShortcut,
}

impl Default for ShortcutManager {
    fn default() -> Self {
        Self {
            save_state: KeyboardShortcut::new(Modifiers::CTRL, Key::S),
            load_state: KeyboardShortcut::new(Modifiers::CTRL, Key::O),
            import_genome: KeyboardShortcut::new(Modifiers::CTRL | Modifiers::SHIFT, Key::I),
            export_genome: KeyboardShortcut::new(Modifiers::CTRL | Modifiers::SHIFT, Key::E),

            play_pause: KeyboardShortcut::new(Modifiers::NONE, Key::Space),
            step_forward: KeyboardShortcut::new(Modifiers::NONE, Key::ArrowRight),
            speed_up: KeyboardShortcut::new(Modifiers::NONE, Key::ArrowUp),
            slow_down: KeyboardShortcut::new(Modifiers::NONE, Key::ArrowDown),

            toggle_metrics: KeyboardShortcut::new(Modifiers::CTRL, Key::M),
            toggle_log: KeyboardShortcut::new(Modifiers::CTRL, Key::L),
            toggle_sidebar: KeyboardShortcut::new(Modifiers::CTRL, Key::B),

            reset_camera: KeyboardShortcut::new(Modifiers::CTRL, Key::R),
            select_all: KeyboardShortcut::new(Modifiers::CTRL, Key::A),
            deselect: KeyboardShortcut::new(Modifiers::NONE, Key::Escape),
            spawn: KeyboardShortcut::new(Modifiers::CTRL, Key::P),
        }
    }
}

impl ShortcutManager {
    /// Poll all shortcuts against the current egui input state.
    ///
    /// Each matching shortcut is consumed and its [`MenuAction`] pushed into
    /// `actions`. Call this once per frame before processing actions.
    pub fn consume_all(&self, ctx: &egui::Context, actions: &mut Vec<MenuAction>) {
        if ctx.input_mut(|i| i.consume_shortcut(&self.save_state)) {
            actions.push(MenuAction::SaveState);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.load_state)) {
            actions.push(MenuAction::LoadState);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.import_genome)) {
            actions.push(MenuAction::ImportGenome);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.export_genome)) {
            actions.push(MenuAction::ExportGenome);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&self.play_pause)) {
            actions.push(MenuAction::TogglePlayPause);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.step_forward)) {
            actions.push(MenuAction::StepForward);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.speed_up)) {
            actions.push(MenuAction::SetSpeedUp);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.slow_down)) {
            actions.push(MenuAction::SetSpeedDown);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&self.toggle_metrics)) {
            actions.push(MenuAction::ToggleMetrics);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.toggle_log)) {
            actions.push(MenuAction::ToggleLog);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.toggle_sidebar)) {
            actions.push(MenuAction::ToggleSidebar);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&self.reset_camera)) {
            actions.push(MenuAction::CameraHome);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.select_all)) {
            actions.push(MenuAction::SelectAll);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.deselect)) {
            actions.push(MenuAction::Deselect);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.spawn)) {
            actions.push(MenuAction::SpawnProtoFish);
        }
    }
}
