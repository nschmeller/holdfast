//! Particles, floating text and hit feedback.
//!
//! All of it is pooled or capped. Feedback should never be the thing that
//! drops the frame rate, because feedback is what tells the player the frame
//! rate matters.

use bevy::prelude::*;

use crate::art::GameArt;
use crate::camera::MainCamera;
use crate::common::*;
use crate::rng::Rng;
use crate::{AppState, GameSet};

/// Hard ceilings. A boss death plus a nova plus a wave clear can request a
/// thousand particles in one frame; the cap turns that into a spike we can
/// afford rather than a stall.
const MAX_PARTICLES: usize = 420;
const MAX_FLOATERS: usize = 44;

#[derive(Component)]
pub struct Particle {
    pub vel: Vec3,
    pub life: f32,
    pub max_life: f32,
    pub spin: Vec3,
    pub gravity: f32,
    pub base_scale: f32,
}

/// A world-anchored piece of UI text that drifts up and fades.
#[derive(Component)]
pub struct Floater {
    pub world: Vec2,
    pub height: f32,
    pub life: f32,
    pub max_life: f32,
    pub rise: f32,
    pub drift: f32,
}

#[derive(Resource, Default)]
pub struct FxCounts {
    pub particles: usize,
    pub floaters: usize,
}

pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FxCounts>()
            .add_systems(
                Update,
                (spawn_bursts, spawn_floaters).in_set(GameSet::Present),
            )
            // Particles and text keep animating while overlays are up, so a
            // level-up does not freeze a half-finished explosion on screen.
            .add_systems(PostUpdate, (tick_particles, tick_floaters));
    }
}

fn spawn_bursts(
    mut commands: Commands,
    art: Res<GameArt>,
    mut rng: ResMut<Rng>,
    mut counts: ResMut<FxCounts>,
    mut events: MessageReader<BurstEvent>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for burst in events.read() {
        let budget = MAX_PARTICLES.saturating_sub(counts.particles);
        if budget == 0 {
            continue;
        }
        let count = (burst.count as usize).min(budget);

        // One material per burst colour. Cheap: bursts are transient and Bevy
        // dedupes nothing here, but the alternative is a per-particle material.
        let material = materials.add(StandardMaterial {
            base_color: burst.color,
            emissive: {
                let l = burst.color.to_linear();
                LinearRgba::rgb(l.red * 2.4, l.green * 2.4, l.blue * 2.4)
            },
            ..default()
        });

        for _ in 0..count {
            let dir = rng.unit_circle();
            let speed = burst.speed * rng.range(0.35, 1.0);
            let vel = Vec3::new(dir.x * speed, rng.range(1.5, 6.5), dir.z * speed);
            let life = rng.range(0.35, 0.85);
            let scale = burst.size * rng.range(0.5, 1.2);

            commands.spawn((
                Particle {
                    vel,
                    life,
                    max_life: life,
                    spin: Vec3::new(
                        rng.range(-9.0, 9.0),
                        rng.range(-9.0, 9.0),
                        rng.range(-9.0, 9.0),
                    ),
                    gravity: 17.0,
                    base_scale: scale,
                },
                Mesh3d(art.particle.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(to_world(burst.pos, burst.height))
                    .with_scale(Vec3::splat(scale)),
                RunEntity,
            ));
            counts.particles += 1;
        }
    }
}

fn tick_particles(
    time: Res<Time<Real>>,
    mut commands: Commands,
    mut counts: ResMut<FxCounts>,
    mut q: Query<(Entity, &mut Particle, &mut Transform)>,
) {
    let dt = time.delta_secs().min(0.05);
    for (entity, mut p, mut transform) in &mut q {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(entity).despawn();
            counts.particles = counts.particles.saturating_sub(1);
            continue;
        }

        p.vel.y -= p.gravity * dt;
        transform.translation += p.vel * dt;

        // Bounce once off the floor instead of sinking through it.
        if transform.translation.y < 0.06 && p.vel.y < 0.0 {
            transform.translation.y = 0.06;
            p.vel.y = -p.vel.y * 0.35;
            p.vel.x *= 0.6;
            p.vel.z *= 0.6;
        }

        transform.rotate_x(p.spin.x * dt);
        transform.rotate_y(p.spin.y * dt);
        transform.rotate_z(p.spin.z * dt);

        // Shrink out; a fade would need per-particle alpha and another
        // material, and at this size the difference is invisible.
        let t = (p.life / p.max_life).clamp(0.0, 1.0);
        transform.scale = Vec3::splat(p.base_scale * t.sqrt());
    }
}

fn spawn_floaters(
    mut commands: Commands,
    mut counts: ResMut<FxCounts>,
    mut events: MessageReader<FloatingTextEvent>,
    mut rng: ResMut<Rng>,
) {
    for ev in events.read() {
        if counts.floaters >= MAX_FLOATERS {
            // Drop the oldest requests silently; damage numbers are the most
            // expendable thing on screen.
            continue;
        }
        counts.floaters += 1;

        let life = 0.85;
        commands.spawn((
            Floater {
                world: ev.pos,
                height: ev.height,
                life,
                max_life: life,
                rise: rng.range(1.4, 2.2),
                drift: rng.range(-0.7, 0.7),
            },
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            Text::new(ev.text.clone()),
            TextFont {
                font_size: crate::hud::px(ev.size),
                ..default()
            },
            TextColor(ev.color),
            // Above the world, below the HUD panels.
            GlobalZIndex(5),
            RunEntity,
        ));
    }
}

fn tick_floaters(
    time: Res<Time<Real>>,
    mut commands: Commands,
    mut counts: ResMut<FxCounts>,
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut q: Query<(Entity, &mut Floater, &mut Node, &mut TextColor)>,
) {
    let dt = time.delta_secs().min(0.05);
    let Ok((cam, cam_transform)) = camera.single() else {
        return;
    };

    for (entity, mut floater, mut node, mut color) in &mut q {
        floater.life -= dt;
        if floater.life <= 0.0 {
            commands.entity(entity).despawn();
            counts.floaters = counts.floaters.saturating_sub(1);
            continue;
        }

        let t = 1.0 - (floater.life / floater.max_life).clamp(0.0, 1.0);
        floater.height += floater.rise * dt;
        floater.world.x += floater.drift * dt;

        let world = to_world(floater.world, floater.height);
        match cam.world_to_viewport(cam_transform, world) {
            Ok(screen) => {
                node.left = Val::Px(screen.x);
                node.top = Val::Px(screen.y);
                node.display = Display::Flex;
            }
            Err(_) => {
                // Behind the camera or off-screen: hide rather than clamp, so
                // numbers never pile up along the screen edge.
                node.display = Display::None;
            }
        }

        // Fade out over the back half of the life.
        let alpha = (1.0 - (t - 0.4).max(0.0) / 0.6).clamp(0.0, 1.0);
        color.0 = color.0.with_alpha(alpha);
    }
}

/// Clear every transient effect, used when a run ends.
pub fn clear_fx(
    mut commands: Commands,
    mut counts: ResMut<FxCounts>,
    particles: Query<Entity, With<Particle>>,
    floaters: Query<Entity, With<Floater>>,
) {
    for e in &particles {
        commands.entity(e).despawn();
    }
    for e in &floaters {
        commands.entity(e).despawn();
    }
    counts.particles = 0;
    counts.floaters = 0;
}

/// Shared by anything that wants a state-scoped despawn.
pub fn despawn_all<T: Component>(mut commands: Commands, q: Query<Entity, With<T>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

pub fn on_exit_menu_clear(_: Commands) {}

/// Keeps `AppState` in scope for the plugin's run conditions.
const _: Option<AppState> = None;
