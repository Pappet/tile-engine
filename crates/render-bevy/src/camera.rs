use bevy::prelude::*;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use crate::ActiveZLayer;

#[derive(Component)]
pub struct TileCamera;

pub fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2dBundle {
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            ..default()
        },
        TileCamera,
    ));
}

const PAN_SPEED: f32 = 400.0;
const ZOOM_SPEED: f32 = 0.1;

pub fn camera_control_system(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut mouse_wheel_events: EventReader<MouseWheel>,
    mut active_z: ResMut<ActiveZLayer>,
    mut query: Query<(&mut Transform, &mut OrthographicProjection), With<TileCamera>>,
) {
    let Ok((mut transform, mut projection)) = query.get_single_mut() else {
        return;
    };

    // Panning
    let mut direction = Vec2::ZERO;
    if keyboard_input.pressed(KeyCode::KeyW) || keyboard_input.pressed(KeyCode::ArrowUp) {
        direction.y += 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyS) || keyboard_input.pressed(KeyCode::ArrowDown) {
        direction.y -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyA) || keyboard_input.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyD) || keyboard_input.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }

    if direction.length_squared() > 0.0 {
        direction = direction.normalize();
        // Adjust speed based on zoom level
        let speed = PAN_SPEED * projection.scale;
        transform.translation += direction.extend(0.0) * speed * time.delta_seconds();
    }

    // Zooming
    for ev in mouse_wheel_events.read() {
        let scroll = match ev.unit {
            MouseScrollUnit::Line => ev.y,
            MouseScrollUnit::Pixel => ev.y * 0.01,
        };
        let log_scale = projection.scale.ln() - scroll * ZOOM_SPEED;
        projection.scale = log_scale.exp().clamp(0.1, 10.0);
    }

    // Z-layer changes
    if keyboard_input.just_pressed(KeyCode::KeyQ) {
        active_z.0 -= 1;
    }
    if keyboard_input.just_pressed(KeyCode::KeyE) {
        active_z.0 += 1;
    }
}
