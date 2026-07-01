use crate::layout::MenuBarNode;
use bevy::prelude::*;

pub struct MenuBarPlugin;

impl Plugin for MenuBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            setup_menubar.after(crate::layout::setup_workbench_layout),
        )
        .add_systems(Update, handle_menubar_interactions);
    }
}

#[derive(Component)]
pub enum MenuAction {
    File,
    Edit,
    View,
    Selection,
    Simulation,
    Tools,
    Help,
}

fn setup_menubar(
    mut commands: Commands,
    query: Query<Entity, With<MenuBarNode>>,
    ui_assets: Res<crate::UiAssets>,
) {
    let Some(menubar_entity) = query.iter().next() else {
        return;
    };

    let spawn_menu_item = |commands: &mut Commands, action: MenuAction, text: &str| {
        commands
            .spawn((
                (
                    Button,
                    Node {
                        padding: UiRect::horizontal(Val::Px(12.0)),
                        height: Val::Percent(100.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
                ),
                action,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new(text),
                    TextFont {
                        font: bevy::prelude::FontSource::Handle(ui_assets.inter_medium.clone()),
                        font_size: bevy::prelude::FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.8, 0.8, 0.8)),
                ));
            })
            .id()
    };

    let items = vec![
        spawn_menu_item(&mut commands, MenuAction::File, "File"),
        spawn_menu_item(&mut commands, MenuAction::Edit, "Edit"),
        spawn_menu_item(&mut commands, MenuAction::View, "View"),
        spawn_menu_item(&mut commands, MenuAction::Selection, "Selection"),
        spawn_menu_item(&mut commands, MenuAction::Simulation, "Simulation"),
        spawn_menu_item(&mut commands, MenuAction::Tools, "Tools"),
        spawn_menu_item(&mut commands, MenuAction::Help, "Help"),
    ];

    commands.entity(menubar_entity).add_children(&items);
}

#[allow(clippy::type_complexity)]
fn handle_menubar_interactions(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &MenuAction),
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
