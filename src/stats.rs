//! Lifetime statistics and achievements.
//!
//! Two halves that need each other. The **ledger** counts everything the
//! player has ever done, across every run; the **achievements** are predicates
//! over that ledger. Keeping the counting separate from the celebrating means
//! a new achievement is one line and needs no new bookkeeping, and it means
//! the numbers are worth showing on their own.
//!
//! Identity is deliberately thin. [`Identity`] is a trait with a local
//! implementation that writes beside the save file; Game Center, Google Play
//! and whatever itch.io offers are all the same shape - a name, and somewhere
//! to push unlocks - and slot in behind it without the game knowing.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use bevy::prelude::*;

use crate::factions::Faction;
use crate::{AppState, GameSet};

/// Everything the player has ever done.
///
/// One flat map rather than a struct with fifty fields: achievements are
/// predicates over named counters, and a map means adding a counter does not
/// touch the save format, the reader, or anything that already works.
#[derive(Resource, Debug, Default, Clone)]
pub struct Ledger {
    counts: BTreeMap<String, f64>,
}

/// The counters the game actually keeps. Named constants rather than bare
/// strings so a typo is a compile error instead of a statistic that silently
/// never moves.
pub mod stat {
    pub const RUNS: &str = "runs";
    pub const BEST_TIME: &str = "best_time";
    pub const TOTAL_TIME: &str = "total_time";
    pub const KILLS: &str = "kills";
    pub const BOSSES: &str = "bosses";
    pub const ELITES: &str = "elites";
    pub const BEST_LEVEL: &str = "best_level";
    pub const LEVELS: &str = "levels";
    pub const SCRAP: &str = "scrap";
    pub const CORES: &str = "cores";
    pub const ZONES_HELD: &str = "zones_held";
    pub const FORTS_TAKEN: &str = "forts_taken";
    pub const FORTS_LOST: &str = "forts_lost";
    pub const NESTS_CLEARED: &str = "nests_cleared";
    pub const ALLIES_RECRUITED: &str = "allies_recruited";
    pub const STRUCTURES_BUILT: &str = "structures_built";
    pub const WAVES_CALLED: &str = "waves_called";
    pub const SURGES: &str = "surges";
    pub const WARS_STARTED: &str = "wars_started";
    pub const AREA_EXPLORED: &str = "area_explored";
    pub const FURTHEST: &str = "furthest";
    pub const DEATHS: &str = "deaths";
    pub const HIGHEST_THREAT: &str = "highest_threat";
    /// One per world, suffixed with the world's short name.
    pub const WORLD_PREFIX: &str = "world_";
}

impl Ledger {
    #[must_use]
    pub fn get(&self, key: &str) -> f64 {
        self.counts.get(key).copied().unwrap_or(0.0)
    }

    pub fn add(&mut self, key: &str, amount: f64) {
        if amount == 0.0 {
            return;
        }
        *self.counts.entry(key.to_owned()).or_insert(0.0) += amount;
    }

    /// Keep the larger of what is there and what just happened. For records
    /// rather than totals.
    pub fn best(&mut self, key: &str, value: f64) {
        let slot = self.counts.entry(key.to_owned()).or_insert(f64::MIN);
        if value > *slot {
            *slot = value;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, f64)> {
        self.counts.iter().map(|(k, v)| (k.as_str(), *v))
    }

    /// Same line-oriented format as the save file, and for the same reasons.
    #[must_use]
    pub fn encode(&self) -> String {
        let mut out = String::new();
        for (key, value) in &self.counts {
            let _ = writeln!(out, "{key} {value}");
        }
        out
    }

    #[must_use]
    pub fn decode(text: &str) -> Self {
        let mut ledger = Self::default();
        for line in text.lines() {
            let mut parts = line.split_whitespace();
            if let (Some(key), Some(value)) = (parts.next(), parts.next())
                && let Ok(value) = value.parse::<f64>()
            {
                ledger.counts.insert(key.to_owned(), value);
            }
        }
        ledger
    }
}

// -- achievements -----------------------------------------------------------

/// How an achievement is measured.
#[derive(Debug, Clone, Copy)]
pub enum Goal {
    /// A counter reaching a threshold.
    AtLeast(&'static str, f64),
    /// Every world played at least once. Kept as its own variant because
    /// "all of a set" is a different question from "enough of a number".
    EveryWorld,
}

#[derive(Debug, Clone, Copy)]
pub struct Achievement {
    pub id: &'static str,
    pub name: &'static str,
    pub detail: &'static str,
    pub goal: Goal,
    /// Hidden until earned. For the ones that would spoil something.
    pub secret: bool,
}

impl Achievement {
    /// How far along this is, from 0 to 1.
    #[must_use]
    pub fn progress(&self, ledger: &Ledger) -> f32 {
        match self.goal {
            Goal::AtLeast(key, target) if target > 0.0 => {
                (ledger.get(key) / target).clamp(0.0, 1.0) as f32
            }
            Goal::AtLeast(..) => 1.0,
            Goal::EveryWorld => {
                let held = crate::environments::EnvKind::ALL
                    .iter()
                    .filter(|env| {
                        ledger.get(&format!("{}{}", stat::WORLD_PREFIX, env.short_name())) > 0.0
                    })
                    .count();
                held as f32 / crate::environments::EnvKind::COUNT as f32
            }
        }
    }

    #[must_use]
    pub fn earned(&self, ledger: &Ledger) -> bool {
        self.progress(ledger) >= 1.0
    }
}

/// The full list.
///
/// Tuned to reward the things the game is *about* - holding ground, setting
/// your own pace, going somewhere - rather than only the things that are easy
/// to count. An achievement list made only of kill totals teaches the player
/// that the game is about kill totals.
pub const ACHIEVEMENTS: &[Achievement] = &[
    Achievement {
        id: "first_stand",
        name: "First Stand",
        detail: "Finish a run.",
        goal: Goal::AtLeast(stat::RUNS, 1.0),
        secret: false,
    },
    Achievement {
        id: "ten_minutes",
        name: "Ten Minutes",
        detail: "Survive ten minutes in a single run.",
        goal: Goal::AtLeast(stat::BEST_TIME, 600.0),
        secret: false,
    },
    Achievement {
        id: "half_hour",
        name: "The Long Watch",
        detail: "Survive thirty minutes in a single run.",
        goal: Goal::AtLeast(stat::BEST_TIME, 1800.0),
        secret: false,
    },
    Achievement {
        id: "tourist",
        name: "Grand Tour",
        detail: "Set foot in all five worlds.",
        goal: Goal::EveryWorld,
        secret: false,
    },
    Achievement {
        id: "thousand",
        name: "Attrition",
        detail: "Defeat a thousand monsters.",
        goal: Goal::AtLeast(stat::KILLS, 1000.0),
        secret: false,
    },
    Achievement {
        id: "hundred_thousand",
        name: "Industrial",
        detail: "Defeat a hundred thousand monsters.",
        goal: Goal::AtLeast(stat::KILLS, 100_000.0),
        secret: false,
    },
    Achievement {
        id: "giant_killer",
        name: "Giant Killer",
        detail: "Bring down twenty-five bosses.",
        goal: Goal::AtLeast(stat::BOSSES, 25.0),
        secret: false,
    },
    Achievement {
        id: "landlord",
        name: "Landlord",
        detail: "Take fifty forts.",
        goal: Goal::AtLeast(stat::FORTS_TAKEN, 50.0),
        secret: false,
    },
    Achievement {
        id: "exterminator",
        name: "Exterminator",
        detail: "Clear five hundred nests.",
        goal: Goal::AtLeast(stat::NESTS_CLEARED, 500.0),
        secret: false,
    },
    Achievement {
        id: "warmonger",
        name: "Warmonger",
        detail: "Set rival factions on each other ten times.",
        goal: Goal::AtLeast(stat::WARS_STARTED, 10.0),
        secret: false,
    },
    Achievement {
        id: "cartographer",
        name: "Cartographer",
        detail: "Explore a hundred thousand square units of ground.",
        goal: Goal::AtLeast(stat::AREA_EXPLORED, 100_000.0),
        secret: false,
    },
    Achievement {
        id: "far_country",
        name: "The Far Country",
        detail: "Get two thousand units from where you landed.",
        goal: Goal::AtLeast(stat::FURTHEST, 2000.0),
        secret: false,
    },
    Achievement {
        id: "impatient",
        name: "Impatient",
        detail: "Call a hundred waves in early.",
        goal: Goal::AtLeast(stat::WAVES_CALLED, 100.0),
        secret: false,
    },
    Achievement {
        id: "redline",
        name: "Redline",
        detail: "Hold the threat dial at maximum.",
        goal: Goal::AtLeast(stat::HIGHEST_THREAT, crate::threat::MAX_INTENT as f64),
        secret: false,
    },
    Achievement {
        id: "commander",
        name: "Commander",
        detail: "Recruit two hundred allies.",
        goal: Goal::AtLeast(stat::ALLIES_RECRUITED, 200.0),
        secret: false,
    },
    Achievement {
        id: "quartermaster",
        name: "Quartermaster",
        detail: "Build a thousand structures.",
        goal: Goal::AtLeast(stat::STRUCTURES_BUILT, 1000.0),
        secret: false,
    },
    Achievement {
        id: "ascendant",
        name: "Ascendant",
        detail: "Reach level fifty in a single run.",
        goal: Goal::AtLeast(stat::BEST_LEVEL, 50.0),
        secret: false,
    },
    Achievement {
        id: "persistent",
        name: "Persistent",
        detail: "Die a hundred times and come back.",
        goal: Goal::AtLeast(stat::DEATHS, 100.0),
        secret: true,
    },
];

/// Which achievements have been earned and announced.
#[derive(Resource, Debug, Default)]
pub struct Unlocked {
    ids: Vec<String>,
}

impl Unlocked {
    #[must_use]
    pub fn has(&self, id: &str) -> bool {
        self.ids.iter().any(|held| held == id)
    }

    fn insert(&mut self, id: &str) -> bool {
        if self.has(id) {
            return false;
        }
        self.ids.push(id.to_owned());
        true
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.ids.len()
    }

    #[must_use]
    pub fn encode(&self) -> String {
        self.ids.join("\n")
    }

    #[must_use]
    pub fn decode(text: &str) -> Self {
        Self {
            ids: text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_owned)
                .collect(),
        }
    }
}

// -- identity ---------------------------------------------------------------

/// Somewhere to hang a player's name and their unlocks.
///
/// Deliberately the smallest surface that Game Center, Google Play and a
/// browser's local storage can all satisfy. The game never learns which one it
/// has; anything richer - friends, leaderboards, cloud sync - would leak a
/// specific platform's shape into code that has no business knowing about it.
pub trait Identity: Send + Sync + 'static {
    /// What to call the player. `None` while a platform service is still
    /// signing in, which is normal and not an error.
    fn display_name(&self) -> Option<String>;

    /// Push an unlock outward. Called once per achievement, ever.
    fn report(&self, id: &str);

    /// Where this identity came from, for the profile screen.
    fn provider(&self) -> &'static str;
}

/// The default: no service, everything kept locally.
#[derive(Debug, Default)]
pub struct LocalIdentity;

impl Identity for LocalIdentity {
    fn display_name(&self) -> Option<String> {
        None
    }

    fn report(&self, _id: &str) {}

    fn provider(&self) -> &'static str {
        "local"
    }
}

/// The identity in use. Replace the boxed value at startup to plug in a
/// platform service.
#[derive(Resource)]
pub struct Profile {
    pub identity: Box<dyn Identity>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            identity: Box::new(LocalIdentity),
        }
    }
}

impl std::fmt::Debug for Profile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Profile")
            .field("provider", &self.identity.provider())
            .finish()
    }
}

// -- plumbing ---------------------------------------------------------------

/// Raised whenever something worth counting happens.
///
/// A message rather than direct mutation so that any system can report without
/// taking a write lock on the ledger, and so the counting stays in one place.
#[derive(Message, Debug, Clone)]
pub struct Record {
    pub key: String,
    pub amount: f64,
    /// True for records (best time, furthest) rather than totals.
    pub is_best: bool,
}

impl Record {
    #[must_use]
    pub fn add(key: &str, amount: f64) -> Self {
        Self {
            key: key.to_owned(),
            amount,
            is_best: false,
        }
    }

    #[must_use]
    pub fn best(key: &str, value: f64) -> Self {
        Self {
            key: key.to_owned(),
            amount: value,
            is_best: true,
        }
    }
}

/// Announced when an achievement is earned.
#[derive(Message, Debug, Clone)]
pub struct Earned(pub &'static Achievement);

#[derive(Debug)]
pub struct StatsPlugin;

impl Plugin for StatsPlugin {
    fn build(&self, app: &mut App) {
        let (ledger, unlocked) = load();
        app.insert_resource(ledger)
            .insert_resource(unlocked)
            .init_resource::<Profile>()
            .add_message::<Record>()
            .add_message::<Earned>()
            .add_systems(
                Update,
                watch_run
                    .in_set(GameSet::Present)
                    .run_if(in_state(AppState::Playing)),
            )
            .add_systems(Update, (apply_records, check_achievements).chain())
            .add_systems(OnEnter(AppState::GameOver), bank_run);
    }
}

fn apply_records(mut ledger: ResMut<Ledger>, mut records: MessageReader<Record>) {
    for record in records.read() {
        if record.is_best {
            ledger.best(&record.key, record.amount);
        } else {
            ledger.add(&record.key, record.amount);
        }
    }
}

fn check_achievements(
    ledger: Res<Ledger>,
    profile: Res<Profile>,
    mut unlocked: ResMut<Unlocked>,
    mut earned: MessageWriter<Earned>,
    mut hints: ResMut<crate::onboarding::HintQueue>,
) {
    if !ledger.is_changed() {
        return;
    }
    for achievement in ACHIEVEMENTS {
        if !achievement.earned(&ledger) || !unlocked.insert(achievement.id) {
            continue;
        }
        profile.identity.report(achievement.id);
        earned.write(Earned(achievement));
        hints.push(
            format!("ACHIEVEMENT: {}", achievement.name),
            achievement.detail,
            crate::onboarding::HintTone::Unlock,
        );
    }
    persist(&ledger, &unlocked);
}

/// The records that are a high-water mark rather than an event.
///
/// Sampled rather than hooked, because "how far did they get from where they
/// landed" and "how high did they push the dial" are properties of a moment,
/// and no single moment raises an event about them.
fn watch_run(
    threat: Res<crate::threat::Threat>,
    player: Query<&crate::common::Body, With<crate::player::Player>>,
    mut records: MessageWriter<Record>,
) {
    if let Some(body) = player.iter().next() {
        records.write(Record::best(stat::FURTHEST, f64::from(body.pos.length())));
    }
    records.write(Record::best(stat::HIGHEST_THREAT, f64::from(threat.intent)));
}

/// Fold a finished run into the lifetime totals.
fn bank_run(
    clock: Res<crate::threat::RunClock>,
    progression: Res<crate::progress::Progression>,
    env: Res<crate::environments::EnvKind>,
    fog: Res<crate::fog::FogMap>,
    mut records: MessageWriter<Record>,
) {
    records.write(Record::add(stat::RUNS, 1.0));
    records.write(Record::add(stat::DEATHS, 1.0));
    records.write(Record::add(stat::TOTAL_TIME, f64::from(clock.elapsed)));
    records.write(Record::best(stat::BEST_TIME, f64::from(clock.elapsed)));
    records.write(Record::best(stat::BEST_LEVEL, f64::from(progression.level)));
    records.write(Record::add(stat::LEVELS, f64::from(progression.level)));
    records.write(Record::add(
        stat::AREA_EXPLORED,
        f64::from(fog.explored_area()),
    ));
    records.write(Record::add(
        &format!("{}{}", stat::WORLD_PREFIX, env.short_name()),
        1.0,
    ));
}

/// Where the ledger lives. Beside the save, for the same reasons.
#[cfg(not(target_arch = "wasm32"))]
fn paths() -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = crate::save::save_path()
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    (
        dir.join("holdfast-stats.txt"),
        dir.join("holdfast-achievements.txt"),
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn load() -> (Ledger, Unlocked) {
    let (stats, achievements) = paths();
    (
        std::fs::read_to_string(stats)
            .map(|t| Ledger::decode(&t))
            .unwrap_or_default(),
        std::fs::read_to_string(achievements)
            .map(|t| Unlocked::decode(&t))
            .unwrap_or_default(),
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn persist(ledger: &Ledger, unlocked: &Unlocked) {
    let (stats, achievements) = paths();
    let _ = std::fs::write(stats, ledger.encode());
    let _ = std::fs::write(achievements, unlocked.encode());
}

#[cfg(target_arch = "wasm32")]
fn store() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

#[cfg(target_arch = "wasm32")]
fn load() -> (Ledger, Unlocked) {
    let Some(store) = store() else {
        return (Ledger::default(), Unlocked::default());
    };
    (
        store
            .get_item("holdfast-stats")
            .ok()
            .flatten()
            .map(|t| Ledger::decode(&t))
            .unwrap_or_default(),
        store
            .get_item("holdfast-achievements")
            .ok()
            .flatten()
            .map(|t| Unlocked::decode(&t))
            .unwrap_or_default(),
    )
}

#[cfg(target_arch = "wasm32")]
fn persist(ledger: &Ledger, unlocked: &Unlocked) {
    if let Some(store) = store() {
        let _ = store.set_item("holdfast-stats", &ledger.encode());
        let _ = store.set_item("holdfast-achievements", &unlocked.encode());
    }
}

/// Faction kills, kept out of the `stat` module because the key is built from
/// a faction rather than being a fixed name.
#[must_use]
pub fn faction_kill_key(faction: Faction) -> String {
    format!("killed_{}", faction.tag().to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_counter_reads_as_zero() {
        let ledger = Ledger::default();
        assert_eq!(ledger.get("never_touched"), 0.0);
    }

    #[test]
    fn totals_accumulate_and_records_keep_the_best() {
        let mut ledger = Ledger::default();
        ledger.add(stat::KILLS, 10.0);
        ledger.add(stat::KILLS, 5.0);
        assert_eq!(ledger.get(stat::KILLS), 15.0);

        ledger.best(stat::BEST_TIME, 100.0);
        ledger.best(stat::BEST_TIME, 40.0);
        assert_eq!(
            ledger.get(stat::BEST_TIME),
            100.0,
            "a record went backwards"
        );
        ledger.best(stat::BEST_TIME, 250.0);
        assert_eq!(ledger.get(stat::BEST_TIME), 250.0);
    }

    #[test]
    fn a_first_record_is_kept_even_when_small() {
        // `best` seeds with f64::MIN; a first value of zero still has to land.
        let mut ledger = Ledger::default();
        ledger.best(stat::FURTHEST, 0.0);
        assert_eq!(ledger.get(stat::FURTHEST), 0.0);
        ledger.best(stat::FURTHEST, -5.0);
        assert_eq!(ledger.get(stat::FURTHEST), 0.0);
    }

    #[test]
    fn adding_nothing_does_not_create_a_counter() {
        let mut ledger = Ledger::default();
        ledger.add("idle", 0.0);
        assert_eq!(ledger.iter().count(), 0);
    }

    #[test]
    fn the_ledger_survives_a_round_trip() {
        let mut ledger = Ledger::default();
        ledger.add(stat::KILLS, 1234.0);
        ledger.add(stat::SCRAP, 99.5);
        ledger.best(stat::BEST_TIME, 1800.0);
        let after = Ledger::decode(&ledger.encode());
        assert_eq!(after.get(stat::KILLS), 1234.0);
        assert_eq!(after.get(stat::SCRAP), 99.5);
        assert_eq!(after.get(stat::BEST_TIME), 1800.0);
    }

    #[test]
    fn a_corrupt_ledger_line_is_skipped_rather_than_fatal() {
        // Lifetime stats are not worth losing to one bad line.
        let ledger = Ledger::decode("kills 500\ngarbage\nbroken not_a_number\ncores 12\n");
        assert_eq!(ledger.get("kills"), 500.0);
        assert_eq!(ledger.get("cores"), 12.0);
        assert_eq!(ledger.get("broken"), 0.0);
    }

    #[test]
    fn every_achievement_is_reachable_and_described() {
        for a in ACHIEVEMENTS {
            assert!(!a.name.is_empty(), "{} has no name", a.id);
            assert!(!a.detail.is_empty(), "{} has no detail", a.id);
            if let Goal::AtLeast(_, target) = a.goal {
                assert!(target > 0.0, "{} can never be earned", a.id);
            }
        }
    }

    #[test]
    fn achievement_ids_are_unique() {
        // The unlock set is keyed on these; a duplicate would mean one can
        // never be earned separately.
        let mut ids: Vec<_> = ACHIEVEMENTS.iter().map(|a| a.id).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count, "duplicate achievement id");
    }

    #[test]
    fn an_empty_ledger_earns_nothing() {
        let ledger = Ledger::default();
        for a in ACHIEVEMENTS {
            assert!(!a.earned(&ledger), "{} was free", a.id);
        }
    }

    #[test]
    fn progress_runs_from_nothing_to_earned() {
        let goal = Achievement {
            id: "t",
            name: "t",
            detail: "t",
            goal: Goal::AtLeast(stat::KILLS, 100.0),
            secret: false,
        };
        let mut ledger = Ledger::default();
        assert!((goal.progress(&ledger) - 0.0).abs() < 1e-6);
        ledger.add(stat::KILLS, 50.0);
        assert!((goal.progress(&ledger) - 0.5).abs() < 1e-6);
        ledger.add(stat::KILLS, 50.0);
        assert!(goal.earned(&ledger));
        // Overshooting does not push progress past full.
        ledger.add(stat::KILLS, 5000.0);
        assert!((goal.progress(&ledger) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_grand_tour_needs_all_five_worlds() {
        let tour = ACHIEVEMENTS.iter().find(|a| a.id == "tourist").unwrap();
        let mut ledger = Ledger::default();
        for (i, env) in crate::environments::EnvKind::ALL.iter().enumerate() {
            assert!(!tour.earned(&ledger), "earned after {i} worlds");
            ledger.add(&format!("{}{}", stat::WORLD_PREFIX, env.short_name()), 1.0);
        }
        assert!(tour.earned(&ledger), "five worlds was not enough");
    }

    #[test]
    fn an_achievement_is_only_announced_once() {
        let mut unlocked = Unlocked::default();
        assert!(unlocked.insert("first_stand"));
        assert!(!unlocked.insert("first_stand"), "announced twice");
        assert_eq!(unlocked.count(), 1);
    }

    #[test]
    fn unlocks_survive_a_round_trip() {
        let mut unlocked = Unlocked::default();
        unlocked.insert("first_stand");
        unlocked.insert("landlord");
        let after = Unlocked::decode(&unlocked.encode());
        assert!(after.has("first_stand"));
        assert!(after.has("landlord"));
        assert!(!after.has("ascendant"));
        assert_eq!(after.count(), 2);
    }

    #[test]
    fn a_blank_unlock_file_is_empty_rather_than_one_blank_id() {
        assert_eq!(Unlocked::decode("").count(), 0);
        assert_eq!(Unlocked::decode("\n\n  \n").count(), 0);
    }

    #[test]
    fn the_default_identity_is_local_and_silent() {
        let profile = Profile::default();
        assert_eq!(profile.identity.provider(), "local");
        assert!(profile.identity.display_name().is_none());
        // Reporting must not panic when there is nowhere to report to.
        profile.identity.report("first_stand");
    }

    #[test]
    fn a_platform_identity_slots_in_without_the_game_knowing() {
        use std::sync::Mutex;

        #[derive(Debug, Default)]
        struct Fake {
            reported: Mutex<Vec<String>>,
        }
        impl Identity for Fake {
            fn display_name(&self) -> Option<String> {
                Some("tester".into())
            }
            fn report(&self, id: &str) {
                self.reported.lock().unwrap().push(id.to_owned());
            }
            fn provider(&self) -> &'static str {
                "fake"
            }
        }

        let profile = Profile {
            identity: Box::new(Fake::default()),
        };
        profile.identity.report("landlord");
        assert_eq!(profile.identity.provider(), "fake");
        assert_eq!(profile.identity.display_name().as_deref(), Some("tester"));
    }

    #[test]
    fn achievements_reward_more_than_killing_things() {
        // A list made only of kill totals teaches the player that the game is
        // about kill totals.
        let kill_keys = [stat::KILLS, stat::BOSSES, stat::ELITES];
        let combat = ACHIEVEMENTS
            .iter()
            .filter(|a| matches!(a.goal, Goal::AtLeast(k, _) if kill_keys.contains(&k)))
            .count();
        assert!(
            combat * 3 < ACHIEVEMENTS.len(),
            "{combat} of {} achievements are kill counts",
            ACHIEVEMENTS.len()
        );
    }

    #[test]
    fn faction_kill_keys_are_distinct_per_faction() {
        let mut keys: Vec<_> = Faction::ALL.iter().map(|f| faction_kill_key(*f)).collect();
        keys.sort();
        let count = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), count);
    }
}
