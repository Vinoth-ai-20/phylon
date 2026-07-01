use analytics::MetricsState;
use bevy::prelude::*;

pub struct GraphsPlugin;

impl Plugin for GraphsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_graphs_system);
    }
}

#[derive(Component)]
pub struct GraphContainer {
    pub metric_type: MetricType,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MetricType {
    Population,
    FPS,
}

#[derive(Component)]
pub struct GraphBar {
    pub index: usize,
    pub metric_type: MetricType,
}

#[macro_export]
macro_rules! spawn_graph {
    ($parent:expr, $metric_type:expr, $title:expr, $color:expr, $num_bars:expr, $font:expr) => {
        $parent
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(100.0),
                    flex_direction: FlexDirection::Column,
                    margin: UiRect::bottom(Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.05, 0.05, 1.0)),
                $crate::ui::graph::GraphContainer {
                    metric_type: $metric_type,
                },
            ))
            .with_children(|container| {
                // Title
                container.spawn((
                    Text::new($title),
                    TextFont {
                        font: bevy::prelude::FontSource::Handle($font.clone()),
                        font_size: bevy::prelude::FontSize::Px(12.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Node {
                        margin: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                ));

                // Bars container
                container
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::FlexEnd,
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                    ))
                    .with_children(|bars| {
                        for i in 0..$num_bars {
                            bars.spawn((
                                Node {
                                    width: Val::Percent(100.0 / $num_bars as f32),
                                    height: Val::Percent(0.0),
                                    ..default()
                                },
                                BackgroundColor($color),
                                $crate::ui::graph::GraphBar {
                                    index: i,
                                    metric_type: $metric_type,
                                },
                            ));
                        }
                    });
            });
    };
}

fn update_graphs_system(metrics: Res<MetricsState>, mut bar_query: Query<(&GraphBar, &mut Node)>) {
    if !metrics.is_changed() {
        return;
    }

    let pop_max = 500.0_f32; // Placeholder max
    let fps_max = 120.0_f32;

    for (bar, mut node) in bar_query.iter_mut() {
        let val = match bar.metric_type {
            MetricType::Population => {
                if bar.index < metrics.producers_history.len() {
                    metrics.producers_history[bar.index][1] as f32
                } else {
                    0.0
                }
            }
            MetricType::FPS => {
                if bar.index < metrics.fps_history.len() {
                    metrics.fps_history[bar.index][1] as f32
                } else {
                    0.0
                }
            }
        };

        let max = match bar.metric_type {
            MetricType::Population => pop_max,
            MetricType::FPS => fps_max,
        };

        let pct = (val / max).clamp(0.0, 1.0) * 100.0;
        node.height = Val::Percent(pct);
    }
}
