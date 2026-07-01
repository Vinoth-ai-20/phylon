use bevy::prelude::*;

pub mod event_log;
pub mod events;
pub mod inspector;
pub mod layout;
pub mod menubar;
pub mod metrics;
pub mod navigation_rail;
pub mod sidebar;
pub mod status_bar;
pub mod toolbar;

#[derive(Resource)]
pub struct UiAssets {
    pub inter_regular: Handle<Font>,
    pub inter_medium: Handle<Font>,
    pub inter_bold: Handle<Font>,
    pub jetbrains_mono: Handle<Font>,
}

fn load_ui_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(UiAssets {
        inter_regular: asset_server.load("fonts/Inter-Regular.ttf"),
        inter_medium: asset_server.load("fonts/Inter-Medium.ttf"),
        inter_bold: asset_server.load("fonts/Inter-Bold.ttf"),
        jetbrains_mono: asset_server.load("fonts/JetBrainsMono-Regular.ttf"),
    });
}

pub struct WorkbenchPlugin;

impl Plugin for WorkbenchPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<events::SimulationControlEvent>()
            .add_message::<events::CameraControlEvent>()
            .add_message::<events::OverlayChangedEvent>();

        app.add_systems(PreStartup, load_ui_assets)
            .add_systems(Startup, layout::setup_workbench_layout)
            .add_plugins(menubar::MenuBarPlugin)
            .add_plugins(navigation_rail::NavigationRailPlugin)
            .add_plugins(toolbar::ToolbarPlugin)
            .add_plugins(sidebar::SidebarPlugin)
            .add_plugins(inspector::InspectorPlugin)
            .add_plugins(status_bar::StatusBarPlugin)
            .add_plugins(metrics::MetricsPlugin)
            .add_plugins(event_log::EventLogPlugin);
    }
}
