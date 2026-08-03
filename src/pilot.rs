//! Agent pilot bridge: a file-based control and telemetry channel.
//!
//! The game is keyboard-only and its whole surface is "look at the screen and
//! press keys", which makes it unverifiable without a human. `devtools` covers
//! scripted one-shots; this covers the other half - an outside process that
//! wants to *play*: read the situation, decide, act, read the result.
//!
//! Set `HOLDFAST_PILOT=<dir>` and the game will:
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
//! | `roam 20` | Wander unaided for twenty seconds |
//! | `chase 8` / `flee 8` | Close on, or run from, the nearest enemy |
//! | `goto -12 6 [limit]` | Walk to a point, ending early on arrival |
//! | `defend 20 -8 [secs]` | Defend a point: stay near it, give ground to a crowd |
//! | `kite 20` | Keep the nearest enemy at arm's length without disengaging |
//!
//! The last four exist because the far side of this channel is usually a
//! language model, which thinks in whole turns of several seconds. Without
//! them the hero stands motionless between decisions, which is neither a fair
//! test of the game nor much to watch.
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

/// How long a `tap` holds its keys down.
///
/// Long enough to survive a state transition, short enough that two taps in a
/// row are still two distinct presses.
const TAP_HOLD: f32 = 0.1;

/// The gap a kiting player tries to keep. Inside most weapon ranges, outside
/// contact range - which is the band where the game is actually winnable.
const KITE_RANGE: f32 = 9.0;

/// How close something has to be before a holding player gives ground.
const HOLD_SPACING: f32 = 5.0;

/// How far from the post a holding player will drift before coming back.
const HOLD_LEASH: f32 = 12.0;

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
    /// Drive the player for a while without further instruction.
    Steer(Steer, f32),
}

/// Autonomous movement, so a slow reader on the far side of the channel does
/// not leave the hero standing still between turns. Each of these presses real
/// WASD keys rather than writing movement directly, so what gets exercised is
/// the same input path a human uses.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Steer {
    /// Wander, changing heading periodically and whenever progress stalls.
    Roam,
    /// Close on the nearest enemy.
    Chase,
    /// Put distance between yourself and the nearest enemy.
    Flee,
    /// Walk to a point, finishing early on arrival.
    Goto(Vec2),
    /// Hold a point: stay near it, but give ground to the crowd rather than
    /// standing in it. What defending a position actually looks like.
    Hold(Vec2),
    /// Keep the nearest enemy at arm's length without disengaging.
    ///
    /// The single most important skill in the game and the hardest to express
    /// as keystrokes: weapons fire themselves, so the whole craft of fighting
    /// is standing at the range where you are hitting and they are not.
    Kite,
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
            Self::Tap(_) | Self::Hold(..) | Self::Wait(_) | Self::Shot(_) | Self::Steer(..)
        )
    }
}

/// The least crowded bearing out of a crush.
///
/// Steering off the single nearest enemy is what a naive kiter does, and it
/// fails exactly when it matters: inside an encirclement the nearest enemy
/// changes every frame, the chosen direction flips with it, and the net
/// displacement is nothing. A strategist measured that - "movement produces no
/// positional change for several seconds while damage keeps landing" - and
/// blamed the game. It was the steering.
///
/// Summing inverse-square repulsion from everything nearby gives a bearing
/// that points at the actual gap in the ring, and is stable frame to frame
/// because it moves only as the whole crowd moves.
fn escape_vector(foes: &Query<&Body, With<Enemy>>, pos: Vec2, previous: Vec2) -> Option<Vec2> {
    const REACH: f32 = 14.0;
    let mut push = Vec2::ZERO;
    let mut any = false;
    for body in foes {
        let delta = pos - body.pos;
        let dist = delta.length();
        if !(1e-3..=REACH).contains(&dist) {
            continue;
        }
        any = true;
        // Inverse square: the ones about to touch you dominate, but a gap on
        // the far side still pulls.
        push += delta / (dist * dist * dist);
    }
    if !any {
        return None;
    }
    Some(blend_escape(push, previous))
}

/// Commit to a bearing rather than recomputing one from scratch each frame.
///
/// A ring that is nearly symmetric sums to nearly zero, and normalising a
/// nearly-zero vector turns a rounding difference into a ninety-degree turn -
/// the player then dithers on the spot, which is the very thing the summed
/// field was meant to fix. My own stability test caught this. Blending with the
/// previous bearing gives the hysteresis a real player has: once you have
/// chosen a way out, you keep going that way unless the crowd genuinely moves.
#[must_use]
pub fn blend_escape(push: Vec2, previous: Vec2) -> Vec2 {
    /// Below this the field carries no information: a ring that is nearly
    /// symmetric cancels to nearly nothing, and normalising the remainder turns
    /// a rounding difference into a ninety-degree turn.
    const MEANINGFUL: f32 = 0.01;

    let fallback = if previous == Vec2::ZERO {
        // Perfectly surrounded with no history. Any committed direction beats
        // standing still and being eaten.
        Vec2::new(1.0, 0.0)
    } else {
        previous
    };

    if push.length() < MEANINGFUL {
        return fallback;
    }
    let Some(fresh) = push.try_normalize() else {
        return fallback;
    };

    // How much to trust the new reading. Normally a little, for steadiness -
    // but when it points the *opposite* way, sticking with the old bearing
    // would walk straight into the crowd, and a fixed blend of two opposed
    // directions can never turn round at all. My own test caught that.
    let weight = if previous.dot(fresh) < 0.0 { 0.55 } else { 0.3 };
    (previous * (1.0 - weight) + fresh * weight)
        .try_normalize()
        .unwrap_or(fresh)
}

/// Which movement keys express a direction, as eight-way.
///
/// The threshold is just under `sin(22.5 degrees)` scaled to the diagonal, so a
/// heading within an eighth-turn of a diagonal presses both of its keys.
fn keys_for_direction(dir: Vec2) -> Vec<KeyCode> {
    const DEADZONE: f32 = 0.38;
    let mut keys = Vec::with_capacity(2);
    // W is -Y and S is +Y: the arena's second axis is world Z, which grows
    // towards the camera.
    if dir.y < -DEADZONE {
        keys.push(KeyCode::KeyW);
    }
    if dir.y > DEADZONE {
        keys.push(KeyCode::KeyS);
    }
    if dir.x < -DEADZONE {
        keys.push(KeyCode::KeyA);
    }
    if dir.x > DEADZONE {
        keys.push(KeyCode::KeyD);
    }
    keys
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

const FUNCTION_KEYS: [KeyCode; 12] = [
    KeyCode::F1,
    KeyCode::F2,
    KeyCode::F3,
    KeyCode::F4,
    KeyCode::F5,
    KeyCode::F6,
    KeyCode::F7,
    KeyCode::F8,
    KeyCode::F9,
    KeyCode::F10,
    KeyCode::F11,
    KeyCode::F12,
];

/// Resolve a key name written by a human or an agent.
fn key_from_name(name: &str) -> Option<KeyCode> {
    let upper = name.to_ascii_uppercase();
    let bytes = upper.as_bytes();

    // Function keys, which the game uses for save and for dev toggles. Checked
    // before the single-character cases so "F5" is not read as the letter F.
    if let Some(digits) = upper.strip_prefix('F')
        && !digits.is_empty()
        && let Ok(n) = digits.parse::<usize>()
        && (1..=12).contains(&n)
    {
        return Some(FUNCTION_KEYS[n - 1]);
    }
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
        // Every spelling anyone has actually typed. A reader who writes
        // "ArrowUp" - the Bevy name, and the one the source suggests - meant
        // the up arrow, and there is nothing else it could mean.
        "UP" | "ARROWUP" | "ARROW_UP" | "UPARROW" => KeyCode::ArrowUp,
        "DOWN" | "ARROWDOWN" | "ARROW_DOWN" | "DOWNARROW" => KeyCode::ArrowDown,
        "LEFT" | "ARROWLEFT" | "ARROW_LEFT" | "LEFTARROW" => KeyCode::ArrowLeft,
        "RIGHT" | "ARROWRIGHT" | "ARROW_RIGHT" | "RIGHTARROW" => KeyCode::ArrowRight,
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
        "roam" => Ok(Some(Cmd::Steer(Steer::Roam, seconds(&verb, rest.first())?))),
        "chase" => Ok(Some(Cmd::Steer(
            Steer::Chase,
            seconds(&verb, rest.first())?,
        ))),
        "flee" => Ok(Some(Cmd::Steer(Steer::Flee, seconds(&verb, rest.first())?))),
        // `defend`, not `hold`: `hold` already means holding a key down, and
        // a verb that means two things is a verb that gets used wrongly.
        "defend" => {
            let x = number(&verb, rest.first())?;
            let z = number(&verb, rest.get(1))?;
            let secs = rest.get(2).map_or(Ok(20.0), |v| number(&verb, Some(v)))?;
            Ok(Some(Cmd::Steer(
                Steer::Hold(Vec2::new(x, z)),
                secs.clamp(0.0, 600.0),
            )))
        }
        "kite" => Ok(Some(Cmd::Steer(Steer::Kite, seconds(&verb, rest.first())?))),
        "goto" => {
            let x = number(&verb, rest.first())?;
            let z = number(&verb, rest.get(1))?;
            // A distance is known; a duration has to be guessed from it. An
            // explicit third argument still wins.
            // The sentinel must survive: clamping it to zero would give a
            // bare `goto` no budget at all and stop it on the spot.
            let limit = match rest.get(2) {
                Some(v) => number(&verb, Some(v))?.clamp(0.0, 600.0),
                None => TRAVEL_BUDGET_UNKNOWN,
            };
            Ok(Some(Cmd::Steer(Steer::Goto(Vec2::new(x, z)), limit)))
        }
        other => Err(format!("unknown command {other:?}")),
    }
}

fn number(verb: &str, arg: Option<&&str>) -> Result<f32, String> {
    arg.ok_or_else(|| format!("{verb}: missing a number"))?
        .parse()
        .map_err(|_| format!("{verb}: {arg:?} is not a number"))
}

fn seconds(verb: &str, arg: Option<&&str>) -> Result<f32, String> {
    Ok(number(verb, arg)?.clamp(0.0, 600.0))
}

// -- the channel ------------------------------------------------------------

/// Sentinel meaning "work the budget out from how far it actually is".
///
/// The distance is not known at parse time - the player's position is not a
/// parser input - so the budget is filled in when the command becomes active.
const TRAVEL_BUDGET_UNKNOWN: f32 = -1.0;

/// Assumed travel speed when budgeting a `goto`, in units per second.
///
/// Deliberately well under the player's ~8.5: the real journey is slowed by
/// crowds, deflected by scenery and interrupted by a level-up screen. Budget
/// for the bad case, because the cost of over-budgeting is a few idle seconds
/// and the cost of under-budgeting is that the tester silently stops in the
/// empty middle of the map and never learns it did not arrive.
const TRAVEL_SPEED: f32 = 3.2;

/// Slack on top, for the level-up screens and the fights on the way.
const TRAVEL_SLACK: f32 = 12.0;

/// The safety net for walking `distance` units.
fn travel_budget(distance: f32) -> f32 {
    (distance / TRAVEL_SPEED + TRAVEL_SLACK).clamp(TRAVEL_SLACK, 600.0)
}

/// Getting round whatever is in the way.
///
/// Walking at a target is only a strategy on open ground. Press one key at a
/// wall and the player grinds against it until the budget runs out, having
/// travelled nothing - and the log looks identical to a journey through a
/// crowd. So travel notices when it has stopped making ground and sidesteps,
/// alternating sides so a corner is escaped rather than paced.
#[derive(Debug)]
struct Travel {
    /// Where we were when progress was last measured.
    last: Vec2,
    since_check: f32,
    /// Time left sidestepping, and which way.
    detour: f32,
    side: f32,
}

impl Default for Travel {
    fn default() -> Self {
        Self {
            last: Vec2::ZERO,
            since_check: 0.0,
            detour: 0.0,
            side: 1.0,
        }
    }
}

impl Travel {
    /// How long to give up on a bearing that is not working.
    const PATIENCE: f32 = 1.5;
    /// Ground that counts as having made progress in that time.
    const PROGRESS: f32 = 1.2;
    /// How long to commit to a sidestep before re-checking.
    const DETOUR: f32 = 1.1;

    /// The bearing to walk, given where we are and where we want to be.
    fn step(&mut self, pos: Vec2, target: Vec2, dt: f32) -> Vec2 {
        let direct = (target - pos).normalize_or_zero();

        if self.detour > 0.0 {
            self.detour -= dt;
            // Along the wall rather than straight at it, with a lean towards
            // the target so the detour still ends up somewhere useful.
            let along = Vec2::new(-direct.y, direct.x) * self.side;
            return (along + direct * 0.35).normalize_or_zero();
        }

        self.since_check += dt;
        if self.since_check >= Self::PATIENCE {
            if self.last.distance(pos) < Self::PROGRESS {
                self.detour = Self::DETOUR;
                // The other way next time: one side of an obstacle is a
                // detour, both sides is a search.
                self.side = -self.side;
            }
            self.since_check = 0.0;
            self.last = pos;
        }
        direct
    }

    /// Called when a new `goto` starts, so the first check measures this
    /// journey rather than the previous one.
    fn begin(&mut self, pos: Vec2) {
        self.last = pos;
        self.since_check = 0.0;
        self.detour = 0.0;
    }
}

/// A timed command currently occupying the queue head.
#[derive(Debug)]
struct Active {
    remaining: f32,
    /// Keys to let go of when the timer runs out.
    release: Vec<KeyCode>,
    /// Set for the autonomous movement commands, which re-decide every frame.
    steer: Option<Steer>,
}

/// Wander state, kept between frames so `roam` looks like someone exploring
/// rather than someone vibrating.
#[derive(Debug)]
struct Wander {
    heading: Vec2,
    /// Time left on the current heading.
    hold: f32,
    /// Where we were when progress was last checked.
    last_pos: Vec2,
    since_check: f32,
    rng: crate::rng::Rng,
}

impl Default for Wander {
    fn default() -> Self {
        Self {
            heading: Vec2::new(0.0, -1.0),
            hold: 0.0,
            last_pos: Vec2::ZERO,
            since_check: 0.0,
            rng: crate::rng::Rng::seeded(0xB01D_FA57),
        }
    }
}

impl Wander {
    /// Decide which way to walk this frame.
    ///
    /// Rerolls the heading on a timer, and again whenever the hero has barely
    /// moved - which means a prop is in the way. Without the stall check a
    /// roaming tester spends whole minutes pressed against the same mug.
    fn step(&mut self, pos: Vec2, dt: f32) -> Vec2 {
        self.hold -= dt;
        self.since_check += dt;
        if self.since_check >= 0.5 {
            let progress = pos.distance(self.last_pos);
            self.last_pos = pos;
            self.since_check = 0.0;
            if progress < 0.4 {
                self.hold = 0.0;
            }
        }
        if self.hold <= 0.0 {
            // `unit_circle` is a world-space XZ vector; the arena works in a
            // flat Vec2 whose second component is that Z.
            let dir = self.rng.unit_circle();
            self.heading = Vec2::new(dir.x, dir.z);
            self.hold = self.rng.range(1.1, 2.6);
        }
        self.heading
    }
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
    /// Movement keys the current steering command is holding down.
    steering: Vec<KeyCode>,
    wander: Wander,
    travel: Travel,
    /// Name for this instance, so a report says which window it came from.
    label: String,
    /// Set by `note strategy=...`, forwarded to the run dossier.
    strategy: Option<String>,
    /// The bearing last chosen out of a crowd, for hysteresis.
    last_escape: Vec2,
    /// Notable changes since the last snapshot.
    events: Vec<String>,
    /// Things that went wrong, kept for the whole run.
    ///
    /// `events` is cleared into every snapshot, so it lives about two hundred
    /// milliseconds - a rejected command was reported once, into a file nobody
    /// was reading at that instant, and was then gone. A tester therefore could
    /// not distinguish "the key did nothing" from "the key was never accepted",
    /// and reported the former. These stay.
    problems: Vec<String>,
    seq: u64,
    since_snapshot: f32,
    wall: f32,
    prev: Previous,
}

impl Pilot {
    fn new(dir: PathBuf) -> Self {
        // Name the instance after its directory unless told otherwise, so a
        // report always says which of several windows it came from.
        let label = env::var("HOLDFAST_LABEL").ok().unwrap_or_else(|| {
            dir.file_name()
                .map_or_else(|| "pilot".to_string(), |n| n.to_string_lossy().into_owned())
        });
        Self {
            dir,
            cursor: 0,
            partial: String::new(),
            queue: VecDeque::new(),
            active: None,
            held: HashSet::new(),
            tapped: Vec::new(),
            steering: Vec::new(),
            wander: Wander::default(),
            travel: Travel::default(),
            label,
            strategy: None,
            last_escape: Vec2::ZERO,
            events: Vec::new(),
            problems: Vec::new(),
            seq: 0,
            since_snapshot: SNAPSHOT_PERIOD,
            wall: 0.0,
            prev: Previous::default(),
        }
    }

    fn commands_path(&self) -> PathBuf {
        self.dir.join("commands")
    }

    /// Record something that went wrong, where it will still be visible later.
    fn problem(&mut self, line: impl Into<String>) {
        const KEEP: usize = 8;
        let line = line.into();
        self.record(line.clone());
        if !self.problems.contains(&line) {
            if self.problems.len() >= KEEP {
                self.problems.remove(0);
            }
            self.problems.push(line);
        }
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
        let Ok(dir) = env::var("HOLDFAST_PILOT") else {
            return;
        };
        let dir = PathBuf::from(dir);
        if let Err(err) = fs::create_dir_all(&dir) {
            error!("HOLDFAST_PILOT: cannot use {}: {err}", dir.display());
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

/// The keys that drive the player, either spelling.
const MOVEMENT_KEYS: [KeyCode; 8] = [
    KeyCode::KeyW,
    KeyCode::KeyA,
    KeyCode::KeyS,
    KeyCode::KeyD,
    KeyCode::ArrowUp,
    KeyCode::ArrowDown,
    KeyCode::ArrowLeft,
    KeyCode::ArrowRight,
];

/// Note a key delivered into a screen that reads it as something else.
///
/// Only the keys that collide: a `W` during a level-up is ignored and harmless,
/// but a `3` is "take card three" and an `ENTER` buys a research node. Warning
/// about every key would bury the ones that matter.
fn warn_modal(pilot: &mut Pilot, modal: Option<&'static str>, keys: &[KeyCode]) {
    let Some(screen) = modal else {
        return;
    };
    for key in keys {
        let collides = matches!(
            key,
            KeyCode::Digit1
                | KeyCode::Digit2
                | KeyCode::Digit3
                | KeyCode::Digit4
                | KeyCode::Digit5
                | KeyCode::Enter
                | KeyCode::NumpadEnter
                | KeyCode::KeyR
                | KeyCode::KeyT
                | KeyCode::Escape
        );
        if collides {
            pilot.problem(format!(
                "{key:?} was delivered while {screen} was open, and read as a {screen} key"
            ));
        }
    }
}

/// The screen a key is about to be delivered into, when it is one that steals
/// keys the play state also uses.
fn modal_screen(state: AppState) -> Option<&'static str> {
    match state {
        AppState::LevelUp => Some("LEVELUP"),
        AppState::SkillTree => Some("RESEARCH"),
        AppState::Paused => Some("PAUSED"),
        AppState::GameOver => Some("GAMEOVER"),
        AppState::Menu => Some("MENU"),
        AppState::Playing => None,
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
            Err(err) => pilot.problem(format!("rejected: {err}")),
        }
    }
}

/// Advance the queue and write the resulting key state.
fn run_queue(
    mut pilot: ResMut<Pilot>,
    time: Res<Time<Real>>,
    state: Res<State<AppState>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    hero: Query<&Body, With<Player>>,
    foes: Query<&Body, With<Enemy>>,
) {
    let dt = time.delta_secs();
    pilot.wall += dt;

    for key in std::mem::take(&mut pilot.tapped) {
        if !pilot.held.contains(&key) {
            keys.release(key);
        }
    }

    let hero_pos = hero.iter().next().map(|body| body.pos);
    // A modal screen eats digits and ENTER. A batch aimed at the play state
    // that runs into a level-up has its `tap 3` read as "take card three", and
    // the tester's structure never gets selected - which is a silent
    // corruption of whatever it was measuring. This cannot be guessed away
    // (answering the modal is also a `tap 3`), so it is reported instead.
    let modal = modal_screen(*state.get());

    if let Some(active) = pilot.active.as_mut() {
        active.remaining -= dt;
        // A `goto` ends on arrival; its duration is only a safety net against
        // a target that turns out to be unreachable.
        let arrived = match (active.steer, hero_pos) {
            (Some(Steer::Goto(target)), Some(pos)) => pos.distance(target) < 1.6,
            _ => false,
        };
        if active.remaining <= 0.0 || arrived {
            // A `goto` that runs out of budget has to say so. Silently
            // stopping is how four subsystems went unexercised for three
            // rounds: the testers were told to walk out past 130 units to
            // where the forts are, the walk quietly ended a third of the way
            // there, and nothing in the report distinguished that from having
            // arrived.
            let abandoned = match (active.steer, hero_pos) {
                (Some(Steer::Goto(target)), Some(pos)) if !arrived => Some((target, pos)),
                _ => None,
            };
            let release = std::mem::take(&mut active.release);
            pilot.active = None;
            if let Some((target, pos)) = abandoned {
                let short = pos.distance(target);
                pilot.record(format!(
                    "gave up walking to ({:.0},{:.0}) - stopped {short:.0} units short",
                    target.x, target.y
                ));
            }
            for key in release {
                pilot.held.remove(&key);
                keys.release(key);
            }
            for key in std::mem::take(&mut pilot.steering) {
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
                warn_modal(&mut pilot, modal, &list);
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
                warn_modal(&mut pilot, modal, &list);
                // Held for a few frames, not one.
                //
                // A single-frame press races Bevy's state transitions: if the
                // tap lands on the frame the game enters `LevelUp`, the system
                // that reads card keys has not started running yet and the
                // press is gone. That is why card selection looked
                // intermittent to testers - "works sometimes" - while never
                // failing when tried by hand a second after the transition.
                // `just_pressed` still fires exactly once, so holding longer
                // cannot double-pick.
                for key in &list {
                    keys.press(*key);
                    pilot.held.insert(*key);
                }
                pilot.active = Some(Active {
                    remaining: TAP_HOLD,
                    release: list,
                    steer: None,
                });
            }
            Cmd::Hold(list, secs) => {
                warn_modal(&mut pilot, modal, &list);
                for key in &list {
                    pilot.held.insert(*key);
                }
                pilot.active = Some(Active {
                    remaining: secs,
                    release: list,
                    steer: None,
                });
            }
            Cmd::Wait(secs) => {
                pilot.active = Some(Active {
                    remaining: secs,
                    release: Vec::new(),
                    steer: None,
                });
            }
            Cmd::Steer(steer, mut secs) => {
                // A steering verb owns movement. A key still down from an
                // earlier `press` is re-pressed every frame and cannot be
                // released by the steering code, so `press w` followed by
                // `kite` left W fighting the S the escape wanted: the two
                // cancelled in `read_move_input` and the player did not move at
                // all. That is the "encirclement freezes movement" reported in
                // every round - HP falling to nothing over twenty seconds of
                // zero positional change - and it was the tester's own earlier
                // command sabotaging this one.
                let stuck: Vec<KeyCode> = MOVEMENT_KEYS
                    .into_iter()
                    .filter(|k| pilot.held.contains(k))
                    .collect();
                if !stuck.is_empty() {
                    for key in &stuck {
                        pilot.held.remove(key);
                        keys.release(*key);
                    }
                    pilot.problem(format!(
                        "let go of {stuck:?}, still held from an earlier press - \
                         a steering verb drives, so a held movement key would \
                         cancel it"
                    ));
                }
                if let (Steer::Goto(target), Some(pos)) = (steer, hero_pos) {
                    if secs <= TRAVEL_BUDGET_UNKNOWN {
                        secs = travel_budget(pos.distance(target));
                    }
                    pilot.travel.begin(pos);
                }
                pilot.record(match steer {
                    Steer::Roam => format!("roaming for {secs:.0}s"),
                    Steer::Chase => format!("chasing for {secs:.0}s"),
                    Steer::Flee => format!("fleeing for {secs:.0}s"),
                    Steer::Goto(t) => {
                        format!(
                            "walking to ({:.0},{:.0}), giving up after {secs:.0}s",
                            t.x, t.y
                        )
                    }
                    Steer::Hold(t) => format!("holding ({:.0},{:.0})", t.x, t.y),
                    Steer::Kite => format!("kiting for {secs:.0}s"),
                });
                pilot.active = Some(Active {
                    remaining: secs,
                    release: Vec::new(),
                    steer: Some(steer),
                });
            }
            Cmd::Shot(path) => {
                pilot.record(format!("screenshot -> {path}"));
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(path));
            }
            Cmd::Note(text) => {
                // `note strategy=turtle` labels the run in the dossier. A
                // tester saying what it is attempting is worth more than
                // inferring it from the numbers, because the gap between the
                // two is exactly what a balance pass wants to see.
                if let Some(label) = text.strip_prefix("strategy=") {
                    pilot.strategy = Some(label.trim().to_ascii_lowercase());
                }
                pilot.record(format!("note: {text}"));
            }
            Cmd::Quit => {
                pilot.record("quit requested");
                exit.write(AppExit::Success);
            }
        }
        if stop {
            break;
        }
    }

    // Steering re-decides which way to walk every frame, so it runs after the
    // queue has had its turn and can have just installed a new one.
    if let (Some(steer), Some(pos)) = (pilot.active.as_ref().and_then(|a| a.steer), hero_pos) {
        let dir = match steer {
            Steer::Roam => pilot.wander.step(pos, dt),
            Steer::Goto(target) => pilot.travel.step(pos, target, dt),
            Steer::Hold(anchor) => {
                // Drift back towards the post, but step away from whatever is
                // closest. Holding ground is not standing still.
                let home = (anchor - pos).normalize_or_zero();
                let nearest = foes
                    .iter()
                    .map(|body| body.pos)
                    .min_by(|a, b| a.distance_squared(pos).total_cmp(&b.distance_squared(pos)));
                let leash = pos.distance(anchor);
                match nearest {
                    Some(foe) if foe.distance(pos) < HOLD_SPACING && leash < HOLD_LEASH => {
                        ((pos - foe).normalize_or_zero() + home * 0.4).normalize_or_zero()
                    }
                    _ if leash > 2.0 => home,
                    _ => Vec2::ZERO,
                }
            }
            Steer::Kite => {
                let nearest = foes
                    .iter()
                    .map(|body| body.pos)
                    .min_by(|a, b| a.distance_squared(pos).total_cmp(&b.distance_squared(pos)));
                let escape = escape_vector(&foes, pos, pilot.last_escape);
                pilot.last_escape = escape.unwrap_or(pilot.last_escape);
                match (nearest, escape) {
                    (None, _) => pilot.wander.step(pos, dt),
                    (Some(foe), Some(escape)) => {
                        let gap = foe.distance(pos);
                        if gap < KITE_RANGE * 0.8 {
                            // Too close: leave along the least crowded bearing,
                            // with a curl so it does not run into the next one.
                            (escape + Vec2::new(-escape.y, escape.x) * 0.5).normalize_or_zero()
                        } else if gap > KITE_RANGE * 1.6 {
                            (foe - pos).normalize_or_zero()
                        } else {
                            // At range: strafe, so the crowd never converges.
                            Vec2::new(-escape.y, escape.x)
                        }
                    }
                    (Some(foe), None) => (pos - foe).normalize_or_zero(),
                }
            }
            Steer::Chase => {
                let nearest = foes
                    .iter()
                    .map(|body| body.pos)
                    .min_by(|a, b| a.distance_squared(pos).total_cmp(&b.distance_squared(pos)));
                match nearest {
                    // With nothing to chase, keep exploring rather than stand
                    // still - a frozen tester finds nothing.
                    None => pilot.wander.step(pos, dt),
                    Some(foe) => (foe - pos).normalize_or_zero(),
                }
            }
            Steer::Flee => {
                let bearing = escape_vector(&foes, pos, pilot.last_escape);
                pilot.last_escape = bearing.unwrap_or(pilot.last_escape);
                bearing.unwrap_or_else(|| pilot.wander.step(pos, dt))
            }
        };
        let wanted = keys_for_direction(dir);
        for key in std::mem::take(&mut pilot.steering) {
            if !wanted.contains(&key) && !pilot.held.contains(&key) {
                keys.release(key);
            }
        }
        for key in &wanted {
            keys.press(*key);
        }
        pilot.steering = wanted;
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
    coverage: Res<'w, crate::coverage::Coverage>,
    fog: Res<'w, crate::fog::FogMap>,
    war: Res<'w, crate::forts::WarRoom>,
    diplomacy: Res<'w, crate::factions::Diplomacy>,
    powers: Res<'w, crate::factions::NearbyPowers>,
    unlocks: Res<'w, Unlocks>,
    plan: Res<'w, crate::command::PlanMode>,
    offer: Res<'w, CardOffer>,
    hints: Res<'w, HintQueue>,
}

#[derive(SystemParam)]
struct Holdings<'w, 's> {
    forts: Query<
        'w,
        's,
        (
            &'static crate::forts::Fort,
            &'static crate::factions::Allegiance,
            &'static Body,
        ),
    >,
    nests: Query<
        'w,
        's,
        (
            &'static crate::forts::Nest,
            &'static crate::factions::Allegiance,
            &'static Body,
        ),
    >,
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
    mut declared: ResMut<crate::dossier::DeclaredStrategy>,
    time: Res<Time<Real>>,
    pacing: Pacing,
    sheet: Sheet,
    meta: Meta,
    field: Field,
    holdings: Holdings,
) {
    pilot.since_snapshot += time.delta_secs();
    if pilot.since_snapshot < SNAPSHOT_PERIOD {
        return;
    }
    pilot.since_snapshot = 0.0;
    pilot.seq += 1;
    if let Some(label) = pilot.strategy.take() {
        declared.0 = label;
    }

    let env = *pacing.env;
    let player = field.player.iter().next();
    let hero_pos = player.map_or(Vec2::ZERO, |(body, _, _)| body.pos);

    note_changes(&mut pilot, &pacing, &sheet, &meta, &field, player, hero_pos);

    let mut json = Json::new();
    json.text("instance", &pilot.label.clone());
    json.count("seq", pilot.seq);
    json.num("wall", pilot.wall);
    json.text("state", &format!("{:?}", pacing.state.get()));
    json.text("world", env.title());
    json.count("queued", pilot.queue.len());

    // Content coverage: what of the game this session has actually exercised,
    // and what is left. Turns "go and see everything" into a task with an
    // answer rather than an instruction to keep playing.
    json.obj("coverage");
    json.num("fraction", meta.coverage.fraction());
    json.count("seen", meta.coverage.count());
    json.arr("missing");
    for tag in meta.coverage.missing() {
        // Append what this world calls it, where that differs. The checklist
        // is keyed on archetypes, but the player only ever sees local names,
        // so "enemy:StainSlime" alone is not something anyone can act on.
        json.push_text(&label_for(&tag, env));
    }
    json.end();
    json.end();

    json.obj("fog");
    json.num("explored_area", meta.fog.explored_area());
    json.count("explored_cells", meta.fog.explored_cells());
    json.count("cells_in_sight", meta.fog.visible_cells());
    json.end();

    json.obj("run");
    json.num("elapsed", pacing.clock.elapsed);
    json.count("kills", pacing.clock.kills);
    json.int("best_streak", pacing.clock.best_streak);
    json.num("furthest", pacing.clock.furthest);
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
    // Forts contribute more to the floor than zones do - 0.35 each against 0.2
    // - and this was invisible, so a fort-holder could not tell what its
    // holdings were costing it.
    json.num("from_forts", pacing.threat.holdings);
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
    json.num("scrap_per_sec", sheet.economy.income_per_second());
    json.num("scrap_per_sec_from_forts", sheet.economy.fort_rate);
    json.end();

    write_forces(&mut json, &sheet, &field, hero_pos, env);
    write_field(&mut json, &field, hero_pos);
    write_meta(&mut json, &meta, &sheet, env);
    write_war(&mut json, &meta, &holdings, hero_pos);

    json.arr("events");
    for event in std::mem::take(&mut pilot.events) {
        json.push_text(&event);
    }
    json.end();

    // Not taken: these persist, so a command the game refused is still visible
    // the next time anyone looks.
    json.arr("problems");
    for problem in &pilot.problems {
        json.push_text(problem);
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

fn write_forces(json: &mut Json, sheet: &Sheet, field: &Field, hero: Vec2, env: EnvKind) {
    json.arr("weapons");
    for slot in &sheet.loadout.slots {
        json.item();
        json.text("name", slot.kind.name(env));
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
        json.text("kind", ally.kind.name(env));
        json.int("level", ally.level);
        json.num("hp", health.current);
        json.end();
    }
    json.end();
    json.end();

    json.arr("turrets");
    for (turret, body, health) in &field.turrets {
        json.item();
        json.text("kind", turret.kind.name(env));
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

fn write_meta(json: &mut Json, meta: &Meta, sheet: &Sheet, env: EnvKind) {
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
    json.text("selected", meta.plan.selected_kind().name(env));
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

/// Add the world's own name for a checklist entry, when it has one.
fn label_for(tag: &str, env: EnvKind) -> String {
    let Some((kind, item)) = tag.split_once(':') else {
        return tag.to_string();
    };
    let local = match kind {
        "enemy" => crate::enemy::EnemyKind::ALL
            .iter()
            .find(|k| format!("{k:?}") == item)
            .map(|k| k.name(env)),
        "weapon" => crate::weapons::WeaponKind::ALL
            .iter()
            .find(|k| format!("{k:?}") == item)
            .map(|k| k.name(env)),
        "turret" => crate::allies::TurretKind::ALL
            .iter()
            .find(|k| format!("{k:?}") == item)
            .map(|k| k.name(env)),
        "ally" => crate::allies::AllyKind::ALL
            .iter()
            .find(|k| format!("{k:?}") == item)
            .map(|k| k.name(env)),
        _ => None,
    };
    match local {
        Some(name) if name != item => format!("{tag} (here: {name})"),
        _ => tag.to_string(),
    }
}

/// Forts, nests and who is at war with whom.
fn write_war(json: &mut Json, meta: &Meta, holdings: &Holdings, hero: Vec2) {
    let mut forts: Vec<_> = holdings
        .forts
        .iter()
        .map(|(fort, owner, body)| (body.pos.distance(hero), fort, owner.0, body.pos))
        .collect();
    forts.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));

    json.arr("forts");
    for (dist, fort, owner, pos) in forts.iter().take(8) {
        json.item();
        json.text("owner", owner.tag());
        json.num("dist", *dist);
        json.vec2("pos", *pos);
        json.num("capture", fort.progress);
        json.flag("contested", fort.contested);
        json.count("garrison", fort.garrison);
        json.count("nests_planted", fort.planted);
        json.end();
    }
    json.end();

    let mut nests: Vec<_> = holdings
        .nests
        .iter()
        .map(|(_, owner, body)| (body.pos.distance(hero), owner.0))
        .collect();
    nests.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    json.arr("nests");
    for (dist, owner) in nests.iter().take(10) {
        json.item();
        json.text("owner", owner.tag());
        json.num("dist", *dist);
        json.end();
    }
    json.end();

    json.arr("factions");
    for faction in crate::factions::Faction::MONSTERS {
        let plan = meta.war.plan(faction);
        json.item();
        json.text("faction", faction.tag());
        json.text("posture", &format!("{:?}", plan.posture));
        json.num("commitment", plan.commitment);
        // How much of this power is within earshot. Two of these being non-zero
        // is exactly the condition for a war being purchasable, so a reader can
        // see why the research node is refusing rather than guess.
        json.num("nearby", meta.powers.0[faction.index()]);
        json.end();
    }
    json.end();

    match meta.powers.feuding_pair() {
        Some((a, b)) => json.text("war_available", &format!("{} vs {}", a.tag(), b.tag())),
        None => json.maybe("war_available", None),
    }

    json.arr("wars");
    for (a, b, left) in meta.diplomacy.active_wars() {
        json.push_text(&format!("{} vs {} ({left:.0}s)", a.tag(), b.tag()));
    }
    json.end();
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
        assert!(Cmd::Steer(Steer::Roam, 5.0).consumes_frame());
        assert!(!Cmd::Press(vec![]).consumes_frame());
        assert!(!Cmd::Note("x".into()).consumes_frame());
    }

    #[test]
    fn the_autonomous_movement_verbs_parse() {
        assert_eq!(
            parse_line("roam 20"),
            Ok(Some(Cmd::Steer(Steer::Roam, 20.0)))
        );
        assert_eq!(
            parse_line("chase 8"),
            Ok(Some(Cmd::Steer(Steer::Chase, 8.0)))
        );
        assert_eq!(
            parse_line("flee 4.5"),
            Ok(Some(Cmd::Steer(Steer::Flee, 4.5)))
        );
        // No third argument means "budget it from the distance", which the
        // parser cannot know - the player's position is not a parser input.
        assert_eq!(
            parse_line("goto -12 6"),
            Ok(Some(Cmd::Steer(
                Steer::Goto(Vec2::new(-12.0, 6.0)),
                TRAVEL_BUDGET_UNKNOWN
            )))
        );
        assert_eq!(
            parse_line("goto 3 4 12"),
            Ok(Some(Cmd::Steer(Steer::Goto(Vec2::new(3.0, 4.0)), 12.0)))
        );
        assert!(parse_line("goto 3").is_err());
        assert!(parse_line("roam").is_err());
    }

    #[test]
    fn a_held_movement_key_and_a_steering_verb_cancel_each_other_out() {
        // The arithmetic behind "encirclement freezes movement": `read_move_input`
        // sums the keys that are down, so W held from an earlier `press` plus the
        // S a `kite` wants comes to nothing at all, and the player stands still
        // while a crowd eats them. This is why a steering verb lets go of them.
        let both_down = |keys: &[KeyCode]| {
            let mut dir = Vec2::ZERO;
            if keys.contains(&KeyCode::KeyW) {
                dir.y -= 1.0;
            }
            if keys.contains(&KeyCode::KeyS) {
                dir.y += 1.0;
            }
            dir
        };
        assert_eq!(both_down(&[KeyCode::KeyW, KeyCode::KeyS]), Vec2::ZERO);
        assert_ne!(both_down(&[KeyCode::KeyS]), Vec2::ZERO);
    }

    #[test]
    fn the_movement_keys_cover_both_spellings() {
        for key in [
            KeyCode::KeyW,
            KeyCode::ArrowUp,
            KeyCode::KeyD,
            KeyCode::ArrowRight,
        ] {
            assert!(MOVEMENT_KEYS.contains(&key), "{key:?}");
        }
        // Not a movement key, and a steer must not steal it - a tester holding
        // SHIFT to dash or a digit to build should keep it.
        for key in [KeyCode::ShiftLeft, KeyCode::Digit1, KeyCode::Space] {
            assert!(!MOVEMENT_KEYS.contains(&key), "{key:?}");
        }
    }

    #[test]
    fn only_the_modal_screens_steal_keys() {
        assert!(modal_screen(AppState::Playing).is_none());
        for state in [
            AppState::LevelUp,
            AppState::SkillTree,
            AppState::Paused,
            AppState::GameOver,
            AppState::Menu,
        ] {
            assert!(modal_screen(state).is_some(), "{state:?}");
        }
    }

    #[test]
    fn a_colliding_key_in_a_modal_is_reported_and_a_harmless_one_is_not() {
        // A `3` during a level-up is "take card three" - the tester's turret
        // never gets selected and whatever it was measuring is quietly wrong.
        // A `W` is ignored, and warning about it would bury the ones that count.
        let mut pilot = Pilot::new(PathBuf::from("/dev/null"));
        warn_modal(&mut pilot, Some("LEVELUP"), &[KeyCode::KeyW]);
        assert!(pilot.problems.is_empty(), "warned about a harmless key");
        warn_modal(&mut pilot, Some("LEVELUP"), &[KeyCode::Digit3]);
        assert_eq!(pilot.problems.len(), 1, "{:?}", pilot.problems);
        warn_modal(&mut pilot, None, &[KeyCode::Digit3]);
        assert_eq!(pilot.problems.len(), 1, "warned with no modal open");
    }

    #[test]
    fn the_arrow_keys_answer_to_every_name_anyone_types() {
        // A reader who writes the Bevy name meant the arrow key; there is
        // nothing else it could mean, and a rejection reads as a dead key.
        for name in ["UP", "ArrowUp", "arrow_up", "uparrow"] {
            assert_eq!(key_from_name(name), Some(KeyCode::ArrowUp), "{name}");
        }
        for name in ["DOWN", "ArrowDown", "LEFT", "ArrowLeft", "RIGHT"] {
            assert!(key_from_name(name).is_some(), "{name}");
        }
    }

    #[test]
    fn a_key_that_means_nothing_is_still_rejected() {
        // Aliasing must not turn into guessing.
        assert!(key_from_name("ARROWSIDEWAYS").is_none());
        assert!(key_from_name("").is_none());
    }

    #[test]
    fn a_long_walk_is_given_time_to_finish() {
        // A flat thirty-second budget covered about 150 units of real travel,
        // and forts start at 130 and cluster past 200. Every tester told to
        // walk out to the war stopped in the empty middle, and the report did
        // not distinguish that from arriving.
        assert!(
            travel_budget(250.0) > 60.0,
            "{}s for 250 units",
            travel_budget(250.0)
        );
        assert!(
            travel_budget(0.0) >= TRAVEL_SLACK,
            "no slack for a short hop"
        );
        assert!(travel_budget(1e9) <= 600.0, "unbounded budget");
        // Monotone, or a further target could get less time than a nearer one.
        assert!(travel_budget(400.0) > travel_budget(200.0));
    }

    /// Frames at a fixed position, returning the last bearing.
    ///
    /// A frame past the requested span, because ninety sixtieths of a second
    /// does not add up to 1.5 in binary and the window would never trip.
    fn pinned(travel: &mut Travel, pos: Vec2, target: Vec2, seconds: f32) -> Vec2 {
        const FRAME: f32 = 1.0 / 60.0;
        let frames = (seconds / FRAME).ceil() as u32 + 1;
        let mut last = Vec2::ZERO;
        for _ in 0..frames {
            last = travel.step(pos, target, FRAME);
        }
        last
    }

    #[test]
    fn travel_sidesteps_when_it_stops_making_ground() {
        let mut travel = Travel::default();
        let pos = Vec2::new(0.0, 0.0);
        let target = Vec2::new(100.0, 0.0);
        travel.begin(pos);
        // Pinned against something for less than the patience window.
        let direct = pinned(&mut travel, pos, target, Travel::PATIENCE * 0.5);
        assert!(direct.x > 0.9, "should head straight at first: {direct}");
        let detour = pinned(&mut travel, pos, target, Travel::PATIENCE);
        assert!(
            detour.y.abs() > 0.5,
            "should sidestep after making no ground: {detour}"
        );
    }

    #[test]
    fn a_detour_tries_the_other_side_next_time() {
        // One side of an obstacle is a detour; both sides is a search. Pacing
        // the same side forever is how a tester dies in a corner.
        const FRAME: f32 = 1.0 / 60.0;
        let mut travel = Travel::default();
        let pos = Vec2::ZERO;
        let target = Vec2::new(100.0, 0.0);
        travel.begin(pos);

        let mut left = false;
        let mut right = false;
        // Long enough for several patience windows to come and go.
        for _ in 0..600 {
            let bearing = travel.step(pos, target, FRAME);
            // Only a sidestep has a lateral component; the direct bearing at
            // this target is exactly +x.
            if bearing.y < -0.5 {
                left = true;
            }
            if bearing.y > 0.5 {
                right = true;
            }
        }
        assert!(
            left && right,
            "only ever went one way: left {left} right {right}"
        );
    }

    #[test]
    fn travel_walks_straight_at_a_target_it_is_reaching() {
        const FRAME: f32 = 1.0 / 60.0;
        let mut travel = Travel::default();
        let target = Vec2::new(100.0, 0.0);
        let mut pos = Vec2::ZERO;
        travel.begin(pos);
        for _ in 0..600 {
            let dir = travel.step(pos, target, FRAME);
            assert!(dir.x > 0.9, "detoured while making ground: {dir}");
            // A real walking pace, so every patience window sees ground gained.
            pos += dir * 8.0 * FRAME;
        }
    }

    #[test]
    fn cardinal_directions_press_one_key_and_diagonals_press_two() {
        // W is negative Y: the arena's second axis is world Z.
        assert_eq!(keys_for_direction(Vec2::NEG_Y), vec![KeyCode::KeyW]);
        assert_eq!(keys_for_direction(Vec2::Y), vec![KeyCode::KeyS]);
        assert_eq!(keys_for_direction(Vec2::NEG_X), vec![KeyCode::KeyA]);
        assert_eq!(keys_for_direction(Vec2::X), vec![KeyCode::KeyD]);

        let northeast = keys_for_direction(Vec2::new(1.0, -1.0).normalize());
        assert_eq!(northeast.len(), 2, "a diagonal should press two keys");
        assert!(northeast.contains(&KeyCode::KeyW) && northeast.contains(&KeyCode::KeyD));
    }

    #[test]
    fn a_zero_direction_presses_nothing() {
        assert!(keys_for_direction(Vec2::ZERO).is_empty());
    }

    #[test]
    fn opposite_keys_are_never_pressed_together() {
        // Pressing W and S at once cancels out in the movement code, so a
        // rounding slip here would silently freeze a roaming tester.
        let mut wander = Wander::default();
        for step in 0..400 {
            let dir = wander.step(Vec2::new(step as f32 * 0.01, 0.0), 0.1);
            let keys = keys_for_direction(dir);
            assert!(!(keys.contains(&KeyCode::KeyW) && keys.contains(&KeyCode::KeyS)));
            assert!(!(keys.contains(&KeyCode::KeyA) && keys.contains(&KeyCode::KeyD)));
        }
    }

    #[test]
    fn roaming_changes_heading_over_time() {
        let mut wander = Wander::default();
        let first = wander.step(Vec2::ZERO, 0.016);
        let mut moved = false;
        for _ in 0..300 {
            if wander.step(Vec2::ZERO, 0.05).distance(first) > 0.1 {
                moved = true;
                break;
            }
        }
        assert!(moved, "a roamer that never turns is just a straight line");
    }

    #[test]
    fn roaming_rerolls_immediately_when_progress_stalls() {
        // Standing still means something is in the way; the heading has to
        // change or the tester leans on a prop for the whole session.
        let mut wander = Wander::default();
        wander.step(Vec2::ZERO, 0.016);
        let stuck = wander.heading;
        for _ in 0..12 {
            wander.step(Vec2::ZERO, 0.1);
        }
        assert_ne!(wander.heading, stuck, "a blocked roamer must turn");
    }
    #[test]
    fn function_keys_resolve_and_do_not_shadow_the_letter_f() {
        assert_eq!(key_from_name("F5"), Some(KeyCode::F5));
        assert_eq!(key_from_name("f12"), Some(KeyCode::F12));
        assert_eq!(
            key_from_name("F"),
            Some(KeyCode::KeyF),
            "F is still a letter"
        );
        assert_eq!(key_from_name("F13"), None);
        assert_eq!(key_from_name("F0"), None);
    }
    #[test]
    fn checklist_entries_carry_the_local_name() {
        // The checklist is keyed on archetypes; the player only ever sees what
        // this world calls a thing. Without both, "enemy:StainSlime" is not
        // something a tester can act on.
        let label = label_for("weapon:RulerSweep", EnvKind::Forest);
        assert!(label.contains("RulerSweep"), "{label}");
        assert!(label.contains("Branch Sweep"), "{label}");

        let deed = label_for("deed:fort-taken", EnvKind::Forest);
        assert_eq!(deed, "deed:fort-taken", "deeds have no local name");

        let unknown = label_for("weapon:NotAThing", EnvKind::Desk);
        assert_eq!(unknown, "weapon:NotAThing");
    }
    #[test]
    fn defending_and_kiting_parse_and_do_not_shadow_holding_a_key() {
        assert_eq!(
            parse_line("defend 20 -8 30"),
            Ok(Some(Cmd::Steer(Steer::Hold(Vec2::new(20.0, -8.0)), 30.0)))
        );
        assert_eq!(
            parse_line("kite 12"),
            Ok(Some(Cmd::Steer(Steer::Kite, 12.0)))
        );
        // `hold` still means the key, which is what it has always meant.
        assert_eq!(
            parse_line("hold W 1.5"),
            Ok(Some(Cmd::Hold(vec![KeyCode::KeyW], 1.5)))
        );
        assert!(parse_line("defend 20").is_err());
    }
    #[test]
    fn a_ring_of_enemies_still_yields_a_direction() {
        // The failure this replaces: steering off the nearest enemy inside an
        // encirclement flips every frame and goes nowhere. A summed field has
        // to produce a usable bearing even when the crowd is nearly symmetric.
        //
        // Tested through the same arithmetic the system uses, since a `Query`
        // cannot be built outside a World.
        let ring: Vec<Vec2> = (0..12)
            .map(|i| {
                let a = i as f32 / 12.0 * std::f32::consts::TAU;
                Vec2::new(a.cos(), a.sin()) * 3.0
            })
            // A deliberate gap: one side of the ring is missing.
            .filter(|p| p.x < 2.0)
            .collect();

        let mut push = Vec2::ZERO;
        for foe in &ring {
            let delta = -*foe;
            let dist = delta.length();
            push += delta / (dist * dist * dist);
        }
        let bearing = push.normalize();
        assert!(
            bearing.x > 0.3,
            "the bearing should point at the gap, got {bearing:?}"
        );
    }

    #[test]
    fn a_committed_bearing_survives_a_symmetric_crowd() {
        // Nearly-symmetric rings sum to nearly zero, so the raw field is
        // unstable exactly when the player is in the most trouble.
        let noisy_a = Vec2::new(0.001, 0.002);
        let noisy_b = Vec2::new(-0.002, 0.001);
        let committed = Vec2::new(1.0, 0.0);
        let a = blend_escape(noisy_a, committed);
        let b = blend_escape(noisy_b, committed);
        assert!(a.distance(b) < 0.2, "bearing swung from {a:?} to {b:?}");
        assert!(a.x > 0.6, "it abandoned a perfectly good direction: {a:?}");
    }

    #[test]
    fn with_no_history_and_no_gap_it_still_picks_something() {
        let dir = blend_escape(Vec2::ZERO, Vec2::ZERO);
        assert!(
            dir.is_normalized(),
            "standing still is the one wrong answer"
        );
    }

    #[test]
    fn a_real_gap_still_turns_the_player_towards_it() {
        // Hysteresis must not become stubbornness.
        let mut dir = Vec2::new(-1.0, 0.0);
        for _ in 0..12 {
            dir = blend_escape(Vec2::new(4.0, 0.0), dir);
        }
        assert!(dir.x > 0.8, "never came round to the gap: {dir:?}");
    }

    #[test]
    fn the_escape_bearing_is_stable_as_the_crowd_creeps() {
        // If it swings wildly for a small change in the crowd, the player
        // dithers in place - which is the bug this exists to prevent.
        let crowd = |shift: f32| -> Vec2 {
            let mut push = Vec2::ZERO;
            for i in 0..8 {
                let a = i as f32 / 8.0 * std::f32::consts::TAU + shift;
                let foe = Vec2::new(a.cos(), a.sin() * 0.6) * 4.0;
                let delta = -foe;
                let dist = delta.length();
                push += delta / (dist * dist * dist);
            }
            push
        };
        // Through the blend, which is how the system actually uses it.
        let mut a = Vec2::new(1.0, 0.0);
        let mut b = a;
        for _ in 0..8 {
            a = blend_escape(crowd(0.0), a);
            b = blend_escape(crowd(0.05), b);
        }
        assert!(a.distance(b) < 0.5, "bearing jumped from {a:?} to {b:?}");
    }
}
