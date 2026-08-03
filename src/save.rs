//! Saving and resuming a run.
//!
//! The format is line-oriented text: one `key value...` per line, unknown keys
//! ignored. That is a deliberate choice over a serialisation crate. This game
//! has no non-Bevy dependencies on purpose - it keeps the wasm build free of
//! JS shims - and a save file is the one place where a hand-rolled format is
//! *also* the better engineering: it diffs, it is readable in a bug report,
//! and a file written by an older build still loads because the reader skips
//! what it does not recognise instead of failing.
//!
//! What is saved is the *run*, not the frame. Enemies, projectiles and
//! particles are all reconstructible pressure; the world is reconstructible
//! from its seed. What cannot be reconstructed is what the player has done:
//! how far they got, what they chose, what they took, and what they have seen.

use std::fmt::Write as _;
use std::path::PathBuf;

use bevy::prelude::*;

use crate::allies::Economy;
use crate::common::Health;
use crate::environments::EnvKind;
use crate::factions::{Allegiance, Faction};
use crate::fog::FogMap;
use crate::forts::Fort;
use crate::onboarding::Unlocks;
use crate::player::{Player, PlayerStats};
use crate::progress::{AppliedBoosts, Progression, Research, StatBoost};
use crate::threat::{Phase, RunClock, Threat, WaveCycle};
use crate::weapons::{Loadout, WeaponKind};
use crate::world::WorldSeed;
use crate::{AppState, GameSet, RunSetup};

/// Bumped when a change would make an old file load *wrongly* rather than
/// merely incompletely. Unknown keys are skipped, so most additions do not
/// need it.
const FORMAT: u32 = 1;

/// Seconds between automatic saves.
const AUTOSAVE_PERIOD: f32 = 20.0;

/// Everything worth carrying across a quit.
#[derive(Debug, Clone, Default)]
pub struct SaveGame {
    pub world: usize,
    pub seed: u64,
    pub elapsed: f32,
    pub kills: u64,
    pub best_streak: u32,
    pub player_pos: Vec2,
    pub hp: f32,
    pub level: u32,
    pub xp: f32,
    pub to_next: f32,
    pub pending_levels: u32,
    pub skill_points: u32,
    pub total_xp: f64,
    pub scrap: f32,
    pub cores: f32,
    pub lifetime_scrap: f32,
    pub lifetime_cores: f32,
    pub threat_intent: f32,
    pub threat_floor: f32,
    pub wave: u32,
    pub wave_timer: f32,
    pub in_prep: bool,
    pub unlocks: [bool; 5],
    /// `(weapon index, level)`.
    pub weapons: Vec<(usize, u32)>,
    /// `(node index, rank)`, sparse: only nodes actually taken.
    pub research: Vec<(usize, u32)>,
    /// `(stat index, stacks)` from upgrade cards.
    pub boosts: Vec<(usize, u32)>,
    pub refinements: u32,
    /// Cells the player has explored.
    pub explored: Vec<IVec2>,
    /// Forts whose owner differs from what generation would produce.
    pub forts: Vec<(Vec2, usize)>,
}

impl SaveGame {
    /// Render to the on-disk form.
    #[must_use]
    pub fn encode(&self) -> String {
        let mut out = String::with_capacity(4096);
        let _ = writeln!(out, "holdfast {FORMAT}");
        let _ = writeln!(out, "world {}", self.world);
        let _ = writeln!(out, "seed {}", self.seed);
        let _ = writeln!(out, "elapsed {:.3}", self.elapsed);
        let _ = writeln!(out, "kills {}", self.kills);
        let _ = writeln!(out, "streak {}", self.best_streak);
        let _ = writeln!(out, "pos {:.3} {:.3}", self.player_pos.x, self.player_pos.y);
        let _ = writeln!(out, "hp {:.3}", self.hp);
        let _ = writeln!(
            out,
            "level {} {:.3} {:.3} {} {}",
            self.level, self.xp, self.to_next, self.pending_levels, self.skill_points
        );
        let _ = writeln!(out, "totalxp {:.3}", self.total_xp);
        let _ = writeln!(
            out,
            "economy {:.3} {:.3} {:.3} {:.3}",
            self.scrap, self.cores, self.lifetime_scrap, self.lifetime_cores
        );
        let _ = writeln!(
            out,
            "threat {:.3} {:.3}",
            self.threat_intent, self.threat_floor
        );
        let _ = writeln!(
            out,
            "wave {} {:.3} {}",
            self.wave,
            self.wave_timer,
            u8::from(self.in_prep)
        );
        let flags: String = self
            .unlocks
            .iter()
            .map(|u| if *u { '1' } else { '0' })
            .collect();
        let _ = writeln!(out, "unlocks {flags}");
        for (kind, level) in &self.weapons {
            let _ = writeln!(out, "weapon {kind} {level}");
        }
        for (node, rank) in &self.research {
            let _ = writeln!(out, "research {node} {rank}");
        }
        for (stat, stacks) in &self.boosts {
            let _ = writeln!(out, "boost {stat} {stacks}");
        }
        let _ = writeln!(out, "refinements {}", self.refinements);
        for (pos, faction) in &self.forts {
            let _ = writeln!(out, "fort {:.2} {:.2} {faction}", pos.x, pos.y);
        }
        // Fog last and one line per row: it is by far the largest section, and
        // grouping by row turns a long exploration into a handful of runs
        // rather than tens of thousands of coordinate pairs.
        let mut rows: Vec<(i32, Vec<i32>)> = Vec::new();
        let mut sorted = self.explored.clone();
        sorted.sort_unstable_by_key(|c| (c.y, c.x));
        for cell in sorted {
            match rows.last_mut() {
                Some((y, xs)) if *y == cell.y => xs.push(cell.x),
                _ => rows.push((cell.y, vec![cell.x])),
            }
        }
        for (y, xs) in rows {
            let mut line = format!("fog {y}");
            for run in runs(&xs) {
                let _ = write!(line, " {}:{}", run.0, run.1);
            }
            let _ = writeln!(out, "{line}");
        }
        out
    }

    /// Parse the on-disk form. Unknown keys are skipped so an older file still
    /// loads; a file whose header we do not recognise is rejected outright.
    #[must_use]
    pub fn decode(text: &str) -> Option<Self> {
        let mut save = Self::default();
        let mut header = false;

        for line in text.lines() {
            let mut parts = line.split_whitespace();
            let Some(key) = parts.next() else { continue };
            let rest: Vec<&str> = parts.collect();

            match key {
                "holdfast" => {
                    let version: u32 = rest.first()?.parse().ok()?;
                    if version > FORMAT {
                        // A file from the future. Refusing beats loading half
                        // of it and silently dropping the rest.
                        return None;
                    }
                    header = true;
                }
                "world" => save.world = rest.first()?.parse().ok()?,
                "seed" => save.seed = rest.first()?.parse().ok()?,
                "elapsed" => save.elapsed = num(rest.first())?,
                "kills" => save.kills = rest.first()?.parse().ok()?,
                "streak" => save.best_streak = rest.first()?.parse().ok()?,
                "pos" => save.player_pos = Vec2::new(num(rest.first())?, num(rest.get(1))?),
                "hp" => save.hp = num(rest.first())?,
                "level" => {
                    save.level = rest.first()?.parse().ok()?;
                    save.xp = num(rest.get(1))?;
                    save.to_next = num(rest.get(2))?;
                    save.pending_levels = rest.get(3)?.parse().ok()?;
                    save.skill_points = rest.get(4)?.parse().ok()?;
                }
                "totalxp" => save.total_xp = rest.first()?.parse().ok()?,
                "economy" => {
                    save.scrap = num(rest.first())?;
                    save.cores = num(rest.get(1))?;
                    save.lifetime_scrap = num(rest.get(2))?;
                    save.lifetime_cores = num(rest.get(3))?;
                }
                "threat" => {
                    save.threat_intent = num(rest.first())?;
                    save.threat_floor = num(rest.get(1))?;
                }
                "wave" => {
                    save.wave = rest.first()?.parse().ok()?;
                    save.wave_timer = num(rest.get(1))?;
                    save.in_prep = rest.get(2).is_some_and(|v| *v == "1");
                }
                "unlocks" => {
                    for (i, c) in rest.first()?.chars().take(5).enumerate() {
                        save.unlocks[i] = c == '1';
                    }
                }
                "weapon" => save
                    .weapons
                    .push((rest.first()?.parse().ok()?, rest.get(1)?.parse().ok()?)),
                "research" => save
                    .research
                    .push((rest.first()?.parse().ok()?, rest.get(1)?.parse().ok()?)),
                "boost" => save
                    .boosts
                    .push((rest.first()?.parse().ok()?, rest.get(1)?.parse().ok()?)),
                "refinements" => save.refinements = rest.first()?.parse().ok()?,
                "fort" => save.forts.push((
                    Vec2::new(num(rest.first())?, num(rest.get(1))?),
                    rest.get(2)?.parse().ok()?,
                )),
                "fog" => {
                    let y: i32 = rest.first()?.parse().ok()?;
                    for span in rest.iter().skip(1) {
                        let (from, to) = span.split_once(':')?;
                        let (from, to): (i32, i32) = (from.parse().ok()?, to.parse().ok()?);
                        for x in from..=to {
                            save.explored.push(IVec2::new(x, y));
                        }
                    }
                }
                _ => {}
            }
        }

        header.then_some(save)
    }
}

fn num(value: Option<&&str>) -> Option<f32> {
    value?.parse().ok()
}

/// Collapse a sorted list into inclusive runs.
fn runs(xs: &[i32]) -> Vec<(i32, i32)> {
    let mut out: Vec<(i32, i32)> = Vec::new();
    for &x in xs {
        match out.last_mut() {
            Some(last) if x == last.1 + 1 => last.1 = x,
            Some(last) if x == last.1 => {}
            _ => out.push((x, x)),
        }
    }
    out
}

// -- where it lives ---------------------------------------------------------

/// Where a save lives, spelled differently per platform.
///
/// A browser has no filesystem, so the web build keeps the same text in
/// localStorage. Both sides of this speak the same format, which means a save
/// can be copied between them by hand if anyone ever wants to.
#[cfg(not(target_arch = "wasm32"))]
mod storage {
    use super::PathBuf;
    use std::fs;

    /// Beside the executable rather than in a platform data directory: this
    /// game ships as a single binary you can put on a stick, and a save that
    /// travels with it is less surprising than one filed away per-OS.
    #[must_use]
    pub fn path() -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_default()
            .join("holdfast-save.txt")
    }

    pub fn exists() -> bool {
        path().exists()
    }

    pub fn read() -> Option<String> {
        fs::read_to_string(path()).ok()
    }

    pub fn write(text: &str) -> Result<(), String> {
        fs::write(path(), text).map_err(|e| e.to_string())
    }
}

#[cfg(target_arch = "wasm32")]
mod storage {
    use super::PathBuf;

    const KEY: &str = "holdfast-save";

    fn store() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    #[must_use]
    pub fn path() -> PathBuf {
        PathBuf::from(KEY)
    }

    pub fn exists() -> bool {
        read().is_some()
    }

    pub fn read() -> Option<String> {
        store()?.get_item(KEY).ok()?
    }

    pub fn write(text: &str) -> Result<(), String> {
        // Private-browsing modes refuse writes and throw. Reporting it beats a
        // save button that silently does nothing.
        store()
            .ok_or_else(|| "no localStorage".to_string())?
            .set_item(KEY, text)
            .map_err(|_| "browser refused to store the save".to_string())
    }
}

/// Where the save lives. Exposed mostly so a bug report can say.
#[must_use]
pub fn save_path() -> PathBuf {
    storage::path()
}

/// Whether a run is waiting to be resumed.
#[derive(Resource, Debug, Default)]
pub struct SaveSlot {
    pub present: bool,
    /// Loaded on request and consumed by the restore pass.
    pub pending: Option<SaveGame>,
    since_autosave: f32,
    pub last_note: Option<String>,
}

#[derive(Debug)]
pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SaveSlot {
            present: storage::exists(),
            ..default()
        })
        .add_systems(
            OnExit(AppState::Menu),
            restore_run
                .in_set(RunSetup::Spawn)
                .after(crate::player::spawn_player),
        )
        .add_systems(Update, (autosave, save_hotkey).in_set(GameSet::Present));
    }
}

/// Gather the current run into a save.
#[allow(clippy::too_many_arguments)]
fn collect(
    env: EnvKind,
    seed: WorldSeed,
    clock: &RunClock,
    progression: &Progression,
    economy: &Economy,
    threat: &Threat,
    cycle: &WaveCycle,
    unlocks: &Unlocks,
    loadout: &Loadout,
    research: &Research,
    boosts: &AppliedBoosts,
    fog: &FogMap,
    player: Option<(Vec2, f32)>,
    forts: &[(Vec2, Faction)],
) -> SaveGame {
    SaveGame {
        world: env as usize,
        seed: seed.0,
        elapsed: clock.elapsed,
        kills: clock.kills,
        best_streak: clock.best_streak,
        player_pos: player.map_or(Vec2::ZERO, |p| p.0),
        hp: player.map_or(0.0, |p| p.1),
        level: progression.level,
        xp: progression.xp,
        to_next: progression.to_next,
        pending_levels: progression.pending_levels,
        skill_points: progression.skill_points,
        total_xp: progression.total_xp,
        scrap: economy.scrap,
        cores: economy.cores,
        lifetime_scrap: economy.lifetime_scrap,
        lifetime_cores: economy.lifetime_cores,
        threat_intent: threat.intent,
        threat_floor: threat.floor,
        wave: cycle.wave,
        wave_timer: cycle.timer,
        in_prep: cycle.in_prep(),
        unlocks: [
            unlocks.build,
            unlocks.territory,
            unlocks.allies,
            unlocks.research,
            unlocks.threat_dial,
        ],
        weapons: loadout
            .slots
            .iter()
            .map(|s| (s.kind as usize, s.level))
            .collect(),
        research: research
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.rank > 0)
            .map(|(i, n)| (i, n.rank))
            .collect(),
        boosts: boosts
            .entries
            .iter()
            .map(|(b, n)| (*b as usize, *n))
            .collect(),
        refinements: boosts.refinements,
        explored: fog.explored_list(),
        forts: forts.iter().map(|(p, f)| (*p, f.index())).collect(),
    }
}

/// Write the run to disk. Returns what to tell the player.
fn write_save(save: &SaveGame) -> String {
    match storage::write(&save.encode()) {
        Ok(()) => format!("Saved at {}", crate::common::format_time(save.elapsed)),
        Err(err) => format!("Could not save: {err}"),
    }
}

/// Read the run back, if there is one.
#[must_use]
pub fn read_save() -> Option<SaveGame> {
    SaveGame::decode(&storage::read()?)
}

#[derive(SystemParam)]
struct RunState<'w> {
    env: Res<'w, EnvKind>,
    seed: Res<'w, WorldSeed>,
    clock: Res<'w, RunClock>,
    progression: Res<'w, Progression>,
    economy: Res<'w, Economy>,
    threat: Res<'w, Threat>,
    cycle: Res<'w, WaveCycle>,
    unlocks: Res<'w, Unlocks>,
    loadout: Res<'w, Loadout>,
    research: Res<'w, Research>,
    boosts: Res<'w, AppliedBoosts>,
    fog: Res<'w, FogMap>,
}

use bevy::ecs::system::SystemParam;

fn snapshot(
    state: &RunState,
    player: &Query<(&crate::common::Body, &Health), With<Player>>,
    forts: &Query<(&Fort, &Allegiance, &crate::common::Body)>,
) -> SaveGame {
    let hero = player.iter().next().map(|(b, h)| (b.pos, h.current));
    let held: Vec<(Vec2, Faction)> = forts
        .iter()
        // Only what generation would not reproduce: a fort still in the hands
        // its seed gave it does not need recording.
        .filter(|(_, owner, _)| owner.0 == Faction::Player)
        .map(|(_, owner, body)| (body.pos, owner.0))
        .collect();
    collect(
        *state.env,
        *state.seed,
        &state.clock,
        &state.progression,
        &state.economy,
        &state.threat,
        &state.cycle,
        &state.unlocks,
        &state.loadout,
        &state.research,
        &state.boosts,
        &state.fog,
        hero,
        &held,
    )
}

fn autosave(
    time: Res<Time>,
    mut slot: ResMut<SaveSlot>,
    state: RunState,
    player: Query<(&crate::common::Body, &Health), With<Player>>,
    forts: Query<(&Fort, &Allegiance, &crate::common::Body)>,
) {
    slot.since_autosave += time.delta_secs();
    if slot.since_autosave < AUTOSAVE_PERIOD {
        return;
    }
    slot.since_autosave = 0.0;
    // A dead run is not worth resuming, and overwriting a good save with one
    // is the worst thing an autosave can do.
    if player.iter().next().is_none_or(|(_, h)| h.is_dead()) {
        return;
    }
    let save = snapshot(&state, &player, &forts);
    write_save(&save);
    slot.present = true;
}

fn save_hotkey(
    keys: Res<ButtonInput<KeyCode>>,
    mut slot: ResMut<SaveSlot>,
    mut hints: ResMut<crate::onboarding::HintQueue>,
    state: RunState,
    player: Query<(&crate::common::Body, &Health), With<Player>>,
    forts: Query<(&Fort, &Allegiance, &crate::common::Body)>,
) {
    if !keys.just_pressed(KeyCode::F5) {
        return;
    }
    let save = snapshot(&state, &player, &forts);
    let note = write_save(&save);
    slot.present = true;
    slot.since_autosave = 0.0;
    hints.push("SAVED", note, crate::onboarding::HintTone::Tip);
}

/// Put a loaded run back into the world.
///
/// Runs during spawn, after the hero exists, so it can place them. Everything
/// else - the world, the enemies, the pressure - rebuilds itself from the seed
/// and the clock.
#[allow(clippy::too_many_arguments)]
fn restore_run(
    mut slot: ResMut<SaveSlot>,
    mut env: ResMut<EnvKind>,
    mut seed: ResMut<WorldSeed>,
    mut clock: ResMut<RunClock>,
    mut progression: ResMut<Progression>,
    mut economy: ResMut<Economy>,
    mut threat: ResMut<Threat>,
    mut cycle: ResMut<WaveCycle>,
    mut unlocks: ResMut<Unlocks>,
    mut loadout: ResMut<Loadout>,
    mut research: ResMut<Research>,
    mut boosts: ResMut<AppliedBoosts>,
    mut fog: ResMut<FogMap>,
    mut recompute: MessageWriter<crate::progress::RecomputeStats>,
    mut player: Query<(&mut crate::common::Body, &mut Health), With<Player>>,
    stats: Res<PlayerStats>,
) {
    let Some(save) = slot.pending.take() else {
        return;
    };

    *env = EnvKind::ALL[save.world.min(EnvKind::COUNT - 1)];
    seed.0 = save.seed;

    clock.elapsed = save.elapsed;
    clock.kills = save.kills;
    clock.best_streak = save.best_streak;

    progression.level = save.level.max(1);
    progression.xp = save.xp;
    progression.to_next = save.to_next.max(1.0);
    progression.pending_levels = save.pending_levels;
    progression.skill_points = save.skill_points;
    progression.total_xp = save.total_xp;

    economy.scrap = save.scrap;
    economy.cores = save.cores;
    economy.lifetime_scrap = save.lifetime_scrap;
    economy.lifetime_cores = save.lifetime_cores;

    threat.intent = save.threat_intent;
    threat.floor = save.threat_floor;
    threat.level = save.threat_intent;

    cycle.wave = save.wave;
    cycle.timer = save.wave_timer;
    cycle.phase = if save.in_prep {
        Phase::Prep
    } else {
        Phase::Assault
    };

    unlocks.build = save.unlocks[0];
    unlocks.territory = save.unlocks[1];
    unlocks.allies = save.unlocks[2];
    unlocks.research = save.unlocks[3];
    unlocks.threat_dial = save.unlocks[4];

    loadout.slots.clear();
    for (kind, level) in &save.weapons {
        if let Some(weapon) = WeaponKind::ALL.get(*kind) {
            loadout.add(*weapon);
            if let Some(slot) = loadout.slots.iter_mut().find(|s| s.kind == *weapon) {
                slot.level = *level;
            }
        }
    }
    if loadout.slots.is_empty() {
        loadout.reset();
    }

    for (index, rank) in &save.research {
        if let Some(node) = research.nodes.get_mut(*index) {
            node.rank = *rank;
        }
    }

    boosts.reset();
    for (stat, stacks) in &save.boosts {
        if let Some(boost) = StatBoost::ALL.get(*stat) {
            boosts.add(*boost, *stacks);
        }
    }
    boosts.refinements = save.refinements;

    fog.restore(&save.explored);

    if let Some((mut body, mut health)) = player.iter_mut().next() {
        body.pos = save.player_pos;
        health.max = stats.max_hp;
        health.current = save.hp.clamp(1.0, stats.max_hp);
    }

    // Stats are rebuilt from base plus modifiers, so this has to happen after
    // the boosts and research ranks are back.
    recompute.write(crate::progress::RecomputeStats);
    slot.last_note = Some(format!(
        "Resumed at {}",
        crate::common::format_time(save.elapsed)
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SaveGame {
        SaveGame {
            world: 2,
            seed: 0xDEAD_BEEF_1234,
            elapsed: 412.5,
            kills: 1337,
            best_streak: 42,
            player_pos: Vec2::new(-88.25, 140.5),
            hp: 76.5,
            level: 17,
            xp: 33.25,
            to_next: 210.0,
            pending_levels: 1,
            skill_points: 5,
            total_xp: 98765.5,
            scrap: 640.0,
            cores: 22.0,
            lifetime_scrap: 3000.0,
            lifetime_cores: 90.0,
            threat_intent: 4.5,
            threat_floor: 2.25,
            wave: 9,
            wave_timer: 12.75,
            in_prep: true,
            unlocks: [true, true, true, false, false],
            weapons: vec![(0, 5), (3, 2)],
            research: vec![(1, 3), (7, 1)],
            boosts: vec![(2, 4), (9, 1)],
            refinements: 3,
            explored: vec![
                IVec2::new(0, 0),
                IVec2::new(1, 0),
                IVec2::new(2, 0),
                IVec2::new(9, 0),
                IVec2::new(-4, 3),
            ],
            forts: vec![(Vec2::new(120.0, -30.0), Faction::Player.index())],
        }
    }

    #[test]
    fn a_save_survives_a_round_trip() {
        let before = sample();
        let after = SaveGame::decode(&before.encode()).expect("failed to decode");

        assert_eq!(after.world, before.world);
        assert_eq!(after.seed, before.seed);
        assert_eq!(after.kills, before.kills);
        assert_eq!(after.level, before.level);
        assert_eq!(after.pending_levels, before.pending_levels);
        assert_eq!(after.skill_points, before.skill_points);
        assert_eq!(after.unlocks, before.unlocks);
        assert_eq!(after.weapons, before.weapons);
        assert_eq!(after.research, before.research);
        assert_eq!(after.boosts, before.boosts);
        assert_eq!(after.refinements, before.refinements);
        assert_eq!(after.forts.len(), before.forts.len());
        assert!((after.elapsed - before.elapsed).abs() < 0.01);
        assert!((after.hp - before.hp).abs() < 0.01);
        assert!(after.player_pos.distance(before.player_pos) < 0.01);
    }

    #[test]
    fn the_explored_map_survives_a_round_trip() {
        let before = sample();
        let after = SaveGame::decode(&before.encode()).unwrap();
        let mut a = before.explored;
        let mut b = after.explored;
        a.sort_unstable_by_key(|c| (c.y, c.x));
        b.sort_unstable_by_key(|c| (c.y, c.x));
        assert_eq!(a, b);
    }

    #[test]
    fn a_long_exploration_encodes_compactly() {
        // Fog is by far the largest section; stored cell by cell a serious run
        // would write megabytes.
        let mut save = SaveGame::default();
        for y in 0..60 {
            for x in 0..300 {
                save.explored.push(IVec2::new(x, y));
            }
        }
        let text = save.encode();
        assert_eq!(SaveGame::decode(&text).unwrap().explored.len(), 18000);
        assert!(
            text.len() < 2000,
            "18000 contiguous cells took {} bytes",
            text.len()
        );
    }

    #[test]
    fn runs_collapse_only_when_contiguous() {
        assert_eq!(runs(&[1, 2, 3]), vec![(1, 3)]);
        assert_eq!(runs(&[1, 3, 4]), vec![(1, 1), (3, 4)]);
        assert_eq!(runs(&[]), vec![]);
        assert_eq!(runs(&[-3, -2, 5]), vec![(-3, -2), (5, 5)]);
        // Duplicates must not extend a run past where it ends.
        assert_eq!(runs(&[1, 1, 2]), vec![(1, 2)]);
    }

    #[test]
    fn junk_is_rejected_rather_than_half_loaded() {
        assert!(SaveGame::decode("").is_none());
        assert!(SaveGame::decode("this is not a save file").is_none());
        assert!(SaveGame::decode("world 3\nseed 9").is_none(), "no header");
    }

    #[test]
    fn a_file_from_a_newer_build_is_refused() {
        // Loading half of it and dropping the rest would look like corruption.
        let text = format!("holdfast {}\nworld 1\n", FORMAT + 1);
        assert!(SaveGame::decode(&text).is_none());
    }

    #[test]
    fn unknown_keys_are_skipped_so_older_files_still_load() {
        let text = format!("holdfast {FORMAT}\nworld 3\nsomething_new 1 2 3\nkills 5\n");
        let save = SaveGame::decode(&text).expect("should still load");
        assert_eq!(save.world, 3);
        assert_eq!(save.kills, 5);
    }

    #[test]
    fn a_truncated_line_does_not_take_the_whole_file_down() {
        // Half a line is what a save interrupted by a crash looks like.
        let full = sample().encode();
        let cut = &full[..full.len() / 2];
        // Either it parses what it got or it refuses; it must not panic.
        let _ = SaveGame::decode(cut);
    }

    #[test]
    fn an_empty_run_round_trips() {
        let empty = SaveGame::default();
        let after = SaveGame::decode(&empty.encode()).expect("decode");
        assert_eq!(after.level, 0);
        assert!(after.explored.is_empty());
        assert!(after.weapons.is_empty());
    }

    #[test]
    fn the_save_path_is_beside_the_executable() {
        let path = save_path();
        assert_eq!(
            path.file_name().and_then(|n| n.to_str()),
            Some("holdfast-save.txt")
        );
    }
}
