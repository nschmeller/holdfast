//! THREAT: the player-steerable pacing dial.
//!
//! The run never ends and never stops escalating, but the player owns the
//! throttle. Raising threat floods the arena faster and hits harder; every
//! point of it also multiplies XP, scrap, drop rates and gear rarity. Lowering
//! it buys breathing room at the cost of falling behind the curve - and the
//! curve keeps moving, because the floor rises with elapsed time regardless.
//!
//! The interesting decisions all live in that tension: bank a quiet minute to
//! repair your turrets, or ride a surge to fund the build you actually want.

use bevy::prelude::*;

/// Absolute bounds on the dial. The ceiling is high enough to be genuinely
/// unsurvivable, which is the point - the player should be able to overreach.
pub const MIN_INTENT: f32 = 0.5;
pub const MAX_INTENT: f32 = 8.0;
const STEP: f32 = 0.25;

#[derive(Resource)]
pub struct Threat {
    /// What the dial actually reads right now; chases `intent`.
    pub level: f32,
    /// Where the player has set the dial.
    pub intent: f32,
    /// Rises with elapsed time. The dial can never be set below this, so
    /// hiding is always a delaying action rather than a strategy.
    pub floor: f32,
    /// Seconds left on an Overclock surge.
    pub surge: f32,
    pub surge_cooldown: f32,
    /// Recent-kill pressure; nudges threat up on its own so aggressive play
    /// escalates without the player having to touch the dial.
    pub streak: f32,
    /// Contribution from held territory, recomputed each frame.
    pub territory: f32,
    /// Purely for the HUD: flashes when the value changed.
    pub flash: f32,
}

impl Default for Threat {
    fn default() -> Self {
        Self {
            level: 1.0,
            intent: 1.0,
            floor: MIN_INTENT,
            surge: 0.0,
            surge_cooldown: 0.0,
            streak: 0.0,
            territory: 0.0,
            flash: 0.0,
        }
    }
}

/// How long an Overclock lasts, and how long before it can be used again.
pub const SURGE_DURATION: f32 = 22.0;
pub const SURGE_COOLDOWN: f32 = 80.0;
const SURGE_BONUS: f32 = 2.5;

impl Threat {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// The value every other system multiplies against.
    pub fn effective(&self) -> f32 {
        self.level + self.streak * 0.5 + self.territory
    }

    pub fn surging(&self) -> bool {
        self.surge > 0.0
    }

    /// XP, scrap and drop-count multiplier. Deliberately superlinear at the top
    /// so that pushing into genuinely dangerous territory pays for itself.
    pub fn reward_mult(&self) -> f32 {
        let e = self.effective();
        let base = e.powf(0.92);
        if self.surging() { base * 1.6 } else { base }
    }

    /// Extra chance for a drop to roll up a rarity tier.
    pub fn rarity_bonus(&self) -> f32 {
        ((self.effective() - 1.0) * 0.06).clamp(0.0, 0.55)
    }

    /// Spawn-rate multiplier.
    pub fn spawn_mult(&self) -> f32 {
        0.45 + self.effective() * 0.72
    }

    /// Enemy stat multiplier contribution.
    pub fn power_mult(&self) -> f32 {
        0.72 + self.effective() * 0.28
    }

    pub fn raise(&mut self) {
        self.intent = (self.intent + STEP).min(MAX_INTENT);
        self.flash = 0.5;
    }

    pub fn lower(&mut self) {
        self.intent = (self.intent - STEP).max(self.floor).max(MIN_INTENT);
        self.flash = 0.5;
    }

    pub fn can_surge(&self) -> bool {
        self.surge <= 0.0 && self.surge_cooldown <= 0.0
    }

    pub fn start_surge(&mut self) {
        if self.can_surge() {
            self.surge = SURGE_DURATION;
            self.surge_cooldown = SURGE_COOLDOWN + SURGE_DURATION;
            self.flash = 1.0;
        }
    }

    pub fn note_kill(&mut self) {
        // Saturates, so a big crowd clear does not launch the dial to the moon.
        self.streak = (self.streak + 0.035).min(1.6);
    }

    /// A short label for the HUD.
    pub fn band(&self) -> &'static str {
        match self.effective() {
            e if e < 1.2 => "LULL",
            e if e < 2.0 => "STIRRING",
            e if e < 3.0 => "BUSY",
            e if e < 4.2 => "SWARMING",
            e if e < 5.6 => "OVERRUN",
            e if e < 7.0 => "CRITICAL",
            _ => "APOCALYPTIC",
        }
    }

    pub fn band_color(&self) -> Color {
        match self.effective() {
            e if e < 1.2 => Color::srgb(0.55, 0.75, 0.95),
            e if e < 2.0 => Color::srgb(0.55, 0.9, 0.7),
            e if e < 3.0 => Color::srgb(0.95, 0.88, 0.45),
            e if e < 4.2 => Color::srgb(1.0, 0.66, 0.3),
            e if e < 5.6 => Color::srgb(1.0, 0.42, 0.3),
            e if e < 7.0 => Color::srgb(1.0, 0.28, 0.45),
            _ => Color::srgb(0.9, 0.3, 1.0),
        }
    }
}

/// Elapsed run time and the derived baseline difficulty. Split from `Threat` so
/// the pacing dial stays purely about player choice.
#[derive(Resource, Default)]
pub struct RunClock {
    pub elapsed: f32,
    pub kills: u64,
    pub best_streak: u32,
}

impl RunClock {
    /// Time-driven difficulty, independent of the dial. Compounding, so minute
    /// 20 is meaningfully worse than minute 10 rather than merely twice as busy.
    pub fn time_power(&self) -> f32 {
        let minutes = self.elapsed / 60.0;
        (1.0 + minutes * 0.42).powf(1.18)
    }

    /// The stage index, purely cosmetic labelling every 90 seconds.
    pub fn stage(&self) -> u32 {
        (self.elapsed / 90.0) as u32 + 1
    }
}

/// Combined enemy power scalar. One function so tuning happens in one place.
pub fn enemy_power(threat: &Threat, clock: &RunClock) -> f32 {
    clock.time_power() * threat.power_mult()
}

// -- the wave cycle ---------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    /// Light trickle only. Build, repair, reposition.
    Prep,
    /// The wave is inbound and the director is spending its budget.
    Assault,
}

/// The main rhythm of a run. Prep gives the player a window to act on the board
/// without being punished for it; calling the wave in early converts the unused
/// window directly into reward. That is the cleanest expression of the pacing
/// pillar: the player is always choosing between safety and payout.
#[derive(Resource)]
pub struct WaveCycle {
    pub phase: Phase,
    pub timer: f32,
    pub wave: u32,
    /// Reward multiplier earned by calling this wave early. Applies for the
    /// duration of the assault, then resets.
    pub early_bonus: f32,
    /// Enemy budget remaining in the current assault.
    pub budget: f32,
    pub prep_length: f32,
    pub announce: f32,
}

impl Default for WaveCycle {
    fn default() -> Self {
        Self {
            phase: Phase::Prep,
            // A generous opening window: the first thing a new player should do
            // is look around, not die.
            timer: 40.0,
            wave: 0,
            early_bonus: 0.0,
            budget: 0.0,
            prep_length: 40.0,
            announce: 0.0,
        }
    }
}

impl WaveCycle {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn in_prep(&self) -> bool {
        self.phase == Phase::Prep
    }

    /// Bonus the player would lock in by calling right now. Each unused second
    /// is worth 2.5%, so a full early call on a 25s window is +62%.
    pub fn pending_bonus(&self) -> f32 {
        if self.in_prep() {
            self.timer.max(0.0) * 0.025
        } else {
            0.0
        }
    }

    /// Total reward multiplier from the wave system alone.
    pub fn reward_mult(&self) -> f32 {
        1.0 + self.early_bonus
    }

    /// How long the current wave's assault runs. Grows slowly - the pressure
    /// is meant to come from density, not from longer sieges.
    pub fn assault_length(&self) -> f32 {
        20.0 + (self.wave as f32 * 0.4).min(12.0)
    }

    pub fn call_early(&mut self) {
        if self.in_prep() {
            self.early_bonus = self.pending_bonus();
            self.timer = 0.0;
        }
    }

    pub fn label(&self) -> &'static str {
        match self.phase {
            Phase::Prep => "PREP",
            Phase::Assault => "ASSAULT",
        }
    }
}

pub fn tick_waves(
    time: Res<Time>,
    mut cycle: ResMut<WaveCycle>,
    threat: Res<Threat>,
    clock: Res<RunClock>,
) {
    let dt = time.delta_secs();
    cycle.timer -= dt;
    cycle.announce = (cycle.announce - dt).max(0.0);

    if cycle.timer > 0.0 {
        return;
    }

    match cycle.phase {
        Phase::Prep => {
            cycle.phase = Phase::Assault;
            cycle.wave += 1;
            // Budget is "enemy value" the director may spend, scaled by both
            // clocks so waves keep pace with the rest of the escalation.
            cycle.budget = (18.0 + cycle.wave as f32 * 6.0)
                * threat.spawn_mult()
                * clock.time_power().sqrt();
            cycle.timer = cycle.assault_length();
            cycle.announce = 2.5;
        }
        Phase::Assault => {
            cycle.phase = Phase::Prep;
            cycle.early_bonus = 0.0;
            // Prep windows shrink as the run goes on, but never below 12s -
            // there is always time to make a decision.
            cycle.prep_length = (34.0 - cycle.wave as f32 * 0.9).max(12.0);
            cycle.timer = cycle.prep_length;
            cycle.announce = 2.0;
        }
    }
}

pub fn tick_threat(time: Res<Time>, mut threat: ResMut<Threat>, mut clock: ResMut<RunClock>) {
    let dt = time.delta_secs();
    clock.elapsed += dt;

    // The floor climbs about one full step every 45 seconds, which keeps a
    // pure-turtling strategy from ever being stable.
    threat.floor = (MIN_INTENT + clock.elapsed / 45.0 * 0.25).min(MAX_INTENT - 1.0);
    if threat.intent < threat.floor {
        threat.intent = threat.floor;
    }

    if threat.surge > 0.0 {
        threat.surge = (threat.surge - dt).max(0.0);
    }
    if threat.surge_cooldown > 0.0 {
        threat.surge_cooldown = (threat.surge_cooldown - dt).max(0.0);
    }

    // Kill pressure bleeds off, so the streak bonus rewards sustained
    // aggression rather than one good crowd clear.
    threat.streak = (threat.streak - dt * 0.22).max(0.0);
    threat.flash = (threat.flash - dt * 2.0).max(0.0);

    let target = threat.intent + if threat.surging() { SURGE_BONUS } else { 0.0 };
    // Threat rises faster than it falls: escalation is easy, de-escalation is
    // a commitment.
    let rate = if target > threat.level { 1.1 } else { 0.45 };
    let delta = target - threat.level;
    threat.level += delta.clamp(-rate * dt * 4.0, rate * dt * 4.0);
    threat.level = threat.level.clamp(MIN_INTENT, MAX_INTENT + SURGE_BONUS);
}
