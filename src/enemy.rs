//! Hostiles: what they are, how they behave, and the director that decides how
//! many of them exist.
//!
//! The same twelve archetypes appear in every environment, renamed and retinted
//! to fit. That is a deliberate design choice as much as a budget one: the
//! player learns one threat vocabulary and can read a crowd instantly no matter
//! where the run is set.

use bevy::prelude::*;

use crate::arena::{Gust, ObstacleField};
use crate::art::{GameArt, Glow};
use crate::common::{
    Altitude, Body, DamageEvent, DamageSource, DeathEvent, Doomed, Health, RunEntity, VisualScale,
    to_world,
};
use crate::environments::EnvKind;
use crate::palette as pal;
use crate::player::Player;
use crate::rng::Rng;
use crate::threat::{RunClock, Threat, enemy_power};
use crate::world::Chasms;
use crate::{AppState, GameSet, RunSetup};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EnemyKind {
    DustBunny,
    Ant,
    ClipCrawler,
    StapleSkitter,
    CrumbBlob,
    TackLobber,
    StainSlime,
    Moth,
    Gremlin,
    BossStapler,
    BossHolePunch,
    BossLamp,
}

impl EnemyKind {
    pub const ALL: [Self; 12] = [
        Self::DustBunny,
        Self::Ant,
        Self::ClipCrawler,
        Self::StapleSkitter,
        Self::CrumbBlob,
        Self::TackLobber,
        Self::StainSlime,
        Self::Moth,
        Self::Gremlin,
        Self::BossStapler,
        Self::BossHolePunch,
        Self::BossLamp,
    ];

    /// The nine that the spawn director draws from, in unlock order.
    pub const FODDER: [Self; 9] = [
        Self::DustBunny,
        Self::Ant,
        Self::ClipCrawler,
        Self::StapleSkitter,
        Self::Moth,
        Self::CrumbBlob,
        Self::TackLobber,
        Self::StainSlime,
        Self::Gremlin,
    ];

    pub const BOSSES: [Self; 3] = [Self::BossStapler, Self::BossHolePunch, Self::BossLamp];

    pub fn is_boss(self) -> bool {
        matches!(
            self,
            Self::BossStapler | Self::BossHolePunch | Self::BossLamp
        )
    }

    pub fn stats(self) -> EnemyStats {
        // Speeds are deliberately below the player's 8.4: this is a game about
        // positioning and attrition, so a crowd should be something you can
        // always walk out of, and losing ground should be a decision rather
        // than a missed dodge.
        let (hp, speed, damage, radius, xp) = match self {
            Self::DustBunny => (16.0, 2.6, 5.0, 0.5, 3.0),
            Self::Ant => (10.0, 4.6, 4.0, 0.36, 3.0),
            Self::ClipCrawler => (26.0, 3.0, 7.0, 0.44, 5.0),
            Self::StapleSkitter => (18.0, 3.6, 6.0, 0.42, 5.0),
            Self::CrumbBlob => (58.0, 1.8, 10.0, 0.72, 9.0),
            Self::TackLobber => (22.0, 2.2, 6.0, 0.45, 8.0),
            Self::StainSlime => (40.0, 2.0, 6.0, 0.62, 8.0),
            Self::Moth => (20.0, 4.2, 6.0, 0.5, 7.0),
            Self::Gremlin => (34.0, 3.2, 8.0, 0.48, 10.0),
            Self::BossStapler => (900.0, 2.2, 18.0, 1.8, 220.0),
            Self::BossHolePunch => (1500.0, 1.9, 21.0, 1.9, 320.0),
            Self::BossLamp => (2100.0, 1.7, 19.0, 2.05, 440.0),
        };
        EnemyStats {
            hp,
            speed,
            damage,
            radius,
            xp,
        }
    }

    pub fn behavior(self) -> Behavior {
        match self {
            Self::DustBunny | Self::CrumbBlob => Behavior::Chase,
            Self::Ant => Behavior::Swarm,
            Self::ClipCrawler => Behavior::Zigzag,
            Self::StapleSkitter => Behavior::Dasher,
            Self::TackLobber => Behavior::Ranged,
            Self::StainSlime => Behavior::Trailer,
            Self::Moth => Behavior::Flyer,
            Self::Gremlin => Behavior::Blinker,
            Self::BossStapler => Behavior::BossCharger,
            Self::BossHolePunch => Behavior::BossSlammer,
            Self::BossLamp => Behavior::BossBeamer,
        }
    }

    /// Flyers ignore obstacles and hover.
    pub fn flies(self) -> bool {
        matches!(self, Self::Moth)
    }

    /// Earliest run-minute this kind can appear.
    pub fn unlock_minute(self) -> f32 {
        match self {
            Self::DustBunny => 0.0,
            Self::Ant => 0.4,
            Self::ClipCrawler => 1.2,
            Self::StapleSkitter => 2.0,
            Self::Moth => 3.0,
            Self::CrumbBlob => 4.0,
            Self::TackLobber => 5.0,
            Self::StainSlime => 6.5,
            Self::Gremlin => 8.0,
            _ => 999.0,
        }
    }

    /// Per-environment name. Same silhouette, same counterplay, local flavour -
    /// so what the player learns on the desk still applies in the Sanctum.
    ///
    /// Columns follow `EnvKind`: Desk, Forest, Rooftop, Grid, Arcane.
    pub fn name(self, env: EnvKind) -> &'static str {
        const NAMES: [[&str; EnvKind::COUNT]; 12] = [
            // Desk              Forest            Rooftop            Grid                Arcane
            [
                "Dust Bunny",
                "Spore Puff",
                "Litter Wad",
                "Nanite Cluster",
                "Mote Wisp",
            ],
            ["Sugar Ant", "Forage Ant", "Roach", "Skitterbot", "Familiar"],
            [
                "Clip Crawler",
                "Pincer Beetle",
                "Wire Crab",
                "Servo Crawler",
                "Rune Scuttler",
            ],
            [
                "Staple Skitter",
                "Thorn Tick",
                "Rebar Tick",
                "Shard Tick",
                "Shard Imp",
            ],
            [
                "Crumb Blob",
                "Moss Lump",
                "Tar Lump",
                "Slag Mass",
                "Golem Spawn",
            ],
            [
                "Tack Lobber",
                "Burr Slinger",
                "Gravel Slinger",
                "Flechette Drone",
                "Hex Caster",
            ],
            [
                "Coffee Stain",
                "Bog Seep",
                "Oil Seep",
                "Coolant Leak",
                "Void Seep",
            ],
            [
                "Lamp Moth",
                "Night Moth",
                "Grease Moth",
                "Hover Mite",
                "Pixie Swarm",
            ],
            [
                "USB Gremlin",
                "Hollow Sprite",
                "Meter Gremlin",
                "Phase Imp",
                "Blink Fiend",
            ],
            [
                "THE STAPLER",
                "THE SNAPJAW",
                "THE CRUSHER",
                "CLAMP UNIT-7",
                "THE MAW GATE",
            ],
            [
                "THE HOLE PUNCH",
                "THE STOMPER",
                "THE PILEDRIVER",
                "SIEGE FRAME",
                "THE STONE WARDEN",
            ],
            [
                "THE DESK LAMP",
                "THE WILL-O-WISP",
                "THE FLOODLIGHT",
                "BEACON PRIME",
                "THE EYE OF DAWN",
            ],
        ];
        NAMES[self as usize][env as usize]
    }

    /// Environment tint, multiplied over the base model colours.
    pub fn tint(self, env: EnvKind) -> Color {
        match env {
            EnvKind::Desk => Color::WHITE,
            EnvKind::Forest => Color::srgb(0.82, 1.0, 0.8),
            EnvKind::Rooftop => Color::srgb(0.86, 0.9, 1.0),
            EnvKind::Grid => Color::srgb(0.78, 0.95, 1.1),
            EnvKind::Arcane => Color::srgb(0.94, 0.82, 1.12),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EnemyStats {
    pub hp: f32,
    pub speed: f32,
    pub damage: f32,
    pub radius: f32,
    pub xp: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Behavior {
    Chase,
    Swarm,
    Zigzag,
    Dasher,
    Ranged,
    Trailer,
    Flyer,
    Blinker,
    BossCharger,
    BossSlammer,
    BossBeamer,
}

/// Elites are ordinary enemies with a modifier stapled on. Cheap to produce,
/// and they give the director a way to raise pressure without raising count.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rank {
    Normal,
    Elite,
    Boss,
}

#[derive(Debug, Component)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub rank: Rank,
    pub speed: f32,
    pub damage: f32,
    pub xp: f32,
    /// Contact damage cooldown, so touching an enemy is not a per-frame grind.
    pub touch_cd: f32,
    /// Generic behaviour timer, reused per archetype.
    pub timer: f32,
    /// Per-entity phase offset so crowds do not animate in lockstep.
    pub phase: f32,
    pub ai_state: f32,
    /// Set when the enemy has been launched past the arena edge.
    pub falling: bool,
}

/// Slow / stun effects applied by hazards and weapons.
#[derive(Debug, Component, Default)]
pub struct StatusEffects {
    pub slow: f32,
    pub slow_time: f32,
    pub stun_time: f32,
    pub burn_dps: f32,
    pub burn_time: f32,
}

impl StatusEffects {
    pub fn speed_mult(&self) -> f32 {
        if self.stun_time > 0.0 {
            0.0
        } else if self.slow_time > 0.0 {
            (1.0 - self.slow).max(0.15)
        } else {
            1.0
        }
    }

    pub fn apply_slow(&mut self, amount: f32, duration: f32) {
        self.slow = self.slow.max(amount);
        self.slow_time = self.slow_time.max(duration);
    }

    pub fn apply_stun(&mut self, duration: f32) {
        self.stun_time = self.stun_time.max(duration);
    }

    pub fn apply_burn(&mut self, dps: f32, duration: f32) {
        self.burn_dps = self.burn_dps.max(dps);
        self.burn_time = self.burn_time.max(duration);
    }
}

/// Marks the health bar that floats above elites and bosses.
#[derive(Debug, Component)]
pub struct BossBarTarget;

// -- the director -----------------------------------------------------------

/// Decides what spawns and when. Reads `Threat` for pressure, `RunClock` for
/// the unlock schedule, and keeps its own budget so bursts feel authored rather
/// than uniformly random.
#[derive(Debug, Resource)]
pub struct Director {
    pub spawn_accum: f32,
    pub elite_timer: f32,
    pub boss_timer: f32,
    pub boss_index: usize,
    pub boss_cycle: u32,
    /// Live count, maintained incrementally to avoid a full query every frame.
    pub alive: u32,
    pub cap: u32,
    pub announce: Option<(String, f32)>,
}

impl Default for Director {
    fn default() -> Self {
        Self {
            spawn_accum: 0.0,
            elite_timer: 32.0,
            boss_timer: 115.0,
            boss_index: 0,
            boss_cycle: 0,
            alive: 0,
            cap: 320,
            announce: None,
        }
    }
}

#[derive(Debug)]
pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Director>()
            .add_systems(
                Update,
                (direct_spawns, enemy_think, enemy_status_tick)
                    .chain()
                    .in_set(GameSet::Think),
            )
            .add_systems(Update, enemy_contact.in_set(GameSet::Combat))
            .add_systems(Update, enemy_fall_off.in_set(GameSet::Resolve))
            .add_systems(
                OnExit(AppState::Menu),
                reset_director.in_set(RunSetup::Reset),
            );
    }
}

fn reset_director(mut director: ResMut<Director>) {
    *director = Director::default();
}

fn direct_spawns(
    mut commands: Commands,
    time: Res<Time>,
    mut director: ResMut<Director>,
    threat: Res<Threat>,
    clock: Res<RunClock>,
    obstacles: Res<ObstacleField>,
    env: Res<EnvKind>,
    art: Res<GameArt>,
    mut rng: ResMut<Rng>,
    player: Query<&Body, With<Player>>,
) {
    let dt = time.delta_secs();
    let minutes = clock.elapsed / 60.0;
    let power = enemy_power(&threat, &clock);

    // -- boss ---------------------------------------------------------------
    director.boss_timer -= dt;
    if director.boss_timer <= 0.0 {
        let kind = EnemyKind::BOSSES[director.boss_index % EnemyKind::BOSSES.len()];
        director.boss_index += 1;
        if director.boss_index.is_multiple_of(EnemyKind::BOSSES.len()) {
            director.boss_cycle += 1;
        }
        // Each full rotation makes bosses meaningfully harder, which is what
        // keeps an endless run from flattening out.
        let cycle_scale = 1.0 + director.boss_cycle as f32 * 0.85;
        let anchor = player.iter().next().map_or(Vec2::ZERO, |b| b.pos);
        let pos = spawn_point(anchor, &obstacles, &mut rng, 2.2);
        spawn_enemy(
            &mut commands,
            &art,
            *env,
            kind,
            Rank::Boss,
            pos,
            power * cycle_scale,
            &mut rng,
        );
        director.alive += 1;
        director.boss_timer = 115.0;

        let title = if director.boss_cycle > 0 {
            format!("{} +{}", kind.name(*env), director.boss_cycle)
        } else {
            kind.name(*env).to_string()
        };
        director.announce = Some((title, 3.5));
    }

    // -- elites -------------------------------------------------------------
    director.elite_timer -= dt * threat.spawn_mult().min(3.0);
    if director.elite_timer <= 0.0 {
        director.elite_timer = (34.0 - minutes * 1.4).max(9.0);
        let count = 1 + (threat.effective() / 3.0) as usize;
        for _ in 0..count {
            let Some(kind) = pick_kind(&mut rng, minutes) else {
                break;
            };
            // Elites prefer to arrive near the player rather than trickling in
            // from the rim, so they read as an event.
            let anchor = player.iter().next().map_or(Vec2::ZERO, |b| b.pos);
            let pos = near_point(&obstacles, &mut rng, anchor, 9.0, 15.0, 1.0);
            spawn_enemy(
                &mut commands,
                &art,
                *env,
                kind,
                Rank::Elite,
                pos,
                power,
                &mut rng,
            );
            director.alive += 1;
        }
    }

    // -- trickle ------------------------------------------------------------
    if director.alive < director.cap {
        // Base rate ramps with time, then the dial multiplies it.
        let base_rate = 1.5 + minutes * 0.75;
        director.spawn_accum += dt * base_rate * threat.spawn_mult();

        let mut budget = 0;
        while director.spawn_accum >= 1.0 && budget < 24 && director.alive < director.cap {
            director.spawn_accum -= 1.0;
            budget += 1;
            let Some(kind) = pick_kind(&mut rng, minutes) else {
                continue;
            };
            let anchor = player.iter().next().map_or(Vec2::ZERO, |b| b.pos);
            let pos = spawn_point(anchor, &obstacles, &mut rng, kind.stats().radius);
            spawn_enemy(
                &mut commands,
                &art,
                *env,
                kind,
                Rank::Normal,
                pos,
                power,
                &mut rng,
            );
            director.alive += 1;
        }
    }

    if let Some((_, t)) = &mut director.announce {
        *t -= dt;
        if *t <= 0.0 {
            director.announce = None;
        }
    }
}

/// Weighted pick from the kinds unlocked so far, biased towards newer ones so
/// the composition of a crowd keeps shifting.
fn pick_kind(rng: &mut Rng, minutes: f32) -> Option<EnemyKind> {
    let unlocked: Vec<EnemyKind> = EnemyKind::FODDER
        .iter()
        .copied()
        .filter(|k| minutes >= k.unlock_minute())
        .collect();
    if unlocked.is_empty() {
        return Some(EnemyKind::DustBunny);
    }
    // Later unlocks get a heavier weight, capped so early kinds never vanish.
    let weights: Vec<f32> = unlocked
        .iter()
        .map(|k| 1.0 + (minutes - k.unlock_minute()).clamp(0.0, 6.0) * 0.18)
        .collect();
    let total: f32 = weights.iter().sum();
    let mut roll = rng.range(0.0, total);
    for (i, w) in weights.iter().enumerate() {
        roll -= w;
        if roll <= 0.0 {
            return Some(unlocked[i]);
        }
    }
    unlocked.last().copied()
}

/// A clear point on the arena rim.
/// Distance from the player that new arrivals appear at.
///
/// Just beyond what the overlook camera frames, so monsters walk into view
/// rather than blinking into existence in front of the player. The world has
/// no perimeter to spawn on any more, so the player *is* the perimeter.
const SPAWN_RING: f32 = 34.0;

/// How far ahead an enemy checks for something to walk around.
const AVOID_LOOKAHEAD: f32 = 1.4;

/// How hard it commits to going around. Enough to clear a corner, not so much
/// that a chase turns into a circle.
const AVOID_STRENGTH: f32 = 0.9;

fn spawn_point(anchor: Vec2, obstacles: &ObstacleField, rng: &mut Rng, radius: f32) -> Vec2 {
    for _ in 0..12 {
        let a = rng.range(0.0, std::f32::consts::TAU);
        let r = rng.range(SPAWN_RING, SPAWN_RING * 1.25);
        let p = anchor + Vec2::new(a.cos() * r, a.sin() * r);
        if !obstacles.overlaps(p, radius) {
            return p;
        }
    }
    let a = rng.range(0.0, std::f32::consts::TAU);
    anchor + Vec2::new(a.cos(), a.sin()) * SPAWN_RING
}

/// A clear point in an annulus around `anchor`.
fn near_point(
    obstacles: &ObstacleField,
    rng: &mut Rng,
    anchor: Vec2,
    min_r: f32,
    max_r: f32,
    radius: f32,
) -> Vec2 {
    for _ in 0..16 {
        let a = rng.range(0.0, std::f32::consts::TAU);
        let r = rng.range(min_r, max_r);
        let p = anchor + Vec2::new(a.cos() * r, a.sin() * r);
        if !obstacles.overlaps(p, radius) {
            return p;
        }
    }
    spawn_point(anchor, obstacles, rng, radius)
}

pub fn spawn_enemy(
    commands: &mut Commands,
    art: &GameArt,
    env: EnvKind,
    kind: EnemyKind,
    rank: Rank,
    pos: Vec2,
    power: f32,
    rng: &mut Rng,
) -> Entity {
    let s = kind.stats();

    let (hp_mult, dmg_mult, scale, xp_mult) = match rank {
        Rank::Elite => (4.2, 1.5, 1.42, 5.0),
        // Bosses already carry their multipliers in their base stats.
        Rank::Normal | Rank::Boss => (1.0, 1.0, 1.0, 1.0),
    };

    let hp = s.hp * power * hp_mult;
    let radius = s.radius * scale;

    let mut entity = commands.spawn((
        Enemy {
            kind,
            rank,
            speed: s.speed * rng.range(0.92, 1.08),
            damage: s.damage * power.sqrt() * dmg_mult,
            xp: s.xp * xp_mult,
            touch_cd: 0.0,
            timer: rng.range(0.0, 3.0),
            phase: rng.range(0.0, std::f32::consts::TAU),
            ai_state: 0.0,
            falling: false,
        },
        StatusEffects::default(),
        // Without this the shared movement pass skips them and `enemy_think`
        // writes a velocity nothing ever integrates.
        crate::combat::Actor {
            collides: !kind.flies(),
            // Deliberately careless: knockback should be able to shove them
            // into a chasm, which `enemy_fall_off` then finishes.
            avoids_chasms: false,
        },
        Health::new(hp),
        Body::new(pos, radius),
        Altitude {
            y: if kind.flies() { 1.6 } else { 0.0 },
            ..default()
        },
        VisualScale::new(scale),
        Mesh3d(art.enemy_mesh(kind)),
        MeshMaterial3d(art.solid.clone()),
        Transform::from_translation(to_world(pos, 0.0)).with_scale(Vec3::splat(scale)),
        RunEntity,
    ));

    // Rank trim: a glowing ring that makes elites and bosses pop out of a
    // crowd without needing a separate model.
    match rank {
        Rank::Elite => {
            entity.insert(RankAura {
                color: pal::ELITE_TRIM,
            });
        }
        Rank::Boss => {
            entity.insert((
                RankAura {
                    color: pal::BOSS_TRIM,
                },
                BossBarTarget,
            ));
        }
        Rank::Normal => {}
    }

    let id = entity.id();

    // Tint marker consumed by the presentation pass.
    commands.entity(id).insert(EnvTint(kind.tint(env)));

    // A glowing halo child for ranked enemies.
    if rank != Rank::Normal {
        let glow = if rank == Rank::Boss {
            Glow::Boss
        } else {
            Glow::Elite
        };
        let halo = commands
            .spawn((
                Mesh3d(art.ring.clone()),
                MeshMaterial3d(art.glow(glow)),
                Transform::from_xyz(0.0, 0.06, 0.0).with_scale(Vec3::new(
                    radius * 1.5,
                    1.0,
                    radius * 1.5,
                )),
                RunEntity,
            ))
            .id();
        commands.entity(id).add_child(halo);
    }

    id
}

#[derive(Debug, Component)]
pub struct RankAura {
    pub color: Color,
}

#[derive(Debug, Component)]
pub struct EnvTint(pub Color);

// -- behaviour --------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn enemy_think(
    time: Res<Time>,
    gust: Res<Gust>,
    obstacles: Res<ObstacleField>,
    mut rng: ResMut<Rng>,
    player: Query<(&Body, Entity), (With<Player>, Without<Enemy>)>,
    mut enemies: Query<(&mut Enemy, &mut Body, &mut Altitude, &StatusEffects)>,
    mut shots: MessageWriter<crate::combat::SpawnShot>,
    mut hazards: MessageWriter<crate::combat::SpawnHazard>,
) {
    let dt = time.delta_secs();
    let Some((player_body, _)) = player.iter().next() else {
        return;
    };
    let target = player_body.pos;

    for (mut enemy, mut body, mut alt, status) in &mut enemies {
        if enemy.falling {
            continue;
        }
        enemy.timer += dt;
        enemy.touch_cd = (enemy.touch_cd - dt).max(0.0);

        let to_player = target - body.pos;
        let dist = to_player.length();
        let dir = if dist > 1e-4 {
            to_player / dist
        } else {
            Vec2::X
        };

        let speed = enemy.speed * status.speed_mult();
        let mut desired = dir * speed;

        // Walk around cover rather than into it.
        //
        // Chasing in a straight line and letting depenetration sort it out
        // looks exactly like what it is: a queue of monsters grinding against
        // the side of a book. Probing ahead and sliding along whatever is in
        // the way costs one obstacle query and turns the same crowd into
        // something that flows around the furniture.
        if !enemy.kind.flies() {
            let reach = body.radius + AVOID_LOOKAHEAD;
            let probe = body.pos + dir * reach;
            let push = obstacles.resolve(probe, body.radius) - probe;
            if push.length_squared() > 1e-6 {
                let normal = push.normalize();
                // Drop the component heading into the surface...
                let into = desired.dot(normal).min(0.0);
                desired -= normal * into;
                // ...and commit to one way around. The side comes from the
                // enemy's own phase, so a crowd splits and flows rather than
                // every member dithering at the same corner.
                let side = if enemy.phase < std::f32::consts::PI {
                    1.0
                } else {
                    -1.0
                };
                let tangent = Vec2::new(-normal.y, normal.x) * side;
                desired += tangent * speed * AVOID_STRENGTH;
            }
        }

        match enemy.kind.behavior() {
            Behavior::Chase => {}
            Behavior::Swarm => {
                // Slight orbital bias makes a mass of ants flow around
                // obstacles instead of piling into them head-on.
                let tangent = Vec2::new(-dir.y, dir.x);
                desired += tangent * (enemy.phase.sin() * speed * 0.35);
            }
            Behavior::Zigzag => {
                let tangent = Vec2::new(-dir.y, dir.x);
                desired += tangent * ((enemy.timer * 3.4 + enemy.phase).sin() * speed * 0.75);
            }
            Behavior::Dasher => {
                // Wind up, then burst. `ai_state` holds the dash timer.
                // The 0.9s freeze is a long, readable telegraph on purpose:
                // reacting to it should be a walking decision, not a reflex.
                if enemy.ai_state > 0.0 {
                    enemy.ai_state -= dt;
                    desired = dir * speed * 2.4;
                } else if enemy.timer > 3.0 {
                    enemy.timer = 0.0;
                    enemy.ai_state = 0.5;
                } else if enemy.timer > 2.1 {
                    desired = Vec2::ZERO;
                }
            }
            Behavior::Ranged => {
                // Hold at range and lob.
                let preferred = 11.0;
                if dist < preferred - 1.5 {
                    desired = -dir * speed;
                } else if dist < preferred + 1.5 {
                    desired = Vec2::new(-dir.y, dir.x) * speed * 0.6;
                }
                if enemy.timer > 2.6 && dist < 20.0 {
                    enemy.timer = 0.0;
                    shots.write(crate::combat::SpawnShot::enemy(
                        body.pos,
                        dir,
                        13.0,
                        enemy.damage * 0.8,
                        crate::combat::ShotVisual::Tack,
                    ));
                }
            }
            Behavior::Trailer => {
                if enemy.timer > 1.1 {
                    enemy.timer = 0.0;
                    hazards.write(crate::combat::SpawnHazard {
                        pos: body.pos,
                        radius: 1.5,
                        dps: enemy.damage * 0.45,
                        life: 5.0,
                        kind: crate::arena::HazardKind::Sticky,
                        hurts_player: true,
                        hurts_enemies: false,
                    });
                }
            }
            Behavior::Flyer => {
                // Erratic drift plus a bob; ignores obstacles entirely.
                let wobble = Vec2::new(
                    (enemy.timer * 2.1 + enemy.phase).sin(),
                    (enemy.timer * 1.7 + enemy.phase * 1.3).cos(),
                );
                desired += wobble * speed * 0.8;
                alt.y = 1.4 + (enemy.timer * 4.0 + enemy.phase).sin() * 0.45;
            }
            Behavior::Blinker => {
                if enemy.timer > 3.0 && dist > 3.0 {
                    enemy.timer = 0.0;
                    // Short teleport toward the player.
                    let hop = dir * dist.min(7.0);
                    body.pos += hop;
                }
            }
            Behavior::BossCharger => {
                if enemy.ai_state > 0.0 {
                    enemy.ai_state -= dt;
                    desired = dir * speed * 2.2;
                } else if enemy.timer > 4.6 {
                    enemy.timer = 0.0;
                    if rng.chance(0.5) {
                        enemy.ai_state = 1.1;
                    } else {
                        // Staple volley in a fan.
                        for i in -3..=3 {
                            let a = i as f32 * 0.16;
                            let (s, c) = a.sin_cos();
                            let d = Vec2::new(dir.x * c - dir.y * s, dir.x * s + dir.y * c);
                            shots.write(crate::combat::SpawnShot::enemy(
                                body.pos,
                                d,
                                16.0,
                                enemy.damage * 0.55,
                                crate::combat::ShotVisual::Staple,
                            ));
                        }
                    }
                }
            }
            Behavior::BossSlammer => {
                if enemy.timer > 3.4 {
                    enemy.timer = 0.0;
                    // Ring of shockwave hazards.
                    for i in 0..10 {
                        let a = i as f32 / 10.0 * std::f32::consts::TAU;
                        hazards.write(crate::combat::SpawnHazard {
                            pos: body.pos + Vec2::new(a.cos(), a.sin()) * 4.2,
                            radius: 2.2,
                            dps: enemy.damage * 1.6,
                            life: 1.1,
                            kind: crate::arena::HazardKind::Scald,
                            hurts_player: true,
                            hurts_enemies: false,
                        });
                    }
                }
            }
            Behavior::BossBeamer => {
                // Sweeps a rotating beam of shots.
                if enemy.timer > 0.12 {
                    enemy.timer = 0.0;
                    enemy.ai_state += 0.22;
                    let a = enemy.ai_state;
                    for k in 0..3 {
                        let ang = a + k as f32 * std::f32::consts::TAU / 3.0;
                        shots.write(crate::combat::SpawnShot::enemy(
                            body.pos,
                            Vec2::new(ang.cos(), ang.sin()),
                            11.0,
                            enemy.damage * 0.3,
                            crate::combat::ShotVisual::Plasma,
                        ));
                    }
                }
            }
        }

        // Environmental push.
        if gust.affects(body.pos) {
            desired += gust.dir * gust.strength * 0.55;
        }

        body.vel = desired;

        if enemy.kind.flies() {
            // Flyers stay airborne; everything else sits on the ground.
        } else {
            alt.y = 0.0;
        }
    }
}

fn enemy_status_tick(
    time: Res<Time>,
    mut damage: MessageWriter<DamageEvent>,
    mut q: Query<(Entity, &mut StatusEffects)>,
) {
    let dt = time.delta_secs();
    for (entity, mut status) in &mut q {
        status.slow_time = (status.slow_time - dt).max(0.0);
        status.stun_time = (status.stun_time - dt).max(0.0);
        if status.slow_time <= 0.0 {
            status.slow = 0.0;
        }
        if status.burn_time > 0.0 {
            status.burn_time = (status.burn_time - dt).max(0.0);
            damage.write(DamageEvent {
                target: entity,
                amount: status.burn_dps * dt,
                crit: false,
                knockback: Vec2::ZERO,
                knockback_force: 0.0,
                source: DamageSource::Hazard,
            });
            if status.burn_time <= 0.0 {
                status.burn_dps = 0.0;
            }
        }
    }
}

/// Contact damage against the player, allies and structures.
fn enemy_contact(
    time: Res<Time>,
    mut enemies: Query<(&mut Enemy, &Body), Without<Player>>,
    targets: Query<(Entity, &Body, &crate::combat::Damageable)>,
    mut damage: MessageWriter<DamageEvent>,
) {
    let dt = time.delta_secs();
    for (mut enemy, ebody) in &mut enemies {
        if enemy.falling || enemy.touch_cd > 0.0 {
            enemy.touch_cd = (enemy.touch_cd - dt).max(0.0);
            continue;
        }
        for (entity, tbody, dmg) in &targets {
            if !dmg.hostile_target {
                continue;
            }
            let reach = ebody.radius + tbody.radius;
            if ebody.pos.distance_squared(tbody.pos) <= reach * reach {
                let dir = (tbody.pos - ebody.pos).normalize_or_zero();
                damage.write(DamageEvent {
                    target: entity,
                    amount: enemy.damage,
                    crit: false,
                    knockback: dir,
                    knockback_force: 6.0,
                    source: DamageSource::Enemy,
                });
                enemy.touch_cd = 0.55;
                break;
            }
        }
    }
}

/// Enemies launched past the arena edge tumble away and die, which makes
/// knockback builds genuinely powerful near the rim.
/// Knockback stays lethal in a world with no edge: chasms are the edge now.
fn enemy_fall_off(
    time: Res<Time>,
    chasms: Res<Chasms>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Enemy, &Body, &mut Altitude, &Health)>,
    mut deaths: MessageWriter<DeathEvent>,
) {
    let dt = time.delta_secs();
    for (entity, mut enemy, body, mut alt, health) in &mut q {
        if enemy.falling {
            alt.vy -= 26.0 * dt;
            alt.y += alt.vy * dt;
            if alt.y < -12.0 {
                // Still counts as a kill: the XP is the reward for the setup.
                deaths.write(DeathEvent {
                    entity,
                    pos: body.pos,
                    by_player: true,
                });
                commands.entity(entity).try_insert(Doomed);
            }
            continue;
        }
        if health.is_dead() {
            continue;
        }
        // Only once well inside a hole, so brushing the lip is survivable and
        // shoving something over it is unambiguous.
        if chasms.contains(body.pos) {
            enemy.falling = true;
            alt.vy = 2.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_archetype_has_usable_stats() {
        for kind in EnemyKind::ALL {
            let s = kind.stats();
            assert!(s.hp > 0.0, "{kind:?} has no health");
            assert!(s.speed > 0.0, "{kind:?} cannot move");
            assert!(s.damage > 0.0, "{kind:?} is harmless");
            assert!(s.radius > 0.0, "{kind:?} has no body");
            assert!(s.xp > 0.0, "{kind:?} is worth nothing");
        }
    }

    #[test]
    fn nothing_outruns_the_player() {
        // A design guarantee: the player must always be able to walk out of a
        // crowd, so no ordinary enemy may exceed their base speed.
        for kind in EnemyKind::FODDER {
            assert!(
                kind.stats().speed < crate::player::BASE_SPEED,
                "{kind:?} at {} outruns the player at {}",
                kind.stats().speed,
                crate::player::BASE_SPEED
            );
        }
    }

    #[test]
    fn dashers_and_chargers_stay_catchable_even_mid_burst() {
        // The dash multiplier is 2.4x and the boss charge is 2.2x; both must
        // stay survivable, which in practice means under about triple.
        let dasher = EnemyKind::StapleSkitter.stats().speed * 2.4;
        assert!(
            dasher < crate::player::BASE_SPEED * 1.2,
            "dash hits {dasher}"
        );
        let charger = EnemyKind::BossStapler.stats().speed * 2.2;
        assert!(charger < crate::player::BASE_SPEED, "charge hits {charger}");
    }

    #[test]
    fn bosses_are_categorically_tougher() {
        let worst_fodder = EnemyKind::FODDER
            .iter()
            .map(|k| k.stats().hp)
            .fold(0.0f32, f32::max);
        for boss in EnemyKind::BOSSES {
            assert!(boss.stats().hp > worst_fodder * 10.0, "{boss:?} is soft");
            assert!(boss.is_boss());
        }
    }

    #[test]
    fn fodder_is_never_marked_as_a_boss() {
        for kind in EnemyKind::FODDER {
            assert!(!kind.is_boss(), "{kind:?} leaked into the fodder pool");
            assert!(kind.unlock_minute() < 100.0, "{kind:?} never unlocks");
        }
    }

    #[test]
    fn bosses_never_appear_in_the_trickle() {
        for boss in EnemyKind::BOSSES {
            assert!(boss.unlock_minute() > 100.0, "{boss:?} could trickle in");
        }
    }

    #[test]
    fn the_first_archetype_is_available_immediately() {
        assert_eq!(EnemyKind::FODDER[0].unlock_minute(), 0.0);
    }

    #[test]
    fn unlocks_are_ordered_and_spread_out() {
        let mut previous = -1.0;
        for kind in EnemyKind::FODDER {
            let t = kind.unlock_minute();
            assert!(t > previous, "{kind:?} unlocks out of order");
            previous = t;
        }
    }

    #[test]
    fn every_archetype_is_named_in_every_world() {
        for kind in EnemyKind::ALL {
            for env in EnvKind::ALL {
                let name = kind.name(env);
                assert!(!name.is_empty(), "{kind:?} unnamed in {env:?}");
            }
        }
    }

    #[test]
    fn names_are_distinct_within_a_world() {
        for env in EnvKind::ALL {
            let mut names: Vec<_> = EnemyKind::ALL.iter().map(|k| k.name(env)).collect();
            let total = names.len();
            names.sort_unstable();
            names.dedup();
            assert_eq!(names.len(), total, "duplicate names in {env:?}");
        }
    }

    #[test]
    fn boss_names_read_as_bosses() {
        for env in EnvKind::ALL {
            for boss in EnemyKind::BOSSES {
                let name = boss.name(env);
                assert_eq!(
                    name,
                    name.to_uppercase(),
                    "{name} should be shouted, not muttered"
                );
            }
        }
    }

    #[test]
    fn only_the_moth_flies() {
        for kind in EnemyKind::ALL {
            assert_eq!(kind.flies(), kind == EnemyKind::Moth);
        }
    }

    #[test]
    fn behaviours_are_assigned_and_bosses_get_boss_behaviours() {
        for kind in EnemyKind::ALL {
            let b = kind.behavior();
            let is_boss_behaviour = matches!(
                b,
                Behavior::BossCharger | Behavior::BossSlammer | Behavior::BossBeamer
            );
            assert_eq!(is_boss_behaviour, kind.is_boss(), "{kind:?} mismatched");
        }
    }

    #[test]
    fn the_name_table_is_indexed_by_the_enum_order() {
        // If someone reorders EnemyKind without reordering the table, this
        // catches it: the desk names are the ones we can eyeball.
        assert_eq!(EnemyKind::DustBunny.name(EnvKind::Desk), "Dust Bunny");
        assert_eq!(EnemyKind::BossLamp.name(EnvKind::Desk), "THE DESK LAMP");
    }

    // -- status effects -----------------------------------------------------

    #[test]
    fn a_clean_status_block_does_not_slow_anything() {
        assert_eq!(StatusEffects::default().speed_mult(), 1.0);
    }

    #[test]
    fn stun_beats_slow() {
        let mut s = StatusEffects::default();
        s.apply_slow(0.5, 1.0);
        s.apply_stun(1.0);
        assert_eq!(s.speed_mult(), 0.0);
    }

    #[test]
    fn slow_has_a_floor_so_nothing_freezes_solid() {
        let mut s = StatusEffects::default();
        s.apply_slow(5.0, 1.0);
        assert!(s.speed_mult() >= 0.15);
    }

    #[test]
    fn effects_take_the_strongest_and_longest() {
        let mut s = StatusEffects::default();
        s.apply_slow(0.3, 5.0);
        s.apply_slow(0.7, 1.0);
        assert!((s.slow - 0.7).abs() < 1e-6);
        assert!((s.slow_time - 5.0).abs() < 1e-6);

        s.apply_burn(3.0, 2.0);
        s.apply_burn(1.0, 8.0);
        assert!((s.burn_dps - 3.0).abs() < 1e-6);
        assert!((s.burn_time - 8.0).abs() < 1e-6);
    }

    #[test]
    fn an_expired_slow_stops_applying() {
        let mut s = StatusEffects::default();
        s.apply_slow(0.5, 0.0);
        assert_eq!(s.speed_mult(), 1.0);
    }

    // -- director -----------------------------------------------------------

    #[test]
    fn the_director_starts_with_headroom() {
        let d = Director::default();
        assert_eq!(d.alive, 0);
        assert!(d.cap > 100);
        assert!(d.boss_timer > d.elite_timer, "elites must precede the boss");
    }

    #[test]
    fn kind_selection_only_offers_unlocked_archetypes() {
        let mut rng = Rng::seeded(1234);
        for minute in [0.0f32, 1.0, 3.5, 7.0, 30.0] {
            for _ in 0..500 {
                let kind = pick_kind(&mut rng, minute).unwrap();
                assert!(
                    kind.unlock_minute() <= minute,
                    "{kind:?} appeared at minute {minute}"
                );
            }
        }
    }

    #[test]
    fn kind_selection_eventually_offers_everything() {
        let mut rng = Rng::seeded(555);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..5000 {
            seen.insert(pick_kind(&mut rng, 30.0).unwrap());
        }
        assert_eq!(seen.len(), EnemyKind::FODDER.len(), "some kind never rolls");
    }

    #[test]
    fn early_kinds_still_appear_late() {
        // The weighting favours new unlocks, but must not starve the old ones.
        let mut rng = Rng::seeded(777);
        let bunnies = (0..3000)
            .filter(|_| pick_kind(&mut rng, 30.0) == Some(EnemyKind::DustBunny))
            .count();
        assert!(bunnies > 60, "dust bunnies vanished entirely ({bunnies})");
    }
}
