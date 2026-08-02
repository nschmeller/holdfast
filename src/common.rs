//! Components and messages shared by more than one system module.

use bevy::prelude::*;

/// Everything that moves on the desk keeps its authoritative position here, in
/// the XZ plane. `Transform` is derived from it during the presentation phase,
/// which keeps collision code free of `Vec3`/`Vec2` conversions.
#[derive(Component, Clone, Copy, Default)]
pub struct Body {
    pub pos: Vec2,
    pub vel: Vec2,
    /// Applied by knockback and gusts, decays exponentially.
    pub impulse: Vec2,
    pub radius: f32,
}

impl Body {
    pub fn new(pos: Vec2, radius: f32) -> Self {
        Self {
            pos,
            radius,
            ..default()
        }
    }

    pub fn push(&mut self, dir: Vec2, force: f32) {
        self.impulse += dir.normalize_or_zero() * force;
    }
}

/// Height above the desk, kept separate from `Body` because it is decorative
/// for most entities (bob, hover, tumble) and never affects collision.
#[derive(Component, Clone, Copy)]
pub struct Altitude {
    pub y: f32,
    pub vy: f32,
    pub gravity: f32,
}

impl Default for Altitude {
    fn default() -> Self {
        Self {
            y: 0.0,
            vy: 0.0,
            gravity: 0.0,
        }
    }
}

#[derive(Component, Clone, Copy)]
pub struct Health {
    pub current: f32,
    pub max: f32,
    /// Counts down after each hit; blocks further damage while positive.
    pub invuln: f32,
}

impl Health {
    pub fn new(max: f32) -> Self {
        Self {
            current: max,
            max,
            invuln: 0.0,
        }
    }

    pub fn fraction(&self) -> f32 {
        if self.max <= 0.0 {
            0.0
        } else {
            (self.current / self.max).clamp(0.0, 1.0)
        }
    }

    pub fn is_dead(&self) -> bool {
        self.current <= 0.0
    }

    pub fn heal(&mut self, amount: f32) {
        self.current = (self.current + amount).min(self.max);
    }
}

/// Marks an entity for removal at the end of the frame. Deferring despawns to
/// one place avoids the classic "system A despawned what system B is holding"
/// crash when several damage sources land on the same tick.
#[derive(Component)]
pub struct Doomed;

/// Fades out and despawns after `life` seconds.
#[derive(Component)]
pub struct Ephemeral {
    pub life: f32,
    pub max_life: f32,
}

impl Ephemeral {
    pub fn new(life: f32) -> Self {
        Self {
            life,
            max_life: life,
        }
    }

    pub fn t(&self) -> f32 {
        if self.max_life <= 0.0 {
            0.0
        } else {
            1.0 - (self.life / self.max_life).clamp(0.0, 1.0)
        }
    }
}

/// Continuous spin, used for pickups and orbiting weapons.
#[derive(Component)]
pub struct Spin {
    pub speed: f32,
    pub axis: Vec3,
}

impl Default for Spin {
    fn default() -> Self {
        Self {
            speed: 2.0,
            axis: Vec3::Y,
        }
    }
}

/// Sinusoidal hover, offset per entity so a crowd does not bob in lockstep.
#[derive(Component)]
pub struct Hover {
    pub base: f32,
    pub amplitude: f32,
    pub speed: f32,
    pub phase: f32,
}

/// Everything spawned as part of a run, so a restart can clear the world in one
/// query without touching the camera, lights or HUD.
#[derive(Component)]
pub struct RunEntity;

/// Scales the entity's visual size independently of `Transform`, so hit-flash
/// squash and spawn pop-in can compose with per-entity base scale.
#[derive(Component)]
pub struct VisualScale {
    pub base: f32,
    pub pulse: f32,
}

impl VisualScale {
    pub fn new(base: f32) -> Self {
        Self { base, pulse: 0.0 }
    }
}

// -- messages ---------------------------------------------------------------

/// A damage application. Routed through a message so that armour, crit, lamp
/// bonuses and on-hit effects all resolve in one place.
#[derive(Message, Clone, Copy)]
pub struct DamageEvent {
    pub target: Entity,
    pub amount: f32,
    pub crit: bool,
    /// Direction to push the target, already normalised.
    pub knockback: Vec2,
    pub knockback_force: f32,
    pub source: DamageSource,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DamageSource {
    Player,
    Enemy,
    Hazard,
}

/// An entity reached zero health.
#[derive(Message, Clone, Copy)]
pub struct DeathEvent {
    pub entity: Entity,
    pub pos: Vec2,
    pub by_player: bool,
}

/// Request a screen shake. Amplitude is in world units.
#[derive(Message, Clone, Copy)]
pub struct ShakeEvent {
    pub amount: f32,
}

/// A short-lived line of text that floats up from a world position.
#[derive(Message, Clone)]
pub struct FloatingTextEvent {
    pub pos: Vec2,
    pub height: f32,
    pub text: String,
    pub color: Color,
    pub size: f32,
}

/// A burst of particles at a point.
#[derive(Message, Clone, Copy)]
pub struct BurstEvent {
    pub pos: Vec2,
    pub height: f32,
    pub color: Color,
    pub count: u32,
    pub speed: f32,
    pub size: f32,
}

/// Play one of the synthesized sound effects.
#[derive(Message, Clone, Copy)]
pub struct SfxEvent {
    pub sound: crate::audio::Sfx,
    /// Multiplies the base volume for this one-shot.
    pub volume: f32,
}

impl SfxEvent {
    pub fn new(sound: crate::audio::Sfx) -> Self {
        Self {
            sound,
            volume: 1.0,
        }
    }

    pub fn at(sound: crate::audio::Sfx, volume: f32) -> Self {
        Self { sound, volume }
    }
}

// -- helpers ----------------------------------------------------------------

/// XZ plane vector to a world position at height `y`.
#[inline]
pub fn to_world(p: Vec2, y: f32) -> Vec3 {
    Vec3::new(p.x, y, p.y)
}

#[inline]
pub fn flat(v: Vec3) -> Vec2 {
    Vec2::new(v.x, v.z)
}

/// Frame-rate independent exponential approach.
#[inline]
pub fn damp(current: f32, target: f32, lambda: f32, dt: f32) -> f32 {
    current + (target - current) * (1.0 - (-lambda * dt).exp())
}

#[inline]
pub fn damp_vec2(current: Vec2, target: Vec2, lambda: f32, dt: f32) -> Vec2 {
    current + (target - current) * (1.0 - (-lambda * dt).exp())
}

#[inline]
pub fn damp_vec3(current: Vec3, target: Vec3, lambda: f32, dt: f32) -> Vec3 {
    current + (target - current) * (1.0 - (-lambda * dt).exp())
}

/// Yaw that makes `+Z` of a model point along `dir`.
#[inline]
pub fn yaw_towards(dir: Vec2) -> f32 {
    if dir.length_squared() < 1e-6 {
        0.0
    } else {
        dir.x.atan2(dir.y)
    }
}

/// Format a duration as `M:SS`.
pub fn format_time(seconds: f32) -> String {
    let total = seconds.max(0.0) as u32;
    format!("{}:{:02}", total / 60, total % 60)
}

/// Compact number formatting for the HUD (`1.2k`, `14.8k`).
pub fn format_count(n: u64) -> String {
    if n < 1000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{:.1}k", n as f32 / 1000.0)
    } else {
        format!("{:.2}M", n as f32 / 1_000_000.0)
    }
}
