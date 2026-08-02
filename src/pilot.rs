//! Agent pilot bridge: a file-based control and telemetry channel.
//!
//! The game is keyboard-only and its whole surface is "look at the screen and
//! press keys", which makes it unverifiable without a human. `devtools` covers
//! scripted one-shots; this covers the other half - an outside process that
//! wants to *play*: read the situation, decide, act, read the result.
//!
//! Set `DFFA_PILOT=<dir>` and the game will:
//!
//! - poll `<dir>/commands` for appended command lines,
//! - inject the resulting key presses straight into [`ButtonInput<KeyCode>`],
//! - rewrite `<dir>/state.json` a few times a second with a full situation
//!   report,
//! - append notable events to `<dir>/log.txt`.
//!
//! # Commands
//!
//! One per line. Blank lines and `#` comments are ignored. Keys are single
//! letters (`W`), digits (`1`), or names (`SPACE`, `ENTER`, `ESC`, `SHIFT`,
//! `UP`, `DOWN`, `LEFT`, `RIGHT`, `MINUS`, `EQUAL`, `TAB`, `BACKSPACE`).
//!
//! | Command | Effect |
//! | --- | --- |
//! | `tap W` | Down for exactly one frame, so `just_pressed` fires once |
//! | `hold W A 1.5` | Down for 1.5 seconds, then up |
//! | `press W` | Down and stay down |
//! | `release W` / `release all` | Up |
//! | `wait 2` | Do nothing for two seconds |
//! | `shot out.png` | Screenshot the window |
//! | `note anything` | Write a line to the log |
//! | `quit` | Exit the game |
//!
//! Commands run in order. `tap`, `hold`, `wait` and `shot` each consume at
//! least a frame; the rest are free, so `press W` and `tap SPACE` on adjacent
//! lines both land promptly.
//!
//! Nothing here is compiled out of release builds, but it is completely inert
//! unless the variable is set: no directory, no systems, no cost.

use std::collections::{HashSet, VecDeque};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::path::{Path, PathBuf};

use bevy::ecs::system::SystemParam;
use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::AppState;
use crate::allies::{Ally, Economy, Squad, Turret, Zone, ZoneOwner};
use crate::common::{Body, Health};
use crate::enemy::{Enemy, Rank};
use crate::environments::EnvKind;
use crate::onboarding::{HintQueue, Unlocks};
use crate::player::{Dash, Player, PlayerStats};
use crate::progress::{CardOffer, Equipped, GearSlot, Progression};
use crate::threat::{RunClock, Threat, WaveCycle};
use crate::weapons::Loadout;

/// How often the situation report is rewritten, in real seconds. Fast enough
/// that a reader never acts on stale information, slow enough that it is not
/// a per-frame file write.
const SNAPSHOT_PERIOD: f32 = 0.2;

/// Cap on the queue. A runaway writer should not be able to grow the game's
/// memory without bound.
const MAX_QUEUED: usize = 4096;

// -- json -------------------------------------------------------------------

/// A minimal JSON writer.
///
/// The crate has no serialisation dependency and this is the only thing that
/// wants one. Containers are opened and closed explicitly; the comma bookkeeping
/// is the only thing worth automating.
#[derive(Debug, Default)]
struct Json {
    buf: String,
    /// Closing brace for each container still open, innermost last.
    stack: Vec<char>,
    /// Whether the container currently being written already has a member.
    comma: bool,
}

impl Json {
    fn new() -> Self {
        let mut json = Self::default();
        json.buf.push('{');
        json.stack.push('}');
        json
    }

    fn sep(&mut self) {
        if self.comma {
            self.buf.push(',');
        }
        self.comma = true;
    }

    fn key(&mut self, key: &str) {
        self.sep();
        self.buf.push('"');
        self.buf.push_str(key);
        self.buf.push_str("\":");
    }

    fn open(&mut self, key: &str, brace: char) {
        self.key(key);
        self.buf.push(brace);
        self.stack.push(if brace == '{' { '}' } else { ']' });
        self.comma = false;
    }

    fn obj(&mut self, key: &str) {
        self.open(key, '{');
    }

    fn arr(&mut self, key: &str) {
        self.open(key, '[');
    }

    /// Start an object as an array element rather than under a key.
    fn item(&mut self) {
        self.sep();
        self.buf.push('{');
        self.stack.push('}');
        self.comma = false;
    }

    /// Close the innermost container. Closing more than was opened is a bug in
    /// the caller, so it is a no-op rather than a panic in a shipped binary.
    fn end(&mut self) {
        if let Some(brace) = self.stack.pop() {
            self.buf.push(brace);
        }
        // Whatever we just closed is now a member of its parent.
        self.comma = true;
    }

    fn num(&mut self, key: &str, value: f32) {
        self.key(key);
        push_num(&mut self.buf, value);
    }

    fn int(&mut self, key: &str, value: impl Into<i64>) {
        self.key(key);
        let _ = write!(self.buf, "{}", value.into());
    }

    /// A count arriving as an unsigned type. Saturates rather than wrapping:
    /// JSON has no unsigned integers, and nothing counted here comes close.
    fn count(&mut self, key: &str, value: impl TryInto<i64>) {
        self.int(key, value.try_into().unwrap_or(i64::MAX));
    }

    fn flag(&mut self, key: &str, value: bool) {
        self.key(key);
        self.buf.push_str(if value { "true" } else { "false" });
    }

    fn text(&mut self, key: &str, value: &str) {
        self.key(key);
        push_str(&mut self.buf, value);
    }

    /// A string field that may be absent, written as `null` when it is.
    fn maybe(&mut self, key: &str, value: Option<&str>) {
        if let Some(v) = value {
            self.text(key, v);
        } else {
            self.key(key);
            self.buf.push_str("null");
        }
    }

    fn vec2(&mut self, key: &str, value: Vec2) {
        self.key(key);
        self.buf.push('[');
        push_num(&mut self.buf, value.x);
        self.buf.push(',');
        push_num(&mut self.buf, value.y);
        self.buf.push(']');
    }

    /// A bare string as an array element.
    fn push_text(&mut self, value: &str) {
        self.sep();
        push_str(&mut self.buf, value);
    }

    fn finish(mut self) -> String {
        while let Some(brace) = self.stack.pop() {
            self.buf.push(brace);
        }
        self.buf.push('\n');
        self.buf
    }
}

/// Round to three decimals and never emit `NaN` or `Infinity`, neither of
/// which is JSON. A reader that gets `null` can say so; one that gets `NaN`
/// just fails to parse the whole report.
fn push_num(buf: &mut String, value: f32) {
    if value.is_finite() {
        let _ = write!(buf, "{value:.3}");
    } else {
        buf.push_str("null");
    }
}

fn push_str(buf: &mut String, value: &str) {
    buf.push('"');
    for c in value.chars() {
        match c {
            '"' => buf.push_str("\\\""),
            '\\' => buf.push_str("\\\\"),
            '\n' => buf.push_str("\\n"),
            '\r' => buf.push_str("\\r"),
            '\t' => buf.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(buf, "\\u{:04x}", c as u32);
            }
            c => buf.push(c),
        }
    }
    buf.push('"');
}

// -- commands ---------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Cmd {
    Press(Vec<KeyCode>),
    Release(Vec<KeyCode>),
    ReleaseAll,
    Tap(Vec<KeyCode>),
    Hold(Vec<KeyCode>, f32),
    Wait(f32),
    Shot(String),
    Note(String),
    Quit,
}

impl Cmd {
    /// Whether running this command should end the frame's queue processing.
    ///
    /// Two taps in one frame would collapse into one `just_pressed`, and a
    /// screenshot wants the frame it was asked for; presses and notes are
    /// unobservable on their own and can batch freely.
    fn consumes_frame(&self) -> bool {
        matches!(
            self,
            Self::Tap(_) | Self::Hold(..) | Self::Wait(_) | Self::Shot(_)
        )
    }
}

const LETTERS: [KeyCode; 26] = [
    KeyCode::KeyA,
    KeyCode::KeyB,
    KeyCode::KeyC,
    KeyCode::KeyD,
    KeyCode::KeyE,
    KeyCode::KeyF,
    KeyCode::KeyG,
    KeyCode::KeyH,
    KeyCode::KeyI,
    KeyCode::KeyJ,
    KeyCode::KeyK,
    KeyCode::KeyL,
    KeyCode::KeyM,
    KeyCode::KeyN,
    KeyCode::KeyO,
    KeyCode::KeyP,
    KeyCode::KeyQ,
    KeyCode::KeyR,
    KeyCode::KeyS,
    KeyCode::KeyT,
    KeyCode::KeyU,
    KeyCode::KeyV,
    KeyCode::KeyW,
    KeyCode::KeyX,
    KeyCode::KeyY,
    KeyCode::KeyZ,
];

const DIGITS: [KeyCode; 10] = [
    KeyCode::Digit0,
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

/// Resolve a key name written by a human or an agent.
fn key_from_name(name: &str) -> Option<KeyCode> {
    let upper = name.to_ascii_uppercase();
    let bytes = upper.as_bytes();
    if bytes.len() == 1 {
        let c = bytes[0];
        if c.is_ascii_uppercase() {
            return Some(LETTERS[(c - b'A') as usize]);
        }
        if c.is_ascii_digit() {
            return Some(DIGITS[(c - b'0') as usize]);
        }
    }
    Some(match upper.as_str() {
        "SPACE" | "SPACEBAR" => KeyCode::Space,
        "ENTER" | "RETURN" => KeyCode::Enter,
        "ESC" | "ESCAPE" => KeyCode::Escape,
        "TAB" => KeyCode::Tab,
        "SHIFT" => KeyCode::ShiftLeft,
        "BACKSPACE" => KeyCode::Backspace,
        "UP" => KeyCode::ArrowUp,
        "DOWN" => KeyCode::ArrowDown,
        "LEFT" => KeyCode::ArrowLeft,
        "RIGHT" => KeyCode::ArrowRight,
        "MINUS" => KeyCode::Minus,
        "EQUAL" | "PLUS" => KeyCode::Equal,
        _ => return None,
    })
}

/// Parse one command line. `Err` carries a message worth logging back.
fn parse_line(line: &str) -> Result<Option<Cmd>, String> {
    let line = line.split('#').next().unwrap_or("").trim();
    if line.is_empty() {
        return Ok(None);
    }
    let mut words = line.split_whitespace();
    let verb = words.next().unwrap_or_default().to_ascii_lowercase();
    let rest: Vec<&str> = words.collect();

    let keys = |args: &[&str]| -> Result<Vec<KeyCode>, String> {
        if args.is_empty() {
            return Err(format!("{verb}: no keys given"));
        }
        args.iter()
            .map(|a| key_from_name(a).ok_or_else(|| format!("unknown key {a:?}")))
            .collect()
    };

    match verb.as_str() {
        "press" => Ok(Some(Cmd::Press(keys(&rest)?))),
        "tap" => Ok(Some(Cmd::Tap(keys(&rest)?))),
        "release" => {
            if rest.first().is_some_and(|a| a.eq_ignore_ascii_case("all")) {
                Ok(Some(Cmd::ReleaseAll))
            } else {
                Ok(Some(Cmd::Release(keys(&rest)?)))
            }
        }
        "hold" => {
            let (last, head) = rest
                .split_last()
                .ok_or_else(|| "hold: expected keys then seconds".to_string())?;
            let secs: f32 = last
                .parse()
                .map_err(|_| format!("hold: {last:?} is not a duration"))?;
            Ok(Some(Cmd::Hold(keys(head)?, secs.clamp(0.0, 600.0))))
        }
        "wait" => {
            let secs: f32 = rest
                .first()
                .ok_or_else(|| "wait: expected seconds".to_string())?
                .parse()
                .map_err(|_| "wait: not a number".to_string())?;
            Ok(Some(Cmd::Wait(secs.clamp(0.0, 600.0))))
        }
        "shot" | "screenshot" => Ok(Some(Cmd::Shot(
            rest.first()
                .ok_or_else(|| "shot: expected a path".to_string())?
                .to_string(),
        ))),
        "note" => Ok(Some(Cmd::Note(rest.join(" ")))),
        "quit" | "exit" => Ok(Some(Cmd::Quit)),
        other => Err(format!("unknown command {other:?}")),
    }
}

// -- the channel ------------------------------------------------------------

/// A timed command currently occupying the queue head.
#[derive(Debug)]
struct Active {
    remaining: f32,
    /// Keys to let go of when the timer runs out.
    release: Vec<KeyCode>,
}

/// Values carried between snapshots so the report can describe change rather
/// than only state.
#[derive(Debug, Default)]
struct Previous {
    hp: f32,
    level: u32,
    wave: u32,
    kills: u64,
    unlocks: u32,
    zones: usize,
    state: String,
    hint: String,
}

#[derive(Resource, Debug)]
struct Pilot {
    dir: PathBuf,
    /// How far into the command file we have already read.
    cursor: u64,
    /// A trailing fragment of a line that has not been terminated yet.
    partial: String,
    queue: VecDeque<Cmd>,
    active: Option<Active>,
    held: HashSet<KeyCode>,
    /// Keys pressed for exactly this frame, released at the start of the next.
    tapped: Vec<KeyCode>,
    /// Notable changes since the last snapshot.
    events: Vec<String>,
    seq: u64,
    since_snapshot: f32,
    wall: f32,
    prev: Previous,
}

impl Pilot {
    fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            cursor: 0,
            partial: String::new(),
            queue: VecDeque::new(),
            active: None,
            held: HashSet::new(),
            tapped: Vec::new(),
            events: Vec::new(),
            seq: 0,
            since_snapshot: SNAPSHOT_PERIOD,
            wall: 0.0,
            prev: Previous::default(),
        }
    }

    fn commands_path(&self) -> PathBuf {
        self.dir.join("commands")
    }

    fn record(&mut self, line: impl Into<String>) {
        let line = line.into();
        append_line(
            &self.dir.join("log.txt"),
            &format!("{:8.2}  {line}", self.wall),
        );
        self.events.push(line);
    }
}

fn append_line(path: &Path, line: &str) {
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

#[derive(Debug)]
pub struct PilotPlugin;

impl Plugin for PilotPlugin {
    fn build(&self, app: &mut App) {
        let Ok(dir) = env::var("DFFA_PILOT") else {
            return;
        };
        let dir = PathBuf::from(dir);
        if let Err(err) = fs::create_dir_all(&dir) {
            error!("DFFA_PILOT: cannot use {}: {err}", dir.display());
            return;
        }
        // Start from a clean channel: a stale command file from a previous run
        // would otherwise be replayed the moment this one starts.
        let _ = fs::write(dir.join("commands"), "");
        let _ = fs::write(dir.join("log.txt"), "");
        info!("pilot bridge listening in {}", dir.display());

        app.insert_resource(Pilot::new(dir))
            // After the real keyboard is polled, because that clears the
            // just-pressed sets at the top of every frame.
            .add_systems(
                PreUpdate,
                (read_commands, run_queue).chain().after(InputSystems),
            )
            .add_systems(Last, write_snapshot);
    }
}

/// Pull any newly appended bytes out of the command file.
fn read_commands(mut pilot: ResMut<Pilot>) {
    let path = pilot.commands_path();
    let Ok(mut file) = fs::File::open(&path) else {
        return;
    };
    let len = file.metadata().map_or(0, |m| m.len());
    if len < pilot.cursor {
        // The file was truncated or replaced; start over rather than read junk.
        pilot.cursor = 0;
        pilot.partial.clear();
    }
    if len == pilot.cursor {
        return;
    }
    if file.seek(SeekFrom::Start(pilot.cursor)).is_err() {
        return;
    }
    let mut fresh = String::new();
    let Ok(read) = file.read_to_string(&mut fresh) else {
        return;
    };
    pilot.cursor += read as u64;

    let mut buffer = std::mem::take(&mut pilot.partial);
    buffer.push_str(&fresh);
    // Anything after the final newline is an incomplete line; hold it back so
    // a half-flushed write is never parsed as a truncated command.
    let (whole, tail) = match buffer.rfind('\n') {
        Some(at) => buffer.split_at(at + 1),
        None => ("", buffer.as_str()),
    };
    let lines: Vec<String> = whole.lines().map(str::to_owned).collect();
    tail.clone_into(&mut pilot.partial);

    for line in lines {
        if pilot.queue.len() >= MAX_QUEUED {
            pilot.record("queue full, dropping commands");
            break;
        }
        match parse_line(&line) {
            Ok(Some(cmd)) => pilot.queue.push_back(cmd),
            Ok(None) => {}
            Err(err) => pilot.record(format!("bad command: {err}")),
        }
    }
}

/// Advance the queue and write the resulting key state.
fn run_queue(
    mut pilot: ResMut<Pilot>,
    time: Res<Time<Real>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let dt = time.delta_secs();
    pilot.wall += dt;

    for key in std::mem::take(&mut pilot.tapped) {
        if !pilot.held.contains(&key) {
            keys.release(key);
        }
    }

    if let Some(active) = pilot.active.as_mut() {
        active.remaining -= dt;
        if active.remaining <= 0.0 {
            let release = std::mem::take(&mut active.release);
            pilot.active = None;
            for key in release {
                pilot.held.remove(&key);
                keys.release(key);
            }
        }
    }

    while pilot.active.is_none() {
        let Some(cmd) = pilot.queue.pop_front() else {
            break;
        };
        let stop = cmd.consumes_frame();
        match cmd {
            Cmd::Press(list) => {
                for key in list {
                    pilot.held.insert(key);
                }
            }
            Cmd::Release(list) => {
                for key in list {
                    pilot.held.remove(&key);
                    keys.release(key);
                }
            }
            Cmd::ReleaseAll => {
                for key in std::mem::take(&mut pilot.held) {
                    keys.release(key);
                }
            }
            Cmd::Tap(list) => {
                for key in list {
                    keys.press(key);
                    pilot.tapped.push(key);
                }
            }
            Cmd::Hold(list, secs) => {
                for key in &list {
                    pilot.held.insert(*key);
                }
                pilot.active = Some(Active {
                    remaining: secs,
                    release: list,
                });
            }
            Cmd::Wait(secs) => {
                pilot.active = Some(Active {
                    remaining: secs,
                    release: Vec::new(),
                });
            }
            Cmd::Shot(path) => {
                pilot.record(format!("screenshot -> {path}"));
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(path));
            }
            Cmd::Note(text) => pilot.record(format!("note: {text}")),
            Cmd::Quit => {
                pilot.record("quit requested");
                exit.write(AppExit::Success);
            }
        }
        if stop {
            break;
        }
    }

    // Re-press every frame: `press` is idempotent once the key is down, and
    // this keeps a held key down across a focus loss, which clears the set.
    let held: Vec<KeyCode> = pilot.held.iter().copied().collect();
    for key in held {
        keys.press(key);
    }
}

// -- the situation report ---------------------------------------------------

#[derive(SystemParam)]
struct Pacing<'w> {
    state: Res<'w, State<AppState>>,
    env: Res<'w, EnvKind>,
    clock: Res<'w, RunClock>,
    threat: Res<'w, Threat>,
    cycle: Res<'w, WaveCycle>,
}

#[derive(SystemParam)]
struct Sheet<'w> {
    stats: Res<'w, PlayerStats>,
    progression: Res<'w, Progression>,
    economy: Res<'w, Economy>,
    squad: Res<'w, Squad>,
    loadout: Res<'w, Loadout>,
    equipped: Res<'w, Equipped>,
}

#[derive(SystemParam)]
struct Meta<'w> {
    unlocks: Res<'w, Unlocks>,
    plan: Res<'w, crate::command::PlanMode>,
    offer: Res<'w, CardOffer>,
    hints: Res<'w, HintQueue>,
}

#[derive(SystemParam)]
struct Field<'w, 's> {
    player: Query<'w, 's, (&'static Body, &'static Health, Option<&'static Dash>), With<Player>>,
    enemies: Query<'w, 's, (&'static Enemy, &'static Body)>,
    zones: Query<'w, 's, (&'static Zone, &'static Body)>,
    turrets: Query<'w, 's, (&'static Turret, &'static Body, &'static Health)>,
    allies: Query<'w, 's, (&'static Ally, &'static Health)>,
}

fn write_snapshot(
    mut pilot: ResMut<Pilot>,
    time: Res<Time<Real>>,
    pacing: Pacing,
    sheet: Sheet,
    meta: Meta,
    field: Field,
) {
    pilot.since_snapshot += time.delta_secs();
    if pilot.since_snapshot < SNAPSHOT_PERIOD {
        return;
    }
    pilot.since_snapshot = 0.0;
    pilot.seq += 1;

    let env = *pacing.env;
    let player = field.player.iter().next();
    let hero_pos = player.map_or(Vec2::ZERO, |(body, _, _)| body.pos);

    note_changes(&mut pilot, &pacing, &sheet, &meta, &field, player, hero_pos);

    let mut json = Json::new();
    json.count("seq", pilot.seq);
    json.num("wall", pilot.wall);
    json.text("state", &format!("{:?}", pacing.state.get()));
    json.text("world", env.title());
    json.count("queued", pilot.queue.len());

    json.obj("run");
    json.num("elapsed", pacing.clock.elapsed);
    json.count("kills", pacing.clock.kills);
    json.int("best_streak", pacing.clock.best_streak);
    json.end();

    json.obj("player");
    if let Some((body, health, dash)) = player {
        json.num("hp", health.current);
        json.num("max_hp", health.max);
        json.vec2("pos", body.pos);
        json.flag("dash_ready", dash.is_none_or(|d| d.cooldown <= 0.0));
    } else {
        json.maybe("hp", None);
    }
    json.int("level", sheet.progression.level);
    json.num("xp_fraction", sheet.progression.fraction());
    json.int("pending_levels", sheet.progression.pending_levels);
    json.int("skill_points", sheet.progression.skill_points);
    json.num("move_speed", sheet.stats.move_speed);
    json.num("damage_mult", sheet.stats.damage_mult);
    json.num("armor", sheet.stats.armor);
    json.end();

    json.obj("threat");
    json.num("level", pacing.threat.level);
    json.num("intent", pacing.threat.intent);
    json.num("floor", pacing.threat.floor);
    json.num("effective", pacing.threat.effective());
    json.num("reward_mult", pacing.threat.reward_mult());
    json.num("from_territory", pacing.threat.territory);
    json.flag("surging", pacing.threat.surging());
    json.flag("surge_ready", pacing.threat.can_surge());
    json.end();

    json.obj("wave");
    json.text("phase", &format!("{:?}", pacing.cycle.phase));
    json.int("number", pacing.cycle.wave);
    json.num("timer", pacing.cycle.timer);
    json.num("budget", pacing.cycle.budget);
    json.num("early_bonus", pacing.cycle.early_bonus);
    json.num("bonus_if_called_now", pacing.cycle.pending_bonus());
    json.end();

    json.obj("economy");
    json.num("scrap", sheet.economy.scrap);
    json.num("cores", sheet.economy.cores);
    json.num("scrap_per_sec", sheet.economy.scrap_rate);
    json.end();

    write_forces(&mut json, &sheet, &field, hero_pos);
    write_field(&mut json, &field, hero_pos);
    write_meta(&mut json, &meta, &sheet);

    json.arr("events");
    for event in std::mem::take(&mut pilot.events) {
        json.push_text(&event);
    }
    json.end();

    let body = json.finish();
    let dir = pilot.dir.clone();
    let tmp = dir.join("state.json.tmp");
    // Write-then-rename: a reader polling the file must never see half a
    // report, and rename is atomic on every platform this ships to.
    if fs::write(&tmp, body).is_ok() {
        let _ = fs::rename(&tmp, dir.join("state.json"));
    }
}

fn write_forces(json: &mut Json, sheet: &Sheet, field: &Field, hero: Vec2) {
    json.arr("weapons");
    for slot in &sheet.loadout.slots {
        json.item();
        json.text("name", slot.kind.name());
        json.int("level", slot.level);
        json.end();
    }
    json.end();

    json.obj("gear");
    for slot in GearSlot::ALL {
        let key = slot.label().to_ascii_lowercase();
        let piece = sheet.equipped.get(slot);
        json.maybe(
            &key,
            piece
                .map(|p| format!("{} ({})", p.name, p.describe()))
                .as_deref(),
        );
    }
    json.end();

    json.obj("squad");
    json.text("stance", &format!("{:?}", sheet.squad.stance));
    json.int("count", sheet.squad.count);
    json.int("cap", sheet.squad.cap);
    json.arr("members");
    for (ally, health) in &field.allies {
        json.item();
        json.text("kind", ally.kind.name());
        json.int("level", ally.level);
        json.num("hp", health.current);
        json.end();
    }
    json.end();
    json.end();

    json.arr("turrets");
    for (turret, body, health) in &field.turrets {
        json.item();
        json.text("kind", turret.kind.name());
        json.int("level", turret.level);
        json.num("hp", health.current);
        json.num("dist", body.pos.distance(hero));
        json.end();
    }
    json.end();
}

fn write_field(json: &mut Json, field: &Field, hero: Vec2) {
    let mut total = 0i64;
    let mut elites = 0i64;
    let mut bosses = 0i64;
    let mut close = 0i64;
    let mut nearest = f32::INFINITY;
    let mut kinds: Vec<(&'static str, i64)> = Vec::new();
    for (enemy, body) in &field.enemies {
        total += 1;
        match enemy.rank {
            Rank::Elite => elites += 1,
            Rank::Boss => bosses += 1,
            Rank::Normal => {}
        }
        let dist = body.pos.distance(hero);
        nearest = nearest.min(dist);
        if dist < 12.0 {
            close += 1;
        }
        let name = enemy.kind.name(EnvKind::Desk);
        match kinds.iter_mut().find(|(k, _)| *k == name) {
            Some((_, n)) => *n += 1,
            None => kinds.push((name, 1)),
        }
    }
    kinds.sort_unstable_by_key(|&(_, n)| -n);

    json.obj("enemies");
    json.int("total", total);
    json.int("elites", elites);
    json.int("bosses", bosses);
    json.int("within_12m", close);
    json.num("nearest_dist", nearest);
    json.arr("by_kind");
    for (name, count) in kinds {
        json.item();
        json.text("kind", name);
        json.int("count", count);
        json.end();
    }
    json.end();
    json.end();

    let mut zones: Vec<(f32, &Zone, Vec2)> = field
        .zones
        .iter()
        .map(|(zone, body)| (body.pos.distance(hero), zone, body.pos))
        .collect();
    zones.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));

    json.arr("zones");
    for (dist, zone, pos) in zones.iter().take(8) {
        json.item();
        json.text("owner", &format!("{:?}", zone.owner));
        json.num("progress", zone.progress);
        json.flag("contested", zone.contested);
        json.num("dist", *dist);
        json.vec2("pos", *pos);
        json.end();
    }
    json.end();
}

fn write_meta(json: &mut Json, meta: &Meta, sheet: &Sheet) {
    json.obj("unlocks");
    json.flag("build", meta.unlocks.build);
    json.flag("territory", meta.unlocks.territory);
    json.flag("allies", meta.unlocks.allies);
    json.flag("research", meta.unlocks.research);
    json.flag("threat_dial", meta.unlocks.threat_dial);
    json.end();

    json.obj("plan_mode");
    json.flag("active", meta.plan.active);
    json.vec2("cursor", meta.plan.cursor);
    json.text("selected", meta.plan.selected_kind().name());
    json.flag("valid_site", meta.plan.valid);
    json.maybe(
        "message",
        meta.plan.message.as_ref().map(|(m, _)| m.as_str()),
    );
    json.end();

    json.arr("card_offer");
    for card in &meta.offer.cards {
        json.item();
        json.text("title", &card.title);
        json.text("detail", &card.detail);
        json.count("rarity", card.rarity);
        json.end();
    }
    json.end();
    json.flag("reroll_available", meta.offer.reroll_available);

    json.obj("hint");
    match &meta.hints.active {
        Some(hint) => {
            json.text("headline", &hint.headline);
            json.text("detail", &hint.detail);
        }
        None => json.maybe("headline", None),
    }
    json.end();

    json.count("weapon_slots_used", sheet.loadout.slots.len());
}

/// Diff against the previous snapshot and record anything a player would have
/// noticed. Reading a stream of these is much cheaper than diffing whole
/// reports on the far side of the channel.
fn note_changes(
    pilot: &mut Pilot,
    pacing: &Pacing,
    sheet: &Sheet,
    meta: &Meta,
    field: &Field,
    player: Option<(&Body, &Health, Option<&Dash>)>,
    _hero: Vec2,
) {
    let state = format!("{:?}", pacing.state.get());
    if state != pilot.prev.state {
        if !pilot.prev.state.is_empty() {
            pilot.record(format!("state {} -> {state}", pilot.prev.state));
        }
        pilot.prev.state = state;
    }

    if let Some((_, health, _)) = player {
        let hp = health.current;
        let drop = pilot.prev.hp - hp;
        if drop >= 1.0 && pilot.prev.hp > 0.0 {
            pilot.record(format!(
                "took {drop:.0} damage, hp {hp:.0}/{:.0}",
                health.max
            ));
        }
        pilot.prev.hp = hp;
    }

    let level = sheet.progression.level;
    if level != pilot.prev.level {
        if pilot.prev.level > 0 {
            pilot.record(format!("reached level {level}"));
        }
        pilot.prev.level = level;
    }

    let wave = pacing.cycle.wave;
    if wave != pilot.prev.wave {
        pilot.record(format!("wave {wave} incoming"));
        pilot.prev.wave = wave;
    }

    let kills = pacing.clock.kills;
    if kills / 25 != pilot.prev.kills / 25 {
        pilot.record(format!("{kills} kills"));
    }
    pilot.prev.kills = kills;

    let online = meta.unlocks.online();
    if online != pilot.prev.unlocks {
        pilot.prev.unlocks = online;
        pilot.record(format!("{online} systems online"));
    }

    let owned = field
        .zones
        .iter()
        .filter(|(zone, _)| zone.owner == ZoneOwner::Player)
        .count();
    if owned != pilot.prev.zones {
        pilot.record(format!("holding {owned} zones"));
        pilot.prev.zones = owned;
    }

    let hint = meta
        .hints
        .active
        .as_ref()
        .map_or_else(String::new, |h| format!("{} - {}", h.headline, h.detail));
    if hint != pilot.prev.hint {
        if !hint.is_empty() {
            pilot.record(format!("hint: {hint}"));
        }
        pilot.prev.hint = hint;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_json_object_round_trips_its_braces() {
        let mut json = Json::new();
        json.int("a", 1);
        json.obj("b");
        json.text("c", "hi");
        json.end();
        json.arr("d");
        json.push_text("x");
        json.end();
        let out = json.finish();
        assert_eq!(out.trim(), r#"{"a":1,"b":{"c":"hi"},"d":["x"]}"#);
    }

    #[test]
    fn empty_containers_still_close_correctly() {
        let mut json = Json::new();
        json.arr("empty");
        json.end();
        json.obj("also");
        json.end();
        assert_eq!(json.finish().trim(), r#"{"empty":[],"also":{}}"#);
    }

    #[test]
    fn nested_arrays_of_objects_close_in_the_right_order() {
        let mut json = Json::new();
        json.arr("list");
        for n in 0..2 {
            json.item();
            json.int("n", n);
            json.end();
        }
        json.end();
        assert_eq!(json.finish().trim(), r#"{"list":[{"n":0},{"n":1}]}"#);
    }

    #[test]
    fn non_finite_numbers_become_null_rather_than_invalid_json() {
        let mut json = Json::new();
        json.num("nan", f32::NAN);
        json.num("inf", f32::INFINITY);
        json.num("ok", 1.5);
        assert_eq!(
            json.finish().trim(),
            r#"{"nan":null,"inf":null,"ok":1.500}"#
        );
    }

    #[test]
    fn strings_escape_quotes_newlines_and_control_characters() {
        let mut json = Json::new();
        json.text("s", "a\"b\\c\nd\u{1}");
        // A hint or a card name containing any of these would otherwise
        // produce a report the far side cannot parse.
        assert_eq!(json.finish().trim(), "{\"s\":\"a\\\"b\\\\c\\nd\\u0001\"}");
    }

    #[test]
    fn vectors_are_written_as_two_element_arrays() {
        let mut json = Json::new();
        json.vec2("p", Vec2::new(1.0, -2.25));
        assert_eq!(json.finish().trim(), r#"{"p":[1.000,-2.250]}"#);
    }

    #[test]
    fn single_letters_and_digits_resolve_to_keys() {
        assert_eq!(key_from_name("w"), Some(KeyCode::KeyW));
        assert_eq!(key_from_name("W"), Some(KeyCode::KeyW));
        assert_eq!(key_from_name("z"), Some(KeyCode::KeyZ));
        assert_eq!(key_from_name("3"), Some(KeyCode::Digit3));
    }

    #[test]
    fn named_keys_are_case_insensitive() {
        assert_eq!(key_from_name("space"), Some(KeyCode::Space));
        assert_eq!(key_from_name("Enter"), Some(KeyCode::Enter));
        assert_eq!(key_from_name("ESC"), Some(KeyCode::Escape));
        assert_eq!(key_from_name("left"), Some(KeyCode::ArrowLeft));
        assert_eq!(key_from_name("nope"), None);
    }

    #[test]
    fn comments_and_blank_lines_produce_nothing() {
        assert_eq!(parse_line(""), Ok(None));
        assert_eq!(parse_line("   "), Ok(None));
        assert_eq!(parse_line("# just talking"), Ok(None));
        assert_eq!(
            parse_line("tap W # move north"),
            Ok(Some(Cmd::Tap(vec![KeyCode::KeyW])))
        );
    }

    #[test]
    fn hold_takes_its_duration_from_the_last_word() {
        assert_eq!(
            parse_line("hold W A 1.5"),
            Ok(Some(Cmd::Hold(vec![KeyCode::KeyW, KeyCode::KeyA], 1.5)))
        );
    }

    #[test]
    fn a_hold_without_a_duration_is_an_error_not_a_silent_no_op() {
        assert!(parse_line("hold W").is_err());
        assert!(parse_line("hold 2").is_err());
    }

    #[test]
    fn release_all_is_distinct_from_releasing_a_key() {
        assert_eq!(parse_line("release all"), Ok(Some(Cmd::ReleaseAll)));
        assert_eq!(
            parse_line("release A"),
            Ok(Some(Cmd::Release(vec![KeyCode::KeyA])))
        );
    }

    #[test]
    fn unknown_verbs_and_keys_report_themselves() {
        let err = parse_line("frobnicate W").unwrap_err();
        assert!(err.contains("frobnicate"), "{err}");
        let err = parse_line("tap frobnicate").unwrap_err();
        assert!(err.contains("frobnicate"), "{err}");
    }

    #[test]
    fn notes_keep_their_whitespace_structure_as_words() {
        assert_eq!(
            parse_line("note the boss felt unfair"),
            Ok(Some(Cmd::Note("the boss felt unfair".into())))
        );
    }

    #[test]
    fn durations_are_clamped_so_a_typo_cannot_wedge_the_queue() {
        let Ok(Some(Cmd::Wait(secs))) = parse_line("wait 99999") else {
            panic!("expected a wait");
        };
        assert!(secs <= 600.0);
    }

    #[test]
    fn only_observable_commands_end_the_frame() {
        assert!(Cmd::Tap(vec![]).consumes_frame());
        assert!(Cmd::Hold(vec![], 1.0).consumes_frame());
        assert!(Cmd::Wait(1.0).consumes_frame());
        assert!(Cmd::Shot("x".into()).consumes_frame());
        assert!(!Cmd::Press(vec![]).consumes_frame());
        assert!(!Cmd::Note("x".into()).consumes_frame());
    }
}
