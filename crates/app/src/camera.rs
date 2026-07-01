use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_camera).add_systems(
            Update,
            (
                spectator_system,
                camera_controller_system,
                camera_control_listener,
            )
                .chain(),
        );
    }
}

/// The state of our simulation camera.
#[derive(Component)]
pub struct MainCamera {
    pub target_pos: Vec2,
    pub target_zoom: f32,
    pub pan_speed: f32,
    pub zoom_speed: f32,
    pub min_zoom: f32,
    pub max_zoom: f32,
    pub bounds: Rect,
    pub spectator_mode: bool,
    pub tracked_entity: Option<Entity>,
    pub last_spectator_switch: f32,
}

impl Default for MainCamera {
    fn default() -> Self {
        Self {
            target_pos: Vec2::ZERO,
            target_zoom: 1.0,
            pan_speed: 500.0,
            zoom_speed: 0.1,
            min_zoom: 0.1,
            max_zoom: 10.0,
            bounds: Rect::from_center_size(Vec2::ZERO, Vec2::new(4000.0, 4000.0)),
            spectator_mode: false,
            tracked_entity: None,
            last_spectator_switch: 0.0,
        }
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((Camera2d, MainCamera::default()));

    // Dummy sprite to force Bevy to run main_opaque_pass_2d,
    // which ensures the screen ping-pong buffers are cleared each frame.
    // Without this, the opaque phase is empty and Bevy skips the clear, leaving trails.
    commands.spawn((
        bevy::sprite::Sprite {
            color: Color::srgb(0.02, 0.02, 0.02),
            custom_size: Some(Vec2::new(1.0, 1.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -999.0),
    ));
}

fn camera_controller_system(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: bevy::prelude::MessageReader<MouseMotion>,
    mut mouse_wheel: bevy::prelude::MessageReader<MouseWheel>,
    mut query: Query<(&mut Transform, &mut Projection, &mut MainCamera)>,
) {
    let dt = time.delta_secs();
    let Some((mut transform, mut projection, mut state)) = query.iter_mut().next() else {
        return;
    };

    // Zooming
    let mut zoom_delta = 0.0;
    for ev in mouse_wheel.read() {
        match ev.unit {
            MouseScrollUnit::Line => zoom_delta += ev.y,
            MouseScrollUnit::Pixel => zoom_delta += ev.y * 0.01,
        }
    }

    // Smooth zoom update
    if zoom_delta != 0.0 {
        let factor = 1.0_f32 - zoom_delta * state.zoom_speed;
        state.target_zoom *= factor.clamp(0.1_f32, 10.0_f32);
        state.target_zoom = state.target_zoom.clamp(state.min_zoom, state.max_zoom);
    }
    if let Projection::Orthographic(ortho) = projection.as_mut() {
        ortho.scale = ortho.scale.lerp(state.target_zoom, 10.0 * dt);
    }

    if keys.just_pressed(KeyCode::Space) {
        state.spectator_mode = !state.spectator_mode;
        if state.spectator_mode {
            info!("Spectator mode enabled");
        } else {
            info!("Spectator mode disabled");
            state.tracked_entity = None;
        }
    }

    // Panning (Keyboard)
    let mut pan = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        pan.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        pan.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        pan.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        pan.x += 1.0;
    }

    // Normalize pan vector
    if pan.length_squared() > 0.0 {
        pan = pan.normalize();
        let scale = if let Projection::Orthographic(ortho) = projection.as_ref() {
            ortho.scale
        } else {
            1.0
        };
        let pan_speed = state.pan_speed;
        state.target_pos += pan * pan_speed * scale * dt;
        state.spectator_mode = false; // Interrupt spectator mode if user manually pans
    }

    // Panning (Mouse drag)
    if mouse_buttons.pressed(MouseButton::Middle) || mouse_buttons.pressed(MouseButton::Right) {
        let mut mouse_pan = Vec2::ZERO;
        for ev in mouse_motion.read() {
            mouse_pan += ev.delta;
        }
        // y is inverted in screen space vs world space
        let scale = if let Projection::Orthographic(ortho) = projection.as_ref() {
            ortho.scale
        } else {
            1.0
        };
        if mouse_pan.length_squared() > 0.0 {
            state.target_pos.x -= mouse_pan.x * scale;
            state.target_pos.y += mouse_pan.y * scale;
            state.spectator_mode = false;
        }
    }

    // If we have a tracked entity and spectator mode is on, follow it
    if state.spectator_mode {
        if let Some(_target_entity) = state.tracked_entity {
            // Note: the position is updated in `spectator_system`
            // We just need to make sure we don't allow panning to override it this frame
        }
    }

    let scale = if let Projection::Orthographic(ortho) = projection.as_ref() {
        ortho.scale
    } else {
        1.0
    };
    let area = if let Projection::Orthographic(ortho) = projection.as_ref() {
        ortho.area
    } else {
        Rect::default()
    };

    let half_width = (area.width() / 2.0) * scale;
    let half_height = (area.height() / 2.0) * scale;

    let min_x = state.bounds.min.x + half_width;
    let max_x = f32::max(min_x, state.bounds.max.x - half_width);
    state.target_pos.x = state.target_pos.x.clamp(min_x, max_x);

    let min_y = state.bounds.min.y + half_height;
    let max_y = f32::max(min_y, state.bounds.max.y - half_height);
    state.target_pos.y = state.target_pos.y.clamp(min_y, max_y);

    // Smooth position interpolation
    let target_vec3 = state.target_pos.extend(transform.translation.z);
    transform.translation = transform.translation.lerp(target_vec3, 15.0 * dt);
}

fn camera_control_listener(
    mut events: bevy::prelude::MessageReader<workbench::events::CameraControlEvent>,
    mut query: Query<(&mut MainCamera, &mut Transform)>,
    selected_entity: Res<crate::selection::SelectedEntity>,
) {
    let Some((mut camera, mut transform)) = query.iter_mut().next() else {
        return;
    };

    for ev in events.read() {
        match ev {
            workbench::events::CameraControlEvent::ResetCamera => {
                camera.target_pos = Vec2::ZERO;
                camera.target_zoom = 1.0;
                transform.translation.x = 0.0;
                transform.translation.y = 0.0;
                camera.spectator_mode = false;
                camera.tracked_entity = None;
            }
            workbench::events::CameraControlEvent::ToggleSpectator => {
                camera.spectator_mode = !camera.spectator_mode;
                if camera.spectator_mode {
                    // Try to track selected entity first, else spectator_system picks best
                    if let Some(e) = selected_entity.0 {
                        camera.tracked_entity = Some(e);
                    }
                    info!("Spectator mode enabled");
                } else {
                    info!("Spectator mode disabled");
                    camera.tracked_entity = None;
                }
            }
        }
    }
}

fn spectator_system(
    time: Res<Time>,
    mut camera_query: Query<&mut MainCamera>,
    organism_query: Query<(
        Entity,
        &physics::ParticleNode,
        &organisms::components::Generation,
        &organisms::components::BiologicalComponents,
    )>,
) {
    let Some(mut state) = camera_query.iter_mut().next() else {
        return;
    };

    if !state.spectator_mode {
        return;
    }

    let current_time = time.elapsed_secs();
    let is_tracked_dead = state.tracked_entity.is_none()
        || organism_query.get(state.tracked_entity.unwrap()).is_err();

    if is_tracked_dead || current_time - state.last_spectator_switch > 15.0 {
        // Find most "interesting" organism (e.g., highest generation or oldest)
        let mut best_entity = None;
        let mut best_score = -1.0;

        for (entity, _node, generation, biology) in organism_query.iter() {
            let score = generation.0 as f32 * 100.0 + biology.age_ticks as f32;
            if score > best_score {
                best_score = score;
                best_entity = Some(entity);
            }
        }

        if let Some(new_target) = best_entity {
            state.tracked_entity = Some(new_target);
            state.last_spectator_switch = current_time;
        }
    }

    if let Some(target_entity) = state.tracked_entity {
        if let Ok((_, node, _, _)) = organism_query.get(target_entity) {
            state.target_pos = Vec2::new(node.position.x, node.position.y);
        }
    }
}
