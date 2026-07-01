use crate::layout::NavigationRailNode;
use bevy::prelude::*;

pub struct NavigationRailPlugin;

impl Plugin for NavigationRailPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            setup_navigation_rail.after(crate::layout::setup_workbench_layout),
        )
        .add_systems(Update, handle_navrail_interactions);
    }
}

#[derive(Component)]
pub enum NavRailAction {
    SelectTool,
    MoveTool,
    AddOrganismTool,
    AddFoodTool,
    Analytics,
    Settings,
}

fn setup_navigation_rail(
    mut commands: Commands,
    query: Query<Entity, With<NavigationRailNode>>,
    ui_assets: Res<crate::UiAssets>,
) {
    let Some(rail_entity) = query.iter().next() else {
        return;
    };

    let spawn_nav_item = |commands: &mut Commands, action: NavRailAction, icon_text: &str| {
        commands
            .spawn((
                (
                    Button,
                    Node {
                        width: Val::Px(36.0),
                        height: Val::Px(36.0),
                        margin: UiRect::bottom(Val::Px(8.0)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        // Basic rounded corner effect in Bevy 0.14+ using border radius if we want,
                        // but sticking to standard Node setup for now.
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
                ),
                action,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new(icon_text),
                    TextFont {
                        font: bevy::prelude::FontSource::Handle(ui_assets.jetbrains_mono.clone()),
                        font_size: bevy::prelude::FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.7, 0.7, 0.7)),
                ));
            })
            .id()
    };

    let items = vec![
        spawn_nav_item(&mut commands, NavRailAction::SelectTool, "[S]"),
        spawn_nav_item(&mut commands, NavRailAction::MoveTool, "[M]"),
        spawn_nav_item(&mut commands, NavRailAction::AddOrganismTool, "[+]"),
        spawn_nav_item(&mut commands, NavRailAction::AddFoodTool, "[F]"),
        // Spacer
        commands
            .spawn(Node {
                flex_grow: 1.0,
                ..default()
            })
            .id(),
        spawn_nav_item(&mut commands, NavRailAction::Analytics, "[A]"),
        spawn_nav_item(&mut commands, NavRailAction::Settings, "[O]"),
    ];

    commands.entity(rail_entity).add_children(&items);
}

#[allow(clippy::type_complexity)]
fn handle_navrail_interactions(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &NavRailAction),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, mut color, _action) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(Color::srgb(0.2, 0.4, 0.8));
            }
            Interaction::Hovered => {
                *color = BackgroundColor(Color::srgb(0.2, 0.2, 0.25));
            }
            Interaction::None => {
                *color = BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0));
            }
        }
    }
}
