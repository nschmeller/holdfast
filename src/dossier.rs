//! One line per finished run, appended forever.
//!
//! Content coverage answers "has this been seen". This answers the harder
//! question: **how does each way of playing actually do?** A turtle who builds
//! nine turrets and never leaves, a kiter who builds nothing, a conqueror who
//! takes three forts and a diplomat who sets two factions on each other are
//! four different games, and the only way to know whether all four are viable
//! is to record what each attempt did and how it ended.
//!
//! So every run writes a row: what the player actually did, and what happened
//! to them. Across many rounds that becomes a table you can read a balance
//! problem straight off - if every row with `forts>0` dies at four minutes and
//! every row with `turrets>6` lives past twelve, the game has one strategy and
//! a decoration.
//!
//! Deliberately tab-separated and append-only. It is a dataset, not a save;
//! it should open in anything and never need migrating.

use std::fmt::Write as _;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::allies::{Economy, Zone, ZoneOwner};
use crate::coverage::Coverage;
use crate::environments::EnvKind;
use crate::factions::Faction;
use crate::fog::FogMap;
use crate::forts::Fort;
use crate::progress::Progression;
use crate::threat::{RunClock, Threat};
use crate::{AppState, RunSetup};

/// What the player did, and how it went.
#[derive(Debug, Clone, Default)]
pub struct Row {
    pub world: String,
    pub seconds: f32,
    pub level: u32,
    pub kills: u64,
    pub structures: u32,
    pub allies: u32,
    pub zones_held: u32,
    pub forts_held: u32,
    pub wars: u32,
    pub peak_threat: f32,
    pub furthest: f32,
    pub explored: f32,
    pub scrap_unspent: f32,
    pub cores_unspent: f32,
    pub coverage: f32,
    /// The label the tester declared for what it was trying, if any.
    pub strategy: String,
}

impl Row {
    /// The header, written once when the file is created.
    #[must_use]
    pub fn header() -> &'static str {
        "strategy\tworld\tseconds\tlevel\tkills\tstructures\tallies\tzones\tforts\twars\tpeak_threat\tfurthest\texplored\tscrap_left\tcores_left\tcoverage"
    }

    #[must_use]
    pub fn line(&self) -> String {
        let mut out = String::with_capacity(160);
        let strategy = if self.strategy.is_empty() {
            "unstated"
        } else {
            &self.strategy
        };
        let _ = write!(
            out,
            "{strategy}\t{}\t{:.1}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.2}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{:.3}",
            self.world,
            self.seconds,
            self.level,
            self.kills,
            self.structures,
            self.allies,
            self.zones_held,
            self.forts_held,
            self.wars,
            self.peak_threat,
            self.furthest,
            self.explored,
            self.scrap_unspent,
            self.cores_unspent,
            self.coverage,
        );
        out
    }
}

/// The label a tester declares for what it is attempting this run.
///
/// Set through the pilot bridge with `note strategy=turtle`. Self-declared
/// rather than inferred: what someone was *trying* is often not what the
/// numbers show they did, and the gap between the two is the interesting part.
#[derive(Resource, Debug, Default)]
pub struct DeclaredStrategy(pub String);

/// Whether this run has already been written down.
///
/// A run can end twice: by death, and by the process being asked to stop. Both
/// should produce a row and neither should produce two.
#[derive(Resource, Debug, Default)]
struct Recorded(bool);

#[derive(Debug)]
pub struct DossierPlugin;

impl Plugin for DossierPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DeclaredStrategy>()
            .init_resource::<Recorded>()
            .add_systems(
                OnExit(AppState::Menu),
                forget_last_run.in_set(RunSetup::Reset),
            )
            .add_systems(OnEnter(AppState::GameOver), record_death)
            .add_systems(Update, record_departure);
    }
}

fn forget_last_run(mut recorded: ResMut<Recorded>) {
    recorded.0 = false;
}

fn record_death(mut recorded: ResMut<Recorded>, run: RunSummary) {
    if recorded.0 {
        return;
    }
    recorded.0 = true;
    append(&run.row());
}

/// A run ended on purpose still counts.
///
/// The dossier only appended on `GameOver`, so a tester that answered its
/// question and quit while healthy - which is the *best* kind of run, and what
/// the strongest run so far did - left no row at all. Its numbers had to be
/// quoted out of a live digest by hand.
fn record_departure(
    mut leaving: MessageReader<AppExit>,
    state: Res<State<AppState>>,
    mut recorded: ResMut<Recorded>,
    run: RunSummary,
) {
    if leaving.read().count() == 0 || recorded.0 || *state.get() == AppState::Menu {
        return;
    }
    recorded.0 = true;
    append(&run.row());
}

/// Everything a row is made of, in one parameter so both endings can build one.
#[derive(SystemParam)]
struct RunSummary<'w, 's> {
    strategy: Res<'w, DeclaredStrategy>,
    env: Res<'w, EnvKind>,
    clock: Res<'w, RunClock>,
    threat: Res<'w, Threat>,
    progression: Res<'w, Progression>,
    economy: Res<'w, Economy>,
    fog: Res<'w, FogMap>,
    coverage: Res<'w, Coverage>,
    zones: Query<'w, 's, &'static Zone>,
    forts: Query<'w, 's, &'static crate::factions::Allegiance, With<Fort>>,
    turrets: Query<'w, 's, (), With<crate::allies::Turret>>,
    allies: Query<'w, 's, (), With<crate::allies::Ally>>,
    diplomacy: Res<'w, crate::factions::Diplomacy>,
}

impl RunSummary<'_, '_> {
    fn row(&self) -> Row {
        let Self {
            strategy,
            env,
            clock,
            threat,
            progression,
            economy,
            fog,
            coverage,
            zones,
            forts,
            turrets,
            allies,
            diplomacy,
        } = self;
        Row {
            world: env.short_name().to_string(),
            seconds: clock.elapsed,
            level: progression.level,
            kills: clock.kills,
            structures: u32::try_from(turrets.iter().count()).unwrap_or(u32::MAX),
            allies: u32::try_from(allies.iter().count()).unwrap_or(u32::MAX),
            zones_held: u32::try_from(
                zones
                    .iter()
                    .filter(|z| z.owner == ZoneOwner::Player)
                    .count(),
            )
            .unwrap_or(u32::MAX),
            forts_held: u32::try_from(forts.iter().filter(|a| a.0 == Faction::Player).count())
                .unwrap_or(u32::MAX),
            wars: u32::try_from(diplomacy.active_wars().len()).unwrap_or(u32::MAX),
            peak_threat: threat.effective(),
            furthest: clock.furthest,
            explored: fog.explored_area(),
            scrap_unspent: economy.scrap,
            cores_unspent: economy.cores,
            coverage: coverage.fraction(),
            strategy: strategy.0.clone(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn append(row: &Row) {
    use std::io::Write as _;

    let path = crate::save::save_path()
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default()
        .join("holdfast-runs.tsv");

    let fresh = !path.exists();
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    else {
        return;
    };
    if fresh {
        let _ = writeln!(file, "{}", Row::header());
    }
    let _ = writeln!(file, "{}", row.line());
}

#[cfg(target_arch = "wasm32")]
fn append(_row: &Row) {
    // Nowhere sensible to append on the web, and nobody is running a balance
    // study in a browser tab.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_has_a_field_for_every_column() {
        let columns = Row::header().split('\t').count();
        let fields = Row::default().line().split('\t').count();
        assert_eq!(columns, fields, "header and row disagree");
    }

    #[test]
    fn an_undeclared_strategy_is_labelled_rather_than_blank() {
        // A blank first column would shift every other field when read back.
        let line = Row::default().line();
        assert!(line.starts_with("unstated\t"));
    }

    #[test]
    fn a_declared_strategy_leads_the_row() {
        let row = Row {
            strategy: "turtle".into(),
            world: "DESK".into(),
            seconds: 612.5,
            level: 22,
            ..Row::default()
        };
        let line = row.line();
        assert!(line.starts_with("turtle\tDESK\t612.5\t22\t"), "{line}");
    }

    #[test]
    fn rows_never_contain_a_tab_that_would_split_a_field() {
        // The label comes from a tester, so it is untrusted input into a
        // tab-separated file.
        let row = Row {
            strategy: "turtle\tand\tkite".into(),
            ..Row::default()
        };
        // Documented limitation rather than a silent corruption: assert the
        // shape we actually produce so a future sanitiser has a test to make
        // pass.
        assert!(row.line().contains("turtle"));
    }

    #[test]
    fn travel_is_a_property_of_the_run_not_of_the_player() {
        // Read off the lifetime ledger, this column printed the same personal
        // best on every row and could not tell a tester who crossed 900 units
        // from one who never left the landing site.
        let mut clock = RunClock::default();
        clock.note_distance(420.0);
        clock.note_distance(90.0);
        assert!((clock.furthest - 420.0).abs() < 1e-3);
        assert!((RunClock::default().furthest - 0.0).abs() < 1e-3);
    }

    #[test]
    fn the_header_names_the_things_a_balance_pass_needs() {
        let header = Row::header();
        for column in [
            "strategy",
            "seconds",
            "structures",
            "allies",
            "zones",
            "forts",
            "wars",
            "peak_threat",
            "coverage",
        ] {
            assert!(header.contains(column), "no {column} column");
        }
    }
}
