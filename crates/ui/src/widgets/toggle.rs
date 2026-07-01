use bevy::prelude::*;

/// Configuration for a reusable widget toggle button
#[derive(Component, Debug, Clone)]
pub struct WidgetToggle {
    pub is_active: bool,
    pub inactive_color: Color,
    pub active_color: Color,
    pub hover_color: Color,
}

impl Default for WidgetToggle {
    fn default() -> Self {
        Self {
            is_active: false,
            inactive_color: Color::srgb(0.2, 0.2, 0.25),
            active_color: Color::srgb(0.2, 0.6, 0.2),
            hover_color: Color::srgb(0.3, 0.3, 0.35),
        }
    }
}

pub struct WidgetTogglePlugin;

impl Plugin for WidgetTogglePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, toggle_interaction_system);
    }
}

#[allow(clippy::type_complexity)]
fn toggle_interaction_system(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &mut WidgetToggle),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, mut color, mut config) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                config.is_active = !config.is_active; // Toggle state
                if config.is_active {
                    *color = BackgroundColor(config.active_color);
                } else {
                    *color = BackgroundColor(config.inactive_color);
                }
            }
            Interaction::Hovered => {
                *color = BackgroundColor(config.hover_color);
            }
            Interaction::None => {
                if config.is_active {
                    *color = BackgroundColor(config.active_color);
                } else {
                    *color = BackgroundColor(config.inactive_color);
                }
            }
        }
    }
}

/// Helper function to easily spawn widget toggles
pub fn spawn_widget_toggle(
    commands: &mut Commands,
    parent: Entity,
    config: WidgetToggle,
    font: Handle<Font>,
    label: &str,
) -> Entity {
    let initial_color = if config.is_active {
        config.active_color
    } else {
        config.inactive_color
    };

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
            BackgroundColor(initial_color),
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
