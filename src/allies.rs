//! The squad, the structures, and the ground they hold.
//!
//! This is the half of the game that is not about the player's own body. Allies
//! and turrets fight on their own; the player's job is deciding what exists and
//! where it stands. Territory ties the two together: zones pay for the army,
//! and the army is what holds the zones.

use bevy::prelude::*;

use crate::arena::ObstacleField;
use crate::art::{GameArt, Glow};
use crate::combat::{Actor, Damageable, EnemyGrid, ShotVisual, SpawnShot};
use crate::common::{
    Altitude, Body, BurstEvent, DamageEvent, DamageSource, Health, RunEntity, SfxEvent,
    VisualScale, damp, to_world, yaw_towards,
};
use crate::enemy::{Enemy, StatusEffects};
use crate::environments::EnvKind;
use crate::player::{Player, PlayerStats};
use crate::threat::Threat;
use crate::{AppState, GameSet, RunSetup};

// -- economy ----------------------------------------------------------------

#[derive(Debug, Resource, Default)]
pub struct Economy {
    pub scrap: f32,
    pub cores: f32,
    /// Accumulated for the HUD's per-second readout.
    /// Scrap a second from zones and structures.
    pub scrap_rate: f32,
    /// Scrap a second from held forts.
    ///
    /// Its own field rather than added to `scrap_rate`, because `zone_income`
    /// *assigns* that one and lives in another plugin's system chain - so
    /// whichever ran second would silently win. Read them through
    /// `income_per_second`.
    pub fort_rate: f32,
    pub lifetime_scrap: f32,
    pub lifetime_cores: f32,
}

impl Economy {
    pub fn reset(&mut self) {
        *self = Self::default();
        // A small float so the first turret is reachable inside the first
        // prep window rather than three minutes in.
        self.scrap = 30.0;
    }

    pub fn can_afford_scrap(&self, cost: f32) -> bool {
        self.scrap >= cost
    }

    pub fn spend_scrap(&mut self, cost: f32) -> bool {
        if self.can_afford_scrap(cost) {
            self.scrap -= cost;
            true
        } else {
            false
        }
    }

    pub fn spend_cores(&mut self, cost: f32) -> bool {
        if self.cores >= cost {
            self.cores -= cost;
            true
        } else {
            false
        }
    }

    /// Everything coming in, per second.
    #[must_use]
    pub fn income_per_second(&self) -> f32 {
        self.scrap_rate + self.fort_rate
    }

    pub fn gain_scrap(&mut self, amount: f32) {
        self.scrap += amount;
        self.lifetime_scrap += amount;
    }

    pub fn gain_cores(&mut self, amount: f32) {
        self.cores += amount;
        self.lifetime_cores += amount;
    }
}

// -- allies -----------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AllyKind {
    Scout,
    Gunner,
    Bulwark,
    Medic,
}

impl AllyKind {
    pub const ALL: [Self; 4] = [Self::Scout, Self::Gunner, Self::Bulwark, Self::Medic];

    /// Per-world names, indexed by `env as usize`.
    ///
    /// Four roles across five worlds.
    const NAMES: [[&str; EnvKind::COUNT]; 4] = [
        // Scout
        ["Scout", "Sprite", "Runner", "Recon Drone", "Wisp"],
        // Gunner
        ["Gunner", "Slinger", "Marksman", "Turret Drone", "Adept"],
        // Bulwark
        ["Bulwark", "Beetle", "Riot Shield", "Aegis Frame", "Golem"],
        // Medic
        [
            "Medic",
            "Moth Healer",
            "Field Medic",
            "Repair Drone",
            "Acolyte",
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

    pub fn blurb(self) -> &'static str {
        match self {
            Self::Scout => "Fast and cheap. Good at holding a far zone.",
            Self::Gunner => "Steady ranged damage. Put it behind a Bulwark.",
            Self::Bulwark => "Soaks hits and blocks a corridor with its body.",
            Self::Medic => "Heals you, the squad, and your structures.",
        }
    }

    pub fn core_cost(self) -> f32 {
        match self {
            Self::Scout => 2.0,
            Self::Gunner => 3.0,
            Self::Bulwark | Self::Medic => 4.0,
        }
    }

    /// (hp, speed, damage, range, cooldown)
    fn stats(self) -> (f32, f32, f32, f32, f32) {
        match self {
            Self::Scout => (62.0, 7.0, 9.0, 2.6, 0.7),
            Self::Gunner => (74.0, 5.0, 13.0, 14.0, 1.0),
            Self::Bulwark => (210.0, 4.0, 15.0, 2.8, 1.2),
            // The medic's "damage" is its heal-per-tick.
            Self::Medic => (86.0, 5.4, 11.0, 8.5, 1.1),
        }
    }

    pub fn trim_color(self) -> Color {
        match self {
            Self::Scout => Color::srgb(0.45, 0.9, 1.0),
            Self::Gunner => Color::srgb(1.0, 0.72, 0.3),
            Self::Bulwark => Color::srgb(0.6, 0.7, 0.85),
            Self::Medic => Color::srgb(0.5, 1.0, 0.65),
        }
    }

    fn ranged(self) -> bool {
        matches!(self, Self::Gunner)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Stance {
    /// Trail the player.
    #[default]
    Follow,
    /// Stay put and fight whatever comes.
    Hold,
    /// Walk to an assigned zone and defend it.
    Guard,
}

impl Stance {
    pub fn label(self) -> &'static str {
        match self {
            Self::Follow => "FOLLOW",
            Self::Hold => "HOLD",
            Self::Guard => "GUARD",
        }
    }

    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Follow => Self::Hold,
            Self::Hold => Self::Guard,
            Self::Guard => Self::Follow,
        }
    }
}

#[derive(Debug, Component)]
pub struct Ally {
    pub kind: AllyKind,
    pub stance: Stance,
    pub anchor: Vec2,
    pub cooldown: f32,
    pub damage: f32,
    pub range: f32,
    pub speed: f32,
    pub fire_rate: f32,
    /// Index into the squad list, used for follow formation spacing.
    pub slot: u32,
    pub level: u32,
}

/// Squad-wide state the HUD and input both need.
#[derive(Debug, Resource, Default)]
pub struct Squad {
    pub stance: Stance,
    pub count: u32,
    pub cap: u32,
}

impl Squad {
    pub fn reset(&mut self) {
        self.stance = Stance::Follow;
        self.count = 0;
        // Beacons raise this; four is enough to feel like a squad without
        // turning the screen into soup.
        self.cap = 4;
    }
}

// -- structures -------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TurretKind {
    Tack,
    Lobber,
    Shocker,
    Barricade,
    Generator,
}

impl TurretKind {
    pub const ALL: [Self; 5] = [
        Self::Tack,
        Self::Lobber,
        Self::Shocker,
        Self::Barricade,
        Self::Generator,
    ];

    /// Per-world names, indexed by `env as usize`.
    ///
    /// Five structures across five worlds.
    const NAMES: [[&str; EnvKind::COUNT]; 5] = [
        // Tack
        [
            "Tack Turret",
            "Thorn Turret",
            "Nail Post",
            "Sentry Node",
            "Warding Spike",
        ],
        // Lobber
        [
            "Lobber",
            "Acorn Lobber",
            "Mortar Pot",
            "Arc Mortar",
            "Hex Mortar",
        ],
        // Shocker
        [
            "Shocker",
            "Bramble Snare",
            "Steam Vent",
            "Stasis Emitter",
            "Frost Sigil",
        ],
        // Barricade
        [
            "Barricade",
            "Log Wall",
            "Sandbags",
            "Hard-Light Wall",
            "Ward Stone",
        ],
        // Generator
        [
            "Generator",
            "Compost Heap",
            "Genset",
            "Power Cell",
            "Mana Well",
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

    pub fn blurb(self) -> &'static str {
        match self {
            Self::Tack => "Rapid single-target fire. Your bread and butter.",
            Self::Lobber => "Arcing splash. Put it behind the line.",
            Self::Shocker => "No damage. Slows everything in a wide radius.",
            Self::Barricade => "No gun. Reshapes where the enemy can walk.",
            Self::Generator => "Pays Scrap every second. Fragile. Guard it.",
        }
    }

    pub fn scrap_cost(self) -> f32 {
        match self {
            Self::Tack => 25.0,
            Self::Lobber => 45.0,
            Self::Shocker => 35.0,
            Self::Barricade => 18.0,
            Self::Generator => 55.0,
        }
    }

    /// (hp, damage, range, cooldown, radius)
    fn stats(self) -> (f32, f32, f32, f32, f32) {
        match self {
            Self::Tack => (130.0, 9.0, 12.0, 0.42, 0.7),
            Self::Lobber => (105.0, 24.0, 16.0, 1.7, 0.8),
            Self::Shocker => (145.0, 0.0, 7.5, 0.5, 0.7),
            Self::Barricade => (420.0, 0.0, 0.0, 0.0, 1.1),
            Self::Generator => (90.0, 0.0, 0.0, 1.0, 0.8),
        }
    }

    pub fn trim_color(self) -> Color {
        match self {
            Self::Tack => Color::srgb(1.0, 0.7, 0.3),
            Self::Lobber => Color::srgb(0.9, 0.45, 0.9),
            Self::Shocker => Color::srgb(0.4, 0.9, 1.0),
            Self::Barricade => Color::srgb(0.7, 0.75, 0.8),
            Self::Generator => Color::srgb(0.5, 1.0, 0.6),
        }
    }

    /// Scrap per second, for the Generator.
    pub fn income(self) -> f32 {
        if self == Self::Generator { 2.4 } else { 0.0 }
    }
}

#[derive(Debug, Component)]
pub struct Turret {
    pub kind: TurretKind,
    pub cooldown: f32,
    pub damage: f32,
    pub range: f32,
    pub fire_rate: f32,
    pub level: u32,
}

// -- territory --------------------------------------------------------------

pub const ZONE_RADIUS: f32 = 4.6;
const CAPTURE_SECONDS: f32 = 8.0;

/// How much space an ally takes up.
///
/// Was 0.42, which is smaller than a Sugar Ant. An ally is a squadmate the
/// player spent Cores on and should be as legible as the things it fights.
const ALLY_RADIUS: f32 = 0.55;

/// And how big it draws, on top of that.
const ALLY_VISUAL_SCALE: f32 = 1.4;

/// Turrets were "really small". They are a deliberate, paid-for change to the
/// board and should look like one.
const TURRET_VISUAL_SCALE: f32 = 1.45;

/// What one structure is worth as presence on a zone.
///
/// The same weight a structure carries on a fort, for the same reason: holding
/// ground is what a turret is for. Less than a body, because it cannot chase
/// anybody off.
const ZONE_STRUCTURE_WEIGHT: f32 = 0.5;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ZoneOwner {
    Neutral,
    Player,
    Enemy,
}

#[derive(Debug, Component)]
pub struct Zone {
    pub owner: ZoneOwner,
    /// -1 fully enemy, 0 neutral, +1 fully player.
    pub progress: f32,
    pub contested: bool,
    pub pulse: f32,
}

/// Ask the arena to place a territory marker.
#[derive(Debug, Message, Clone, Copy)]
pub struct SpawnZone {
    pub pos: Vec2,
    /// The chunk that asked for it, so unloading can take it away again.
    pub chunk: IVec2,
}

/// Recruit request, raised by the input layer.
#[derive(Debug, Message, Clone, Copy)]
pub struct RecruitRequest {
    pub kind: AllyKind,
}

/// Build request, raised by plan mode.
#[derive(Debug, Message, Clone, Copy)]
pub struct BuildRequest {
    pub kind: TurretKind,
    pub pos: Vec2,
}

#[derive(Debug)]
pub struct AlliesPlugin;

impl Plugin for AlliesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Economy>()
            .init_resource::<Squad>()
            .add_message::<SpawnZone>()
            .add_message::<RecruitRequest>()
            .add_message::<BuildRequest>()
            .add_systems(Update, spawn_zones)
            .add_systems(Update, (ally_think, turret_think).in_set(GameSet::Think))
            .add_systems(
                Update,
                (
                    handle_recruit,
                    handle_build,
                    zone_capture,
                    zone_income,
                    structure_income,
                )
                    .chain()
                    .in_set(GameSet::Resolve),
            )
            .add_systems(Update, zone_visuals.in_set(GameSet::Present))
            .add_systems(
                OnExit(AppState::Menu),
                reset_command_state.in_set(RunSetup::Reset),
            );
    }
}

fn reset_command_state(mut economy: ResMut<Economy>, mut squad: ResMut<Squad>) {
    economy.reset();
    squad.reset();
}

fn spawn_zones(mut commands: Commands, art: Res<GameArt>, mut requests: MessageReader<SpawnZone>) {
    for req in requests.read() {
        commands.spawn((
            crate::world::ChunkEntity(req.chunk),
            Zone {
                owner: ZoneOwner::Neutral,
                progress: 0.0,
                contested: false,
                pulse: 0.0,
            },
            Body::new(req.pos, ZONE_RADIUS),
            Mesh3d(art.zone_pillar.clone()),
            MeshMaterial3d(art.glow(Glow::Zone)),
            Transform::from_translation(to_world(req.pos, 0.0)),
            crate::environments::EnvEntity,
        ));
        // The floor ring that shows the capture radius.
        commands.spawn((
            ZoneRing(req.pos),
            Mesh3d(art.ring.clone()),
            MeshMaterial3d(art.glow(Glow::Zone)),
            Transform::from_translation(to_world(req.pos, 0.05)).with_scale(Vec3::new(
                ZONE_RADIUS,
                1.0,
                ZONE_RADIUS,
            )),
            crate::environments::EnvEntity,
        ));
    }
}

#[derive(Debug, Component)]
pub struct ZoneRing(pub Vec2);

fn handle_recruit(
    mut commands: Commands,
    art: Res<GameArt>,
    threat: Res<Threat>,
    clock: Res<crate::threat::RunClock>,
    progression: Res<crate::progress::Progression>,
    mut seen: MessageWriter<crate::coverage::Seen>,
    stats: Res<PlayerStats>,
    mut economy: ResMut<Economy>,
    mut squad: ResMut<Squad>,
    mut requests: MessageReader<RecruitRequest>,
    player: Query<&Body, With<Player>>,
    mut hints: ResMut<crate::onboarding::HintQueue>,
    mut sfx: MessageWriter<SfxEvent>,
) {
    for req in requests.read() {
        if squad.count >= squad.cap {
            hints.push(
                "SQUAD FULL",
                "Four is the cap. Keep them alive.",
                crate::onboarding::HintTone::Tip,
            );
            continue;
        }
        if !economy.spend_cores(req.kind.core_cost()) {
            hints.push(
                "NOT ENOUGH CORES",
                "Cores drop from elites and captured zones.",
                crate::onboarding::HintTone::Tip,
            );
            continue;
        }

        let origin = player.iter().next().map_or(Vec2::ZERO, |b| b.pos);
        let (hp, speed, damage, range, cooldown) = req.kind.stats();
        let scale = defence_scale(crate::threat::enemy_power(
            &threat,
            &clock,
            progression.level,
        ));
        let slot = squad.count;
        squad.count += 1;

        commands.spawn((
            Ally {
                kind: req.kind,
                stance: squad.stance,
                anchor: origin,
                cooldown: 0.0,
                damage: damage * stats.ally_damage * scale,
                range,
                speed,
                fire_rate: cooldown,
                slot,
                level: 1,
            },
            Health::new(hp * stats.ally_health * scale),
            Body::new(origin + Vec2::new(1.5, 1.5), ALLY_RADIUS),
            Altitude::default(),
            Actor::default(),
            StatusEffects::default(),
            VisualScale::new(ALLY_VISUAL_SCALE),
            Damageable {
                hostile_target: true,
            },
            Mesh3d(art.allies[req.kind as usize].clone()),
            MeshMaterial3d(art.solid.clone()),
            Transform::from_translation(to_world(origin, 0.0))
                .with_scale(Vec3::splat(ALLY_VISUAL_SCALE)),
            RunEntity,
            children![(
                // A green ring, the same idea as an elite's halo. "I have not
                // seen any allies" was a fair report: they were radius 0.42 in
                // the plain untinted material, smaller than most monsters and
                // marked as nobody's.
                Mesh3d(art.ring.clone()),
                MeshMaterial3d(art.glow(Glow::Friend)),
                Transform::from_xyz(0.0, 0.06, 0.0).with_scale(Vec3::new(0.95, 1.0, 0.95)),
            )],
        ));

        seen.write(crate::coverage::Seen(format!("ally:{:?}", req.kind)));
        seen.write(crate::coverage::Seen(String::from("deed:recruit")));
        sfx.write(SfxEvent::new(crate::audio::Sfx::Recruit));
    }
}

/// How much a structure or ally is worth against the current opposition.
///
/// Their stats were flat constants while enemy health and damage compounded
/// with time, threat and level. By minute sixteen a Tack Turret needed
/// 162 seconds to kill one Dust Bunny and the tankiest ally died to three
/// touches - so the scrap economy had no sink, because the things it buys had
/// stopped working. Scaled by the square root of enemy power rather than by
/// power itself: defences should stay relevant, not stay equal.
#[must_use]
pub fn defence_scale(power: f32) -> f32 {
    power.max(1.0).sqrt()
}

fn handle_build(
    threat: Res<Threat>,
    clock: Res<crate::threat::RunClock>,
    progression: Res<crate::progress::Progression>,
    mut seen: MessageWriter<crate::coverage::Seen>,
    mut commands: Commands,
    art: Res<GameArt>,
    stats: Res<PlayerStats>,
    mut economy: ResMut<Economy>,
    mut obstacles: ResMut<ObstacleField>,
    mut requests: MessageReader<BuildRequest>,
    mut sfx: MessageWriter<SfxEvent>,
    mut bursts: MessageWriter<BurstEvent>,
) {
    for req in requests.read() {
        let cost = req.kind.scrap_cost() * (1.0 - stats.build_discount);
        if !economy.spend_scrap(cost) {
            continue;
        }

        let (hp, damage, range, cooldown, radius) = req.kind.stats();
        let scale = defence_scale(crate::threat::enemy_power(
            &threat,
            &clock,
            progression.level,
        ));

        // Barricades are the only structure that becomes terrain. Everything
        // else stays walkable so the player cannot accidentally wall
        // themselves in with their own guns.
        if req.kind == TurretKind::Barricade {
            obstacles.push(
                req.pos,
                crate::arena::ColliderShape::rect(1.0, 0.35),
                true,
                1.0,
            );
        }

        seen.write(crate::coverage::Seen(format!("turret:{:?}", req.kind)));
        seen.write(crate::coverage::Seen(String::from("deed:build")));
        commands.spawn((
            Turret {
                kind: req.kind,
                cooldown: 0.0,
                damage: damage * stats.structure_damage * scale,
                range,
                fire_rate: cooldown,
                level: 1,
            },
            Health::new(hp * stats.structure_health * scale),
            Body::new(req.pos, radius),
            Altitude::default(),
            VisualScale::new(1.0),
            Damageable {
                hostile_target: true,
            },
            Mesh3d(art.turrets[req.kind as usize].clone()),
            MeshMaterial3d(art.solid.clone()),
            Transform::from_translation(to_world(req.pos, 0.0))
                .with_scale(Vec3::splat(TURRET_VISUAL_SCALE)),
            RunEntity,
            children![(
                // Ringed in the player's green like an ally, so the board reads
                // at a glance as mine-versus-theirs rather than as clutter.
                Mesh3d(art.ring.clone()),
                MeshMaterial3d(art.glow(Glow::Friend)),
                Transform::from_xyz(0.0, 0.05, 0.0).with_scale(Vec3::new(0.9, 1.0, 0.9)),
            )],
        ));

        bursts.write(BurstEvent {
            pos: req.pos,
            height: 0.4,
            color: req.kind.trim_color(),
            count: 14,
            speed: 5.0,
            size: 0.6,
        });
        sfx.write(SfxEvent::new(crate::audio::Sfx::Build));
    }
}

#[allow(clippy::too_many_arguments)]
fn ally_think(
    time: Res<Time>,
    grid: Res<EnemyGrid>,
    obstacles: Res<ObstacleField>,
    squad: Res<Squad>,
    player: Query<&Body, (With<Player>, Without<Ally>)>,
    zones: Query<(&Zone, &Body), Without<Ally>>,
    mut allies: Query<(&mut Ally, &mut Body, &StatusEffects)>,
    mut healths: Query<&mut Health, Without<Enemy>>,
    mut damage: MessageWriter<DamageEvent>,
    mut shots: MessageWriter<SpawnShot>,
) {
    let dt = time.delta_secs();
    let player_pos = player.iter().next().map_or(Vec2::ZERO, |b| b.pos);

    for (mut ally, mut body, status) in &mut allies {
        ally.cooldown = (ally.cooldown - dt).max(0.0);

        // -- where do I want to be? ----------------------------------------
        let goal = match ally.stance {
            Stance::Follow => {
                // Spread the squad around the player rather than stacking.
                let a = ally.slot as f32 / squad.cap.max(1) as f32 * std::f32::consts::TAU;
                player_pos + Vec2::new(a.cos(), a.sin()) * 2.8
            }
            Stance::Hold => ally.anchor,
            Stance::Guard => {
                // Prefer a contested or enemy-held zone, else the nearest.
                let mut best: Option<(f32, Vec2)> = None;
                for (zone, zbody) in &zones {
                    let priority = match zone.owner {
                        ZoneOwner::Enemy => 0.0,
                        ZoneOwner::Neutral => 40.0,
                        ZoneOwner::Player => {
                            if zone.contested {
                                10.0
                            } else {
                                200.0
                            }
                        }
                    };
                    let score = priority + zbody.pos.distance(body.pos);
                    if best.is_none_or(|(bs, _)| score < bs) {
                        best = Some((score, zbody.pos));
                    }
                }
                best.map_or(ally.anchor, |(_, p)| p)
            }
        };

        // -- fight what is in reach ----------------------------------------
        let target = grid.nearest_visible(body.pos, ally.range + 2.0, &obstacles);

        // Chase a target only a short way from the goal, so a Guard does not
        // get walked off its zone by a single wandering ant.
        let leash = match ally.stance {
            Stance::Follow => 7.0,
            Stance::Hold | Stance::Guard => 5.5,
        };

        let move_target = match target {
            Some(t) if t.pos.distance(goal) < leash => {
                if ally.kind.ranged() {
                    // Hold at the edge of range.
                    let to = (body.pos - t.pos).normalize_or_zero();
                    t.pos + to * (ally.range * 0.7)
                } else {
                    t.pos
                }
            }
            _ => goal,
        };

        let to_goal = move_target - body.pos;
        let dist = to_goal.length();
        // A dead zone stops the shuffle-in-place jitter that otherwise happens
        // whenever an ally is already where it wants to be.
        body.vel = if dist > 0.6 {
            to_goal / dist * ally.speed * status.speed_mult()
        } else {
            Vec2::ZERO
        };

        if ally.cooldown > 0.0 {
            continue;
        }

        // -- medics heal instead of shooting --------------------------------
        if ally.kind == AllyKind::Medic {
            let mut healed = false;
            let heal = ally.damage;
            for mut health in &mut healths {
                if health.current < health.max && health.current > 0.0 {
                    health.heal(heal);
                    healed = true;
                    break;
                }
            }
            if healed {
                ally.cooldown = ally.fire_rate;
            }
            continue;
        }

        let Some(t) = target else { continue };
        let to_target = t.pos - body.pos;
        if to_target.length() > ally.range {
            continue;
        }

        ally.cooldown = ally.fire_rate;
        if ally.kind.ranged() {
            let mut shot = SpawnShot::friendly(
                body.pos,
                to_target.normalize_or_zero(),
                20.0,
                ally.damage,
                ShotVisual::Pellet,
            );
            shot.radius = 0.24;
            shots.write(shot);
        } else {
            damage.write(DamageEvent {
                target: t.entity,
                amount: ally.damage,
                crit: false,
                knockback: to_target.normalize_or_zero(),
                knockback_force: if ally.kind == AllyKind::Bulwark {
                    16.0
                } else {
                    5.0
                },
                source: DamageSource::Player,
            });
        }
    }
}

fn turret_think(
    time: Res<Time>,
    grid: Res<EnemyGrid>,
    obstacles: Res<ObstacleField>,
    mut turrets: Query<(&mut Turret, &Body, &mut Transform)>,
    mut shots: MessageWriter<SpawnShot>,
    mut statuses: Query<&mut StatusEffects, With<Enemy>>,
) {
    let dt = time.delta_secs();
    for (mut turret, body, mut transform) in &mut turrets {
        if turret.fire_rate <= 0.0 {
            continue;
        }
        turret.cooldown = (turret.cooldown - dt).max(0.0);

        // Shockers are an aura, not a gun.
        if turret.kind == TurretKind::Shocker {
            if turret.cooldown > 0.0 {
                continue;
            }
            turret.cooldown = turret.fire_rate;
            let range = turret.range;
            grid.for_each_near(body.pos, range, |e| {
                if let Ok(mut status) = statuses.get_mut(e.entity) {
                    status.apply_slow(0.5, 0.8);
                }
            });
            continue;
        }

        if turret.kind == TurretKind::Barricade || turret.kind == TurretKind::Generator {
            continue;
        }

        let Some(t) = grid.best_visible_target(body.pos, turret.range, &obstacles) else {
            continue;
        };

        // Track the target even while reloading; a turret that only turns when
        // it fires looks broken.
        let dir = (t.pos - body.pos).normalize_or_zero();
        transform.rotation = Quat::from_rotation_y(yaw_towards(dir));

        if turret.cooldown > 0.0 {
            continue;
        }
        turret.cooldown = turret.fire_rate;

        match turret.kind {
            TurretKind::Tack => {
                let mut shot =
                    SpawnShot::friendly(body.pos, dir, 30.0, turret.damage, ShotVisual::Tack);
                shot.height = 0.55;
                shots.write(shot);
            }
            TurretKind::Lobber => {
                let mut shot =
                    SpawnShot::friendly(body.pos, dir, 17.0, turret.damage, ShotVisual::Pellet);
                shot.aoe = 3.2;
                shot.height = 0.9;
                shot.scale = 1.5;
                shots.write(shot);
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn zone_capture(
    time: Res<Time>,
    stats: Res<PlayerStats>,
    mut zones: Query<(&mut Zone, &Body)>,
    player: Query<&Body, With<Player>>,
    allies: Query<&Body, (With<Ally>, Without<Zone>)>,
    structures: Query<&Body, (With<Turret>, Without<Zone>)>,
    enemies: Query<&Body, (With<Enemy>, Without<Zone>)>,
    mut economy: ResMut<Economy>,
    mut hints: ResMut<crate::onboarding::HintQueue>,
    mut sfx: MessageWriter<SfxEvent>,
) {
    let dt = time.delta_secs();

    for (mut zone, zbody) in &mut zones {
        let r_sq = ZONE_RADIUS * ZONE_RADIUS;

        let mut friendly = 0.0f32;
        if player
            .iter()
            .any(|b| b.pos.distance_squared(zbody.pos) <= r_sq)
        {
            // The player alone captures at full speed; allies are a force
            // multiplier, not a replacement.
            friendly += 1.0;
        }
        friendly += allies
            .iter()
            .filter(|b| b.pos.distance_squared(zbody.pos) <= r_sq)
            .count() as f32
            * 0.6;
        // Structures hold ground. Without this the only way to keep a zone was
        // to stand on it or park allies on it, so a second zone cost the first
        // one and "hold territory" could never be more than one flag at a time.
        // A garrison of scrap and turrets is a better ongoing cost than an
        // ongoing cost in attention, and it is the same idea as fortifying a
        // captured fort.
        friendly += structures
            .iter()
            .filter(|b| b.pos.distance_squared(zbody.pos) <= r_sq)
            .count() as f32
            * ZONE_STRUCTURE_WEIGHT;

        let hostile = enemies
            .iter()
            .filter(|b| b.pos.distance_squared(zbody.pos) <= r_sq)
            .count() as f32
            * 0.22;

        zone.contested = friendly > 0.0 && hostile > 0.0;

        let net = friendly - hostile;
        if net.abs() > 0.01 {
            zone.progress += net.signum()
                * net.abs().min(3.0)
                * (dt / CAPTURE_SECONDS)
                * stats.zone_capture_rate;
            zone.progress = zone.progress.clamp(-1.0, 1.0);
        } else if friendly == 0.0 && hostile == 0.0 {
            // Ungarrisoned zones decay slowly back towards neutral. A flag in
            // open ground is not a place; unlike a fort, it does not hold
            // itself.
            zone.progress = damp(zone.progress, 0.0, 0.06, dt);
        }

        let was = zone.owner;
        zone.owner = if zone.progress >= 0.999 {
            ZoneOwner::Player
        } else if zone.progress <= -0.999 {
            ZoneOwner::Enemy
        } else if zone.owner != ZoneOwner::Neutral && zone.progress.abs() < 0.35 {
            ZoneOwner::Neutral
        } else {
            zone.owner
        };

        if was != zone.owner {
            match zone.owner {
                ZoneOwner::Player => {
                    economy.gain_cores(1.0);
                    zone.pulse = 1.0;
                    sfx.write(SfxEvent::new(crate::audio::Sfx::Capture));
                    hints.push_once(
                        "zone-first",
                        "ZONE HELD",
                        "It pays income and boosts you - and the enemy will come for it.",
                        crate::onboarding::HintTone::Tip,
                    );
                }
                ZoneOwner::Enemy => {
                    zone.pulse = 1.0;
                    sfx.write(SfxEvent::new(crate::audio::Sfx::Lost));
                }
                ZoneOwner::Neutral => {}
            }
        }
    }
}

/// Held zones pay out, and raise the threat floor for holding them.
fn zone_income(
    time: Res<Time>,
    stats: Res<PlayerStats>,
    zones: Query<&Zone>,
    mut economy: ResMut<Economy>,
    mut threat: ResMut<Threat>,
    mut xp: ResMut<crate::progress::Progression>,
) {
    let dt = time.delta_secs();
    let held = zones
        .iter()
        .filter(|z| z.owner == ZoneOwner::Player)
        .count() as f32;

    // Holding ground is loud. This is the tradeoff, stated in one line.
    threat.territory = held * 0.2;

    if held > 0.0 {
        let rate = held * 1.6 * stats.income_mult;
        economy.gain_scrap(rate * dt);
        economy.gain_cores(held * 0.05 * dt * stats.core_mult);
        xp.gain(held * 1.2 * dt * stats.xp_mult);
        economy.scrap_rate = rate;
    } else {
        economy.scrap_rate = 0.0;
    }
}

fn structure_income(
    time: Res<Time>,
    stats: Res<PlayerStats>,
    turrets: Query<(&Turret, &Health)>,
    mut economy: ResMut<Economy>,
) {
    let dt = time.delta_secs();
    let mut rate = 0.0;
    for (turret, health) in &turrets {
        if health.is_dead() {
            continue;
        }
        rate += turret.kind.income();
    }
    if rate > 0.0 {
        let gain = rate * stats.income_mult;
        economy.gain_scrap(gain * dt);
        economy.scrap_rate += gain;
    }
}

/// Recolour zone markers and rings to match ownership.
fn zone_visuals(
    time: Res<Time>,
    art: Res<GameArt>,
    mut zones: Query<(
        &mut Zone,
        &Body,
        &mut Transform,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
    mut rings: Query<
        (
            &ZoneRing,
            &mut Transform,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        Without<Zone>,
    >,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();

    for (mut zone, _, mut transform, mut material) in &mut zones {
        zone.pulse = damp(zone.pulse, 0.0, 3.0, dt);
        let glow = match zone.owner {
            ZoneOwner::Player => Glow::ZoneHeld,
            ZoneOwner::Enemy => Glow::Warning,
            ZoneOwner::Neutral => Glow::Zone,
        };
        material.0 = art.glow(glow);

        // Spin faster while contested: motion is the cheapest way to say
        // "look here" without adding UI.
        let spin = if zone.contested { 3.2 } else { 0.7 };
        transform.rotation = Quat::from_rotation_y(t * spin);
        let bob = 1.0 + zone.pulse * 0.4 + (t * 2.0).sin() * 0.03;
        transform.scale = Vec3::splat(bob);
    }

    // Rings show capture progress by scaling with it.
    let mut owners: Vec<(Vec2, f32, ZoneOwner)> = Vec::new();
    for (zone, body, _, _) in &mut zones {
        owners.push((body.pos, zone.progress, zone.owner));
    }
    for (ring, mut transform, mut material) in &mut rings {
        let Some((_, progress, owner)) = owners.iter().copied().min_by(|a, b| {
            a.0.distance_squared(ring.0)
                .total_cmp(&b.0.distance_squared(ring.0))
        }) else {
            continue;
        };
        material.0 = art.glow(match owner {
            ZoneOwner::Player => Glow::ZoneHeld,
            ZoneOwner::Enemy => Glow::Warning,
            ZoneOwner::Neutral => Glow::Zone,
        });
        let fill = ZONE_RADIUS * (0.35 + progress.abs() * 0.65);
        transform.scale = Vec3::new(fill, 1.0, fill);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn income_from_two_plugins_does_not_cancel_out() {
        // `zone_income` assigns its figure and `fort_income` lives in another
        // plugin's chain, so sharing one field meant whichever ran second won
        // silently. A fort-holder read a rate with its forts left out.
        let economy = Economy {
            scrap_rate: 4.8,
            fort_rate: 2.4,
            ..Economy::default()
        };
        assert!((economy.income_per_second() - 7.2).abs() < 1e-4);
    }

    #[test]
    fn no_holdings_means_no_income() {
        assert!((Economy::default().income_per_second() - 0.0).abs() < 1e-6);
    }

    /// The net presence on a zone, as `zone_capture` computes it.
    fn zone_net(standing_there: bool, allies: u32, turrets: u32, monsters: u32) -> f32 {
        let mut friendly = if standing_there { 1.0 } else { 0.0 };
        friendly += allies as f32 * 0.6;
        friendly += turrets as f32 * ZONE_STRUCTURE_WEIGHT;
        let hostile = monsters as f32 * 0.22;
        friendly - hostile
    }

    #[test]
    fn a_zone_can_be_held_without_standing_on_it() {
        // Otherwise a second zone costs you the first, and territory is one
        // flag at a time forever.
        assert!(
            zone_net(false, 0, 0, 0) <= 0.0,
            "held itself with nothing on it"
        );
        assert!(
            zone_net(false, 0, 2, 0) > 0.0,
            "two turrets should hold a zone"
        );
    }

    #[test]
    fn a_garrisoned_zone_still_falls_to_a_real_push() {
        // Sticky, not invulnerable.
        assert!(zone_net(false, 0, 2, 12) < 0.0, "twelve monsters is a push");
    }

    #[test]
    fn a_reset_economy_can_afford_something_immediately() {
        let mut e = Economy::default();
        e.reset();
        let cheapest = TurretKind::ALL
            .iter()
            .map(|k| k.scrap_cost())
            .fold(f32::MAX, f32::min);
        assert!(
            e.can_afford_scrap(cheapest),
            "the first prep window must allow at least one build"
        );
    }

    #[test]
    fn spending_only_succeeds_when_affordable() {
        let mut e = Economy::default();
        e.gain_scrap(50.0);
        assert!(e.spend_scrap(30.0));
        assert!((e.scrap - 20.0).abs() < 1e-5);
        assert!(!e.spend_scrap(30.0), "overspending must fail");
        assert!(
            (e.scrap - 20.0).abs() < 1e-5,
            "a failed spend must not deduct"
        );
    }

    #[test]
    fn cores_behave_the_same_way() {
        let mut e = Economy::default();
        e.gain_cores(4.0);
        assert!(e.spend_cores(4.0));
        assert!(!e.spend_cores(0.5));
        assert_eq!(e.cores, 0.0);
    }

    #[test]
    fn lifetime_totals_ignore_spending() {
        let mut e = Economy::default();
        e.gain_scrap(100.0);
        e.spend_scrap(90.0);
        e.gain_cores(5.0);
        e.spend_cores(5.0);
        assert!((e.lifetime_scrap - 100.0).abs() < 1e-5);
        assert!((e.lifetime_cores - 5.0).abs() < 1e-5);
    }

    #[test]
    fn reset_wipes_the_ledger() {
        let mut e = Economy::default();
        e.gain_scrap(500.0);
        e.gain_cores(50.0);
        e.reset();
        assert_eq!(e.cores, 0.0);
        assert_eq!(e.lifetime_scrap, 0.0);
    }

    #[test]
    fn every_ally_is_described_and_priced() {
        for kind in AllyKind::ALL {
            assert!(!kind.name(EnvKind::Desk).is_empty());
            assert!(!kind.blurb().is_empty());
            assert!(kind.core_cost() > 0.0);
            let (hp, speed, damage, range, cooldown) = kind.stats();
            assert!(hp > 0.0 && speed > 0.0 && damage > 0.0);
            assert!(range > 0.0 && cooldown > 0.0);
        }
    }

    #[test]
    fn allies_keep_pace_with_the_player() {
        // A squad that cannot follow is useless, so nobody may be much slower
        // than the hero.
        for kind in AllyKind::ALL {
            let speed = kind.stats().1;
            assert!(
                speed > crate::player::BASE_SPEED * 0.4,
                "{kind:?} at {speed} will always be left behind"
            );
        }
    }

    #[test]
    fn the_bulwark_is_the_tank_and_the_scout_is_the_cheap_one() {
        let bulwark_hp = AllyKind::Bulwark.stats().0;
        for kind in AllyKind::ALL {
            if kind != AllyKind::Bulwark {
                assert!(
                    bulwark_hp > kind.stats().0,
                    "{kind:?} out-tanks the Bulwark"
                );
            }
            assert!(AllyKind::Scout.core_cost() <= kind.core_cost());
        }
    }

    #[test]
    fn only_the_gunner_fights_at_range() {
        for kind in AllyKind::ALL {
            assert_eq!(kind.ranged(), kind == AllyKind::Gunner);
        }
        assert!(AllyKind::Gunner.stats().3 > AllyKind::Bulwark.stats().3);
    }

    #[test]
    fn ally_trim_colours_are_distinct() {
        for a in AllyKind::ALL {
            for b in AllyKind::ALL {
                if a as usize >= b as usize {
                    continue;
                }
                let (x, y) = (a.trim_color().to_linear(), b.trim_color().to_linear());
                let delta =
                    (x.red - y.red).abs() + (x.green - y.green).abs() + (x.blue - y.blue).abs();
                assert!(delta > 0.15, "{a:?} and {b:?} look alike");
            }
        }
    }

    #[test]
    fn stances_cycle_through_all_three() {
        let mut s = Stance::Follow;
        let mut seen = vec![s];
        for _ in 0..2 {
            s = s.next();
            seen.push(s);
        }
        assert_eq!(s.next(), Stance::Follow, "the cycle must close");
        seen.sort_by_key(|s| s.label());
        seen.dedup();
        assert_eq!(seen.len(), 3);
    }

    #[test]
    fn every_stance_has_a_label() {
        for s in [Stance::Follow, Stance::Hold, Stance::Guard] {
            assert!(!s.label().is_empty());
        }
    }

    #[test]
    fn every_structure_is_described_and_priced() {
        for kind in TurretKind::ALL {
            assert!(!kind.name(EnvKind::Desk).is_empty());
            assert!(!kind.blurb().is_empty());
            assert!(kind.scrap_cost() > 0.0);
            let (hp, ..) = kind.stats();
            assert!(hp > 0.0, "{kind:?} would die instantly");
        }
    }

    #[test]
    fn the_barricade_is_the_cheapest_and_toughest_thing_you_can_build() {
        let (barricade_hp, damage, range, rate, _) = TurretKind::Barricade.stats();
        assert_eq!(damage, 0.0, "a barricade must not shoot");
        assert_eq!(range, 0.0);
        assert_eq!(rate, 0.0);
        for kind in TurretKind::ALL {
            assert!(
                barricade_hp >= kind.stats().0,
                "{kind:?} out-tanks the wall"
            );
            assert!(TurretKind::Barricade.scrap_cost() <= kind.scrap_cost());
        }
    }

    #[test]
    fn only_the_generator_pays_income() {
        for kind in TurretKind::ALL {
            let income = kind.income();
            assert_eq!(
                income > 0.0,
                kind == TurretKind::Generator,
                "{kind:?} income is wrong"
            );
        }
    }

    #[test]
    fn the_generator_eventually_pays_for_itself() {
        let cost = TurretKind::Generator.scrap_cost();
        let payback = cost / TurretKind::Generator.income();
        assert!(
            (10.0..120.0).contains(&payback),
            "payback in {payback}s is not a real decision"
        );
    }

    #[test]
    fn the_shocker_controls_rather_than_kills() {
        let (_, damage, range, rate, _) = TurretKind::Shocker.stats();
        assert_eq!(damage, 0.0);
        assert!(range > 0.0 && rate > 0.0, "it still needs to tick");
    }

    #[test]
    fn damage_turrets_trade_rate_against_punch() {
        let (_, tack_dmg, _, tack_rate, _) = TurretKind::Tack.stats();
        let (_, lob_dmg, _, lob_rate, _) = TurretKind::Lobber.stats();
        assert!(
            tack_rate < lob_rate,
            "the tack turret should be the fast one"
        );
        assert!(lob_dmg > tack_dmg, "the lobber should be the heavy one");
    }

    #[test]
    fn a_fresh_squad_has_a_cap_and_no_members() {
        let mut s = Squad::default();
        s.reset();
        assert_eq!(s.count, 0);
        assert!(s.cap > 0);
        assert_eq!(s.stance, Stance::Follow);
    }

    #[test]
    fn zones_start_neutral() {
        let z = Zone {
            owner: ZoneOwner::Neutral,
            progress: 0.0,
            contested: false,
            pulse: 0.0,
        };
        assert_eq!(z.owner, ZoneOwner::Neutral);
        assert!(!z.contested);
    }
}
