use bevy::prelude::*;

/// Configuration for a reusable widget radio button
#[derive(Component, Debug, Clone)]
pub struct WidgetRadio {
    pub group_id: String,
    pub is_active: bool,
    pub box_color: Color,
    pub active_color: Color,
}

impl Default for WidgetRadio {
    fn default() -> Self {
        Self {
            group_id: "default".to_string(),
            is_active: false,
            box_color: Color::srgb(0.15, 0.15, 0.18),
            active_color: Color::srgb(0.9, 0.9, 0.9),
        }
    }
}

/// Marker component for the radio's inner visual indicator
#[derive(Component)]
pub struct RadioIndicator;

pub struct WidgetRadioPlugin;

impl Plugin for WidgetRadioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, radio_interaction_system);
    }
}

#[allow(clippy::type_complexity)]
fn radio_interaction_system(
    interaction_query: Query<
        (Entity, &Interaction, &WidgetRadio),
        (Changed<Interaction>, With<Button>),
    >,
    mut all_radios: Query<(Entity, &mut WidgetRadio, &Children)>,
    mut indicator_query: Query<&mut BackgroundColor, With<RadioIndicator>>,
) {
    // First, find if any radio was pressed
    let mut pressed_entity = None;
    let mut pressed_group = None;

    for (entity, interaction, config) in &interaction_query {
        if *interaction == Interaction::Pressed {
            pressed_entity = Some(entity);
            pressed_group = Some(config.group_id.clone());
            break; // Only handle one press per frame
        }
    }

    // If a radio was pressed, update the whole group
    if let (Some(pressed_e), Some(group_id)) = (pressed_entity, pressed_group) {
        for (entity, mut config, children) in &mut all_radios {
            if config.group_id == group_id {
                let is_pressed = entity == pressed_e;
                config.is_active = is_pressed;

                // Update indicator visual
                for child in children.iter() {
                    if let Ok(mut bg) = indicator_query.get_mut(child) {
                        if is_pressed {
                            bg.0 = config.active_color;
                        } else {
                            bg.0 = Color::NONE;
                        }
                    }
                }
            }
        }
    }
}

/// Helper function to easily spawn widget radios
pub fn spawn_widget_radio(
    commands: &mut Commands,
    parent: Entity,
    config: WidgetRadio,
    font: Handle<Font>,
    label: &str,
) -> Entity {
    let initial_indicator_color = if config.is_active {
        config.active_color
    } else {
        Color::NONE
    };
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
            // The clickable box (acting as radio circle)
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
                    // The inner indicator
                    box_parent.spawn((
                        Node {
                            width: Val::Px(12.0),
                            height: Val::Px(12.0),
                            ..default()
                        },
                        BackgroundColor(initial_indicator_color),
                        RadioIndicator,
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
