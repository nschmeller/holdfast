//! Optional: let a language model retune the enemy AI while you play.
//!
//! If a foundation model happens to be reachable on this machine - Ollama, LM
//! Studio, anything speaking either of their APIs - the game will periodically
//! hand it a short digest of how the fight is going and ask for tactical
//! adjustments. The result nudges how hard factions commit to forts, how far
//! they chase, how fast they expand and how tightly they mass. The intent is an
//! opponent that is *wily* - that notices a player who never leaves their
//! turrets, or one who always runs, and stops obliging them.
//!
//! Three rules this is built around, in order of importance:
//!
//! 1. **Absence is the normal case.** Most machines have no such endpoint.
//!    Nothing here runs unless one answers a probe, and the hand-written
//!    director stays in charge either way. This is a garnish, not a dependency.
//! 2. **It must never block a frame.** Every request runs on the async task
//!    pool and is polled; a model that takes nine seconds costs nothing.
//! 3. **It must never be trusted.** A local 3B model will happily return prose,
//!    contradictions, or `aggression=999`. Every field is clamped into a range
//!    the game is known to survive, and anything unparseable leaves the
//!    previous value alone.
//!
//! There is no HTTP crate here, for the same reason there is no serialisation
//! crate: the dependency budget is zero. A POST to localhost is a socket, a
//! dozen lines of header, and a body - and the reply is parsed as `key=value`
//! lines rather than JSON, which is both easier to read out of a text response
//! and markedly more reliable from small models than asking for valid JSON.

use std::io::{Read as _, Write as _};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future};

use crate::factions::Faction;
use crate::forts::WarRoom;
use crate::threat::{RunClock, Threat};
use crate::{AppState, GameSet};

/// How long between consultations. Slow on purpose: the point is a shifting
/// disposition across a long run, not a twitch response, and a local model
/// answering every few seconds would heat the machine for nothing.
const CONSULT_PERIOD: f32 = 50.0;

/// Give up on a request after this. A model still thinking after half a minute
/// has missed the moment it was asked about.
const TIMEOUT: Duration = Duration::from_secs(30);

/// Endpoints probed when nothing is configured, in order.
const CANDIDATES: [(&str, Api); 2] = [
    ("127.0.0.1:11434", Api::Ollama),
    ("127.0.0.1:1234", Api::OpenAi),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Api {
    /// Ollama's own `/api/generate`.
    Ollama,
    /// The OpenAI-compatible `/v1/chat/completions` that LM Studio, llama.cpp
    /// and most local servers also speak.
    OpenAi,
}

/// The dials a model is allowed to touch.
///
/// A small, closed set with hard bounds. Handing a model the whole tuning
/// surface would make the game unshippable: every combination has to be
/// survivable, and this is the largest set that can be reasoned about.
#[derive(Resource, Debug, Clone)]
pub struct Tactics {
    /// Scales how readily factions mass on a fort rather than hunt.
    pub ambition: f32,
    /// Scales how far a monster will pursue before losing interest.
    pub aggression: f32,
    /// Scales seeder output, and so how fast the map fills.
    pub expansion: f32,
    /// High values mass the horde; low values spread it to surround.
    pub cohesion: f32,
    /// The model's one-line reasoning, shown in the HUD.
    pub note: String,
    /// False until a model has actually answered.
    pub live: bool,
}

impl Default for Tactics {
    fn default() -> Self {
        Self {
            ambition: 1.0,
            aggression: 1.0,
            expansion: 1.0,
            cohesion: 1.0,
            note: String::new(),
            live: false,
        }
    }
}

impl Tactics {
    /// Fold a model's reply in, clamping everything.
    ///
    /// Returns whether anything changed, so a reply that parsed to nothing
    /// does not get announced as an adjustment.
    pub fn apply(&mut self, reply: &str) -> bool {
        let mut touched = false;
        for line in reply.lines() {
            let line = line.trim().trim_start_matches(['-', '*', ' ']);
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let raw = value.trim().trim_matches(|c: char| !c.is_ascii_graphic());
            if key == "note" {
                // Keep it short; this goes in a HUD line, not a paragraph.
                self.note = raw.chars().take(70).collect();
                touched = true;
                continue;
            }
            let Ok(number) = raw.parse::<f32>() else {
                continue;
            };
            if !number.is_finite() {
                continue;
            }
            // Every dial lands in a band the game is known to survive. A model
            // asking for 999 gets 2.0 and no say in the matter.
            let clamped = number.clamp(0.4, 2.0);
            match key.as_str() {
                "ambition" => self.ambition = clamped,
                "aggression" => self.aggression = clamped,
                "expansion" => self.expansion = clamped,
                "cohesion" => self.cohesion = clamped,
                _ => continue,
            }
            touched = true;
        }
        if touched {
            self.live = true;
        }
        touched
    }
}

/// What the model is told. Deliberately small: a compact digest is both
/// cheaper and, with a 3B model, markedly more reliably answered than a dump.
#[derive(Debug, Clone, Default)]
pub struct Digest {
    pub minutes: f32,
    pub player_level: u32,
    pub player_hp_fraction: f32,
    pub deaths_inflicted: u32,
    pub kills_by_player: u64,
    pub forts_player: u32,
    pub forts_enemy: u32,
    pub monsters_alive: u32,
    pub threat: f32,
    /// How far the player has strayed from where they landed.
    pub distance_from_home: f32,
    /// Seconds since the player last took damage. A large number means
    /// whatever is being sent at them is not working.
    pub since_hurt: f32,
}

impl Digest {
    /// The whole prompt.
    ///
    /// Asks for `key=value` lines rather than JSON. Small local models produce
    /// malformed JSON often enough to matter, and never fail to produce a line
    /// with an equals sign in it.
    #[must_use]
    pub fn prompt(&self) -> String {
        format!(
            "You are tuning the enemy AI of a real-time strategy survival game.\n\
             Make the opposition wily and unpredictable: notice what the player is \
             getting away with, and stop letting them.\n\n\
             Situation:\n\
             - elapsed: {:.1} minutes\n\
             - player level: {}\n\
             - player health: {:.0}% \n\
             - seconds since the player last took damage: {:.0}\n\
             - player kills: {}\n\
             - times the player has died: {}\n\
             - forts held by player: {}\n\
             - forts held by monsters: {}\n\
             - monsters on the field: {}\n\
             - threat dial: {:.1} of 8\n\
             - player distance from their landing point: {:.0}\n\n\
             Reply with exactly these five lines and nothing else. \
             Numbers are multipliers between 0.4 and 2.0, where 1.0 is unchanged.\n\
             ambition=<how hard factions should commit to taking forts>\n\
             aggression=<how far monsters should chase the player>\n\
             expansion=<how fast monsters should plant new nests>\n\
             cohesion=<high to mass into one force, low to spread and surround>\n\
             note=<under ten words on why>",
            self.minutes,
            self.player_level,
            self.player_hp_fraction * 100.0,
            self.since_hurt,
            self.kills_by_player,
            self.deaths_inflicted,
            self.forts_player,
            self.forts_enemy,
            self.monsters_alive,
            self.threat,
            self.distance_from_home,
        )
    }
}

// -- transport --------------------------------------------------------------

/// Escape a string for inclusion in a JSON body. The only JSON this module
/// writes, and it is one field.
fn json_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// Pull the model's text back out of either API's response shape.
///
/// A hand-rolled scan rather than a JSON parse: both shapes put the text in a
/// single known field, and the reply only has to survive being read, not being
/// round-tripped.
#[must_use]
pub fn extract_text(body: &str, api: Api) -> Option<String> {
    let key = match api {
        Api::Ollama => "\"response\":",
        Api::OpenAi => "\"content\":",
    };
    let start = body.find(key)? + key.len();
    let rest = body[start..].trim_start();
    let rest = rest.strip_prefix('"')?;

    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => {}
                Some('u') => {
                    // Skip the four hex digits; nothing we need arrives as an
                    // escape, and mangling one is better than bailing out.
                    for _ in 0..4 {
                        chars.next();
                    }
                }
                Some(other) => out.push(other),
                None => break,
            },
            c => out.push(c),
        }
    }
    None
}

fn request_body(api: Api, model: &str, prompt: &str) -> String {
    let prompt = json_escape(prompt);
    match api {
        Api::Ollama => format!(
            "{{\"model\":\"{model}\",\"prompt\":\"{prompt}\",\"stream\":false,\
             \"think\":false,\"options\":{{\"temperature\":0.8,\"num_predict\":120}}}}"
        ),
        Api::OpenAi => format!(
            "{{\"model\":\"{model}\",\"messages\":[{{\"role\":\"user\",\"content\":\"{prompt}\"}}],\
             \"temperature\":0.8,\"max_tokens\":120,\"stream\":false}}"
        ),
    }
}

fn path_for(api: Api) -> &'static str {
    match api {
        Api::Ollama => "/api/generate",
        Api::OpenAi => "/v1/chat/completions",
    }
}

/// One blocking HTTP/1.1 POST. Only ever called from the async task pool.
fn post(host: &str, api: Api, model: &str, prompt: &str) -> Option<String> {
    let addr = host.to_socket_addrs().ok()?.next()?;
    let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).ok()?;
    stream.set_read_timeout(Some(TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(TIMEOUT)).ok()?;
    let mut stream = stream;

    let body = request_body(api, model, prompt);
    let head = format!(
        "POST {} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n",
        path_for(api),
        body.len()
    );
    stream.write_all(head.as_bytes()).ok()?;
    stream.write_all(body.as_bytes()).ok()?;
    stream.flush().ok()?;

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).ok()?;
    let text = String::from_utf8_lossy(&raw);
    let (_, payload) = text.split_once("\r\n\r\n")?;
    extract_text(payload, api)
}

/// Is anything listening?
fn probe(host: &str) -> bool {
    host.to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next())
        .is_some_and(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok())
}

/// Ask Ollama which model to use. Anything is better than a guess at a name.
fn first_ollama_model(host: &str) -> Option<String> {
    let addr = host.to_socket_addrs().ok()?.next()?;
    let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut stream = stream;
    let head = format!("GET /api/tags HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    stream.write_all(head.as_bytes()).ok()?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).ok()?;
    let text = String::from_utf8_lossy(&raw);
    // `"name":"qwen3:4b"` - the first one will do.
    let at = text.find("\"name\":\"")? + 8;
    let end = text[at..].find('"')?;
    Some(text[at..at + end].to_string())
}

// -- the platform's own model ------------------------------------------------

/// A callback the platform wrapper installs so the game can reach the model
/// the *device* provides rather than one over a socket.
///
/// iOS 18 has Apple's on-device foundation model behind a Swift-only
/// framework, and Android has Gemini Nano behind `AICore` - neither is callable
/// from Rust, and neither should be reimplemented here. So the wrapper does
/// the call in its own language and hands the text back through one function
/// pointer. Rust owns the prompt and the parsing; the platform owns the model.
///
/// The contract: take a NUL-terminated prompt, return a NUL-terminated reply
/// allocated by the host, or null if unavailable. The host frees it through
/// `free`, which is called with the pointer it returned.
pub type AskHost = extern "C" fn(prompt: *const std::ffi::c_char) -> *mut std::ffi::c_char;
pub type FreeReply = extern "C" fn(reply: *mut std::ffi::c_char);

static HOST_BRIDGE: std::sync::OnceLock<(AskHost, FreeReply)> = std::sync::OnceLock::new();

/// Install the platform's model. Called once by the iOS or Android wrapper
/// before `holdfast_main` or `android_main`.
///
/// Safe to never call: with no bridge installed the game probes for a local
/// HTTP endpoint instead, and failing that runs its own director.
#[expect(
    unsafe_code,
    reason = "the platform wrapper links this symbol by name; the body is safe"
)]
#[unsafe(no_mangle)]
pub extern "C" fn holdfast_set_model_bridge(ask: AskHost, free: FreeReply) {
    let _ = HOST_BRIDGE.set((ask, free));
}

#[must_use]
pub fn host_bridge_installed() -> bool {
    HOST_BRIDGE.get().is_some()
}

/// Ask the platform's model. Blocking; only ever called from the task pool.
fn ask_host(prompt: &str) -> Option<String> {
    let (ask, free) = HOST_BRIDGE.get()?;
    let c_prompt = std::ffi::CString::new(prompt).ok()?;
    let reply = ask(c_prompt.as_ptr());
    if reply.is_null() {
        return None;
    }
    // The host allocated this and the host frees it; copying it into a Rust
    // String first means the lifetime question never leaves this function.
    #[expect(
        unsafe_code,
        reason = "reading a NUL-terminated buffer the host just handed us"
    )]
    let text = unsafe { std::ffi::CStr::from_ptr(reply) }
        .to_string_lossy()
        .into_owned();
    free(reply);
    Some(text)
}

// -- plumbing ---------------------------------------------------------------

/// Where the advice comes from.
#[derive(Resource, Debug, Clone)]
pub enum Advisor {
    /// The device's own model, through the platform wrapper.
    Host,
    /// Something speaking HTTP on this machine.
    Endpoint {
        host: String,
        api: Api,
        model: String,
    },
}

impl Advisor {
    fn consult(&self, prompt: &str) -> Option<String> {
        match self {
            Self::Host => ask_host(prompt),
            Self::Endpoint { host, api, model } => post(host, *api, model, prompt),
        }
    }

    fn describe(&self) -> String {
        match self {
            Self::Host => "the platform's own model".to_string(),
            Self::Endpoint { host, model, .. } => format!("{model} at {host}"),
        }
    }
}

#[derive(Resource)]
struct Pending(Option<Task<Option<String>>>);

/// How long before the *first* consultation. Much shorter than the interval:
/// an early read is worth having, and waiting a full period means a run that
/// ends inside a minute never sees the feature at all.
const FIRST_CONSULT: f32 = 18.0;

#[derive(Resource, Debug)]
struct Cadence {
    since: f32,
    /// Seconds since the player last took a hit, tracked here because nothing
    /// else needs it.
    since_hurt: f32,
    last_hp: f32,
}

impl Default for Cadence {
    fn default() -> Self {
        Self {
            since: CONSULT_PERIOD - FIRST_CONSULT,
            since_hurt: 0.0,
            last_hp: f32::MAX,
        }
    }
}

#[derive(Debug)]
pub struct TacticianPlugin;

impl Plugin for TacticianPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tactics>();

        // The device's own model wins if the wrapper installed one: it is
        // faster, it is private, and it works with the network off.
        if host_bridge_installed() {
            info!("tactician: using the platform's own model");
            app.insert_resource(Advisor::Host)
                .insert_resource(Pending(None))
                .init_resource::<Cadence>()
                .add_systems(
                    Update,
                    (consult, collect_reply)
                        .chain()
                        .in_set(GameSet::Present)
                        .run_if(in_state(AppState::Playing)),
                );
            return;
        }

        // Probing costs a connect attempt with a quarter-second timeout, once,
        // at startup. Configured endpoints skip the probe entirely.
        let configured = std::env::var("HOLDFAST_LLM").ok();
        let found = configured
            .as_deref()
            .filter(|h| !h.is_empty())
            .map(|host| {
                let api = if host.ends_with(":11434") {
                    Api::Ollama
                } else {
                    Api::OpenAi
                };
                (host.to_string(), api)
            })
            .or_else(|| {
                CANDIDATES
                    .iter()
                    .find(|(host, _)| probe(host))
                    .map(|(host, api)| ((*host).to_string(), *api))
            });

        let Some((host, api)) = found else {
            info!("tactician: no local model endpoint; using the built-in director");
            return;
        };

        let model = std::env::var("HOLDFAST_LLM_MODEL").ok().or_else(|| {
            if api == Api::Ollama {
                first_ollama_model(&host)
            } else {
                None
            }
        });
        let Some(model) = model else {
            info!("tactician: {host} answered but offers no model; using the built-in director");
            return;
        };

        let advisor = Advisor::Endpoint { host, api, model };
        info!(
            "tactician: consulting {} every {CONSULT_PERIOD:.0}s",
            advisor.describe()
        );
        app.insert_resource(advisor)
            .insert_resource(Pending(None))
            .init_resource::<Cadence>()
            .add_systems(
                Update,
                (consult, collect_reply)
                    .chain()
                    .in_set(GameSet::Present)
                    .run_if(in_state(AppState::Playing)),
            );
    }
}

#[allow(clippy::too_many_arguments)]
fn consult(
    time: Res<Time>,
    advisor: Res<Advisor>,
    mut pending: ResMut<Pending>,
    mut cadence: ResMut<Cadence>,
    clock: Res<RunClock>,
    threat: Res<Threat>,
    progression: Res<crate::progress::Progression>,
    ledger: Res<crate::stats::Ledger>,
    player: Query<(&crate::common::Body, &crate::common::Health), With<crate::player::Player>>,
    enemies: Query<(), With<crate::enemy::Enemy>>,
    forts: Query<&crate::factions::Allegiance, With<crate::forts::Fort>>,
) {
    let dt = time.delta_secs();
    cadence.since += dt;
    cadence.since_hurt += dt;

    let Some((body, health)) = player.iter().next() else {
        return;
    };
    if health.current < cadence.last_hp {
        cadence.since_hurt = 0.0;
    }
    cadence.last_hp = health.current;

    if cadence.since < CONSULT_PERIOD || pending.0.is_some() {
        return;
    }
    cadence.since = 0.0;

    let digest = Digest {
        minutes: clock.elapsed / 60.0,
        player_level: progression.level,
        player_hp_fraction: health.current / health.max.max(1.0),
        deaths_inflicted: ledger.get(crate::stats::stat::DEATHS) as u32,
        kills_by_player: clock.kills,
        forts_player: forts.iter().filter(|a| a.0 == Faction::Player).count() as u32,
        forts_enemy: forts.iter().filter(|a| a.0 != Faction::Player).count() as u32,
        monsters_alive: enemies.iter().count() as u32,
        threat: threat.effective(),
        distance_from_home: body.pos.length(),
        since_hurt: cadence.since_hurt,
    };

    let source = advisor.clone();
    let prompt = digest.prompt();
    pending.0 = Some(AsyncComputeTaskPool::get().spawn(async move { source.consult(&prompt) }));
}

fn collect_reply(
    mut pending: ResMut<Pending>,
    mut tactics: ResMut<Tactics>,
    mut war: ResMut<WarRoom>,
    mut hints: ResMut<crate::onboarding::HintQueue>,
) {
    let Some(task) = pending.0.as_mut() else {
        return;
    };
    let Some(result) = block_on(future::poll_once(task)) else {
        return;
    };
    pending.0 = None;

    let Some(reply) = result else {
        // A failed request is not worth telling the player about. The
        // built-in director never stopped running.
        return;
    };
    if !tactics.apply(&reply) {
        return;
    }

    // Make the adjustment visible. An opponent that changes its mind silently
    // is indistinguishable from one that does not.
    let note = if tactics.note.is_empty() {
        "The opposition is rethinking.".to_string()
    } else {
        tactics.note.clone()
    };
    hints.push("THEY ADAPT", note, crate::onboarding::HintTone::Discovery);
    // Nudge the war room to re-plan against the new disposition rather than
    // waiting out its own timer.
    war.headline = Some("THE ENEMY CHANGES ITS APPROACH".to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_well_formed_reply_moves_every_dial() {
        let mut tactics = Tactics::default();
        assert!(tactics.apply(
            "ambition=1.6\naggression=0.7\nexpansion=1.3\ncohesion=0.5\nnote=turtling, spread out"
        ));
        assert!((tactics.ambition - 1.6).abs() < 1e-6);
        assert!((tactics.aggression - 0.7).abs() < 1e-6);
        assert!((tactics.expansion - 1.3).abs() < 1e-6);
        assert!((tactics.cohesion - 0.5).abs() < 1e-6);
        assert_eq!(tactics.note, "turtling, spread out");
        assert!(tactics.live);
    }

    #[test]
    fn absurd_values_are_clamped_rather_than_obeyed() {
        // A 3B model will absolutely return this.
        let mut tactics = Tactics::default();
        tactics.apply("ambition=999\naggression=-40\nexpansion=1e9\ncohesion=0");
        for value in [
            tactics.ambition,
            tactics.aggression,
            tactics.expansion,
            tactics.cohesion,
        ] {
            assert!((0.4..=2.0).contains(&value), "{value} escaped the clamp");
        }
    }

    #[test]
    fn nonsense_leaves_the_previous_tuning_alone() {
        let mut tactics = Tactics::default();
        tactics.apply("ambition=1.5");
        let before = tactics.ambition;
        assert!(!tactics.apply("I think the enemy should be more aggressive!"));
        assert!((tactics.ambition - before).abs() < 1e-6);
    }

    #[test]
    fn non_finite_numbers_are_ignored() {
        let mut tactics = Tactics::default();
        tactics.apply("ambition=NaN\naggression=inf");
        assert!((tactics.ambition - 1.0).abs() < 1e-6);
        assert!((tactics.aggression - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_chatty_model_still_gets_parsed() {
        // Small models pad with prose and bullets no matter how firmly asked.
        let mut tactics = Tactics::default();
        let reply = "Sure! Here is my analysis.\n\n\
                     - ambition=1.4\n  * aggression = 0.8 \n\
                     Some more thoughts here.\nexpansion=1.1\n\
                     Hope that helps!";
        assert!(tactics.apply(reply));
        assert!((tactics.ambition - 1.4).abs() < 1e-6);
        assert!((tactics.aggression - 0.8).abs() < 1e-6);
        assert!((tactics.expansion - 1.1).abs() < 1e-6);
    }

    #[test]
    fn an_unknown_key_is_not_a_way_in() {
        let mut tactics = Tactics::default();
        assert!(!tactics.apply("player_health=0\nspawn_rate=50"));
        assert!(!tactics.live);
    }

    #[test]
    fn a_rambling_note_is_cut_to_a_hud_line() {
        let mut tactics = Tactics::default();
        tactics.apply(&format!("note={}", "x".repeat(400)));
        assert!(tactics.note.len() <= 70);
    }

    #[test]
    fn defaults_are_neutral_and_not_live() {
        let tactics = Tactics::default();
        assert!((tactics.ambition - 1.0).abs() < 1e-6);
        assert!(!tactics.live, "nothing has answered yet");
    }

    #[test]
    fn ollama_and_openai_replies_both_yield_their_text() {
        let ollama = r#"{"model":"qwen3:4b","response":"ambition=1.2\nnote=fine","done":true}"#;
        assert_eq!(
            extract_text(ollama, Api::Ollama).as_deref(),
            Some("ambition=1.2\nnote=fine")
        );

        let openai = r#"{"choices":[{"message":{"role":"assistant","content":"ambition=0.9"}}]}"#;
        assert_eq!(
            extract_text(openai, Api::OpenAi).as_deref(),
            Some("ambition=0.9")
        );
    }

    #[test]
    fn a_reply_with_quotes_in_it_survives_extraction() {
        let body = r#"{"response":"note=said \"hold\" and meant it"}"#;
        assert_eq!(
            extract_text(body, Api::Ollama).as_deref(),
            Some("note=said \"hold\" and meant it")
        );
    }

    #[test]
    fn a_truncated_response_yields_nothing_rather_than_garbage() {
        assert!(extract_text(r#"{"response":"ambition=1.2"#, Api::Ollama).is_none());
        assert!(extract_text("not json at all", Api::Ollama).is_none());
        assert!(extract_text("", Api::OpenAi).is_none());
    }

    #[test]
    fn the_prompt_carries_the_situation_and_asks_for_the_format() {
        let digest = Digest {
            minutes: 12.5,
            player_level: 22,
            player_hp_fraction: 0.4,
            forts_player: 3,
            monsters_alive: 77,
            ..Digest::default()
        };
        let prompt = digest.prompt();
        for needle in ["12.5", "22", "77", "ambition=", "cohesion=", "note="] {
            assert!(prompt.contains(needle), "prompt is missing {needle}");
        }
    }

    #[test]
    fn the_request_body_is_valid_for_both_apis() {
        // Newlines in the prompt have to be escaped or the body is malformed.
        let prompt = "line one\nline \"two\"";
        for api in [Api::Ollama, Api::OpenAi] {
            let body = request_body(api, "m", prompt);
            assert!(!body.contains('\n'), "{api:?} body has a raw newline");
            assert!(body.contains("line one\\nline \\\"two\\\""));
            assert!(body.starts_with('{') && body.ends_with('}'));
        }
    }

    #[test]
    fn escaping_covers_what_a_prompt_can_contain() {
        assert_eq!(json_escape("a\"b\\c\nd\te"), "a\\\"b\\\\c\\nd\\te");
        assert_eq!(json_escape("bell\u{7}"), "bell ");
    }
}
