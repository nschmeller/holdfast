//! The hero unit and the stat block every other system multiplies against.
//!
//! The player is fast enough to always be able to walk out of a crowd. That is
//! a design guarantee, not a tuning accident: losing ground should be a
//! decision you made two minutes ago, never a dodge you fluffed.

use bevy::prelude::*;

use crate::arena::{ArenaBounds, Gust, Hazard, ObstacleField, Spotlight};
use crate::art::GameArt;
use crate::combat::Damageable;
use crate::common::{
    Altitude, Body, DamageEvent, DamageSource, Health, RunEntity, VisualScale, damp, damp_vec2,
    yaw_towards,
};
use crate::enemy::StatusEffects;
use crate::{AppState, GameSet, RunSetup};

/// The player's base movement speed. Every enemy is tuned below this.
pub const BASE_SPEED: f32 = 8.4;
pub const PLAYER_RADIUS: f32 = 0.52;

#[derive(Debug, Component)]
pub struct Player;

/// Everything that upgrades, gear and research modify. Recomputed from a base
/// plus accumulated modifiers whenever anything changes, so there is exactly
/// one place that decides what a stat is worth.
#[derive(Resource, Clone, Debug)]
pub struct PlayerStats {
    pub max_hp: f32,
    pub move_speed: f32,
    pub damage_mult: f32,
    /// Multiplies weapon fire rate.
    pub haste: f32,
    /// Scales weapon areas and blast radii.
    pub area: f32,
    pub projectile_speed: f32,
    pub extra_projectiles: u32,
    pub crit_chance: f32,
    pub crit_mult: f32,
    /// Flat damage reduction, applied before the percentage floor.
    pub armor: f32,
    pub regen: f32,
    pub pickup_radius: f32,
    pub xp_mult: f32,
    pub scrap_mult: f32,
    pub core_mult: f32,
    /// Improves rarity rolls on drops.
    pub luck: f32,
    pub knockback: f32,
    /// Multiplies damage dealt by allies.
    pub ally_damage: f32,
    /// Multiplies damage dealt by structures.
    pub structure_damage: f32,
    pub ally_health: f32,
    pub structure_health: f32,
    /// Fractional discount on build costs, 0.0 to 0.75.
    pub build_discount: f32,
    /// Scales durations of hazards, buffs and status effects we apply.
    pub duration: f32,
    pub dash_cooldown: f32,
    pub zone_capture_rate: f32,
    pub income_mult: f32,
}

impl Default for PlayerStats {
    fn default() -> Self {
        Self {
            max_hp: 120.0,
            move_speed: BASE_SPEED,
            damage_mult: 1.0,
            haste: 1.0,
            area: 1.0,
            projectile_speed: 1.0,
            extra_projectiles: 0,
            crit_chance: 0.05,
            crit_mult: 2.0,
            armor: 0.0,
            regen: 0.4,
            pickup_radius: 3.2,
            xp_mult: 1.0,
            scrap_mult: 1.0,
            core_mult: 1.0,
            luck: 0.0,
            knockback: 1.0,
            ally_damage: 1.0,
            structure_damage: 1.0,
            ally_health: 1.0,
            structure_health: 1.0,
            build_discount: 0.0,
            duration: 1.0,
            dash_cooldown: 3.0,
            zone_capture_rate: 1.0,
            income_mult: 1.0,
        }
    }
}

impl PlayerStats {
    /// Damage after armour. A flat subtraction alone would eventually make the
    /// player immortal, so it is floored at 12% of the incoming hit.
    pub fn mitigate(&self, raw: f32) -> f32 {
        (raw - self.armor).max(raw * 0.12)
    }
}

/// Per-frame movement intent, written by input and consumed by movement so the
/// two can be tested and re-sourced (touch, gamepad) independently.
#[derive(Debug, Component, Default)]
pub struct Intent {
    pub move_dir: Vec2,
    pub dash: bool,
}

#[derive(Debug, Component)]
pub struct Dash {
    pub cooldown: f32,
    pub active: f32,
    pub dir: Vec2,
}

impl Default for Dash {
    fn default() -> Self {
        Self {
            cooldown: 0.0,
            active: 0.0,
            dir: Vec2::Y,
        }
    }
}

/// Which way the model is facing, smoothed so it never snaps.
#[derive(Debug, Component, Default)]
pub struct Facing {
    pub yaw: f32,
    pub target: f32,
}

#[derive(Debug)]
pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerStats>()
            .add_systems(Update, read_move_input.in_set(GameSet::Input))
            .add_systems(
                Update,
                (move_player, apply_hazards_to_player)
                    .chain()
                    .in_set(GameSet::Move),
            )
            .add_systems(Update, player_regen.in_set(GameSet::Resolve))
            .add_systems(OnExit(AppState::Menu), spawn_player.in_set(RunSetup::Spawn));
    }
}

pub fn spawn_player(
    mut commands: Commands,
    art: Res<GameArt>,
    stats: Res<PlayerStats>,
    existing: Query<Entity, With<Player>>,
) {
    // A restart re-enters this state; clear the previous hero first.
    for e in &existing {
        commands.entity(e).despawn();
    }

    commands.spawn((
        Player,
        Intent::default(),
        Dash::default(),
        Facing::default(),
        StatusEffects::default(),
        Health::new(stats.max_hp),
        Body::new(Vec2::ZERO, PLAYER_RADIUS),
        Altitude::default(),
        VisualScale::new(1.0),
        Damageable {
            hostile_target: true,
        },
        Mesh3d(art.player.clone()),
        MeshMaterial3d(art.solid.clone()),
        Transform::from_translation(Vec3::ZERO),
        RunEntity,
    ));
}

/// WASD and the arrow keys both drive movement. In plan mode the arrows are
/// reassigned to the build cursor, so movement falls back to WASD only.
fn read_move_input(
    keys: Res<ButtonInput<KeyCode>>,
    plan: Res<crate::command::PlanMode>,
    mut q: Query<(&mut Intent, &mut Dash)>,
) {
    let mut dir = Vec2::ZERO;
    if keys.any_pressed([KeyCode::KeyW]) {
        dir.y -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyS]) {
        dir.y += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyA]) {
        dir.x -= 1.0;
    }
    if keys.any_pressed([KeyCode::KeyD]) {
        dir.x += 1.0;
    }
    if !plan.active {
        if keys.pressed(KeyCode::ArrowUp) {
            dir.y -= 1.0;
        }
        if keys.pressed(KeyCode::ArrowDown) {
            dir.y += 1.0;
        }
        if keys.pressed(KeyCode::ArrowLeft) {
            dir.x -= 1.0;
        }
        if keys.pressed(KeyCode::ArrowRight) {
            dir.x += 1.0;
        }
    }

    let dash = keys.just_pressed(KeyCode::ShiftLeft) || keys.just_pressed(KeyCode::ShiftRight);

    for (mut intent, mut dash_state) in &mut q {
        intent.move_dir = dir.normalize_or_zero();
        intent.dash = dash;
        if dash && dash_state.cooldown <= 0.0 && dash_state.active <= 0.0 {
            dash_state.dir = if intent.move_dir == Vec2::ZERO {
                Vec2::new(0.0, 1.0)
            } else {
                intent.move_dir
            };
            dash_state.active = 0.18;
        }
    }
}

fn move_player(
    time: Res<Time>,
    stats: Res<PlayerStats>,
    bounds: Res<ArenaBounds>,
    obstacles: Res<ObstacleField>,
    gust: Res<Gust>,
    camera_yaw: Res<crate::camera::CameraRig>,
    mut q: Query<
        (
            &Intent,
            &mut Body,
            &mut Dash,
            &mut Facing,
            &StatusEffects,
            &mut Altitude,
        ),
        With<Player>,
    >,
) {
    let dt = time.delta_secs();
    for (intent, mut body, mut dash, mut facing, status, mut alt) in &mut q {
        dash.cooldown = (dash.cooldown - dt).max(0.0);

        // Input is camera-relative, so "up" always means "away from the
        // viewer" no matter how the rig is rotated.
        let (sin, cos) = camera_yaw.yaw.sin_cos();
        let world_dir = Vec2::new(
            intent.move_dir.x * cos - intent.move_dir.y * sin,
            intent.move_dir.x * sin + intent.move_dir.y * cos,
        );

        let speed = stats.move_speed * status.speed_mult();

        if dash.active > 0.0 {
            dash.active -= dt;
            let (dsin, dcos) = camera_yaw.yaw.sin_cos();
            let ddir = Vec2::new(
                dash.dir.x * dcos - dash.dir.y * dsin,
                dash.dir.x * dsin + dash.dir.y * dcos,
            );
            body.vel = ddir * speed * 3.2;
            if dash.active <= 0.0 {
                dash.cooldown = stats.dash_cooldown;
            }
        } else {
            body.vel = world_dir * speed;
        }

        if gust.affects(body.pos) {
            body.vel += gust.dir * gust.strength * 0.4;
        }

        // Integrate, then depenetrate, then clamp. Order matters: clamping last
        // guarantees the player can never be pushed off the arena by a prop
        // sitting flush against the rim.
        // Read through the `Mut` before writing: a compound assignment to
        // `body.pos` would hold a mutable borrow across the reads of `vel`.
        let radius = body.radius;
        let step = (body.vel + body.impulse) * dt;
        body.pos += step;
        body.impulse = damp_vec2(body.impulse, Vec2::ZERO, 9.0, dt);
        body.pos = obstacles.resolve(body.pos, radius);
        body.pos = bounds.clamp(body.pos, radius);

        // Face the direction of travel; a little lean sells the momentum.
        if world_dir.length_squared() > 0.01 {
            facing.target = yaw_towards(world_dir);
        }
        facing.yaw = angle_lerp(facing.yaw, facing.target, 14.0 * dt);

        // Walk bob.
        let moving = body.vel.length_squared() > 1.0;
        let bob_target = if moving { 0.09 } else { 0.0 };
        alt.y = damp(alt.y, bob_target, 8.0, dt);
    }
}

/// Shortest-path angle interpolation, so turning past pi does not spin the
/// model the long way round.
pub fn angle_lerp(from: f32, to: f32, t: f32) -> f32 {
    let mut delta = (to - from) % std::f32::consts::TAU;
    if delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    } else if delta < -std::f32::consts::PI {
        delta += std::f32::consts::TAU;
    }
    from + delta * t.clamp(0.0, 1.0)
}

fn apply_hazards_to_player(
    time: Res<Time>,
    hazards: Query<(&Hazard, &Body), Without<Player>>,
    mut players: Query<(Entity, &Body, &mut StatusEffects), With<Player>>,
    mut damage: MessageWriter<DamageEvent>,
) {
    let dt = time.delta_secs();
    for (entity, body, mut status) in &mut players {
        for (hazard, hbody) in &hazards {
            if !hazard.hurts_player {
                continue;
            }
            let reach = hazard.radius + body.radius;
            if body.pos.distance_squared(hbody.pos) > reach * reach {
                continue;
            }
            if hazard.dps > 0.0 {
                damage.write(DamageEvent {
                    target: entity,
                    amount: hazard.dps * dt,
                    crit: false,
                    knockback: Vec2::ZERO,
                    knockback_force: 0.0,
                    source: DamageSource::Hazard,
                });
            }
            if hazard.slow < 1.0 {
                status.apply_slow(1.0 - hazard.slow, 0.25);
            }
        }
    }
}

fn player_regen(
    time: Res<Time>,
    stats: Res<PlayerStats>,
    spotlight: Res<Spotlight>,
    mut q: Query<(&mut Health, &Body), With<Player>>,
) {
    let dt = time.delta_secs();
    for (mut health, body) in &mut q {
        health.invuln = (health.invuln - dt).max(0.0);
        let mut rate = stats.regen;
        // Standing in the light heals faster: a reason to contest the spot the
        // director is already aiming elites at.
        if spotlight.contains(body.pos) {
            rate += 1.4;
        }
        if health.current > 0.0 {
            health.heal(rate * dt);
        }
    }
}
