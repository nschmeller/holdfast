//! Forts, nests, seeders, and the war between factions.
//!
//! The world is not a spawn table. It is a map of **holdings**:
//!
//! - A **fort** is a stronghold. It sends out assaults at whoever is near, and
//!   it sends out **seeders** - single monsters that walk somewhere else and
//!   turn into nests. That is how a faction grows.
//! - A **nest** trickles out monsters forever until something kills it. Nests
//!   are seeded into the world at generation time and planted by seeders.
//! - Forts **change hands**. Stand near one with no defenders left and it
//!   becomes yours, and then it works for you exactly as it worked for them -
//!   your assaults, your seeders, your nests. And they will come to take it
//!   back.
//!
//! A fort is captured by *presence*, not by damage: whoever is standing in the
//! ring pushes the meter their way. That is what lets allies take one on their
//! own, and it means holding ground is a positioning problem rather than a
//! damage-per-second problem, which is what this game is supposed to be about.
//!
//! Nests, by contrast, are destroyed rather than converted. Forts are places;
//! nests are infestations.

use bevy::prelude::*;

use crate::art::GameArt;
use crate::combat::Damageable;
use crate::common::{
    Body, BurstEvent, DeathEvent, Doomed, Health, RunEntity, SfxEvent, VisualScale, to_world,
};
use crate::enemy::{Enemy, EnemyKind, Rank, spawn_enemy};
use crate::environments::EnvKind;
use crate::factions::{Allegiance, Diplomacy, Faction};
use crate::player::Player;
use crate::rng::Rng;
use crate::threat::{RunClock, Threat, enemy_power};
use crate::{AppState, GameSet, RunSetup};

/// Footprint of a fort.
pub const FORT_RADIUS: f32 = 3.2;
/// Stand inside this to contest a fort.
pub const FORT_CAPTURE_RADIUS: f32 = 7.5;
/// Seconds of uncontested presence to flip a fort.
const FORT_CAPTURE_SECONDS: f32 = 11.0;
/// Footprint of a nest.
pub const NEST_RADIUS: f32 = 1.3;

/// How near the player has to be before a fort bothers assaulting.
///
/// A fort three screens away throwing monsters at nothing is wasted
/// simulation and, worse, an invisible drip of enemies from nowhere.
const ASSAULT_RANGE: f32 = 66.0;

/// How far a seeder walks before it settles.
const SEEDER_TRAVEL: f32 = 34.0;

// -- the holdings -----------------------------------------------------------

/// A stronghold. Owned by whoever last held the ring.
#[derive(Component, Debug)]
pub struct Fort {
    /// -1 fully hostile, +1 fully the player's. Forts owned by a monster
    /// faction sit at -1 and the faction is carried in `Allegiance`.
    pub progress: f32,
    pub contested: bool,
    pub assault: f32,
    pub seeding: f32,
    /// Nests this fort has planted and not yet lost.
    pub planted: u32,
    pub pulse: f32,
}

impl Default for Fort {
    fn default() -> Self {
        Self {
            progress: -1.0,
            contested: false,
            // Staggered so a cluster of forts does not fire in lockstep.
            assault: 18.0,
            seeding: 26.0,
            planted: 0,
            pulse: 0.0,
        }
    }
}

/// A nest. Trickles out monsters until killed.
#[derive(Component, Debug)]
pub struct Nest {
    pub timer: f32,
    /// The fort that planted this, so its tally can be corrected on death.
    pub home: Option<Entity>,
}

/// A monster carrying a nest. Walks somewhere and settles.
#[derive(Component, Debug)]
pub struct Seeder {
    pub target: Vec2,
    /// Gives up and settles where it stands when this runs out, so a seeder
    /// walled off from its target never wanders forever.
    pub patience: f32,
    pub home: Option<Entity>,
}

/// What a monster is currently trying to do.
///
/// Without this every enemy walks at the player forever and the map is
/// scenery. With it, a faction can besiege a fort it wants while ignoring a
/// player it cannot catch.
#[derive(Component, Debug, Clone, Copy)]
pub struct Objective {
    pub kind: ObjectiveKind,
    pub pos: Vec2,
    pub fort: Option<Entity>,
    /// Time until this monster reconsiders.
    pub review: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ObjectiveKind {
    HuntPlayer,
    TakeFort,
    DefendFort,
}

/// Ask the world to place a fort.
#[derive(Message, Debug, Clone, Copy)]
pub struct SpawnFort {
    pub pos: Vec2,
    pub faction: Faction,
}

/// Ask the world to place a nest.
#[derive(Message, Debug, Clone, Copy)]
pub struct SpawnNest {
    pub pos: Vec2,
    pub faction: Faction,
    pub home: Option<Entity>,
}

// -- the war plan -----------------------------------------------------------

/// What a faction is trying to achieve right now.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Posture {
    /// The player is soft or nearby; everything goes at them.
    HuntPlayer,
    /// Mass on one fort and take it.
    MassOnFort,
    /// Something of ours is being taken; go home.
    Defend,
    /// Split: some pressure the player, some push a fort.
    Split,
}

/// One faction's current intent.
#[derive(Debug, Clone, Copy)]
pub struct Plan {
    pub posture: Posture,
    pub focus: Option<Entity>,
    pub focus_pos: Vec2,
    /// Share of this faction's monsters committed to the fort rather than the
    /// player, from 0 to 1.
    pub commitment: f32,
}

impl Default for Plan {
    fn default() -> Self {
        Self {
            posture: Posture::HuntPlayer,
            focus: None,
            focus_pos: Vec2::ZERO,
            commitment: 0.0,
        }
    }
}

/// Every faction's plan, re-decided a few times a minute.
#[derive(Resource, Debug, Default)]
pub struct WarRoom {
    plans: [Plan; Faction::COUNT],
    review: f32,
    /// Human-readable summary for the HUD and for the pilot bridge.
    pub headline: Option<String>,
}

impl WarRoom {
    #[must_use]
    pub fn plan(&self, faction: Faction) -> Plan {
        self.plans[faction.index()]
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// What the director knows about one fort when it is choosing.
#[derive(Debug, Clone, Copy)]
pub struct FortView {
    pub entity: Entity,
    pub pos: Vec2,
    pub owner: Faction,
    /// Bodies of the owning side standing in the ring.
    pub defenders: u32,
    /// How far this fort is from the faction's own strength.
    pub distance: f32,
    /// True when the owner is losing the ring right now.
    pub falling: bool,
}

/// Decide what one faction should do.
///
/// Pulled out as a pure function because it is the only genuinely interesting
/// decision in the system, and because "should they mass or should they hunt"
/// is exactly the kind of thing that is easy to get subtly and permanently
/// wrong without a test saying otherwise.
#[must_use]
pub fn decide(faction: Faction, player_softness: f32, strength: u32, forts: &[FortView]) -> Plan {
    let temperament = faction.temperament();

    // Anything of ours actively being taken outranks every other consideration:
    // losing a fort is worse than missing a chance at one.
    if let Some(home) = forts
        .iter()
        .filter(|f| f.owner == faction && f.falling)
        .min_by(|a, b| a.distance.total_cmp(&b.distance))
    {
        return Plan {
            posture: Posture::Defend,
            focus: Some(home.entity),
            focus_pos: home.pos,
            commitment: (0.55 * temperament.garrison).clamp(0.3, 0.9),
        };
    }

    // Otherwise, score the best fort worth taking. Close, weakly held, and
    // somebody else's.
    let prize = forts
        .iter()
        .filter(|f| f.owner != faction)
        .map(|f| {
            // Nearer is better, thinner garrison is better. Both matter, and a
            // distant undefended fort should still lose to a near one that is
            // merely lightly held.
            let reach = 1.0 / (1.0 + f.distance / 60.0);
            let ease = 1.0 / (1.0 + f.defenders as f32 * 0.6);
            (f, reach * ease)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1));

    let Some((target, score)) = prize else {
        return Plan {
            posture: Posture::HuntPlayer,
            ..Plan::default()
        };
    };

    // Can we actually take it? Ambition scales how optimistic that estimate is.
    let needed = target.defenders as f32 * 1.6 + 3.0;
    let can_take = strength as f32 * temperament.ambition >= needed;

    // A soft player is worth chasing; a player who has been shrugging off
    // everything sent at them is not, however tempting.
    let hunting = player_softness * 1.4;
    let besieging = score * 2.2 * temperament.ambition;

    if !can_take || hunting > besieging * 1.5 {
        Plan {
            posture: Posture::HuntPlayer,
            focus: Some(target.entity),
            focus_pos: target.pos,
            commitment: 0.0,
        }
    } else if besieging > hunting * 1.5 {
        Plan {
            posture: Posture::MassOnFort,
            focus: Some(target.entity),
            focus_pos: target.pos,
            commitment: (0.85 * temperament.ambition).clamp(0.4, 0.95),
        }
    } else {
        // Neither is clearly right, so do both. This is the interesting case
        // and it should be common.
        Plan {
            posture: Posture::Split,
            focus: Some(target.entity),
            focus_pos: target.pos,
            commitment: 0.45,
        }
    }
}

#[derive(Debug)]
pub struct FortPlugin;

impl Plugin for FortPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WarRoom>()
            .add_message::<SpawnFort>()
            .add_message::<SpawnNest>()
            .add_systems(OnExit(AppState::Menu), reset_war.in_set(RunSetup::Reset))
            .add_systems(Update, (place_forts, place_nests))
            .add_systems(
                Update,
                (plan_war, assign_objectives, feud_targets, tick_seeders)
                    .chain()
                    .in_set(GameSet::Think),
            )
            .add_systems(
                Update,
                (capture_forts, tick_forts, tick_nests, reap_nests)
                    .chain()
                    .in_set(GameSet::Resolve),
            )
            .add_systems(Update, holding_visuals.in_set(GameSet::Present));
    }
}

fn reset_war(mut war: ResMut<WarRoom>) {
    war.reset();
}

// -- placement --------------------------------------------------------------

fn place_forts(mut commands: Commands, art: Res<GameArt>, mut requests: MessageReader<SpawnFort>) {
    for req in requests.read() {
        let player_owned = req.faction == Faction::Player;
        commands.spawn((
            Fort {
                progress: if player_owned { 1.0 } else { -1.0 },
                ..default()
            },
            Allegiance(req.faction),
            Body::new(req.pos, FORT_RADIUS),
            Health::new(1.0),
            Mesh3d(art.fort.clone()),
            MeshMaterial3d(art.banner(req.faction)),
            Transform::from_translation(to_world(req.pos, 0.0)),
            crate::fog::FogOccluded::default(),
            RunEntity,
        ));
    }
}

fn place_nests(mut commands: Commands, art: Res<GameArt>, mut requests: MessageReader<SpawnNest>) {
    for req in requests.read() {
        commands.spawn((
            Nest {
                // Staggered so a fort's escort does not pulse together.
                timer: 3.0 + (req.pos.x.abs() % 4.0),
                home: req.home,
            },
            Allegiance(req.faction),
            Body::new(req.pos, NEST_RADIUS),
            // Nests are killed rather than captured, so they need health and
            // they need to be a legal target for both sides.
            Health::new(60.0),
            Damageable {
                hostile_target: req.faction == Faction::Player,
            },
            VisualScale::new(1.0),
            Mesh3d(art.nest.clone()),
            MeshMaterial3d(art.banner(req.faction)),
            Transform::from_translation(to_world(req.pos, 0.0)),
            crate::fog::FogOccluded::default(),
            RunEntity,
        ));
    }
}

// -- capture ----------------------------------------------------------------

/// Presence decides who owns a fort.
///
/// Deliberately not damage: allies have to be able to take one without the
/// player, and holding ground should be about where bodies are standing.
#[allow(clippy::too_many_arguments)]
fn capture_forts(
    time: Res<Time>,
    stats: Res<crate::player::PlayerStats>,
    mut forts: Query<(&mut Fort, &mut Allegiance, &Body)>,
    player: Query<&Body, With<Player>>,
    allies: Query<&Body, (With<crate::allies::Ally>, Without<Fort>)>,
    enemies: Query<(&Body, Option<&Allegiance>), (With<Enemy>, Without<Fort>)>,
    mut economy: ResMut<crate::allies::Economy>,
    mut hints: ResMut<crate::onboarding::HintQueue>,
    mut sfx: MessageWriter<SfxEvent>,
) {
    let dt = time.delta_secs();
    let r_sq = FORT_CAPTURE_RADIUS * FORT_CAPTURE_RADIUS;

    for (mut fort, mut owner, body) in &mut forts {
        let inside = |p: Vec2| p.distance_squared(body.pos) <= r_sq;

        let mut friendly = 0.0f32;
        if player.iter().any(|b| inside(b.pos)) {
            friendly += 1.0;
        }
        friendly += allies.iter().filter(|b| inside(b.pos)).count() as f32 * 0.7;

        // Only monsters loyal to the current owner defend it. A rival faction
        // standing in the ring is not helping anybody hold it.
        let mut defenders = 0.0f32;
        let mut rivals = 0.0f32;
        for (enemy_body, allegiance) in &enemies {
            if !inside(enemy_body.pos) {
                continue;
            }
            let side = allegiance.map_or(Faction::Swarm, |a| a.0);
            if side == owner.0 {
                defenders += 0.34;
            } else {
                rivals += 0.34;
            }
        }

        let toward_player = friendly - defenders;
        fort.contested = friendly > 0.0 && defenders > 0.0;

        if owner.0 == Faction::Player {
            // The player holds it; monsters of any stripe push it back.
            let pressure = defenders.max(rivals);
            let net = friendly - pressure;
            fort.progress = (fort.progress
                + net.signum() * net.abs().min(3.0) * dt / FORT_CAPTURE_SECONDS)
                .clamp(-1.0, 1.0);
            if fort.progress <= -0.999 {
                // Whoever pushed hardest inherits it.
                let taker = enemies
                    .iter()
                    .filter(|(b, _)| inside(b.pos))
                    .find_map(|(_, a)| a.map(|a| a.0))
                    .unwrap_or(Faction::Swarm);
                owner.0 = taker;
                fort.pulse = 1.0;
                hints.push(
                    "FORT LOST",
                    format!("{} took it back.", taker.name()),
                    crate::onboarding::HintTone::Tip,
                );
                sfx.write(SfxEvent::new(crate::audio::Sfx::Lost));
            }
        } else if toward_player.abs() > 0.01 {
            fort.progress = (fort.progress
                + toward_player.signum()
                    * toward_player.abs().min(3.0)
                    * (dt / FORT_CAPTURE_SECONDS)
                    * stats.zone_capture_rate)
                .clamp(-1.0, 1.0);
            if fort.progress >= 0.999 {
                owner.0 = Faction::Player;
                fort.pulse = 1.0;
                fort.assault = 12.0;
                fort.seeding = 20.0;
                economy.gain_cores(3.0);
                hints.push_once(
                    "fort-first",
                    "FORT TAKEN",
                    "It works for you now - and they will come for it.",
                    crate::onboarding::HintTone::Unlock,
                );
                sfx.write(SfxEvent::new(crate::audio::Sfx::Capture));
            }
        }
    }
}

// -- output -----------------------------------------------------------------

/// Forts send assaults and seeders.
#[allow(clippy::too_many_arguments)]
fn tick_forts(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<GameArt>,
    env: Res<EnvKind>,
    threat: Res<Threat>,
    clock: Res<RunClock>,
    progression: Res<crate::progress::Progression>,
    mut rng: ResMut<Rng>,
    mut forts: Query<(Entity, &mut Fort, &Allegiance, &Body)>,
    player: Query<&Body, With<Player>>,
) {
    let dt = time.delta_secs();
    let Some(hero) = player.iter().next().map(|b| b.pos) else {
        return;
    };
    let power = enemy_power(&threat, &clock, progression.level);

    for (entity, mut fort, owner, body) in &mut forts {
        fort.pulse = (fort.pulse - dt * 1.5).max(0.0);
        // The player's forts do not manufacture enemies, and a fort nobody is
        // near does not need simulating.
        if owner.0 == Faction::Player || body.pos.distance(hero) > ASSAULT_RANGE {
            continue;
        }
        let temperament = owner.0.temperament();

        fort.assault -= dt;
        if fort.assault <= 0.0 {
            fort.assault = rng.range(16.0, 26.0) / temperament.garrison.max(0.3);
            let count = 2 + rng.below(3);
            for _ in 0..count {
                let offset = rng.in_disc(FORT_RADIUS + 2.0).truncate();
                let kind = assault_kind(&mut rng, clock.elapsed / 60.0);
                let spawned = spawn_enemy(
                    &mut commands,
                    &art,
                    *env,
                    kind,
                    Rank::Normal,
                    body.pos + offset,
                    power,
                    &mut rng,
                );
                commands.entity(spawned).insert(Allegiance(owner.0));
            }
        }

        fort.seeding -= dt;
        if fort.seeding <= 0.0 {
            fort.seeding = rng.range(22.0, 38.0) / temperament.expansion.max(0.25);
            // A fort will not carpet the world; past a few nests it stops.
            if fort.planted < 4 {
                let angle = rng.range(0.0, std::f32::consts::TAU);
                let target =
                    body.pos + Vec2::new(angle.cos(), angle.sin()) * rng.range(20.0, SEEDER_TRAVEL);
                let spawned = spawn_enemy(
                    &mut commands,
                    &art,
                    *env,
                    EnemyKind::DustBunny,
                    Rank::Normal,
                    body.pos,
                    power * 1.4,
                    &mut rng,
                );
                commands.entity(spawned).insert((
                    Allegiance(owner.0),
                    Seeder {
                        target,
                        patience: 26.0,
                        home: Some(entity),
                    },
                    VisualScale::new(1.35),
                ));
                fort.planted += 1;
            }
        }
    }
}

/// Which archetype a fort throws at you. Widens as the run goes on.
fn assault_kind(rng: &mut Rng, minutes: f32) -> EnemyKind {
    let pool: Vec<EnemyKind> = EnemyKind::ALL
        .iter()
        .copied()
        .filter(|k| !k.is_boss() && k.unlock_minute() <= minutes)
        .collect();
    rng.pick(&pool).copied().unwrap_or(EnemyKind::DustBunny)
}

/// Seeders walk, then settle into a nest.
fn tick_seeders(
    mut commands: Commands,
    time: Res<Time>,
    mut seeders: Query<(Entity, &mut Seeder, &Body, &Allegiance)>,
    mut nests: MessageWriter<SpawnNest>,
    mut bursts: MessageWriter<BurstEvent>,
) {
    let dt = time.delta_secs();
    for (entity, mut seeder, body, owner) in &mut seeders {
        seeder.patience -= dt;
        let arrived = body.pos.distance(seeder.target) < 3.0;
        if !arrived && seeder.patience > 0.0 {
            continue;
        }
        // Settle where it stands. A seeder that cannot reach its spot still
        // plants one, which is what stops an obstructed faction stalling out.
        nests.write(SpawnNest {
            pos: body.pos,
            faction: owner.0,
            home: seeder.home,
        });
        bursts.write(BurstEvent {
            pos: body.pos,
            height: 0.6,
            color: owner.0.color(),
            count: 14,
            speed: 5.0,
            size: 0.14,
        });
        commands.entity(entity).try_insert(Doomed);
    }
}

/// Nests trickle.
#[allow(clippy::too_many_arguments)]
fn tick_nests(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<GameArt>,
    env: Res<EnvKind>,
    threat: Res<Threat>,
    clock: Res<RunClock>,
    progression: Res<crate::progress::Progression>,
    mut rng: ResMut<Rng>,
    mut nests: Query<(&mut Nest, &Allegiance, &Body)>,
    player: Query<&Body, With<Player>>,
) {
    let dt = time.delta_secs();
    let Some(hero) = player.iter().next().map(|b| b.pos) else {
        return;
    };
    let power = enemy_power(&threat, &clock, progression.level);

    for (mut nest, owner, body) in &mut nests {
        if owner.0 == Faction::Player || body.pos.distance(hero) > ASSAULT_RANGE {
            continue;
        }
        nest.timer -= dt;
        if nest.timer > 0.0 {
            continue;
        }
        nest.timer = rng.range(12.0, 19.0) / owner.0.temperament().expansion.max(0.3);
        let kind = assault_kind(&mut rng, clock.elapsed / 60.0);
        let offset = rng.in_disc(2.0).truncate();
        let spawned = spawn_enemy(
            &mut commands,
            &art,
            *env,
            kind,
            Rank::Normal,
            body.pos + offset,
            power,
            &mut rng,
        );
        commands.entity(spawned).insert(Allegiance(owner.0));
    }
}

/// A nest that has been shot to pieces stops being a nest.
fn reap_nests(
    mut commands: Commands,
    nests: Query<(Entity, &Nest, &Health, &Body, &Allegiance)>,
    mut forts: Query<&mut Fort>,
    mut deaths: MessageWriter<DeathEvent>,
    mut bursts: MessageWriter<BurstEvent>,
) {
    for (entity, nest, health, body, owner) in &nests {
        if !health.is_dead() {
            continue;
        }
        if let Some(home) = nest.home
            && let Ok(mut fort) = forts.get_mut(home)
        {
            // Let the fort plant another; otherwise clearing nests would
            // permanently defang it and the map would stop moving.
            fort.planted = fort.planted.saturating_sub(1);
        }
        bursts.write(BurstEvent {
            pos: body.pos,
            height: 0.8,
            color: owner.0.color(),
            count: 24,
            speed: 7.0,
            size: 0.18,
        });
        deaths.write(DeathEvent {
            entity,
            pos: body.pos,
            by_player: true,
        });
        commands.entity(entity).try_insert(Doomed);
    }
}

// -- the director -----------------------------------------------------------

/// Re-decide what each faction is doing.
#[allow(clippy::too_many_arguments)]
fn plan_war(
    time: Res<Time>,
    mut war: ResMut<WarRoom>,
    diplomacy: Res<Diplomacy>,
    player_health: Query<(&Health, &Body), With<Player>>,
    forts: Query<(Entity, &Fort, &Allegiance, &Body)>,
    enemies: Query<(&Body, Option<&Allegiance>), With<Enemy>>,
) {
    war.review -= time.delta_secs();
    if war.review > 0.0 {
        return;
    }
    // Slow on purpose. A director that re-decides every frame produces
    // monsters that pirouette on the spot instead of committing to anything.
    war.review = 4.0;

    let Some((health, hero)) = player_health.iter().next() else {
        return;
    };
    // A player on low health with nothing around them is worth chasing.
    let softness = (1.0 - health.current / health.max.max(1.0)).clamp(0.0, 1.0);

    let mut headline = None;
    for faction in Faction::MONSTERS {
        let strength = enemies
            .iter()
            .filter(|(_, a)| a.is_some_and(|a| a.0 == faction))
            .count() as u32;

        // Where this faction's centre of mass is; distances are judged from
        // there rather than from the player, or a faction would always want
        // whatever the player happens to be standing next to.
        let mut sum = Vec2::ZERO;
        let mut n = 0.0f32;
        for (body, a) in &enemies {
            if a.is_some_and(|a| a.0 == faction) {
                sum += body.pos;
                n += 1.0;
            }
        }
        let anchor = if n > 0.0 { sum / n } else { hero.pos };

        let views: Vec<FortView> = forts
            .iter()
            .filter(|(_, _, owner, _)| {
                // Only forts we are willing to fight over: our own, the
                // player's, and anyone we are actually at war with.
                owner.0 == faction
                    || owner.0 == Faction::Player
                    || diplomacy.hostile(faction, owner.0)
            })
            .map(|(entity, fort, owner, body)| FortView {
                entity,
                pos: body.pos,
                owner: owner.0,
                defenders: enemies
                    .iter()
                    .filter(|(b, a)| {
                        a.is_some_and(|a| a.0 == owner.0)
                            && b.pos.distance(body.pos) < FORT_CAPTURE_RADIUS * 1.5
                    })
                    .count() as u32,
                distance: body.pos.distance(anchor),
                falling: fort.contested && owner.0 == faction,
            })
            .collect();

        let plan = decide(faction, softness, strength, &views);
        if plan.posture == Posture::MassOnFort && war.plans[faction.index()].posture != plan.posture
        {
            headline = Some(format!("{} IS MASSING", faction.name()));
        }
        war.plans[faction.index()] = plan;
    }
    war.headline = headline;
}

/// Hand each monster the objective its faction's plan implies.
fn assign_objectives(
    time: Res<Time>,
    war: Res<WarRoom>,
    mut commands: Commands,
    player: Query<&Body, With<Player>>,
    mut enemies: Query<(Entity, &Enemy, Option<&Allegiance>, Option<&mut Objective>)>,
) {
    let dt = time.delta_secs();
    let Some(hero) = player.iter().next().map(|b| b.pos) else {
        return;
    };

    for (entity, enemy, allegiance, objective) in &mut enemies {
        if let Some(mut current) = objective {
            current.review -= dt;
            if current.review > 0.0 {
                // Keep the destination fresh even while committed.
                if current.kind == ObjectiveKind::HuntPlayer {
                    current.pos = hero;
                }
                continue;
            }
        }

        let faction = allegiance.map_or(Faction::Swarm, |a| a.0);
        let plan = war.plan(faction);

        // Split the horde deterministically by entity, not randomly: a monster
        // that re-rolls its side of the split every few seconds walks in
        // circles between two objectives.
        let share = f32::from(u16::try_from(entity.index().index() % 100).unwrap_or(0)) / 100.0;
        let on_fort = plan.focus.is_some() && share < plan.commitment;

        let next = if on_fort {
            Objective {
                kind: match plan.posture {
                    Posture::Defend => ObjectiveKind::DefendFort,
                    _ => ObjectiveKind::TakeFort,
                },
                pos: plan.focus_pos,
                fort: plan.focus,
                review: 5.0 + share * 3.0,
            }
        } else {
            Objective {
                kind: ObjectiveKind::HuntPlayer,
                pos: hero,
                fort: None,
                review: 4.0 + share * 3.0,
            }
        };

        // Bosses never garrison. They exist to come at the player.
        let next = if enemy.kind.is_boss() {
            Objective {
                kind: ObjectiveKind::HuntPlayer,
                pos: hero,
                fort: None,
                review: 6.0,
            }
        } else {
            next
        };

        commands.entity(entity).try_insert(next);
    }
}

/// Monsters at war with each other actually fight.
///
/// Without this, inciting a war is a line in the HUD and nothing else. A
/// faction at war looks for the nearest enemy of its rival and goes at it
/// instead of the player - which is the entire product the player just bought
/// with their Cores.
fn feud_targets(
    diplomacy: Res<Diplomacy>,
    mut commands: Commands,
    combatants: Query<(Entity, &Body, &Allegiance), With<Enemy>>,
) {
    if diplomacy.active_wars().is_empty() {
        return;
    }
    // Small N: only monsters belonging to a faction that is currently at war
    // with another are considered, and those are the ones the player paid for.
    let at_war: Vec<(Entity, Vec2, Faction)> = combatants
        .iter()
        .filter(|(_, _, a)| {
            Faction::MONSTERS
                .iter()
                .any(|other| diplomacy.hostile(a.0, *other))
        })
        .map(|(e, b, a)| (e, b.pos, a.0))
        .collect();

    for (entity, pos, faction) in &at_war {
        let quarry = at_war
            .iter()
            .filter(|(other, _, side)| other != entity && diplomacy.hostile(*faction, *side))
            .min_by(|a, b| {
                a.1.distance_squared(*pos)
                    .total_cmp(&b.1.distance_squared(*pos))
            });
        if let Some((_, target, _)) = quarry {
            commands.entity(*entity).try_insert(Objective {
                kind: ObjectiveKind::HuntPlayer,
                pos: *target,
                fort: None,
                // Short, so they re-acquire as the brawl moves.
                review: 1.5,
            });
        }
    }
}

/// Banner colours and the capture pulse.
fn holding_visuals(
    art: Res<GameArt>,
    mut forts: Query<
        (&Fort, &Allegiance, &mut MeshMaterial3d<StandardMaterial>),
        Changed<Allegiance>,
    >,
    mut nests: Query<
        (&Allegiance, &mut MeshMaterial3d<StandardMaterial>),
        (With<Nest>, Changed<Allegiance>, Without<Fort>),
    >,
) {
    for (_fort, owner, mut material) in &mut forts {
        material.0 = art.banner(owner.0);
    }
    for (owner, mut material) in &mut nests {
        material.0 = art.banner(owner.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(id: u32, owner: Faction, defenders: u32, distance: f32, falling: bool) -> FortView {
        FortView {
            entity: Entity::from_raw_u32(id).unwrap(),
            pos: Vec2::new(distance, 0.0),
            owner,
            defenders,
            distance,
            falling,
        }
    }

    #[test]
    fn a_faction_with_nothing_to_take_hunts_the_player() {
        let plan = decide(Faction::Swarm, 0.2, 10, &[]);
        assert_eq!(plan.posture, Posture::HuntPlayer);
        assert_eq!(plan.commitment, 0.0);
    }

    #[test]
    fn losing_a_fort_outranks_taking_one() {
        // A tempting undefended prize next door does not excuse letting your
        // own fort fall.
        let forts = [
            view(1, Faction::Swarm, 0, 8.0, true),
            view(2, Faction::Player, 0, 12.0, false),
        ];
        let plan = decide(Faction::Swarm, 0.9, 20, &forts);
        assert_eq!(plan.posture, Posture::Defend);
        assert_eq!(plan.focus, Some(Entity::from_raw_u32(1).unwrap()));
        assert!(plan.commitment > 0.25);
    }

    #[test]
    fn a_faction_masses_on_a_weakly_held_fort_it_can_take() {
        let forts = [view(1, Faction::Player, 0, 10.0, false)];
        let plan = decide(Faction::Void, 0.0, 30, &forts);
        assert_eq!(plan.posture, Posture::MassOnFort);
        assert!(plan.commitment > 0.4);
    }

    #[test]
    fn a_faction_too_weak_to_take_a_fort_goes_back_to_hunting() {
        // Throwing three monsters at a garrison of twenty is not strategy.
        let forts = [view(1, Faction::Player, 20, 10.0, false)];
        let plan = decide(Faction::Swarm, 0.3, 3, &forts);
        assert_eq!(plan.posture, Posture::HuntPlayer);
    }

    #[test]
    fn a_dying_player_is_worth_chasing_over_a_distant_fort() {
        let forts = [view(1, Faction::Player, 2, 260.0, false)];
        let plan = decide(Faction::Swarm, 1.0, 40, &forts);
        assert_eq!(plan.posture, Posture::HuntPlayer);
    }

    #[test]
    fn a_healthy_player_next_to_a_soft_fort_gets_ignored() {
        let forts = [view(1, Faction::Player, 0, 6.0, false)];
        let plan = decide(Faction::Void, 0.0, 40, &forts);
        assert_ne!(plan.posture, Posture::HuntPlayer);
    }

    #[test]
    fn the_nearer_of_two_equal_prizes_wins() {
        let forts = [
            view(1, Faction::Player, 1, 150.0, false),
            view(2, Faction::Player, 1, 20.0, false),
        ];
        let plan = decide(Faction::Void, 0.1, 40, &forts);
        assert_eq!(plan.focus, Some(Entity::from_raw_u32(2).unwrap()));
    }

    #[test]
    fn a_thinner_garrison_beats_a_slightly_closer_fort() {
        let forts = [
            view(1, Faction::Player, 12, 30.0, false),
            view(2, Faction::Player, 0, 45.0, false),
        ];
        let plan = decide(Faction::Void, 0.1, 40, &forts);
        assert_eq!(plan.focus, Some(Entity::from_raw_u32(2).unwrap()));
    }

    #[test]
    fn a_faction_never_besieges_its_own_fort() {
        let forts = [view(1, Faction::Swarm, 0, 5.0, false)];
        let plan = decide(Faction::Swarm, 0.1, 40, &forts);
        assert_eq!(plan.posture, Posture::HuntPlayer, "{plan:?}");
    }

    #[test]
    fn temperament_changes_what_a_faction_does_with_the_same_board() {
        // The whole point of four factions: identical circumstances, different
        // answers.
        let forts = [view(1, Faction::Player, 4, 40.0, false)];
        let ambitious = decide(Faction::Void, 0.35, 18, &forts);
        let cautious = decide(Faction::Bloom, 0.35, 18, &forts);
        assert_ne!(
            (ambitious.posture, cautious.posture),
            (Posture::HuntPlayer, Posture::HuntPlayer),
            "neither faction reacted to the fort at all"
        );
        assert_ne!(
            ambitious.posture, cautious.posture,
            "Void and Bloom made the same call on the same board"
        );
    }

    #[test]
    fn commitment_never_sends_everyone_or_nobody() {
        // All-in leaves the player unopposed; none-in means the plan is a lie.
        let boards = [
            vec![view(1, Faction::Player, 0, 10.0, false)],
            vec![view(1, Faction::Swarm, 1, 10.0, true)],
            vec![view(1, Faction::Player, 3, 55.0, false)],
        ];
        for faction in Faction::MONSTERS {
            for board in &boards {
                let plan = decide(faction, 0.4, 25, board);
                assert!(
                    (0.0..=0.95).contains(&plan.commitment),
                    "{faction:?} committed {}",
                    plan.commitment
                );
                if plan.posture != Posture::HuntPlayer {
                    assert!(plan.commitment > 0.0, "{faction:?} planned nothing");
                }
            }
        }
    }

    #[test]
    fn a_split_leaves_forces_on_both_objectives() {
        // The interesting middle case has to actually be reachable.
        let mut split_seen = false;
        for defenders in 0..8 {
            for softness in [0.2, 0.4, 0.6] {
                let forts = [view(1, Faction::Player, defenders, 45.0, false)];
                let plan = decide(Faction::Swarm, softness, 22, &forts);
                if plan.posture == Posture::Split {
                    split_seen = true;
                    assert!(plan.commitment > 0.1 && plan.commitment < 0.9);
                }
            }
        }
        assert!(split_seen, "no board produces a split; the case is dead");
    }

    #[test]
    fn a_fort_starts_in_hostile_hands_and_a_players_starts_in_theirs() {
        let hostile = Fort::default();
        assert!(hostile.progress < 0.0);
        assert_eq!(hostile.planted, 0);
        assert!(hostile.assault > 0.0 && hostile.seeding > 0.0);
    }
}
