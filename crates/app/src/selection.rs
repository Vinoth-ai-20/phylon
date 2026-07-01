use crate::camera::MainCamera;
use bevy::prelude::*;

pub struct SelectionPlugin;

impl Plugin for SelectionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectedEntity>()
            .init_resource::<HoveredEntity>()
            .add_message::<SelectionChanged>()
            .add_message::<HoverChanged>()
            .init_resource::<MouseClickTracker>()
            .add_systems(
                Update,
                (selection_picking_system, selection_highlight_system),
            );
    }
}

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedEntity(pub Option<Entity>);

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct HoveredEntity(pub Option<Entity>);

#[derive(Message, Debug, Clone, Copy)]
pub struct SelectionChanged {
    pub entity: Option<Entity>,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct HoverChanged {
    pub entity: Option<Entity>,
}

#[derive(Resource, Default)]
pub struct MouseClickTracker {
    pub last_click_time: f32,
    pub last_clicked_entity: Option<Entity>,
}

#[allow(clippy::too_many_arguments)]
fn selection_picking_system(
    _commands: Commands,
    windows: Query<&Window>,
    mut camera_query: Query<(&Camera, &GlobalTransform, &mut MainCamera)>,
    organism_query: Query<(Entity, &physics::ParticleNode)>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    mut hovered: ResMut<HoveredEntity>,
    mut selected: ResMut<SelectedEntity>,
    mut click_tracker: ResMut<MouseClickTracker>,
    mut selection_evw: bevy::prelude::MessageWriter<SelectionChanged>,
    mut hover_evw: bevy::prelude::MessageWriter<HoverChanged>,
) {
    let Some(window) = windows.iter().next() else {
        return;
    };
    let Some((camera, camera_transform, mut main_camera)) = camera_query.iter_mut().next() else {
        return;
    };

    // Update hovered entity
    let mut new_hovered = None;

    if let Some(cursor_position) = window.cursor_position() {
        if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_position) {
            let mut closest_dist = f32::MAX;
            for (entity, node) in organism_query.iter() {
                let dist = world_pos.distance(Vec2::new(node.position.x, node.position.y));
                // Using 20.0 as a generic hit radius for now, can be refined based on radius component
                if dist < 20.0 && dist < closest_dist {
                    closest_dist = dist;
                    new_hovered = Some(entity);
                }
            }
        }
    }

    if hovered.0 != new_hovered {
        hovered.0 = new_hovered;
        hover_evw.write(HoverChanged {
            entity: new_hovered,
        });
    }

    // Handle clicking
    if mouse_input.just_pressed(MouseButton::Left) {
        if selected.0 != hovered.0 {
            selected.0 = hovered.0;
            selection_evw.write(SelectionChanged { entity: hovered.0 });
        }

        let current_time = time.elapsed_secs();
        let is_double_click = (current_time - click_tracker.last_click_time) < 0.3
            && click_tracker.last_clicked_entity == hovered.0;

        click_tracker.last_click_time = current_time;
        click_tracker.last_clicked_entity = hovered.0;

        // Double click to focus
        if is_double_click {
            if let Some(target) = hovered.0 {
                // Focus camera on this entity and enable spectator mode to follow it
                main_camera.tracked_entity = Some(target);
                main_camera.spectator_mode = true;
            }
        }
    }
}

fn selection_highlight_system(
    mut gizmos: Gizmos,
    selected: Res<SelectedEntity>,
    hovered: Res<HoveredEntity>,
    organism_query: Query<&physics::ParticleNode>,
) {
    if let Some(hovered_ent) = hovered.0 {
        if let Ok(node) = organism_query.get(hovered_ent) {
            gizmos.circle_2d(
                Vec2::new(node.position.x, node.position.y),
                25.0,
                Color::WHITE,
            );
        }
    }

    if let Some(selected_ent) = selected.0 {
        if let Ok(node) = organism_query.get(selected_ent) {
            gizmos.circle_2d(
                Vec2::new(node.position.x, node.position.y),
                30.0,
                bevy::color::Color::srgb(1.0, 1.0, 0.0),
            );
        }
    }
}
