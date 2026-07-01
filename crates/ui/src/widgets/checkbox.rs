use bevy::prelude::*;

/// Configuration for a reusable widget checkbox
#[derive(Component, Debug, Clone)]
pub struct WidgetCheckbox {
    pub is_checked: bool,
    pub box_color: Color,
    pub check_color: Color,
}

impl Default for WidgetCheckbox {
    fn default() -> Self {
        Self {
            is_checked: false,
            box_color: Color::srgb(0.15, 0.15, 0.18),
            check_color: Color::srgb(0.9, 0.9, 0.9),
        }
    }
}

/// Marker component for the checkbox's inner text (` ` vs `X`)
#[derive(Component)]
pub struct CheckboxText;

pub struct WidgetCheckboxPlugin;

impl Plugin for WidgetCheckboxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, checkbox_interaction_system);
    }
}

#[allow(clippy::type_complexity)]
fn checkbox_interaction_system(
    mut interaction_query: Query<
        (&Interaction, &mut WidgetCheckbox, &Children),
        (Changed<Interaction>, With<Button>),
    >,
    mut text_query: Query<&mut Text, With<CheckboxText>>,
) {
    for (interaction, mut config, children) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            config.is_checked = !config.is_checked; // Toggle state

            for child in children.iter() {
                if let Ok(mut text) = text_query.get_mut(child) {
                    if config.is_checked {
                        text.0 = "X".to_string();
                    } else {
                        text.0 = " ".to_string();
                    }
                }
            }
        }
    }
}

/// Helper function to easily spawn widget checkboxes
pub fn spawn_widget_checkbox(
    commands: &mut Commands,
    parent: Entity,
    config: WidgetCheckbox,
    font: Handle<Font>,
    label: &str,
) -> Entity {
    let initial_text = if config.is_checked { "X" } else { " " };
    let check_color = config.check_color;
    let box_color = config.box_color;

    let mut root_id = Entity::PLACEHOLDER;
    commands.entity(parent).with_children(|p| {
        let mut entity_cmds = p.spawn((Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            margin: UiRect::bottom(Val::Px(5.0)),
            ..default()
        },));

        entity_cmds.with_children(|inner_parent| {
            // The clickable box
            inner_parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(24.0),
                        height: Val::Px(24.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(box_color),
                    config,
                ))
                .with_children(|box_parent| {
                    box_parent.spawn((
                        Text::new(initial_text.to_string()),
                        TextFont {
                            font: bevy::prelude::FontSource::Handle(font.clone()),
                            font_size: bevy::prelude::FontSize::Px(16.0),
                            ..default()
                        },
                        TextColor(check_color),
                        CheckboxText,
                    ));
                });

            // The label
            inner_parent.spawn((
                Text::new(label.to_string()),
                TextFont {
                    font: bevy::prelude::FontSource::Handle(font),
                    font_size: bevy::prelude::FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::left(Val::Px(10.0)),
                    ..default()
                },
            ));
        });

        root_id = entity_cmds.id();
    });

    root_id
}
