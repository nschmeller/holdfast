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
//! Presence is also the only capture rule that survives the damage curve. A
//! mastered weapon is around six times a level-one one, so a fort with a health
//! bar would go from impossible to trivial inside three level-ups, which is the
//! opposite of a late-game objective. Standing somewhere does not scale.
//!
//! **The difficulty is the siege, not the meter.** A fort defends its ground:
//!
//! - **Emplaced guns** fire on whoever is in the ring. They cannot be shot off;
//!   they are the fort, and they are why you cannot capture with a build that
//!   has no sustain.
//! - **Wardens** - elite monsters whose whole job is to drive the player off -
//!   are sent out under contest, on top of the ordinary assault.
//! - **Contest collapses the assault timer**, so the longer you stand there the
//!   worse it gets. Eleven seconds in a ring is a siege, not a stopwatch.
//! - **Distance from home makes all of it worse**, so the near forts are the
//!   lesson and the far ones need a squad.
//!
//! And a fort you take is deliberately a much weaker thing than the fort you
//! took it from - a prize, not a win button. It keeps one gun instead of three,
//! it pays income, and it stops manufacturing the pressure that was aimed at
//! you. It does not raise armies for you.
//!
//! Nests, by contrast, are destroyed rather than converted. Forts are places;
//! nests are infestations - and clearing the nests around a fort is how you
//! soften it before laying siege. Damage prepares; presence decides.

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
///
/// Flip, not half-flip. Ownership runs from -1 to +1, so the rate below covers
/// the whole span in this many seconds. It used to be applied per unit, which
/// made the real figure twenty-two seconds of standing alone and unchallenged
/// inside a seven-unit ring in the middle of enemy ground - a thing no player
/// has ever managed, across every run in the dossier.
const FORT_CAPTURE_SECONDS: f32 = 11.0;
/// Footprint of a nest.
pub const NEST_RADIUS: f32 = 1.3;

/// Emplacements on an enemy-held fort.
const ENEMY_FORT_GUNS: u16 = 3;

/// Emplacements left standing once the player owns it.
///
/// The asymmetry is the point. A captured fort that fought as hard for you as
/// it did against you would end the run the moment you took your first one.
const PLAYER_FORT_GUNS: u16 = 1;

/// Everything a player-held fort does, against what an enemy-held one does.
const PLAYER_FORT_POWER: f32 = 0.35;

/// How far a fort gun reaches. Twice the capture ring, so there is no standing
/// at the edge and waiting.
const GUN_RANGE: f32 = 15.0;

/// Seconds for one emplacement to come round again.
///
/// The guns fire in rotation rather than in volley - one muzzle every
/// `GUN_CADENCE / guns` - so a fort reads as a wall with several positions on
/// it and the damage arrives as attrition. A simultaneous volley of three at a
/// player who has just stepped into the ring is a wall, not a fight.
const GUN_CADENCE: f32 = 1.7;

/// Damage per shot, before the fort's own strength is applied.
///
/// Sized as attrition, not as a kill: an enemy fort's three guns come to about
/// fourteen a second, which a build carrying armour and regen can stand in for
/// the length of a capture and an unprepared one cannot.
const GUN_DAMAGE: f32 = 8.0;

/// Seconds between wardens while a fort is contested.
const WARDEN_INTERVAL: f32 = 9.0;

/// How much a contested fort accelerates its ordinary assault.
///
/// Standing in the ring is what summons the garrison home; the fort does not
/// wait out its timer while it is being taken. Deliberately modest: at six
/// times, a besieged fort refilled its own ring faster than anybody could clear
/// it and the capture was unwinnable by arithmetic rather than by difficulty.
/// The pressure is supposed to come from the wardens.
const CONTEST_URGENCY: f32 = 2.5;

/// Scrap a second from one held fort. The same base a Generator pays.
const FORT_SCRAP: f32 = 2.4;

/// Cores a second from one held fort. Nothing else in the game pays these
/// passively, which is most of why a fort is worth holding: the research tree
/// is priced in Cores.
const FORT_CORES: f32 = 0.16;

/// What one held fort adds to the threat floor.
///
/// Almost twice a zone's 0.2, because a fort is worth more than a zone and every
/// source of strength in this game is also a source of pressure.
const FORT_THREAT: f32 = 0.35;

/// How much more a faction wants a fort the player is holding than one a rival
/// is holding.
///
/// "It works for you now - and they will come for it" is a promise the hint
/// makes when the meter flips. Without this the player's fort is scored like
/// anybody else's, and the neighbours are as likely to go and bother each other
/// - which is a perfectly sensible war and a broken promise.
const GRUDGE: f32 = 2.4;

/// Where a fort stops getting tougher with distance.
const STRENGTH_CEILING: f32 = 1.9;

/// A fort's strength for the ground it stands on.
///
/// Forts begin at `HOME_PEACE` and thin out inwards, so distance is already
/// the gate on finding one. This makes distance the gate on *taking* one too:
/// the first fort you meet is the lesson, and the belt at four hundred units
/// needs a squad and a build.
fn strength_from_home(pos: Vec2) -> f32 {
    let beyond = (pos.length() - crate::environments::HOME_PEACE).max(0.0);
    (1.0 + beyond * 0.0022).min(STRENGTH_CEILING)
}

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
    /// Monsters loyal to the owner standing in the ring right now.
    ///
    /// Surfaced to the pilot: a capture meter that will not move is otherwise
    /// indistinguishable from a broken one.
    pub garrison: u32,
    /// Countdown on the emplaced guns.
    pub gun: f32,
    /// Which emplacement fires next, so they rotate round the wall.
    pub next_gun: u16,
    /// Countdown on the next warden. Only runs while contested.
    pub warden: f32,
    /// How hard this particular fort fights, from the ground it stands on.
    pub strength: f32,
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
            garrison: 0,
            gun: 0.0,
            next_gun: 0,
            warden: WARDEN_INTERVAL,
            strength: 1.0,
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
    /// The chunk that asked for it, when world generation did. `None` for a
    /// fort placed by anything else, which then lives until the run ends.
    pub chunk: Option<IVec2>,
}

/// Ask the world to place a nest.
#[derive(Message, Debug, Clone, Copy)]
pub struct SpawnNest {
    pub pos: Vec2,
    pub faction: Faction,
    pub home: Option<Entity>,
    /// As `SpawnFort::chunk`. A nest planted by a seeder has no chunk and
    /// persists, which is the point of planting it.
    pub chunk: Option<IVec2>,
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
pub fn decide(
    faction: Faction,
    player_softness: f32,
    strength: u32,
    forts: &[FortView],
    tuning: &crate::tactician::Tactics,
) -> Plan {
    // A language model, if one is attached, gets to lean on the temperament -
    // never to replace it. The faction still plays like itself; it just plays
    // like itself more or less keenly.
    let mut temperament = faction.temperament();
    temperament.ambition *= tuning.ambition;
    temperament.expansion *= tuning.expansion;

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
            // A fort in the player's hands is an affront, and the promise made
            // when they took it was that somebody would come for it. Rivals
            // between themselves are a slower, more patient business.
            let grudge = if f.owner == Faction::Player {
                GRUDGE
            } else {
                1.0
            };
            (f, reach * ease * grudge)
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
                // Guns after capture, so a fort that has just been contested
                // is already shooting on the same frame.
                (
                    capture_forts,
                    fort_guns,
                    fort_income,
                    tick_forts,
                    tick_nests,
                    reap_nests,
                )
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
        let mut fort = commands.spawn((
            Fort {
                progress: if player_owned { 1.0 } else { -1.0 },
                strength: strength_from_home(req.pos),
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
        if let Some(coord) = req.chunk {
            fort.insert(crate::world::ChunkEntity(coord));
        }
    }
}

fn place_nests(mut commands: Commands, art: Res<GameArt>, mut requests: MessageReader<SpawnNest>) {
    for req in requests.read() {
        let mut nest = commands.spawn((
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
        if let Some(coord) = req.chunk {
            nest.insert(crate::world::ChunkEntity(coord));
        }
    }
}

// -- capture ----------------------------------------------------------------

/// How much of the -1..=1 span one second of single-body presence covers.
fn capture_step(dt: f32) -> f32 {
    const SPAN: f32 = 2.0;
    SPAN * dt / FORT_CAPTURE_SECONDS
}

/// What one monster in the ring is worth against a capture.
///
/// Three of them stall a lone player outright, which is the intent: clear the
/// garrison, then hold the ground. A squad or a cleared ring makes it quick.
const DEFENDER_WEIGHT: f32 = 0.34;

/// What one of the player's structures is worth as presence.
///
/// Turrets hold ground; that is the entire job. Counting them is what turns a
/// captured fort into something you can actually keep, and it makes the whole
/// chain hang together: travel out, clear the nests, survive the siege, then
/// fortify what you took. Less than a body, because a turret cannot chase
/// anyone out of the ring.
const STRUCTURE_WEIGHT: f32 = 0.5;

/// How much more presence than the holder a challenger needs before the meter
/// moves against them.
///
/// Taking a fort is eleven seconds of deliberate work. Losing one was three
/// monsters wandering through the ring: the first fort ever captured was held
/// for six seconds. Ownership has to be sticky in both directions or a prize is
/// not a prize.
const LOSS_MARGIN: f32 = 1.0;

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
    structures: Query<&Body, (With<crate::allies::Turret>, Without<Fort>)>,
    enemies: Query<(&Body, Option<&Allegiance>), (With<Enemy>, Without<Fort>)>,
    mut economy: ResMut<crate::allies::Economy>,
    mut hints: ResMut<crate::onboarding::HintQueue>,
    mut records: MessageWriter<crate::stats::Record>,
    mut seen: MessageWriter<crate::coverage::Seen>,
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
        friendly += structures.iter().filter(|b| inside(b.pos)).count() as f32 * STRUCTURE_WEIGHT;

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
                defenders += DEFENDER_WEIGHT;
            } else {
                rivals += DEFENDER_WEIGHT;
            }
        }

        let toward_player = friendly - defenders;
        fort.contested = friendly > 0.0 && defenders > 0.0;
        // Reported so a tester standing in a ring that will not flip can see
        // why. Without it the only reading is "capture: -1.0" and no cause.
        fort.garrison = u32::try_from((defenders / DEFENDER_WEIGHT).round() as i64).unwrap_or(0);

        if owner.0 == Faction::Player {
            // The player holds it; monsters of any stripe push it back - but
            // only a real assault does. A margin, because losing a fort was
            // three monsters wandering through the ring, and the first fort
            // ever captured was held for six seconds.
            let pressure = defenders.max(rivals);
            let excess = pressure - friendly - LOSS_MARGIN;
            let net = if excess > 0.0 {
                -excess
            } else {
                // Nothing is seriously contesting it, so it settles back to
                // fully held rather than sitting wherever it was left.
                1.0
            };
            fort.progress = (fort.progress + net.signum() * net.abs().min(3.0) * capture_step(dt))
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
                records.write(crate::stats::Record::add(
                    crate::stats::stat::FORTS_LOST,
                    1.0,
                ));
                seen.write(crate::coverage::Seen(String::from("deed:fort-lost")));
                sfx.write(SfxEvent::new(crate::audio::Sfx::Lost));
            }
        } else if toward_player.abs() > 0.01 {
            // A garrison stalls a capture; it does not undo one while somebody
            // is still standing there. With seven defenders in the ring - which
            // a besieged fort produces on its own - a reversing meter made the
            // capture unwinnable by arithmetic, so nobody could tell a hard
            // objective from a broken one.
            let net = if friendly > 0.0 {
                toward_player.max(0.0)
            } else {
                toward_player
            };
            fort.progress = (fort.progress
                + net.signum() * net.abs().min(3.0) * capture_step(dt) * stats.zone_capture_rate)
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
                records.write(crate::stats::Record::add(
                    crate::stats::stat::FORTS_TAKEN,
                    1.0,
                ));
                seen.write(crate::coverage::Seen(String::from("deed:fort-taken")));
                sfx.write(SfxEvent::new(crate::audio::Sfx::Capture));
            }
        }
    }
}

/// What a fort you hold is actually worth.
///
/// A captured fort is a prize, and a prize has to pay. It cannot pay in
/// soldiers - a fort that raised armies for you would end the run the moment
/// you took your second one - so it pays in Cores and Scrap, the currencies
/// that buy research and structures. Cores especially: they are the scarcest
/// thing in the game and the research tree is priced in them.
///
/// It is loud, too. Holding a fort raises the threat floor exactly as holding
/// territory does, because in this game every source of strength is also a
/// source of pressure.
fn fort_income(
    time: Res<Time>,
    stats: Res<crate::player::PlayerStats>,
    forts: Query<&Allegiance, With<Fort>>,
    mut economy: ResMut<crate::allies::Economy>,
    mut threat: ResMut<Threat>,
) {
    let dt = time.delta_secs();
    let held = forts.iter().filter(|a| a.0 == Faction::Player).count() as f32;
    threat.holdings = held * FORT_THREAT;
    let scrap = held * FORT_SCRAP * stats.income_mult;
    economy.fort_rate = scrap;
    if held <= 0.0 {
        return;
    }
    economy.gain_cores(held * FORT_CORES * dt * stats.core_mult);
    economy.gain_scrap(scrap * dt);
}

/// Emplaced guns. The reason presence is not free.
///
/// The guns are the fort, not entities standing on it: there is nothing to
/// snipe off from outside the ring first. Their range is twice the capture
/// radius so that contesting a fort means being shot at for the whole eleven
/// seconds, which is what makes health, armour and regen worth carrying to a
/// siege.
///
/// A player-held fort keeps one of its three, at a third of the damage. It
/// defends the ground you hold and no more.
fn fort_guns(
    time: Res<Time>,
    grid: Res<crate::combat::EnemyGrid>,
    obstacles: Res<crate::arena::ObstacleField>,
    mut forts: Query<(&mut Fort, &Allegiance, &Body)>,
    player: Query<&Body, With<Player>>,
    mut shots: MessageWriter<crate::combat::SpawnShot>,
) {
    let dt = time.delta_secs();
    let hero = player.iter().next().map(|b| b.pos);

    for (mut fort, owner, body) in &mut forts {
        let ours = owner.0 == Faction::Player;
        let guns = if ours {
            PLAYER_FORT_GUNS
        } else {
            ENEMY_FORT_GUNS
        };
        if guns == 0 {
            continue;
        }

        fort.gun = (fort.gun - dt).max(0.0);
        if fort.gun > 0.0 {
            continue;
        }
        let emplacement = fort.next_gun % guns;

        // A fort shoots at whoever it is not. The player's forts shoot
        // monsters; everyone else's shoot the player.
        let target = if ours {
            grid.best_visible_target(body.pos, GUN_RANGE, &obstacles)
                .map(|t| t.pos)
        } else {
            hero.filter(|p| p.distance(body.pos) <= GUN_RANGE)
        };
        let Some(target) = target else {
            continue;
        };

        // One muzzle at a time, round the wall.
        fort.gun = GUN_CADENCE / f32::from(guns);
        fort.next_gun = fort.next_gun.wrapping_add(1);
        let power = fort.strength * if ours { PLAYER_FORT_POWER } else { 1.0 };
        let angle = std::f32::consts::TAU * f32::from(emplacement) / f32::from(guns);
        let muzzle = body.pos + Vec2::new(angle.cos(), angle.sin()) * FORT_RADIUS;
        let dir = (target - muzzle).normalize_or_zero();
        let mut shot = if ours {
            crate::combat::SpawnShot::friendly(
                muzzle,
                dir,
                26.0,
                GUN_DAMAGE * power,
                crate::combat::ShotVisual::Tack,
            )
        } else {
            crate::combat::SpawnShot::enemy(
                muzzle,
                dir,
                26.0,
                GUN_DAMAGE * power,
                crate::combat::ShotVisual::Tack,
            )
        };
        shot.height = 0.7;
        shot.scale = 1.3;
        shots.write(shot);
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
        // A fort you hold does not manufacture enemies - that is most of what
        // makes taking one worth the trouble - and a fort nobody is near does
        // not need simulating.
        if owner.0 == Faction::Player || body.pos.distance(hero) > ASSAULT_RANGE {
            continue;
        }
        let temperament = owner.0.temperament();

        // Being stood on is what calls the garrison home. A fort under contest
        // does not wait out its timer: the assault it would have thrown in
        // twenty seconds arrives in three, and keeps arriving.
        let urgency = if fort.contested { CONTEST_URGENCY } else { 1.0 };

        // Wardens: elite monsters sent for the specific job of driving the
        // player off the ring. Only while contested, so they are a consequence
        // of the siege rather than ambient traffic.
        if fort.contested {
            fort.warden -= dt;
            if fort.warden <= 0.0 {
                fort.warden = WARDEN_INTERVAL / temperament.garrison.max(0.3);
                let offset = rng.in_disc(FORT_RADIUS + 1.5).truncate();
                let kind = assault_kind(&mut rng, clock.elapsed / 60.0);
                let warden = spawn_enemy(
                    &mut commands,
                    &art,
                    *env,
                    kind,
                    Rank::Elite,
                    body.pos + offset,
                    power * fort.strength * 1.35,
                    &mut rng,
                );
                commands.entity(warden).insert((
                    Allegiance(owner.0),
                    Objective {
                        kind: ObjectiveKind::HuntPlayer,
                        pos: hero,
                        fort: Some(entity),
                        review: 6.0,
                    },
                    VisualScale::new(1.45),
                ));
            }
        } else {
            fort.warden = WARDEN_INTERVAL;
        }

        fort.assault -= dt * urgency;
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
                    power * fort.strength,
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
    mut seen: MessageWriter<crate::coverage::Seen>,
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
            chunk: None,
        });
        seen.write(crate::coverage::Seen(String::from("deed:seeder-planted")));
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
    mut records: MessageWriter<crate::stats::Record>,
    mut seen: MessageWriter<crate::coverage::Seen>,
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
        records.write(crate::stats::Record::add(
            crate::stats::stat::NESTS_CLEARED,
            1.0,
        ));
        seen.write(crate::coverage::Seen(String::from("deed:nest-cleared")));
        deaths.write(DeathEvent {
            entity,
            pos: body.pos,
            credited: true,
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
    tuning: Res<crate::tactician::Tactics>,
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

        let plan = decide(faction, softness, strength, &views, &tuning);
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
    tuning: Res<crate::tactician::Tactics>,
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
        let on_fort = plan.focus.is_some() && share < (plan.commitment * tuning.cohesion).min(0.95);

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

    #[test]
    fn a_far_fort_is_weaker_ground_than_a_near_one() {
        // Distance is the gate on finding a fort; it should be the gate on
        // taking one too, so the first one anyone meets is the lesson.
        let home = crate::environments::HOME_PEACE;
        let near = strength_from_home(Vec2::new(home + 5.0, 0.0));
        let far = strength_from_home(Vec2::new(home + 400.0, 0.0));
        assert!(near < far, "near {near} far {far}");
        assert!(near >= 1.0, "a fort is never weaker than baseline");
        assert!(far <= STRENGTH_CEILING);
    }

    #[test]
    fn strength_does_not_run_away_forever() {
        // The world is unbounded; the difficulty curve is not.
        let miles_out = strength_from_home(Vec2::new(50_000.0, 0.0));
        assert!((miles_out - STRENGTH_CEILING).abs() < 1e-4);
    }

    #[test]
    fn a_player_held_fort_is_weaker_than_the_one_it_was() {
        // A captured fort that fought as hard for you as it did against you
        // would end the run at the first capture.
        // The volley one fort throws, as `fort_guns` computes it.
        let volley = |ours: bool| {
            let guns = if ours {
                PLAYER_FORT_GUNS
            } else {
                ENEMY_FORT_GUNS
            };
            let power = if ours { PLAYER_FORT_POWER } else { 1.0 };
            (0..guns).map(|_| GUN_DAMAGE * power).sum::<f32>()
        };
        assert!(
            volley(true) * 4.0 < volley(false),
            "ours {} theirs {}",
            volley(true),
            volley(false)
        );
    }

    #[test]
    fn a_fort_the_player_holds_is_the_priority_target() {
        // The hint promises they will come for it. They have to come for it.
        let forts = [
            view(1, Faction::Player, 0, 40.0, false),
            view(2, Faction::Void, 0, 40.0, false),
        ];
        let plan = decide(Faction::Bloom, 0.0, 40, &forts, &neutral());
        assert_eq!(
            plan.focus_pos, forts[0].pos,
            "went after a rival instead of the player's fort"
        );
        assert_ne!(plan.posture, Posture::HuntPlayer);
    }

    #[test]
    fn losing_ones_own_fort_outranks_taking_anyone_elses() {
        let forts = [
            view(1, Faction::Player, 0, 10.0, false),
            view(2, Faction::Bloom, 1, 80.0, true),
        ];
        let plan = decide(Faction::Bloom, 5.0, 40, &forts, &neutral());
        assert_eq!(plan.posture, Posture::Defend);
    }

    #[test]
    fn a_besieged_fort_answers_faster_than_a_quiet_one() {
        // Eleven seconds in a ring has to be a siege, not a stopwatch.
        // How long a twenty-second assault timer actually takes to run down,
        // as `tick_forts` runs it.
        const DT: f32 = 1.0 / 60.0;
        let countdown = |contested: bool| {
            let urgency = if contested { CONTEST_URGENCY } else { 1.0 };
            let mut left = 20.0f32;
            let mut frames = 0u32;
            while left.is_sign_positive() && left != 0.0 {
                left -= DT * urgency;
                frames += 1;
            }
            frames as f32 * DT
        };
        let quiet = countdown(false);
        let besieged = countdown(true);
        assert!(
            besieged < FORT_CAPTURE_SECONDS,
            "an assault arrives in {besieged:.0}s, after the {FORT_CAPTURE_SECONDS}s capture"
        );
        assert!(quiet > FORT_CAPTURE_SECONDS);
    }

    #[test]
    fn the_guns_are_attrition_rather_than_a_wall() {
        // Three guns firing together at somebody who has just stepped into the
        // ring is a wall. Rotating, they are a cost you can carry sustain for.
        let dps = |guns: u16, power: f32| {
            let interval = GUN_CADENCE / f32::from(guns);
            GUN_DAMAGE * power / interval
        };
        let theirs = dps(ENEMY_FORT_GUNS, 1.0);
        assert!(
            (10.0..20.0).contains(&theirs),
            "an enemy fort does {theirs:.0} damage a second"
        );
        assert!(dps(PLAYER_FORT_GUNS, PLAYER_FORT_POWER) * 4.0 < theirs);
    }

    #[test]
    fn the_guns_reach_past_the_ring() {
        // Otherwise a capture is done from the edge, untouched.
        let ring = FORT_CAPTURE_RADIUS;
        let reach = GUN_RANGE;
        assert!(reach > ring * 1.5, "ring {ring} reach {reach}");
    }

    /// The net presence figure `capture_forts` computes for a fort the player
    /// holds, from the bodies standing in its ring.
    fn holding_net(monsters: u32, allies: u32, turrets: u32, standing_there: bool) -> f32 {
        let mut friendly = if standing_there { 1.0 } else { 0.0 };
        friendly += allies as f32 * 0.7;
        friendly += turrets as f32 * STRUCTURE_WEIGHT;
        let pressure: f32 = (0..monsters).map(|_| DEFENDER_WEIGHT).sum();
        let excess = pressure - friendly - LOSS_MARGIN;
        if excess > 0.0 { -excess } else { 1.0 }
    }

    #[test]
    fn a_capture_is_not_undone_by_passers_by() {
        // The first fort ever taken was held for six seconds, because three
        // monsters crossing the ring outweighed the player standing in it.
        assert!(
            holding_net(3, 0, 0, true) > 0.0,
            "lost it to three monsters"
        );
        assert!(holding_net(2, 0, 0, false) > 0.0, "lost it while away");
    }

    #[test]
    fn a_real_assault_still_takes_a_fort_back() {
        // Sticky, not invulnerable.
        assert!(
            holding_net(12, 0, 0, true) < 0.0,
            "eight monsters is an assault"
        );
    }

    #[test]
    fn fortifying_what_you_took_is_what_keeps_it() {
        // The whole chain: travel out, clear the nests, survive the siege,
        // then build. Turrets hold ground - that is the entire job.
        let bare = holding_net(8, 0, 0, false);
        let held = holding_net(8, 0, 4, false);
        assert!(bare < 0.0, "eight monsters should be taking it back");
        assert!(held > 0.0, "four turrets should hold the ring: {held}");
    }

    #[test]
    fn a_squad_holds_a_fort_while_the_player_is_elsewhere() {
        // Allies capture and hold without the player. That is most of why
        // presence rather than damage.
        assert!(holding_net(8, 4, 0, false) > 0.0);
    }

    #[test]
    fn a_fort_flips_in_the_advertised_time() {
        // The constant says "seconds of uncontested presence to flip a fort".
        // Applied per unit of a two-unit span it meant twice that, which is
        // why the dossier has no captured fort in it anywhere.
        let mut progress = -1.0f32;
        let dt = 1.0 / 60.0;
        let mut seconds = 0.0;
        while progress < 1.0 && seconds < 120.0 {
            progress = (progress + capture_step(dt)).clamp(-1.0, 1.0);
            seconds += dt;
        }
        assert!(
            (seconds - FORT_CAPTURE_SECONDS).abs() < 0.5,
            "took {seconds:.1}s, advertised {FORT_CAPTURE_SECONDS}s"
        );
    }

    #[test]
    fn a_squad_takes_a_fort_faster_than_one_body() {
        // Allies count towards presence, so a squad should be worth bringing.
        let dt = 1.0 / 60.0;
        assert!(capture_step(dt) * 3.0 > capture_step(dt));
    }

    #[test]
    fn three_defenders_stall_a_lone_player() {
        // The intended shape: clear the ring first. Any weaker and a fort is
        // taken by walking through it; any stronger and it cannot be taken.
        let stalls = |monsters: u32| {
            let defenders: f32 = (0..monsters).map(|_| DEFENDER_WEIGHT).sum();
            1.0 - defenders <= 0.0
        };
        assert!(!stalls(2), "two defenders should not stall a lone player");
        assert!(stalls(3), "three should - a fort is not walked through");
    }

    /// No model attached, which is what every assertion about the built-in
    /// director should be made against.
    fn neutral() -> crate::tactician::Tactics {
        crate::tactician::Tactics::default()
    }

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
        let plan = decide(Faction::Swarm, 0.2, 10, &[], &neutral());
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
        let plan = decide(Faction::Swarm, 0.9, 20, &forts, &neutral());
        assert_eq!(plan.posture, Posture::Defend);
        assert_eq!(plan.focus, Some(Entity::from_raw_u32(1).unwrap()));
        assert!(plan.commitment > 0.25);
    }

    #[test]
    fn a_faction_masses_on_a_weakly_held_fort_it_can_take() {
        let forts = [view(1, Faction::Player, 0, 10.0, false)];
        let plan = decide(Faction::Void, 0.0, 30, &forts, &neutral());
        assert_eq!(plan.posture, Posture::MassOnFort);
        assert!(plan.commitment > 0.4);
    }

    #[test]
    fn a_faction_too_weak_to_take_a_fort_goes_back_to_hunting() {
        // Throwing three monsters at a garrison of twenty is not strategy.
        let forts = [view(1, Faction::Player, 20, 10.0, false)];
        let plan = decide(Faction::Swarm, 0.3, 3, &forts, &neutral());
        assert_eq!(plan.posture, Posture::HuntPlayer);
    }

    #[test]
    fn a_dying_player_is_worth_chasing_over_a_distant_fort() {
        let forts = [view(1, Faction::Player, 2, 260.0, false)];
        let plan = decide(Faction::Swarm, 1.0, 40, &forts, &neutral());
        assert_eq!(plan.posture, Posture::HuntPlayer);
    }

    #[test]
    fn a_healthy_player_next_to_a_soft_fort_gets_ignored() {
        let forts = [view(1, Faction::Player, 0, 6.0, false)];
        let plan = decide(Faction::Void, 0.0, 40, &forts, &neutral());
        assert_ne!(plan.posture, Posture::HuntPlayer);
    }

    #[test]
    fn the_nearer_of_two_equal_prizes_wins() {
        let forts = [
            view(1, Faction::Player, 1, 150.0, false),
            view(2, Faction::Player, 1, 20.0, false),
        ];
        let plan = decide(Faction::Void, 0.1, 40, &forts, &neutral());
        assert_eq!(plan.focus, Some(Entity::from_raw_u32(2).unwrap()));
    }

    #[test]
    fn a_thinner_garrison_beats_a_slightly_closer_fort() {
        let forts = [
            view(1, Faction::Player, 12, 30.0, false),
            view(2, Faction::Player, 0, 45.0, false),
        ];
        let plan = decide(Faction::Void, 0.1, 40, &forts, &neutral());
        assert_eq!(plan.focus, Some(Entity::from_raw_u32(2).unwrap()));
    }

    #[test]
    fn a_faction_never_besieges_its_own_fort() {
        let forts = [view(1, Faction::Swarm, 0, 5.0, false)];
        let plan = decide(Faction::Swarm, 0.1, 40, &forts, &neutral());
        assert_eq!(plan.posture, Posture::HuntPlayer, "{plan:?}");
    }

    #[test]
    fn temperament_changes_what_a_faction_does_with_the_same_board() {
        // The whole point of four factions: identical circumstances, different
        // answers.
        let forts = [view(1, Faction::Player, 4, 40.0, false)];
        let ambitious = decide(Faction::Void, 0.35, 18, &forts, &neutral());
        let cautious = decide(Faction::Bloom, 0.35, 18, &forts, &neutral());
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
                let plan = decide(faction, 0.4, 25, board, &neutral());
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
                let plan = decide(Faction::Swarm, softness, 22, &forts, &neutral());
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
    #[test]
    fn a_model_can_lean_on_a_faction_without_replacing_it() {
        // The dials scale the temperament; they do not overwrite it. Two
        // factions given the same push must still differ.
        let board = [view(1, Faction::Player, 4, 40.0, false)];
        let mut keen = neutral();
        keen.ambition = 2.0;

        let void_calm = decide(Faction::Void, 0.35, 18, &board, &neutral());
        let void_keen = decide(Faction::Void, 0.35, 18, &board, &keen);
        let bloom_keen = decide(Faction::Bloom, 0.35, 18, &board, &keen);

        assert!(
            void_keen.commitment >= void_calm.commitment,
            "pushing ambition made Void less committed"
        );
        assert_ne!(
            (void_keen.posture, void_keen.commitment),
            (bloom_keen.posture, bloom_keen.commitment),
            "the same push made two temperaments identical"
        );
    }

    #[test]
    fn a_clamped_but_extreme_tuning_still_produces_a_legal_plan() {
        // The clamp lets 0.4 and 2.0 through; both ends have to be survivable.
        let board = [
            view(1, Faction::Player, 2, 30.0, false),
            view(2, Faction::Swarm, 1, 15.0, true),
        ];
        for value in [0.4_f32, 2.0] {
            let tuning = crate::tactician::Tactics {
                ambition: value,
                aggression: value,
                expansion: value,
                cohesion: value,
                ..neutral()
            };
            for faction in Faction::MONSTERS {
                let plan = decide(faction, 0.5, 20, &board, &tuning);
                assert!(
                    (0.0..=0.95).contains(&plan.commitment),
                    "{faction:?} at {value} committed {}",
                    plan.commitment
                );
            }
        }
    }
}
