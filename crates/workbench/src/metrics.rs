use bevy::prelude::*;

use std::collections::VecDeque;

#[derive(Resource)]
pub struct HistoryBuffer<T> {
    pub data: VecDeque<T>,
    pub max_size: usize,
}

impl<T> HistoryBuffer<T> {
    pub fn new(max_size: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    pub fn push(&mut self, item: T) {
        if self.data.len() == self.max_size {
            self.data.pop_front();
        }
        self.data.push_back(item);
    }
}

#[derive(Component, Debug, PartialEq)]
pub enum MetricsField {
    Fps,
    Tps,
    FrameTime,
    Entities,
    CameraPos,
    Zoom,
    Population,
    Births,
    Deaths,
    Species,
    Biomass,
    Resources,
    Energy,
}

pub struct MetricsPlugin;

impl Plugin for MetricsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            setup_metrics.after(crate::layout::setup_workbench_layout),
        );
    }
}

fn setup_metrics(
    mut commands: Commands,
    query: Query<Entity, With<crate::layout::MetricsNode>>,
    ui_assets: Res<crate::UiAssets>,
) {
    let Some(metrics_entity) = query.iter().next() else {
        return;
    };

    let cols = vec!["Diagnostics", "Simulation Metrics"];

    let mut col_nodes = vec![];
    for col in cols {
        let node = commands
            .spawn(Node {
                width: Val::Percent(50.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            })
            .with_children(|parent| {
                parent.spawn((
                    Text::new(col),
                    TextFont {
                        font: bevy::prelude::FontSource::Handle(ui_assets.inter_bold.clone()),
                        font_size: bevy::prelude::FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));

                let items = if col == "Diagnostics" {
                    vec![
                        ("FPS: 0", MetricsField::Fps),
                        ("TPS: 0", MetricsField::Tps),
                        ("Frame Time: 0ms", MetricsField::FrameTime),
                        ("Entities: 0", MetricsField::Entities),
                        ("Camera Pos: (0, 0)", MetricsField::CameraPos),
                        ("Zoom: 1.0x", MetricsField::Zoom),
                    ]
                } else {
                    vec![
                        ("Population: 0", MetricsField::Population),
                        ("Births: 0", MetricsField::Births),
                        ("Deaths: 0", MetricsField::Deaths),
                        ("Species: 0", MetricsField::Species),
                        ("Biomass: 0", MetricsField::Biomass),
                        ("Resources: 0", MetricsField::Resources),
                        ("Energy: 0", MetricsField::Energy),
                    ]
                };

                for (item, field) in items {
                    parent.spawn((
                        Text::new(item),
                        TextFont {
                            font: bevy::prelude::FontSource::Handle(
                                ui_assets.jetbrains_mono.clone(),
                            ),
                            font_size: bevy::prelude::FontSize::Px(14.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.8, 0.8, 0.8)),
                        field,
                    ));
                }
            })
            .id();
        col_nodes.push(node);
    }

    commands
        .entity(metrics_entity)
        .insert(Node {
            flex_direction: FlexDirection::Row,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        })
        .add_children(&col_nodes);
}
