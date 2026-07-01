use super::button::*;
use bevy::prelude::*;

/// Configuration for a reusable widget collapsible header/panel
#[derive(Component, Debug, Clone)]
pub struct WidgetCollapsible {
    pub is_open: bool,
}

impl Default for WidgetCollapsible {
    fn default() -> Self {
        Self { is_open: true } // Usually open by default
    }
}

pub struct WidgetCollapsiblePlugin;

impl Plugin for WidgetCollapsiblePlugin {
    fn build(&self, _app: &mut App) {
        // Interaction logic would toggle visibility of content
    }
}

/// Helper function to easily spawn a collapsible panel
pub fn spawn_widget_collapsible(
    commands: &mut Commands,
    parent: Entity,
    config: WidgetCollapsible,
    font: Handle<Font>,
    label: &str,
) -> Entity {
    let collapsible_id = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                width: Val::Percent(100.0),
                margin: UiRect::bottom(Val::Px(10.0)),
                ..default()
            },
            config,
        ))
        .id();

    commands.entity(parent).add_child(collapsible_id);

    // Header Button
    spawn_widget_button(
        commands,
        collapsible_id,
        WidgetButton {
            default_color: Color::srgb(0.15, 0.15, 0.18),
            ..default()
        },
        font.clone(),
        label,
    );

    // Content Container
    commands.entity(collapsible_id).with_children(|p| {
        p.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.2)),
        ));
    });

    collapsible_id
}
