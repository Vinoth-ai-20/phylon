//! Navigation plugin — workspace switcher (future: top-level tab bar).
//!
//! Currently a placeholder; workspace navigation is handled by the activity bar in sidebar.rs.

use crate::types::MenuAction;

/// Navigation rail (currently no-op — workspace switching is in the activity bar).
pub fn navigation_ui(
    _ctx: &egui::Context,
    _ui: &mut egui::Ui,
    _state: &mut crate::WorkbenchState,
    _world: &mut world::World,
    _actions: &mut Vec<MenuAction>,
) {
}
