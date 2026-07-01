use crate::layout::SidebarNode;
use bevy::prelude::*;

pub struct SidebarPlugin;

impl Plugin for SidebarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            setup_sidebar.after(crate::layout::setup_workbench_layout),
        );
    }
}

#[derive(Component, Debug, PartialEq)]
pub enum SidebarPanel {
    Ecology,
    Environment,
    Population,
    Species,
    Resources,
    Climate,
    Selection,
}

fn setup_sidebar(
    mut commands: Commands,
    query: Query<Entity, With<SidebarNode>>,
    ui_assets: Res<crate::UiAssets>,
) {
    let Some(sidebar_entity) = query.iter().next() else {
        return;
    };

    let sections = vec![
        (
            SidebarPanel::Ecology,
            "Ecology",
            "Producers: 0`nHerbivores: 0`nCarnivores: 0",
        ),
        (
            SidebarPanel::Environment,
            "Environment",
            "Time: 00:00`nTemp: 25C",
        ),
        (
            SidebarPanel::Population,
            "Population",
            "Total: 0`nBirths: 0`nDeaths: 0",
        ),
        (SidebarPanel::Species, "Species", "Active Lineages: 0"),
        (
            SidebarPanel::Resources,
            "Resources",
            "Food Pellets: 0`nMinerals: 0",
        ),
        (
            SidebarPanel::Climate,
            "Climate",
            "Weather: Clear`nHazards: None",
        ),
        (SidebarPanel::Selection, "Selection", "None Selected"),
    ];

    let mut section_nodes = vec![];

    for (panel, title, content_text) in sections {
        let section = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    width: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(5.0)),
                    border: UiRect::bottom(Val::Px(1.0)),
                    ..default()
                },
                BorderColor::all(Color::srgb(0.2, 0.2, 0.2)),
            ))
            .with_children(|parent| {
                // Title
                parent.spawn((
                    Text::new(title),
                    TextFont {
                        font: bevy::prelude::FontSource::Handle(ui_assets.inter_bold.clone()),
                        font_size: bevy::prelude::FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
                // Content
                parent.spawn((
                    Text::new(content_text),
                    TextFont {
                        font: bevy::prelude::FontSource::Handle(ui_assets.jetbrains_mono.clone()),
                        font_size: bevy::prelude::FontSize::Px(12.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.7, 0.7, 0.7)),
                    panel,
                ));
            })
            .id();
        section_nodes.push(section);
    }

    commands.entity(sidebar_entity).add_children(&section_nodes);
}
