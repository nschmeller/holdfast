//! A third-person camera that overlooks the arena.
//!
//! The pitch is deliberately steep. This is a game about reading a board -
//! where the crowd is, which zone is being contested, whether the turret line
//! still holds - so the camera favours legibility of the whole field over any
//! over-the-shoulder intimacy. It pulls back further still in plan mode.

use bevy::camera::Hdr;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;

use crate::AppState;
use crate::common::{ShakeEvent, damp, damp_vec3, to_world};
use crate::player::Player;

/// Camera framing. Yaw is player-controlled in 45-degree steps; everything
/// else is derived.
#[derive(Debug, Resource)]
pub struct CameraRig {
    pub yaw: f32,
    pub target_yaw: f32,
    /// Horizontal distance from the focus point.
    pub distance: f32,
    pub target_distance: f32,
    /// Radians below the horizon. Higher means more top-down.
    pub pitch: f32,
    pub focus: Vec3,
    pub shake: f32,
}

impl Default for CameraRig {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            target_yaw: 0.0,
            distance: BASE_DISTANCE,
            target_distance: BASE_DISTANCE,
            pitch: 0.86,
            focus: Vec3::ZERO,
            shake: 0.0,
        }
    }
}

/// Tuned by eye against the desk arena, which is the smallest of the five: far
/// enough back that roughly two thirds of the board is on screen, so the player
/// can see a flank collapsing before it reaches them, but close enough that
/// individual enemies still read as individuals.
const BASE_DISTANCE: f32 = 34.0;
/// How far the camera pulls back when the player enters plan mode, so the whole
/// board is visible while they think.
const PLAN_DISTANCE: f32 = 52.0;

#[derive(Debug, Component)]
pub struct MainCamera;

#[derive(Debug)]
pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraRig>()
            .add_systems(Startup, spawn_camera)
            .add_systems(
                Update,
                (rotate_camera, absorb_shake).run_if(not(in_state(AppState::Menu))),
            )
            // Runs outside the gameplay set so the camera keeps tracking while
            // overlays are open and time is stopped.
            .add_systems(PostUpdate, drive_camera);
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        // HDR is what makes the emissive materials actually bloom rather than
        // just clip to white.
        Hdr,
        Tonemapping::TonyMcMapface,
        Bloom {
            intensity: 0.22,
            ..Bloom::NATURAL
        },
        Msaa::Sample4,
        Projection::Perspective(PerspectiveProjection {
            fov: 42.0f32.to_radians(),
            near: 0.5,
            far: 400.0,
            ..default()
        }),
        Transform::from_xyz(0.0, 22.0, 22.0).looking_at(Vec3::ZERO, Vec3::Y),
        MainCamera,
    ));
}

fn rotate_camera(
    keys: Res<ButtonInput<KeyCode>>,
    plan: Res<crate::command::PlanMode>,
    mut rig: ResMut<CameraRig>,
) {
    use std::f32::consts::FRAC_PI_4;
    if keys.just_pressed(KeyCode::KeyQ) {
        rig.target_yaw -= FRAC_PI_4;
    }
    if keys.just_pressed(KeyCode::KeyE) {
        rig.target_yaw += FRAC_PI_4;
    }
    rig.target_distance = if plan.active {
        PLAN_DISTANCE
    } else {
        BASE_DISTANCE
    };
}

fn absorb_shake(mut rig: ResMut<CameraRig>, mut shakes: MessageReader<ShakeEvent>) {
    for s in shakes.read() {
        // Saturating rather than additive, so a hundred simultaneous hits do
        // not turn the screen into a blender.
        rig.shake = (rig.shake + s.amount).min(1.4);
    }
}

fn drive_camera(
    time: Res<Time>,
    plan: Res<crate::command::PlanMode>,
    mut rig: ResMut<CameraRig>,
    player: Query<&crate::common::Body, With<Player>>,
    mut cam: Query<&mut Transform, With<MainCamera>>,
) {
    // Uses unscaled time on purpose: in plan mode the world crawls but the
    // camera should still feel responsive to the player's input.
    let dt = time.delta_secs().min(0.05);

    let Ok(mut transform) = cam.single_mut() else {
        return;
    };

    // Follow the player, or the build cursor while placing.
    let anchor = if plan.active {
        plan.cursor
    } else {
        player.iter().next().map_or(Vec2::ZERO, |b| b.pos)
    };

    // The world is unbounded, so the camera simply follows: there is no centre
    // to bias towards and no corner to hug.
    let target_focus = to_world(anchor, 0.0);

    rig.focus = damp_vec3(rig.focus, target_focus, 7.0, dt);
    rig.yaw = crate::player::angle_lerp(rig.yaw, rig.target_yaw, 9.0 * dt);
    rig.distance = damp(rig.distance, rig.target_distance, 5.0, dt);
    rig.shake = damp(rig.shake, 0.0, 6.0, dt);

    let dist = rig.distance;

    let (sin, cos) = rig.yaw.sin_cos();
    let horizontal = Vec3::new(sin, 0.0, cos) * (dist * rig.pitch.cos());
    let height = dist * rig.pitch.sin();
    let mut eye = rig.focus + horizontal + Vec3::Y * height;

    if rig.shake > 0.001 {
        // Cheap deterministic jitter from the clock; no RNG resource needed
        // and it stays smooth rather than snapping between frames.
        let t = time.elapsed_secs();
        let amp = rig.shake * rig.shake * 0.85;
        eye += Vec3::new(
            (t * 51.0).sin() * amp,
            (t * 43.0).cos() * amp * 0.6,
            (t * 37.0).sin() * amp,
        );
    }

    transform.translation = eye;
    transform.look_at(rig.focus, Vec3::Y);
}
