//! Components and messages shared by more than one system module.

use bevy::prelude::*;

/// Authoritative position and velocity, in the XZ plane.
///
/// Everything that moves keeps its state here rather than in `Transform`, which
/// is derived from it during the presentation phase. That keeps the collision
/// code free of `Vec3`/`Vec2` conversions.
#[derive(Debug, Component, Clone, Copy, Default)]
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
#[derive(Debug, Component, Clone, Copy)]
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

#[derive(Debug, Component, Clone, Copy)]
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

/// Marks an entity for removal at the end of the frame.
///
/// Deferring despawns to one place avoids the classic "system A despawned what
/// system B is holding" crash when several damage sources land on one tick.
#[derive(Debug, Component)]
pub struct Doomed;

/// Fades out and despawns after `life` seconds.
#[derive(Debug, Component)]
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
#[derive(Debug, Component)]
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
#[derive(Debug, Component)]
pub struct Hover {
    pub base: f32,
    pub amplitude: f32,
    pub speed: f32,
    pub phase: f32,
}

/// Everything spawned as part of a run, so a restart can clear the world in one
/// query without touching the camera, lights or HUD.
#[derive(Debug, Component)]
pub struct RunEntity;

/// Scales the entity's visual size independently of `Transform`, so hit-flash
/// squash and spawn pop-in can compose with per-entity base scale.
#[derive(Debug, Component)]
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
#[derive(Debug, Message, Clone, Copy)]
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
#[derive(Debug, Message, Clone, Copy)]
pub struct DeathEvent {
    pub entity: Entity,
    pub pos: Vec2,
    pub by_player: bool,
}

/// Request a screen shake. Amplitude is in world units.
#[derive(Debug, Message, Clone, Copy)]
pub struct ShakeEvent {
    pub amount: f32,
}

/// A short-lived line of text that floats up from a world position.
#[derive(Debug, Message, Clone)]
pub struct FloatingTextEvent {
    pub pos: Vec2,
    pub height: f32,
    pub text: String,
    pub color: Color,
    pub size: f32,
}

/// A burst of particles at a point.
#[derive(Debug, Message, Clone, Copy)]
pub struct BurstEvent {
    pub pos: Vec2,
    pub height: f32,
    pub color: Color,
    pub count: u32,
    pub speed: f32,
    pub size: f32,
}

/// Play one of the synthesized sound effects.
#[derive(Debug, Message, Clone, Copy)]
pub struct SfxEvent {
    pub sound: crate::audio::Sfx,
    /// Multiplies the base volume for this one-shot.
    pub volume: f32,
}

impl SfxEvent {
    pub fn new(sound: crate::audio::Sfx) -> Self {
        Self { sound, volume: 1.0 }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_tracks_its_fraction() {
        let mut h = Health::new(80.0);
        assert_eq!(h.fraction(), 1.0);
        h.current = 20.0;
        assert!((h.fraction() - 0.25).abs() < 1e-6);
        h.current = -50.0;
        assert_eq!(h.fraction(), 0.0, "fraction must never go negative");
        h.current = 500.0;
        assert_eq!(h.fraction(), 1.0, "fraction must never exceed one");
    }

    #[test]
    fn zero_max_health_does_not_divide_by_zero() {
        let h = Health {
            current: 0.0,
            max: 0.0,
            invuln: 0.0,
        };
        assert_eq!(h.fraction(), 0.0);
    }

    #[test]
    fn heal_clamps_to_max() {
        let mut h = Health::new(100.0);
        h.current = 40.0;
        h.heal(25.0);
        assert!((h.current - 65.0).abs() < 1e-6);
        h.heal(1000.0);
        assert_eq!(h.current, 100.0);
    }

    #[test]
    fn death_is_at_or_below_zero() {
        let mut h = Health::new(10.0);
        assert!(!h.is_dead());
        h.current = 0.0;
        assert!(h.is_dead());
        h.current = -1.0;
        assert!(h.is_dead());
    }

    #[test]
    fn body_push_accumulates_normalised_impulse() {
        let mut b = Body::new(Vec2::ZERO, 0.5);
        b.push(Vec2::new(10.0, 0.0), 3.0);
        assert!((b.impulse - Vec2::new(3.0, 0.0)).length() < 1e-5);
        b.push(Vec2::new(0.0, -4.0), 2.0);
        assert!((b.impulse - Vec2::new(3.0, -2.0)).length() < 1e-5);
    }

    #[test]
    fn body_push_ignores_zero_direction() {
        let mut b = Body::new(Vec2::ZERO, 0.5);
        b.push(Vec2::ZERO, 5.0);
        assert_eq!(b.impulse, Vec2::ZERO);
    }

    #[test]
    fn ephemeral_progress_runs_zero_to_one() {
        let mut e = Ephemeral::new(2.0);
        assert!(e.t().abs() < 1e-6);
        e.life = 1.0;
        assert!((e.t() - 0.5).abs() < 1e-6);
        e.life = 0.0;
        assert!((e.t() - 1.0).abs() < 1e-6);
        e.life = -3.0;
        assert!((e.t() - 1.0).abs() < 1e-6, "overrun must saturate at one");
    }

    #[test]
    fn ephemeral_with_zero_life_is_safe() {
        let e = Ephemeral::new(0.0);
        assert_eq!(e.t(), 0.0);
    }

    #[test]
    fn damp_moves_towards_the_target_and_converges() {
        let mut v = 0.0;
        for _ in 0..600 {
            v = damp(v, 10.0, 5.0, 1.0 / 60.0);
        }
        assert!((v - 10.0).abs() < 0.01, "converged to {v}");
    }

    #[test]
    fn damp_never_overshoots() {
        // Even with an absurd dt the exponential form cannot pass the target.
        let v = damp(0.0, 1.0, 20.0, 10.0);
        assert!(v <= 1.0 + 1e-6, "overshot to {v}");
    }

    #[test]
    fn damp_with_zero_dt_is_a_no_op() {
        assert_eq!(damp(3.0, 99.0, 5.0, 0.0), 3.0);
    }

    #[test]
    fn damp_vec_variants_track_the_scalar_form() {
        let v = damp_vec2(Vec2::ZERO, Vec2::new(4.0, -2.0), 6.0, 0.1);
        let x = damp(0.0, 4.0, 6.0, 0.1);
        assert!((v.x - x).abs() < 1e-6);
        let v3 = damp_vec3(Vec3::ZERO, Vec3::splat(4.0), 6.0, 0.1);
        assert!((v3.z - x).abs() < 1e-6);
    }

    #[test]
    fn yaw_towards_faces_the_cardinal_directions() {
        use std::f32::consts::{FRAC_PI_2, PI};
        assert!(yaw_towards(Vec2::new(0.0, 1.0)).abs() < 1e-6);
        assert!((yaw_towards(Vec2::new(1.0, 0.0)) - FRAC_PI_2).abs() < 1e-6);
        assert!((yaw_towards(Vec2::new(0.0, -1.0)).abs() - PI).abs() < 1e-6);
    }

    #[test]
    fn yaw_towards_is_stable_for_a_zero_vector() {
        assert_eq!(yaw_towards(Vec2::ZERO), 0.0);
    }

    #[test]
    fn world_and_flat_round_trip() {
        let p = Vec2::new(3.0, -7.0);
        assert_eq!(flat(to_world(p, 2.5)), p);
        assert_eq!(to_world(p, 2.5).y, 2.5);
    }

    #[test]
    fn time_formats_as_minutes_and_seconds() {
        assert_eq!(format_time(0.0), "0:00");
        assert_eq!(format_time(9.9), "0:09");
        assert_eq!(format_time(60.0), "1:00");
        assert_eq!(format_time(671.0), "11:11");
        assert_eq!(format_time(-5.0), "0:00", "negative time must not wrap");
    }

    #[test]
    fn counts_format_compactly() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1000), "1.0k");
        assert_eq!(format_count(14_800), "14.8k");
        assert_eq!(format_count(2_500_000), "2.50M");
    }
}
