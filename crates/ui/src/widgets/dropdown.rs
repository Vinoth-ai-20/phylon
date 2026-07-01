use super::button::*;
use bevy::prelude::*;

/// Configuration for a reusable widget dropdown
#[derive(Component, Debug, Clone, Default)]
pub struct WidgetDropdown {
    pub is_open: bool,
}

pub struct WidgetDropdownPlugin;

impl Plugin for WidgetDropdownPlugin {
    fn build(&self, _app: &mut App) {
        // Interaction logic would toggle `is_open` and set visibility of child container
    }
}

/// Helper function to easily spawn a dropdown
pub fn spawn_widget_dropdown(
    commands: &mut Commands,
    parent: Entity,
    config: WidgetDropdown,
    font: Handle<Font>,
    label: &str,
) -> Entity {
    let dropdown_id = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                width: Val::Percent(100.0),
                ..default()
            },
            config,
        ))
        .id();

    // Commands is now free to be used
    commands.entity(parent).add_child(dropdown_id);

    // Header Button
    spawn_widget_button(
        commands,
        dropdown_id,
        WidgetButton::default(),
        font.clone(),
        label,
    );

    // Options Container (Hidden initially)
    commands.entity(dropdown_id).with_children(|p| {
        p.spawn((Node {
            flex_direction: FlexDirection::Column,
            display: Display::None,
            ..default()
        },));
    });

    dropdown_id
}
