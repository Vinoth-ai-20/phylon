use crate::layout::StatusBarNode;
use bevy::prelude::*;

pub struct StatusBarPlugin;

impl Plugin for StatusBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            setup_status_bar.after(crate::layout::setup_workbench_layout),
        );
    }
}

#[derive(Component, Debug)]
pub enum StatusBarField {
    State,
    Tick,
    FPS,
    Camera,
    Zoom,
    Selected,
    Hovered,
    TimeScale,
    Overlay,
}

fn setup_status_bar(
    mut commands: Commands,
    query: Query<Entity, With<StatusBarNode>>,
    ui_assets: Res<crate::UiAssets>,
) {
    let Some(status_entity) = query.iter().next() else {
        return;
    };

    let fields = vec![
        (StatusBarField::State, "State: PAUSED"),
        (StatusBarField::Tick, "Tick: 0"),
        (StatusBarField::FPS, "FPS: 0"),
        (StatusBarField::Camera, "Camera: (0, 0)"),
        (StatusBarField::Zoom, "Zoom: 1.0x"),
        (StatusBarField::Selected, "Selected: None"),
        (StatusBarField::Hovered, "Hovered: None"),
        (StatusBarField::TimeScale, "Time Scale: 1.0x"),
        (StatusBarField::Overlay, "Overlay: None"),
    ];

    let mut field_nodes = vec![];

    for (field_type, initial_text) in fields {
        let node = commands
            .spawn((
                Text::new(initial_text),
                TextFont {
                    font: bevy::prelude::FontSource::Handle(ui_assets.jetbrains_mono.clone()),
                    font_size: bevy::prelude::FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.8, 0.8, 0.8)),
                field_type,
            ))
            .id();
        field_nodes.push(node);
    }

    commands.entity(status_entity).add_children(&field_nodes);
}
