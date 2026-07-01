use bevy::prelude::*;

/// Configuration for a reusable widget scroll view
#[derive(Component, Debug, Clone)]
pub struct WidgetScrollView {
    pub scroll_speed: f32,
    pub max_scroll: f32,
}

impl Default for WidgetScrollView {
    fn default() -> Self {
        Self {
            scroll_speed: 20.0,
            max_scroll: 1000.0,
        }
    }
}

/// Marker component for the scroll view's moving inner container
#[derive(Component)]
pub struct ScrollViewContent;

pub struct WidgetScrollViewPlugin;

impl Plugin for WidgetScrollViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, scroll_system);
    }
}

pub fn scroll_system(
    mut mouse_wheel: bevy::prelude::MessageReader<bevy::input::mouse::MouseWheel>,
    query: Query<(&Interaction, &WidgetScrollView, &Children)>,
    mut content_query: Query<&mut Node, With<ScrollViewContent>>,
) {
    let mut total_scroll = 0.0;
    for ev in mouse_wheel.read() {
        total_scroll += ev.y;
    }

    if total_scroll == 0.0 {
        return;
    }

    for (interaction, config, children) in query.iter() {
        if *interaction == Interaction::Hovered {
            for child in children.iter() {
                if let Ok(mut node) = content_query.get_mut(child) {
                    let current_top = match node.top {
                        Val::Px(val) => val,
                        _ => 0.0,
                    };
                    let new_top = current_top + total_scroll * config.scroll_speed;
                    // Note: Since content scrolls UP to show lower parts, top is negative.
                    let new_top = new_top.clamp(-config.max_scroll, 0.0);
                    node.top = Val::Px(new_top);
                }
            }
        }
    }
}

/// Helper function to easily spawn a widget scroll view
pub fn spawn_widget_scroll_view(
    commands: &mut Commands,
    parent: Entity,
    config: WidgetScrollView,
    bg_color: Color,
    height: Val,
) -> Entity {
    let mut inner_id = Entity::PLACEHOLDER;

    commands.entity(parent).with_children(|p| {
        // Outer Container (Clips content)
        p.spawn((
            Button, // Acts as interaction receiver for hover events
            Node {
                width: Val::Percent(100.0),
                height,
                overflow: Overflow::clip(),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(bg_color),
            config,
        ))
        .with_children(|outer_parent| {
            // Inner Container (Moves up and down)
            let inner = outer_parent.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Auto,
                    flex_direction: FlexDirection::Column,
                    top: Val::Px(0.0), // Start at top
                    ..default()
                },
                ScrollViewContent,
            ));
            inner_id = inner.id();
        });
    });

    inner_id // Return the inner ID so the caller can attach children to it!
}
