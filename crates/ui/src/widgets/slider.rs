use bevy::{ecs::relationship::Relationship, prelude::*, window::PrimaryWindow};

/// Configuration for a reusable widget slider
#[derive(Component, Debug, Clone)]
pub struct WidgetSlider {
    pub min: f32,
    pub max: f32,
    pub value: f32,
    pub track_color: Color,
    pub fill_color: Color,
}

impl Default for WidgetSlider {
    fn default() -> Self {
        Self {
            min: 0.0,
            max: 1.0,
            value: 0.5,
            track_color: Color::srgb(0.1, 0.1, 0.15),
            fill_color: Color::srgb(0.3, 0.6, 0.9),
        }
    }
}

/// Marker component for the draggable slider container
#[derive(Component)]
pub struct SliderContainer;

/// Marker component for the slider fill area
#[derive(Component)]
pub struct SliderFill;

pub struct WidgetSliderPlugin;

impl Plugin for WidgetSliderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (slider_interaction_system, slider_render_system));
    }
}

fn slider_interaction_system(
    mut interaction_query: Query<
        (
            &Interaction,
            &mut WidgetSlider,
            &ComputedNode,
            &GlobalTransform,
        ),
        With<SliderContainer>,
    >,
    window_query: Query<&Window, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
) {
    let Some(window) = window_query.iter().next() else {
        return;
    };

    // Check if mouse is pressed
    let left_pressed = buttons.pressed(MouseButton::Left);

    if let Some(cursor_pos) = window.cursor_position() {
        for (interaction, mut slider, computed_node, transform) in &mut interaction_query {
            // We want to drag if we clicked on the slider and are still holding it down,
            // but Bevy UI Interaction only gives Pressed if hovered.
            // For a robust slider we'd track drag state, but for this iteration we'll rely on Pressed.
            if *interaction == Interaction::Pressed
                || (left_pressed && *interaction == Interaction::Hovered)
            {
                let node_pos = transform.translation().truncate(); // Center of node
                let node_size = computed_node.size(); // Physical size

                let rect_min = node_pos - node_size / 2.0;

                // Calculate percentage
                let mut percent = (cursor_pos.x - rect_min.x) / node_size.x;
                percent = percent.clamp(0.0, 1.0);

                slider.value = slider.min + percent * (slider.max - slider.min);
            }
        }
    }
}

fn slider_render_system(
    mut fill_query: Query<(&ChildOf, &mut Node), With<SliderFill>>,
    slider_query: Query<&WidgetSlider, With<SliderContainer>>,
) {
    for (parent, mut node) in &mut fill_query {
        if let Ok(slider) = slider_query.get(parent.get()) {
            let percent = (slider.value - slider.min) / (slider.max - slider.min);
            let percent = percent.clamp(0.0, 1.0);
            node.width = Val::Percent(percent * 100.0);
        }
    }
}

/// Helper function to easily spawn widget sliders
pub fn spawn_widget_slider(
    commands: &mut Commands,
    parent: Entity,
    config: WidgetSlider,
) -> Entity {
    let mut root_id = Entity::PLACEHOLDER;
    commands.entity(parent).with_children(|p| {
        let mut entity_cmds = p.spawn((
            Button, // Acts as interaction receiver
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(20.0),
                margin: UiRect::bottom(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(config.track_color),
            config.clone(),
            SliderContainer,
        ));

        entity_cmds.with_children(|inner_parent| {
            inner_parent.spawn((
                Node {
                    width: Val::Percent(0.0), // Will be updated by system
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(config.fill_color),
                SliderFill,
            ));
        });

        root_id = entity_cmds.id();
    });

    root_id
}
