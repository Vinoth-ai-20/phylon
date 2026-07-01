use bevy::prelude::*;

/// Configuration for a reusable widget button
#[derive(Component, Debug, Clone)]
pub struct WidgetButton {
    pub default_color: Color,
    pub hover_color: Color,
    pub pressed_color: Color,
}

impl Default for WidgetButton {
    fn default() -> Self {
        Self {
            default_color: Color::srgb(0.2, 0.2, 0.25),
            hover_color: Color::srgb(0.3, 0.3, 0.35),
            pressed_color: Color::srgb(0.4, 0.4, 0.45),
        }
    }
}

pub struct WidgetButtonPlugin;

impl Plugin for WidgetButtonPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, button_interaction_system);
    }
}

#[allow(clippy::type_complexity)]
fn button_interaction_system(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &WidgetButton),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, mut color, config) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(config.pressed_color);
            }
            Interaction::Hovered => {
                *color = BackgroundColor(config.hover_color);
            }
            Interaction::None => {
                *color = BackgroundColor(config.default_color);
            }
        }
    }
}

/// Helper function to easily spawn widget buttons
pub fn spawn_widget_button(
    commands: &mut Commands,
    parent: Entity,
    config: WidgetButton,
    font: Handle<Font>,
    label: &str,
) -> Entity {
    let mut button_id = Entity::PLACEHOLDER;
    commands.entity(parent).with_children(|p| {
        let mut entity_cmds = p.spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(40.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                margin: UiRect::bottom(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(config.default_color),
            config,
        ));

        entity_cmds.with_children(|inner_parent| {
            inner_parent.spawn((
                Text::new(label.to_string()),
                TextFont {
                    font: bevy::prelude::FontSource::Handle(font),
                    font_size: bevy::prelude::FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });

        button_id = entity_cmds.id();
    });

    button_id
}
