use crate::layout::InspectorNode;
use bevy::prelude::*;

#[derive(Component, Debug, PartialEq)]
pub enum InspectorField {
    EntityId,
    Species,
    GenomeId,
    Generation,
    Age,
    Health,
    Energy,
    Atp,
    Glucose,
    Position,
    Velocity,
    Metabolism,
    Behaviour,
}

pub struct InspectorPlugin;

impl Plugin for InspectorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            setup_inspector.after(crate::layout::setup_workbench_layout),
        );
    }
}

fn setup_inspector(
    mut commands: Commands,
    query: Query<Entity, With<InspectorNode>>,
    ui_assets: Res<crate::UiAssets>,
) {
    let Some(inspector_entity) = query.iter().next() else {
        return;
    };

    let fields = vec![
        (InspectorField::EntityId, "Entity ID:"),
        (InspectorField::Species, "Species:"),
        (InspectorField::GenomeId, "Genome ID:"),
        (InspectorField::Generation, "Generation:"),
        (InspectorField::Age, "Age:"),
        (InspectorField::Health, "Health:"),
        (InspectorField::Energy, "Energy:"),
        (InspectorField::Atp, "ATP:"),
        (InspectorField::Glucose, "Glucose:"),
        (InspectorField::Position, "Position:"),
        (InspectorField::Velocity, "Velocity:"),
        (InspectorField::Metabolism, "Metabolism:"),
        (InspectorField::Behaviour, "Behaviour:"),
    ];

    let mut field_nodes = vec![];

    let title = commands
        .spawn((
            Node {
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(Color::srgb(0.2, 0.2, 0.2)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Inspector"),
                TextFont {
                    font: bevy::prelude::FontSource::Handle(ui_assets.inter_bold.clone()),
                    font_size: bevy::prelude::FontSize::Px(18.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        })
        .id();

    field_nodes.push(title);

    for (field_enum, field_name) in fields {
        let node = commands
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(Val::Px(5.0)),
                ..default()
            })
            .with_children(|parent| {
                parent.spawn((
                    Text::new(field_name),
                    TextFont {
                        font: bevy::prelude::FontSource::Handle(ui_assets.inter_regular.clone()),
                        font_size: bevy::prelude::FontSize::Px(14.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.7, 0.7, 0.7)),
                ));
                parent.spawn((
                    Text::new("-"),
                    TextFont {
                        font: bevy::prelude::FontSource::Handle(ui_assets.jetbrains_mono.clone()),
                        font_size: bevy::prelude::FontSize::Px(14.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    field_enum,
                ));
            })
            .id();
        field_nodes.push(node);
    }

    commands.entity(inspector_entity).add_children(&field_nodes);
}
