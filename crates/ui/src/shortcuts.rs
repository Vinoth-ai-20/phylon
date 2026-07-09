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
    /// Capture a screenshot of the current viewport.
    pub take_screenshot: KeyboardShortcut,
    /// Start/stop recording an animated GIF of the viewport.
    pub toggle_recording: KeyboardShortcut,

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

    /// Reset the viewport camera to the home position (Home/Num0 also do
    /// this — see `consume_all` — this is the Ctrl-modified alternative).
    pub reset_camera: KeyboardShortcut,
    /// Select all entities.
    pub select_all: KeyboardShortcut,
    /// Clear the current selection.
    pub deselect: KeyboardShortcut,
    /// Spawn a prototype organism at the cursor.
    pub spawn: KeyboardShortcut,
    /// Toggle the Command Palette (Phase 2, M15).
    pub command_palette: KeyboardShortcut,
    /// Toggle Global Search (Phase 7, W6a).
    pub global_search: KeyboardShortcut,
}

impl Default for ShortcutManager {
    fn default() -> Self {
        Self {
            save_state: KeyboardShortcut::new(Modifiers::CTRL, Key::S),
            load_state: KeyboardShortcut::new(Modifiers::CTRL, Key::O),
            import_genome: KeyboardShortcut::new(Modifiers::CTRL | Modifiers::SHIFT, Key::I),
            export_genome: KeyboardShortcut::new(Modifiers::CTRL | Modifiers::SHIFT, Key::E),
            take_screenshot: KeyboardShortcut::new(Modifiers::CTRL | Modifiers::SHIFT, Key::S),
            toggle_recording: KeyboardShortcut::new(Modifiers::CTRL | Modifiers::SHIFT, Key::R),

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
            command_palette: KeyboardShortcut::new(Modifiers::CTRL | Modifiers::SHIFT, Key::P),
            global_search: KeyboardShortcut::new(Modifiers::CTRL, Key::F),
        }
    }
}

impl ShortcutManager {
    /// Poll all shortcuts against the current egui input state.
    ///
    /// Each matching shortcut is consumed and its [`MenuAction`] pushed into
    /// `actions`. Call this once per frame before processing actions.
    pub fn consume_all(&self, ctx: &egui::Context, actions: &mut Vec<MenuAction>) {
        // `consume_shortcut` matches modifiers *logically* (extra Shift/Alt
        // ignored), so e.g. Ctrl+Shift+S also satisfies plain Ctrl+S. The
        // more specific Shift-combos must be checked (and consume the key
        // event) before their less-specific counterparts, or the wrong
        // action fires — check those first.
        // Checked before `spawn` (Ctrl+P): both share the P key, and the
        // more specific Ctrl+Shift+P combo must consume the event first, or
        // it would satisfy the less-specific Ctrl+P check instead (same
        // ordering rule as take_screenshot/toggle_recording below).
        if ctx.input_mut(|i| i.consume_shortcut(&self.command_palette)) {
            actions.push(MenuAction::ToggleCommandPalette);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.global_search)) {
            actions.push(MenuAction::ToggleGlobalSearch);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.take_screenshot)) {
            actions.push(MenuAction::TakeScreenshot);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.toggle_recording)) {
            actions.push(MenuAction::ToggleRecording);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.import_genome)) {
            actions.push(MenuAction::ImportGenome);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.export_genome)) {
            actions.push(MenuAction::ExportGenome);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&self.save_state)) {
            actions.push(MenuAction::SaveState);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.load_state)) {
            actions.push(MenuAction::LoadState);
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

        // Raw, unmodified single-key shortcuts (only when egui doesn't want
        // keyboard input elsewhere, e.g. not while typing in a text field) —
        // a Blender-style scene-manipulation scheme, unadvertised in any
        // menu today, preserved as-is from the previous `render.rs`
        // implementation this method replaces.
        //
        // Phase 6, Epic J: G/C/V/J (Grab/Duplicate/Paste/Join Selection),
        // plus Ctrl+Z/Ctrl+Y (Undo/Redo) above, were removed from here —
        // each only ever pushed a `MenuAction` whose handler logged a
        // warning and did nothing. X (Delete) and F (Toggle Stationary)
        // are real and stay.
        if !ctx.wants_keyboard_input() {
            if ctx.input(|i| i.key_pressed(Key::X)) {
                actions.push(MenuAction::DeleteSelection);
            }
            if ctx.input(|i| i.key_pressed(Key::F)) {
                actions.push(MenuAction::ToggleStationary);
            }
        }

        // Camera zoom — always active, no modifier, not gated by
        // `wants_keyboard_input` (matches the previous implementation).
        if ctx.input(|i| i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals)) {
            actions.push(MenuAction::CameraZoomIn);
        }
        if ctx.input(|i| i.key_pressed(Key::Minus)) {
            actions.push(MenuAction::CameraZoomOut);
        }
        if ctx.input(|i| i.key_pressed(Key::Home) || i.key_pressed(Key::Num0)) {
            actions.push(MenuAction::CameraHome);
        }
    }
}
