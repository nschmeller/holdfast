//! Weapons. All of them fire themselves.
//!
//! Nothing in here reads the keyboard. Targeting is automatic and the player's
//! only weapon decisions are which to take, which to level, and where to stand
//! - which is exactly the split this game wants.

use bevy::prelude::*;

use crate::art::GameArt;
use crate::combat::{Damageable, EnemyGrid, ShotVisual, SpawnHazard, SpawnShot};
use crate::common::{Body, BurstEvent, DamageEvent, DamageSource, RunEntity, SfxEvent};
use crate::enemy::{Enemy, StatusEffects};
use crate::environments::EnvKind;
use crate::player::{Player, PlayerStats};
use crate::rng::Rng;
use crate::{AppState, GameSet, RunSetup};

pub const MAX_WEAPONS: usize = 6;
pub const MAX_LEVEL: u32 = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WeaponKind {
    PencilDart,
    RulerSweep,
    RubberBand,
    Stapler,
    Highlighter,
    TackMines,
    CoffeeNova,
    ClipOrbit,
    FanBlast,
    LaserPointer,
}

impl WeaponKind {
    pub const ALL: [Self; 10] = [
        Self::PencilDart,
        Self::RulerSweep,
        Self::RubberBand,
        Self::Stapler,
        Self::Highlighter,
        Self::TackMines,
        Self::CoffeeNova,
        Self::ClipOrbit,
        Self::FanBlast,
        Self::LaserPointer,
    ];

    /// Per-world names, indexed by `env as usize`.
    ///
    /// Ten archetypes across five worlds, so the loadout reads as native wherever the run is.
    const NAMES: [[&str; EnvKind::COUNT]; 10] = [
        // PencilDart
        [
            "Pencil Dart",
            "Thorn Dart",
            "Rivet Gun",
            "Pulse Lance",
            "Mote Bolt",
        ],
        // RulerSweep
        [
            "Ruler Sweep",
            "Branch Sweep",
            "Rebar Sweep",
            "Arc Blade",
            "Ward Sweep",
        ],
        // RubberBand
        [
            "Rubber Band",
            "Whip Vine",
            "Ricochet Bolt",
            "Bounce Round",
            "Echo Shard",
        ],
        // Stapler
        [
            "Stapler",
            "Burr Spray",
            "Scrap Shotgun",
            "Flechette Burst",
            "Splinter Hex",
        ],
        // Highlighter
        [
            "Highlighter",
            "Sap Lance",
            "Cutting Torch",
            "Rail Beam",
            "Sunbeam Rune",
        ],
        // TackMines
        [
            "Tack Mines",
            "Seed Pods",
            "Glass Traps",
            "Proximity Nodes",
            "Glyph Traps",
        ],
        // CoffeeNova
        [
            "Coffee Nova",
            "Spore Bloom",
            "Steam Burst",
            "Plasma Nova",
            "Mana Nova",
        ],
        // ClipOrbit
        [
            "Clip Orbit",
            "Hornet Ring",
            "Bolt Orbit",
            "Drone Ring",
            "Orbiting Sigils",
        ],
        // FanBlast
        [
            "Fan Blast",
            "Gale Gust",
            "Downdraft",
            "Repulsor Cone",
            "Banish Wind",
        ],
        // LaserPointer
        [
            "Laser Pointer",
            "Firefly Beam",
            "Targeting Laser",
            "Lock Beam",
            "Doom Gaze",
        ],
    ];

    /// What this is called in `env`.
    ///
    /// The mechanic never changes; only what the world calls it does. Firing a
    /// pencil at a forest is the sort of detail that quietly tells a player
    /// nobody was paying attention.
    pub fn name(self, env: EnvKind) -> &'static str {
        Self::NAMES[self as usize][env as usize]
    }

    /// What this weapon does, in one line.
    ///
    /// Describes the *mechanic* and never names a prop, because the name already
    /// carries the local flavour and these do not: "Paperclips orbit you" was
    /// shown for the Arcane Sanctum's Orbiting Sigils, and "Cone of wind" for
    /// the Grid's Vent Surge. A blurb that names an object is wrong in four
    /// worlds out of five, and fifty hand-written variants would be fifty
    /// chances to get it wrong again. Mechanics read correctly everywhere.
    pub fn blurb(self) -> &'static str {
        match self {
            Self::PencilDart => "Fires at the nearest threat. Reliable, always on.",
            Self::RulerSweep => "Wide arc around you. Clears anything that closes.",
            Self::RubberBand => "Ricochets off walls and props. Loves tight rooms.",
            Self::Stapler => "Short-range spread. Brutal up close, useless far away.",
            Self::Highlighter => "Pierces a whole line of enemies.",
            Self::TackMines => "Leaves a trail behind you. Rewards kiting.",
            Self::CoffeeNova => "Scalding ring centred on you. Big, slow, satisfying.",
            Self::ClipOrbit => "Circles you and shreds whatever touches it.",
            Self::FanBlast => "Wide cone. Low damage, huge knockback - use the edge.",
            Self::LaserPointer => "Snaps to the biggest threat on the field.",
        }
    }

    /// (cooldown, damage, range)
    fn base(self) -> (f32, f32, f32) {
        match self {
            Self::PencilDart => (0.85, 13.0, 20.0),
            Self::RulerSweep => (1.15, 17.0, 4.0),
            Self::RubberBand => (1.4, 11.0, 22.0),
            Self::Stapler => (1.6, 8.0, 9.0),
            Self::Highlighter => (0.95, 10.0, 24.0),
            Self::TackMines => (2.2, 22.0, 0.0),
            Self::CoffeeNova => (4.2, 30.0, 5.5),
            Self::ClipOrbit => (0.0, 9.0, 2.6),
            Self::FanBlast => (2.6, 9.0, 8.0),
            Self::LaserPointer => (3.0, 62.0, 26.0),
        }
    }

    pub fn damage_at(self, level: u32, stats: &PlayerStats) -> f32 {
        let (_, dmg, _) = self.base();
        dmg * (1.0 + (level.saturating_sub(1)) as f32 * 0.34) * stats.damage_mult
    }

    pub fn cooldown_at(self, level: u32, stats: &PlayerStats) -> f32 {
        let (cd, _, _) = self.base();
        cd / ((1.0 + (level.saturating_sub(1)) as f32 * 0.1) * stats.haste)
    }

    pub fn range_at(self, level: u32, stats: &PlayerStats) -> f32 {
        let (_, _, range) = self.base();
        range * (1.0 + (level.saturating_sub(1)) as f32 * 0.05) * stats.area
    }

    /// The level-8 payoff, described for the upgrade card.
    pub fn mastery(self) -> &'static str {
        match self {
            Self::PencilDart => "MASTERY: darts pierce everything in their path.",
            Self::RulerSweep => "MASTERY: the sweep becomes a full circle.",
            Self::RubberBand => "MASTERY: bounces are unlimited for 3 seconds.",
            Self::Stapler => "MASTERY: doubles pellet count and staggers.",
            Self::Highlighter => "MASTERY: the beam burns everything it crosses.",
            Self::TackMines => "MASTERY: mines detonate in a chain.",
            Self::CoffeeNova => "MASTERY: leaves a scalding pool behind.",
            Self::ClipOrbit => "MASTERY: clips orbit twice as fast and twice as far.",
            Self::FanBlast => "MASTERY: the cone becomes a sustained gale.",
            Self::LaserPointer => "MASTERY: fires a second beam at a second target.",
        }
    }
}

#[derive(Debug, Clone)]
pub struct WeaponSlot {
    pub kind: WeaponKind,
    pub level: u32,
    pub timer: f32,
}

#[derive(Debug, Resource, Default)]
pub struct Loadout {
    pub slots: Vec<WeaponSlot>,
}

impl Loadout {
    pub fn reset(&mut self) {
        self.slots.clear();
        self.add(WeaponKind::PencilDart);
    }

    pub fn add(&mut self, kind: WeaponKind) {
        if self.slots.iter().any(|s| s.kind == kind) {
            self.level_up(kind);
        } else if self.slots.len() < MAX_WEAPONS {
            self.slots.push(WeaponSlot {
                kind,
                level: 1,
                timer: 0.0,
            });
        }
    }

    pub fn level_up(&mut self, kind: WeaponKind) {
        if let Some(s) = self.slots.iter_mut().find(|s| s.kind == kind) {
            s.level = (s.level + 1).min(MAX_LEVEL);
        }
    }

    pub fn level_of(&self, kind: WeaponKind) -> Option<u32> {
        self.slots.iter().find(|s| s.kind == kind).map(|s| s.level)
    }

    pub fn has_room(&self) -> bool {
        self.slots.len() < MAX_WEAPONS
    }

    /// Kinds that could still be offered as an upgrade.
    pub fn offerable(&self) -> Vec<WeaponKind> {
        WeaponKind::ALL
            .iter()
            .copied()
            .filter(|k| match self.level_of(*k) {
                Some(l) => l < MAX_LEVEL,
                None => self.has_room(),
            })
            .collect()
    }
}

/// A paperclip circling the player.
#[derive(Debug, Component)]
pub struct Orbiter {
    pub index: u32,
    pub count: u32,
    pub radius: f32,
    pub speed: f32,
    pub damage: f32,
    /// Per-target cooldown so a clip does not tick 60 times a second.
    pub cooldown: f32,
}

#[derive(Debug)]
pub struct WeaponPlugin;

impl Plugin for WeaponPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Loadout>()
            .add_systems(
                Update,
                (fire_weapons, sync_orbiters, orbiter_damage)
                    .chain()
                    .in_set(GameSet::Combat),
            )
            .add_systems(
                OnExit(AppState::Menu),
                reset_loadout.in_set(RunSetup::Reset),
            );
    }
}

fn reset_loadout(mut loadout: ResMut<Loadout>) {
    loadout.reset();
}

#[allow(clippy::too_many_arguments)]
fn fire_weapons(
    time: Res<Time>,
    stats: Res<PlayerStats>,
    obstacles: Res<crate::arena::ObstacleField>,
    pools: Res<crate::world::LightPools>,
    grid: Res<EnemyGrid>,
    mut loadout: ResMut<Loadout>,
    mut rng: ResMut<Rng>,
    player: Query<(&Body, &crate::player::Facing), With<Player>>,
    mut shots: MessageWriter<SpawnShot>,
    mut hazards: MessageWriter<SpawnHazard>,
    mut damage: MessageWriter<DamageEvent>,
    mut bursts: MessageWriter<BurstEvent>,
    mut sfx: MessageWriter<SfxEvent>,
    mut statuses: Query<&mut StatusEffects, With<Enemy>>,
    mut seen: MessageWriter<crate::coverage::Seen>,
) {
    let dt = time.delta_secs();
    let Some((body, facing)) = player.iter().next() else {
        return;
    };
    let origin = body.pos;

    // Standing in a pool of light is a real, quantified incentive - and now
    // the pools are places you find rather than one fixed spot.
    let light_bonus = pools.bonus_at(origin);

    let facing_dir = Vec2::new(facing.yaw.sin(), facing.yaw.cos());

    for slot in &mut loadout.slots {
        seen.write(crate::coverage::Seen(format!("weapon:{:?}", slot.kind)));
        let kind = slot.kind;
        if kind == WeaponKind::ClipOrbit {
            continue; // Persistent; handled by the orbiter systems.
        }

        slot.timer -= dt;
        if slot.timer > 0.0 {
            continue;
        }
        slot.timer = kind.cooldown_at(slot.level, &stats);

        let level = slot.level;
        let mastered = level >= MAX_LEVEL;
        let range = kind.range_at(level, &stats);
        let crit = rng.chance(stats.crit_chance);
        let mut dmg = kind.damage_at(level, &stats) * light_bonus;
        if crit {
            dmg *= stats.crit_mult;
        }

        // Most weapons need a target; the area ones do not.
        let target = grid.nearest_visible(origin, range, &obstacles);
        let aim = target.map_or(facing_dir, |t| (t.pos - origin).normalize_or_zero());

        match kind {
            WeaponKind::PencilDart => {
                if target.is_none() {
                    slot.timer *= 0.3;
                    continue;
                }
                let count = 1 + level / 3 + stats.extra_projectiles;
                for i in 0..count {
                    // Fan the extras rather than stacking them.
                    let spread = (i as f32 - (count - 1) as f32 * 0.5) * 0.13;
                    let d = rotate(aim, spread);
                    let mut shot = SpawnShot::friendly(
                        origin,
                        d,
                        26.0 * stats.projectile_speed,
                        dmg,
                        ShotVisual::Dart,
                    );
                    shot.crit = crit;
                    shot.pierce = if mastered { 99 } else { i32::from(level >= 4) };
                    shot.knockback = stats.knockback;
                    shots.write(shot);
                }
                sfx.write(SfxEvent::at(crate::audio::Sfx::Dart, 0.5));
            }

            WeaponKind::RulerSweep => {
                // An instant arc rather than a projectile: it should feel like
                // a shove, and a shove has no travel time.
                let arc = if mastered {
                    std::f32::consts::PI
                } else {
                    (1.2 + level as f32 * 0.08).min(2.6)
                };
                let mut any = false;
                grid.for_each_near(origin, range, |e| {
                    let to = e.pos - origin;
                    if to.length_squared() < 1e-4 {
                        return;
                    }
                    let angle = aim.angle_to(to.normalize());
                    if angle.abs() > arc {
                        return;
                    }
                    any = true;
                    damage.write(DamageEvent {
                        target: e.entity,
                        amount: dmg,
                        crit,
                        knockback: to.normalize_or_zero(),
                        knockback_force: 14.0 * stats.knockback,
                        source: DamageSource::Player,
                    });
                });
                bursts.write(BurstEvent {
                    pos: origin + aim * range * 0.55,
                    height: 0.7,
                    color: crate::palette::KEYCAP,
                    count: 8,
                    speed: 6.0,
                    size: 0.7,
                });
                if any {
                    sfx.write(SfxEvent::at(crate::audio::Sfx::Sweep, 0.6));
                } else {
                    slot.timer *= 0.5;
                }
            }

            WeaponKind::RubberBand => {
                let count = 1 + level / 4;
                for i in 0..count {
                    let d = rotate(aim, (i as f32 - (count - 1) as f32 * 0.5) * 0.5);
                    let mut shot = SpawnShot::friendly(
                        origin,
                        d,
                        18.0 * stats.projectile_speed,
                        dmg,
                        ShotVisual::Band,
                    );
                    shot.bounces = if mastered {
                        99
                    } else {
                        2 + i32::try_from(level / 2).unwrap_or(i32::MAX)
                    };
                    shot.life = if mastered { 3.0 } else { 2.4 };
                    shot.pierce = 1;
                    shot.crit = crit;
                    shot.spin = true;
                    shots.write(shot);
                }
                sfx.write(SfxEvent::at(crate::audio::Sfx::Band, 0.5));
            }

            WeaponKind::Stapler => {
                let pellets = (4 + level) * if mastered { 2 } else { 1 };
                for i in 0..pellets {
                    let spread = (i as f32 / (pellets - 1).max(1) as f32 - 0.5) * 0.7;
                    let d = rotate(aim, spread + rng.range(-0.05, 0.05));
                    let mut shot = SpawnShot::friendly(origin, d, 22.0, dmg, ShotVisual::Staple);
                    // Short life is what makes this a close-range weapon.
                    shot.life = range / 22.0;
                    shot.crit = crit;
                    shot.knockback = 1.4 * stats.knockback;
                    shots.write(shot);
                }
                sfx.write(SfxEvent::at(crate::audio::Sfx::Stapler, 0.7));
            }

            WeaponKind::Highlighter => {
                if target.is_none() {
                    slot.timer *= 0.3;
                    continue;
                }
                let mut shot = SpawnShot::friendly(origin, aim, 34.0, dmg, ShotVisual::Beam);
                shot.pierce = 99;
                shot.crit = crit;
                shot.radius = 0.5 * stats.area;
                shot.scale = stats.area;
                shot.life = range / 34.0;
                shot.burn = if mastered { dmg * 0.3 } else { 0.0 };
                shots.write(shot);
                sfx.write(SfxEvent::at(crate::audio::Sfx::Beam, 0.5));
            }

            WeaponKind::TackMines => {
                hazards.write(SpawnHazard {
                    pos: origin,
                    radius: (1.6 + level as f32 * 0.1) * stats.area,
                    dps: dmg,
                    life: 6.0 * stats.duration,
                    kind: crate::arena::HazardKind::Scald,
                    hurts_player: false,
                    hurts_enemies: true,
                });
                sfx.write(SfxEvent::at(crate::audio::Sfx::Place, 0.4));
            }

            WeaponKind::CoffeeNova => {
                let r = range;
                grid.for_each_near(origin, r, |e| {
                    damage.write(DamageEvent {
                        target: e.entity,
                        amount: dmg,
                        crit,
                        knockback: (e.pos - origin).normalize_or_zero(),
                        knockback_force: 18.0 * stats.knockback,
                        source: DamageSource::Player,
                    });
                });
                if mastered {
                    hazards.write(SpawnHazard {
                        pos: origin,
                        radius: r * 0.7,
                        dps: dmg * 0.25,
                        life: 4.0 * stats.duration,
                        kind: crate::arena::HazardKind::Scald,
                        hurts_player: false,
                        hurts_enemies: true,
                    });
                }
                bursts.write(BurstEvent {
                    pos: origin,
                    height: 0.4,
                    color: crate::palette::COFFEE,
                    count: 26,
                    speed: r * 2.0,
                    size: 1.1,
                });
                sfx.write(SfxEvent::new(crate::audio::Sfx::Nova));
            }

            WeaponKind::FanBlast => {
                let mut any = false;
                grid.for_each_near(origin, range, |e| {
                    let to = e.pos - origin;
                    if to.length_squared() < 1e-4 || aim.angle_to(to.normalize()).abs() > 0.7 {
                        return;
                    }
                    any = true;
                    damage.write(DamageEvent {
                        target: e.entity,
                        amount: dmg,
                        crit,
                        knockback: to.normalize_or_zero(),
                        // The point of this weapon: shove things off the edge.
                        knockback_force: (34.0 + level as f32 * 4.0) * stats.knockback,
                        source: DamageSource::Player,
                    });
                    if let Ok(mut status) = statuses.get_mut(e.entity) {
                        status.apply_slow(0.4, 1.2 * stats.duration);
                    }
                });
                bursts.write(BurstEvent {
                    pos: origin + aim * 2.0,
                    height: 0.6,
                    color: crate::palette::SCREEN_GLOW,
                    count: 14,
                    speed: 12.0,
                    size: 0.6,
                });
                if any {
                    sfx.write(SfxEvent::at(crate::audio::Sfx::Fan, 0.6));
                }
            }

            WeaponKind::LaserPointer => {
                let shots_to_fire = if mastered { 2 } else { 1 };
                let mut hit_any = false;
                let mut excluded: Option<Entity> = None;
                for _ in 0..shots_to_fire {
                    let Some(t) = grid
                        .best_visible_target(origin, range, &obstacles)
                        .filter(|t| excluded.is_none_or(|ex| ex != t.entity))
                    else {
                        break;
                    };
                    excluded = Some(t.entity);
                    hit_any = true;
                    damage.write(DamageEvent {
                        target: t.entity,
                        amount: dmg,
                        crit,
                        knockback: (t.pos - origin).normalize_or_zero(),
                        knockback_force: 4.0,
                        source: DamageSource::Player,
                    });
                    bursts.write(BurstEvent {
                        pos: t.pos,
                        height: 0.8,
                        color: crate::palette::DANGER,
                        count: 10,
                        speed: 7.0,
                        size: 0.5,
                    });
                }
                if hit_any {
                    sfx.write(SfxEvent::at(crate::audio::Sfx::Laser, 0.7));
                } else {
                    slot.timer *= 0.3;
                }
            }

            WeaponKind::ClipOrbit => unreachable!("handled by the orbiter systems"),
        }
    }
}

fn rotate(v: Vec2, angle: f32) -> Vec2 {
    let (s, c) = angle.sin_cos();
    Vec2::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

/// Keeps the live orbiter entities matching the weapon's current level.
fn sync_orbiters(
    mut commands: Commands,
    art: Res<GameArt>,
    stats: Res<PlayerStats>,
    loadout: Res<Loadout>,
    existing: Query<Entity, With<Orbiter>>,
    player: Query<Entity, With<Player>>,
) {
    let Some(slot) = loadout
        .slots
        .iter()
        .find(|s| s.kind == WeaponKind::ClipOrbit)
    else {
        for e in &existing {
            commands.entity(e).despawn();
        }
        return;
    };

    let mastered = slot.level >= MAX_LEVEL;
    let want = 2 + slot.level;
    let have = existing.iter().count() as u32;
    if have == want {
        return;
    }

    for e in &existing {
        commands.entity(e).despawn();
    }
    let Ok(player_entity) = player.single() else {
        return;
    };

    let radius = (2.4 + slot.level as f32 * 0.12) * stats.area * if mastered { 2.0 } else { 1.0 };
    let speed = 2.2 * if mastered { 2.0 } else { 1.0 };
    let damage = WeaponKind::ClipOrbit.damage_at(slot.level, &stats);

    for i in 0..want {
        let child = commands
            .spawn((
                Orbiter {
                    index: i,
                    count: want,
                    radius,
                    speed,
                    damage,
                    cooldown: 0.0,
                },
                Mesh3d(art.clip_orbit.clone()),
                MeshMaterial3d(art.metal.clone()),
                Transform::default(),
                RunEntity,
            ))
            .id();
        commands.entity(player_entity).add_child(child);
    }
}

fn orbiter_damage(
    time: Res<Time>,
    grid: Res<EnemyGrid>,
    stats: Res<PlayerStats>,
    player: Query<&Body, With<Player>>,
    mut orbiters: Query<(&mut Orbiter, &mut Transform)>,
    mut damage: MessageWriter<DamageEvent>,
) {
    let dt = time.delta_secs();
    let Some(body) = player.iter().next() else {
        return;
    };
    let t = time.elapsed_secs();

    for (mut orb, mut transform) in &mut orbiters {
        let angle = t * orb.speed + orb.index as f32 / orb.count as f32 * std::f32::consts::TAU;
        let offset = Vec2::new(angle.cos(), angle.sin()) * orb.radius;
        // Child transforms are parent-relative, so no world position needed.
        transform.translation = Vec3::new(offset.x, 0.6, offset.y);
        transform.rotation = Quat::from_rotation_y(-angle);

        orb.cooldown = (orb.cooldown - dt).max(0.0);
        if orb.cooldown > 0.0 {
            continue;
        }

        let world = body.pos + offset;
        let mut struck = false;
        grid.for_each_near(world, 0.75, |e| {
            if struck {
                return;
            }
            struck = true;
            damage.write(DamageEvent {
                target: e.entity,
                amount: orb.damage,
                crit: false,
                knockback: (e.pos - world).normalize_or_zero(),
                knockback_force: 7.0 * stats.knockback,
                source: DamageSource::Player,
            });
        });
        if struck {
            orb.cooldown = 0.32;
        }
    }
}

/// Structures and allies both need "is this thing a valid friendly target".
pub fn friendly_damageable() -> Damageable {
    Damageable {
        hostile_target: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_blurb_names_a_prop_from_one_particular_world() {
        // "Paperclips orbit you" was shown for the Arcane Sanctum's Orbiting
        // Sigils. A blurb that names an object is wrong in four worlds of five.
        const LOCAL: [&str; 8] = [
            "paperclip",
            "pencil",
            "stapler",
            "coffee",
            "wind",
            "sigil",
            "thorn",
            "rune",
        ];
        for kind in WeaponKind::ALL {
            let blurb = kind.blurb().to_ascii_lowercase();
            for word in LOCAL {
                assert!(
                    !blurb.contains(word),
                    "{kind:?} blurb names {word:?}: {blurb:?}"
                );
            }
        }
    }

    #[test]
    fn a_fresh_loadout_has_exactly_the_starter_weapon() {
        let mut l = Loadout::default();
        l.reset();
        assert_eq!(l.slots.len(), 1);
        assert_eq!(l.level_of(WeaponKind::PencilDart), Some(1));
    }

    #[test]
    fn adding_a_held_weapon_levels_it_instead_of_duplicating() {
        let mut l = Loadout::default();
        l.reset();
        l.add(WeaponKind::PencilDart);
        assert_eq!(l.slots.len(), 1);
        assert_eq!(l.level_of(WeaponKind::PencilDart), Some(2));
    }

    #[test]
    fn the_loadout_respects_its_slot_cap() {
        let mut l = Loadout::default();
        l.reset();
        for kind in WeaponKind::ALL {
            l.add(kind);
        }
        assert_eq!(l.slots.len(), MAX_WEAPONS);
        assert!(!l.has_room());
    }

    #[test]
    fn weapon_levels_stop_at_the_cap() {
        let mut l = Loadout::default();
        l.reset();
        for _ in 0..50 {
            l.level_up(WeaponKind::PencilDart);
        }
        assert_eq!(l.level_of(WeaponKind::PencilDart), Some(MAX_LEVEL));
    }

    #[test]
    fn levelling_an_unheld_weapon_is_a_no_op() {
        let mut l = Loadout::default();
        l.reset();
        l.level_up(WeaponKind::Stapler);
        assert_eq!(l.level_of(WeaponKind::Stapler), None);
    }

    #[test]
    fn a_full_loadout_only_offers_what_it_already_holds() {
        let mut l = Loadout::default();
        l.reset();
        for kind in WeaponKind::ALL {
            l.add(kind);
        }
        for kind in l.offerable() {
            assert!(
                l.level_of(kind).is_some(),
                "{kind:?} offered with no slot free"
            );
        }
    }

    #[test]
    fn maxed_weapons_drop_out_of_the_offer_pool() {
        let mut l = Loadout::default();
        l.reset();
        for _ in 0..MAX_LEVEL {
            l.level_up(WeaponKind::PencilDart);
        }
        assert!(!l.offerable().contains(&WeaponKind::PencilDart));
    }

    #[test]
    fn an_empty_loadout_can_offer_everything() {
        let l = Loadout::default();
        assert_eq!(l.offerable().len(), WeaponKind::ALL.len());
    }

    #[test]
    fn every_weapon_is_described() {
        for kind in WeaponKind::ALL {
            assert!(!kind.name(EnvKind::Desk).is_empty());
            assert!(!kind.blurb().is_empty());
            assert!(kind.mastery().starts_with("MASTERY:"));
        }
    }

    #[test]
    fn weapon_names_are_unique() {
        let mut names: Vec<_> = WeaponKind::ALL
            .iter()
            .map(|k| k.name(EnvKind::Desk))
            .collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total);
    }

    #[test]
    fn levelling_a_weapon_strictly_improves_it() {
        let stats = PlayerStats::default();
        for kind in WeaponKind::ALL {
            let mut previous_damage = 0.0;
            for level in 1..=MAX_LEVEL {
                let d = kind.damage_at(level, &stats);
                assert!(d > previous_damage, "{kind:?} damage stalled at {level}");
                previous_damage = d;
            }
            // The orbit weapon has no cooldown by design.
            if kind != WeaponKind::ClipOrbit {
                assert!(
                    kind.cooldown_at(MAX_LEVEL, &stats) < kind.cooldown_at(1, &stats),
                    "{kind:?} never fires faster"
                );
            }
            assert!(kind.range_at(MAX_LEVEL, &stats) >= kind.range_at(1, &stats));
        }
    }

    #[test]
    fn player_stats_scale_weapon_output() {
        let buffed = PlayerStats {
            damage_mult: 2.0,
            haste: 2.0,
            area: 2.0,
            ..PlayerStats::default()
        };
        let base = PlayerStats::default();

        let kind = WeaponKind::PencilDart;
        assert!((kind.damage_at(1, &buffed) / kind.damage_at(1, &base) - 2.0).abs() < 1e-4);
        assert!((kind.cooldown_at(1, &base) / kind.cooldown_at(1, &buffed) - 2.0).abs() < 1e-4);
        assert!((kind.range_at(1, &buffed) / kind.range_at(1, &base) - 2.0).abs() < 1e-4);
    }

    #[test]
    fn cooldowns_stay_positive_under_extreme_haste() {
        let stats = PlayerStats {
            haste: 1000.0,
            ..PlayerStats::default()
        };
        for kind in WeaponKind::ALL {
            let cd = kind.cooldown_at(MAX_LEVEL, &stats);
            assert!(cd >= 0.0 && cd.is_finite(), "{kind:?} cooldown {cd}");
        }
    }

    #[test]
    fn short_range_weapons_stay_short_range() {
        let stats = PlayerStats::default();
        let stapler = WeaponKind::Stapler.range_at(MAX_LEVEL, &stats);
        let dart = WeaponKind::PencilDart.range_at(1, &stats);
        assert!(stapler < dart, "the stapler lost its identity");
    }

    #[test]
    fn rotate_preserves_length_and_turns_the_right_way() {
        let v = Vec2::new(1.0, 0.0);
        let r = rotate(v, std::f32::consts::FRAC_PI_2);
        assert!((r.length() - 1.0).abs() < 1e-5);
        assert!((r - Vec2::new(0.0, 1.0)).length() < 1e-5);
        assert!((rotate(v, 0.0) - v).length() < 1e-6);
    }
}
