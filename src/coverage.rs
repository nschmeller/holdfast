//! What of the game has actually been seen.
//!
//! "Explore all the content" is not a thing you can ask a playtester to do
//! unless somebody is counting, because the content is spread across five
//! worlds, ten weapons, twelve monsters, five structures, four allies, four
//! factions, a research tree and a set of things that only happen a hundred
//! and thirty units from where you land. A tester with a checklist and a
//! progress readout can go and get the missing ones; a tester without one
//! plays the opening again.
//!
//! So this keeps a live tally of every distinct piece of content exercised,
//! and knows the full list to compare it against. It is surfaced through the
//! pilot bridge, which turns "go and see everything" into a task with an
//! answer.

use std::collections::BTreeSet;

use bevy::prelude::*;

use crate::allies::{AllyKind, TurretKind};
use crate::arena::HazardKind;
use crate::enemy::EnemyKind;
use crate::environments::EnvKind;
use crate::factions::Faction;
use crate::weapons::WeaponKind;
use crate::{AppState, RunSetup};

/// Raised the first time a piece of content is exercised. Cheap to send
/// repeatedly; the set does the deduplicating.
#[derive(Message, Debug, Clone)]
pub struct Seen(pub String);

impl Seen {
    #[must_use]
    pub fn of(category: &str, item: &str) -> Self {
        Self(format!("{category}:{item}"))
    }
}

/// Everything exercised so far.
///
/// Not per-run: a coverage sweep spans several runs and several worlds by
/// necessity, and resetting on death would make the goal impossible rather than
/// merely long.
///
/// And not per-session either, any more. It used to live only in memory, so
/// every restart threw the sweep away - a tester that had visited three worlds
/// and then relaunched was back at zero, which it could only find out by
/// noticing the number had moved backwards. One tour worked around it by
/// abandoning runs to the menu rather than restarting the process. The file is
/// one tag per line beside the save, deliberately trivial to read and to delete.
#[derive(Resource, Debug, Default)]
pub struct Coverage {
    seen: BTreeSet<String>,
    /// Set when something new landed and the file is behind.
    dirty: bool,
}

impl Coverage {
    pub fn mark(&mut self, tag: impl Into<String>) -> bool {
        let fresh = self.seen.insert(tag.into());
        self.dirty |= fresh;
        fresh
    }

    #[must_use]
    pub fn has(&self, tag: &str) -> bool {
        self.seen.contains(tag)
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.seen.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.seen.iter().map(String::as_str)
    }

    /// What has not been seen yet, in the order of the full list.
    #[must_use]
    pub fn missing(&self) -> Vec<String> {
        expected()
            .into_iter()
            .filter(|tag| !self.seen.contains(tag))
            .collect()
    }

    /// Fraction of the game seen, 0 to 1.
    #[must_use]
    pub fn fraction(&self) -> f32 {
        let all = expected();
        if all.is_empty() {
            return 1.0;
        }
        let hit = all.iter().filter(|tag| self.seen.contains(*tag)).count();
        hit as f32 / all.len() as f32
    }
}

/// The full checklist.
///
/// Derived from the enums rather than written out, so a new weapon or monster
/// joins the list by existing and cannot be quietly forgotten.
///
/// Items are named by their archetype, not by what a given world calls them:
/// a Pencil Dart and a Thorn Dart are the same content seen twice, and a
/// coverage sweep should say so.
#[must_use]
pub fn expected() -> Vec<String> {
    let mut out = Vec::new();
    for env in EnvKind::ALL {
        out.push(format!("world:{}", env.short_name()));
    }
    for weapon in WeaponKind::ALL {
        out.push(format!("weapon:{weapon:?}"));
    }
    for kind in EnemyKind::ALL {
        out.push(format!("enemy:{kind:?}"));
    }
    for turret in TurretKind::ALL {
        out.push(format!("turret:{turret:?}"));
    }
    for ally in AllyKind::ALL {
        out.push(format!("ally:{ally:?}"));
    }
    for faction in Faction::MONSTERS {
        out.push(format!("faction:{}", faction.tag()));
    }
    for hazard in [
        HazardKind::Scald,
        HazardKind::Sticky,
        HazardKind::Shock,
        HazardKind::Font,
    ] {
        out.push(format!("hazard:{hazard:?}"));
    }
    // The verbs and the milestones - the things that are content but are not
    // an entry in an enum anywhere.
    for deed in [
        "plan-mode",
        "dash",
        "build",
        "recruit",
        "research",
        "gear",
        "zone-held",
        "fort-taken",
        "fort-lost",
        "nest-cleared",
        "seeder-planted",
        "war-incited",
        "wave-called",
        "overclock",
        "boss-killed",
        "elite-killed",
        "level-10",
        "ten-minutes",
        "far-country",
    ] {
        out.push(format!("deed:{deed}"));
    }
    out
}

#[derive(Debug)]
pub struct CoveragePlugin;

impl Plugin for CoveragePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(load())
            .add_message::<Seen>()
            .add_systems(Update, (absorb, persist).chain())
            .add_systems(Update, note_milestones.run_if(in_state(AppState::Playing)))
            .add_systems(OnExit(AppState::Menu), note_world.in_set(RunSetup::Reset));
    }
}

fn absorb(mut coverage: ResMut<Coverage>, mut seen: MessageReader<Seen>) {
    for item in seen.read() {
        coverage.mark(item.0.clone());
    }
}

/// Write the sweep out whenever it grows.
///
/// Only on a change, so this is a no-op on almost every frame - and the file is
/// small enough that rewriting it whole is cheaper than tracking appends.
fn persist(mut coverage: ResMut<Coverage>) {
    if !coverage.dirty {
        return;
    }
    coverage.dirty = false;
    let body = coverage.seen.iter().cloned().collect::<Vec<_>>().join("\n");
    store(&body);
}

#[cfg(not(target_arch = "wasm32"))]
fn path() -> std::path::PathBuf {
    crate::save::save_path()
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default()
        .join("holdfast-coverage.txt")
}

#[cfg(not(target_arch = "wasm32"))]
fn store(body: &str) {
    let _ = std::fs::write(path(), body);
}

#[cfg(not(target_arch = "wasm32"))]
fn load() -> Coverage {
    let Ok(body) = std::fs::read_to_string(path()) else {
        return Coverage::default();
    };
    let known: BTreeSet<String> = expected().into_iter().collect();
    Coverage {
        // Filtered against the checklist on the way in, so a renamed weapon
        // leaves behind a stale tag rather than an inflated score.
        seen: body
            .lines()
            .map(str::trim)
            .filter(|line| known.contains(*line))
            .map(String::from)
            .collect(),
        dirty: false,
    }
}

#[cfg(target_arch = "wasm32")]
fn store(_body: &str) {
    // Nobody is running a content sweep in a browser tab.
}

#[cfg(target_arch = "wasm32")]
fn load() -> Coverage {
    Coverage::default()
}

fn note_world(env: Res<EnvKind>, mut seen: MessageWriter<Seen>) {
    seen.write(Seen::of("world", env.short_name()));
}

/// The four entries that are states rather than events.
///
/// "Reached level ten" and "held a zone" raise nothing anywhere, so they are
/// sampled. Ten of the nineteen deeds had no writer at all and coverage could
/// never pass 41% - the checklist was lying to whoever used it to decide what
/// to test next.
fn note_milestones(
    clock: Res<crate::threat::RunClock>,
    progression: Res<crate::progress::Progression>,
    equipped: Res<crate::progress::Equipped>,
    zones: Query<&crate::allies::Zone>,
    player: Query<&crate::common::Body, With<crate::player::Player>>,
    mut seen: MessageWriter<Seen>,
) {
    if progression.level >= 10 {
        seen.write(Seen(String::from("deed:level-10")));
    }
    if clock.elapsed >= 600.0 {
        seen.write(Seen(String::from("deed:ten-minutes")));
    }
    if crate::progress::GearSlot::ALL
        .iter()
        .any(|slot| equipped.get(*slot).is_some())
    {
        seen.write(Seen(String::from("deed:gear")));
    }
    if zones
        .iter()
        .any(|zone| zone.owner == crate::allies::ZoneOwner::Player)
    {
        seen.write(Seen(String::from("deed:zone-held")));
    }
    if player.iter().any(|body| body.pos.length() >= 2000.0) {
        seen.write(Seen(String::from("deed:far-country")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_checklist_covers_every_enum_the_game_has() {
        let all = expected();
        // If a weapon or a monster is added and this does not grow, the sweep
        // would report full coverage while missing it.
        assert_eq!(
            all.iter().filter(|t| t.starts_with("weapon:")).count(),
            WeaponKind::ALL.len()
        );
        assert_eq!(
            all.iter().filter(|t| t.starts_with("enemy:")).count(),
            EnemyKind::ALL.len()
        );
        assert_eq!(
            all.iter().filter(|t| t.starts_with("world:")).count(),
            EnvKind::COUNT
        );
        assert_eq!(
            all.iter().filter(|t| t.starts_with("turret:")).count(),
            TurretKind::ALL.len()
        );
        assert_eq!(
            all.iter().filter(|t| t.starts_with("ally:")).count(),
            AllyKind::ALL.len()
        );
        assert_eq!(
            all.iter().filter(|t| t.starts_with("faction:")).count(),
            Faction::MONSTERS.len()
        );
    }

    #[test]
    fn the_checklist_has_no_duplicates() {
        // A duplicate would make full coverage unreachable, since one tag can
        // only be marked once.
        let all = expected();
        let unique: BTreeSet<_> = all.iter().collect();
        assert_eq!(unique.len(), all.len());
    }

    #[test]
    fn nothing_is_covered_before_anything_happens() {
        let coverage = Coverage::default();
        assert_eq!(coverage.count(), 0);
        assert!((coverage.fraction() - 0.0).abs() < 1e-6);
        assert_eq!(coverage.missing().len(), expected().len());
    }

    #[test]
    fn marking_the_same_thing_twice_counts_once() {
        let mut coverage = Coverage::default();
        assert!(coverage.mark("weapon:PencilDart"));
        assert!(!coverage.mark("weapon:PencilDart"));
        assert_eq!(coverage.count(), 1);
    }

    #[test]
    fn seeing_everything_reads_as_complete() {
        let mut coverage = Coverage::default();
        for tag in expected() {
            coverage.mark(tag);
        }
        assert!(coverage.missing().is_empty());
        assert!((coverage.fraction() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn unexpected_tags_do_not_inflate_the_score() {
        // Marking something that is not on the list must not count towards it,
        // or a typo at a call site would read as progress.
        let mut coverage = Coverage::default();
        coverage.mark("weapon:Nonexistent Gun");
        assert!((coverage.fraction() - 0.0).abs() < 1e-6);
        assert_eq!(coverage.missing().len(), expected().len());
    }

    #[test]
    fn every_hazard_on_the_checklist_is_placed_by_some_world() {
        // `Shock` sat on the list, and in the enum, and no world had ever
        // placed one - so the sweep could not reach 100% however well anybody
        // played, and the checklist was lying about what was left to do.
        let mut rng = crate::rng::Rng::seeded(0x51E7);
        let mut placed = BTreeSet::new();
        for env in EnvKind::ALL {
            for x in -6..=6 {
                for z in -6..=6 {
                    let content = env.generate_chunk(IVec2::new(x, z), &mut rng);
                    for hazard in content.hazards {
                        placed.insert(format!("hazard:{:?}", hazard.kind));
                    }
                }
            }
        }
        for tag in expected().iter().filter(|t| t.starts_with("hazard:")) {
            assert!(placed.contains(tag), "{tag} is on the list but unplaceable");
        }
    }

    #[test]
    fn a_reload_keeps_what_was_seen_and_drops_what_is_gone() {
        // A restart used to throw the whole sweep away, and the only sign was
        // the number having moved backwards.
        let known = expected();
        let body = format!("{}\n{}\nweapon:SomethingRenamed", known[0], known[1]);
        let restored: BTreeSet<String> = {
            let allowed: BTreeSet<String> = known.iter().cloned().collect();
            body.lines()
                .map(str::trim)
                .filter(|l| allowed.contains(*l))
                .map(String::from)
                .collect()
        };
        assert!(restored.contains(&known[0]));
        assert_eq!(
            restored.len(),
            2,
            "a stale tag was let back in: {restored:?}"
        );
    }

    #[test]
    fn marking_something_new_asks_for_a_write_and_marking_it_twice_does_not() {
        let mut coverage = Coverage::default();
        assert!(!coverage.dirty, "wants a write before anything happened");
        coverage.mark("weapon:PencilDart");
        assert!(coverage.dirty);
        coverage.dirty = false;
        coverage.mark("weapon:PencilDart");
        assert!(!coverage.dirty, "rewrote the file for a duplicate");
    }

    #[test]
    fn the_checklist_is_worth_the_trouble() {
        // Small enough to finish, large enough to be a real sweep.
        let all = expected();
        assert!(all.len() > 40, "only {} items to see", all.len());
        assert!(
            all.len() < 120,
            "{} items is a chore, not a sweep",
            all.len()
        );
    }

    #[test]
    fn a_tag_is_categorised() {
        assert_eq!(Seen::of("weapon", "PencilDart").0, "weapon:PencilDart");
    }
}
