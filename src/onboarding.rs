//! Staged unlocks and contextual hints.
//!
//! A new player should be able to hold `W` and survive the first minute. Every
//! subsystem stays hidden until the run has earned it, and each one announces
//! itself with a single line naming the one key that matters. Nothing here
//! blocks input for an experienced player - the systems unlock on a timer they
//! cannot fail, and the hints are advisory.

use bevy::platform::collections::HashSet;
use bevy::prelude::*;

use crate::enemy::EnemyKind;
use crate::environments::EnvKind;
use crate::threat::RunClock;
use crate::{AppState, GameSet, RunSetup};

/// When each subsystem comes online, in run-seconds. Ordered so the player is
/// only ever learning one new verb at a time.
///
/// The threat dial used to be last, at 300 seconds. Measured median survival
/// across every recorded run is about 145 seconds, and the best run anyone has
/// managed reached the dial at 318 - so the game's *central* claim, that the
/// player owns the throttle, was something almost nobody ever got to do. It is
/// now the second thing they learn, right after building, which is also the
/// first moment they have anything to spend the extra rewards on.
pub const UNLOCK_BUILD: f32 = 45.0;
pub const UNLOCK_THREAT: f32 = 75.0;
pub const UNLOCK_TERRITORY: f32 = 110.0;
pub const UNLOCK_ALLIES: f32 = 165.0;
pub const UNLOCK_RESEARCH: f32 = 230.0;

// Five independent feature switches. A bitfield would be smaller and far less
// readable at every call site, and these are read far more often than stored.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Resource, Default)]
pub struct Unlocks {
    pub build: bool,
    pub territory: bool,
    pub allies: bool,
    pub research: bool,
    /// Until this is true the dial is driven automatically, ramping gently.
    pub threat_dial: bool,
    /// Kinds already introduced, so each gets exactly one "new threat" banner.
    pub seen_enemies: HashSet<u8>,
    pub seen_weapons: HashSet<u8>,
}

impl Unlocks {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Count of systems online, for the "discovered" line on the results screen.
    pub fn online(&self) -> u32 {
        u32::from(self.build)
            + u32::from(self.territory)
            + u32::from(self.allies)
            + u32::from(self.research)
            + u32::from(self.threat_dial)
    }
}

/// A queued banner. The HUD drains this; nothing else needs to know it exists.
#[derive(Debug, Clone)]
pub struct Hint {
    pub headline: String,
    pub detail: String,
    pub tone: HintTone,
    pub life: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintTone {
    /// A new system is available.
    Unlock,
    /// Something new showed up on the field.
    Discovery,
    /// Advisory nudge.
    Tip,
}

/// Tell the player why a key did nothing.
///
/// Every locked system used to fail silently, which is indistinguishable from
/// a broken keybinding - and that is exactly how it was reported.
pub fn locked_hint(hints: &mut HintQueue, system: &str, at: f32, now: f32) {
    let left = (at - now).max(0.0).ceil() as u32;
    hints.push(
        format!("{system} LOCKED"),
        if left > 0 {
            format!("Comes online at {}:{:02}.", at as u32 / 60, at as u32 % 60)
        } else {
            "Not available yet.".to_string()
        },
        HintTone::Tip,
    );
    let _ = left;
}

#[derive(Debug, Resource, Default)]
pub struct HintQueue {
    pub active: Option<Hint>,
    pub pending: Vec<Hint>,
    /// Tips that should only ever fire once per run.
    fired: HashSet<&'static str>,
}

impl HintQueue {
    pub fn reset(&mut self) {
        self.active = None;
        self.pending.clear();
        self.fired.clear();
    }

    pub fn push(&mut self, headline: impl Into<String>, detail: impl Into<String>, tone: HintTone) {
        self.pending.push(Hint {
            headline: headline.into(),
            detail: detail.into(),
            tone,
            life: match tone {
                HintTone::Unlock => 7.0,
                HintTone::Discovery => 3.2,
                HintTone::Tip => 5.0,
            },
        });
    }

    /// Push a tip at most once per run, keyed by a stable id.
    pub fn push_once(
        &mut self,
        id: &'static str,
        headline: impl Into<String>,
        detail: impl Into<String>,
        tone: HintTone,
    ) {
        if self.fired.insert(id) {
            self.push(headline, detail, tone);
        }
    }
}

#[derive(Debug)]
pub struct OnboardingPlugin;

impl Plugin for OnboardingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Unlocks>()
            .init_resource::<HintQueue>()
            .add_systems(
                Update,
                (
                    tick_unlocks,
                    announce_skill_points,
                    tick_hints,
                    notice_new_enemies,
                )
                    .in_set(GameSet::Present),
            )
            .add_systems(
                OnExit(AppState::Menu),
                reset_onboarding.in_set(RunSetup::Reset),
            );
    }
}

fn reset_onboarding(env: Res<EnvKind>, mut unlocks: ResMut<Unlocks>, mut hints: ResMut<HintQueue>) {
    unlocks.reset();
    hints.reset();
    // Named after wherever you actually are. This said "HOLD THE DESK" in the
    // Undergrowth, the Sanctum and everywhere else - the same class of mistake
    // as shooting pencil darts in the forest.
    hints.push(
        format!("HOLD {}", env.title()),
        "WASD to move. You attack automatically.",
        HintTone::Tip,
    );
    hints.push_once(
        "plan-intro",
        "PRESS SPACE TO PLAN",
        "Time slows to a crawl. Take as long as you like.",
        HintTone::Tip,
    );
}

fn tick_unlocks(clock: Res<RunClock>, mut unlocks: ResMut<Unlocks>, mut hints: ResMut<HintQueue>) {
    let t = clock.elapsed;

    if !unlocks.build && t >= UNLOCK_BUILD {
        unlocks.build = true;
        hints.push(
            "SALVAGE ONLINE",
            "Press B to build turrets with Scrap. Space slows time while you place.",
            HintTone::Unlock,
        );
    }
    if !unlocks.territory && t >= UNLOCK_TERRITORY {
        unlocks.territory = true;
        hints.push(
            "TERRITORY CONTESTED",
            "Stand on a marker to capture it. Held ground pays income.",
            HintTone::Unlock,
        );
    }
    if !unlocks.allies && t >= UNLOCK_ALLIES {
        unlocks.allies = true;
        hints.push(
            "SQUAD AVAILABLE",
            "Press R to recruit, it costs Cores. F rallies them, G cycles stance.",
            HintTone::Unlock,
        );
    }
    if !unlocks.research && t >= UNLOCK_RESEARCH {
        unlocks.research = true;
        hints.push(
            "RESEARCH UNLOCKED",
            "Press T to spend Cores - and your Skill Points - on permanent upgrades.",
            HintTone::Unlock,
        );
    }
    if !unlocks.threat_dial && t >= UNLOCK_THREAT {
        unlocks.threat_dial = true;
        hints.push(
            "THREAT DIAL RELEASED",
            "- and = set the pace. Higher threat pays more. O to Overclock.",
            HintTone::Unlock,
        );
    }
}

/// Say what a Skill Point is for, the first time one arrives.
///
/// They land at level three, which is usually inside the first minute, and
/// research does not open until 230 seconds - so a newcomer carried an unexplained
/// counter for three minutes with nothing anywhere telling them what it was or
/// what would ever take it. They reported exactly that.
fn announce_skill_points(
    progression: Res<crate::progress::Progression>,
    mut hints: ResMut<HintQueue>,
) {
    if progression.skill_points == 0 {
        return;
    }
    hints.push_once(
        "skill-points",
        "SKILL POINT EARNED",
        "One every third level. The deepest research nodes cost these as well as Cores.",
        HintTone::Unlock,
    );
}

fn tick_hints(time: Res<Time>, mut hints: ResMut<HintQueue>) {
    let dt = time.delta_secs();
    if let Some(active) = &mut hints.active {
        active.life -= dt;
        if active.life <= 0.0 {
            hints.active = None;
        }
    }
    if hints.active.is_none() && !hints.pending.is_empty() {
        hints.active = Some(hints.pending.remove(0));
    }
}

/// Announce each enemy archetype the first time it appears, which turns the
/// difficulty ramp into a stream of small reveals rather than a wall.
fn notice_new_enemies(
    env: Res<EnvKind>,
    mut unlocks: ResMut<Unlocks>,
    mut hints: ResMut<HintQueue>,
    q: Query<&crate::enemy::Enemy, Added<crate::enemy::Enemy>>,
) {
    for enemy in &q {
        let key = enemy.kind as u8;
        if unlocks.seen_enemies.insert(key) {
            let tone = if enemy.kind.is_boss() {
                HintTone::Unlock
            } else {
                HintTone::Discovery
            };
            let detail = enemy_tell(enemy.kind);
            hints.push(
                if enemy.kind.is_boss() {
                    format!("BOSS: {}", enemy.kind.name(*env))
                } else {
                    format!("NEW: {}", enemy.kind.name(*env))
                },
                detail,
                tone,
            );
        }
    }
}

/// One line per archetype describing the thing the player must actually do
/// about it. Teaching the counter matters more than describing the model.
fn enemy_tell(kind: EnemyKind) -> &'static str {
    match kind {
        EnemyKind::DustBunny => "Slow and harmless alone. Dangerous in a mass.",
        EnemyKind::Ant => "Fast, fragile. Let a turret handle the swarm.",
        EnemyKind::ClipCrawler => "Weaves as it closes. Splash damage beats it.",
        EnemyKind::StapleSkitter => "Freezes, then lunges. Step aside during the pause.",
        EnemyKind::CrumbBlob => "A slow wall of health. Ignore it or focus it down.",
        EnemyKind::TackLobber => "Hangs back and throws. Close the distance or block it.",
        EnemyKind::StainSlime => "Leaves a slowing trail. Do not fight it in a corridor.",
        EnemyKind::Moth => "Flies over barricades. Your turrets still see it.",
        EnemyKind::Gremlin => "Blinks past your defences. Keep a guard near your core.",
        EnemyKind::BossStapler => "Charges, then volleys. Barricades absorb the charge.",
        EnemyKind::BossHolePunch => "Slams a ring of shockwaves. Stand close or far, not mid.",
        EnemyKind::BossLamp => "Sweeps rotating beams. Circle in the same direction.",
    }
}
