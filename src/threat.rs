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

#[derive(Debug, Resource)]
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
#[derive(Debug, Resource, Default)]
pub struct RunClock {
    pub elapsed: f32,
    pub kills: u64,
    pub best_streak: u32,
    /// Furthest from the landing site *this run*.
    ///
    /// The dossier used to read this off the lifetime ledger, which is a
    /// personal best across every run ever played. So every row after the
    /// first reported the same number and the column - meant to answer "did
    /// this strategy leave home?" - could not distinguish a tester who
    /// travelled 900 units from one who never moved.
    pub furthest: f32,
}

impl RunClock {
    /// Remember the highest kill streak reached.
    ///
    /// `best_streak` was saved, loaded, reported by the pilot and shown on the
    /// results screen while nothing ever assigned it - every dump read zero
    /// after thousands of kills.
    /// Widen the run's travel record.
    pub fn note_distance(&mut self, from_origin: f32) {
        self.furthest = self.furthest.max(from_origin);
    }

    pub fn note_streak(&mut self, streak: f32) {
        let peak = u32::try_from(streak.max(0.0) as u64).unwrap_or(u32::MAX);
        self.best_streak = self.best_streak.max(peak);
    }

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
pub fn enemy_power(threat: &Threat, clock: &RunClock, level: u32) -> f32 {
    clock.time_power() * threat.power_mult() * level_power(level) * opening_grace(clock.elapsed)
}

/// How much of its full strength the opposition brings in the opening.
///
/// The first ninety seconds are the only part of a run the player has no tools
/// for: one weapon, no structures, no squad, no research, and a level curve
/// that has not started paying out. Playtesting kept ending at thirty to forty
/// seconds, which is not difficulty, it is a game that never began.
///
/// This ramps rather than switching, so there is no moment where the fight
/// visibly changes gear, and it is a separate multiplier rather than a bend in
/// `time_power` so the long curve it modifies stays readable on its own.
#[must_use]
pub fn opening_grace(elapsed: f32) -> f32 {
    const GRACE: f32 = 90.0;
    const OPENING: f32 = 0.45;
    if elapsed >= GRACE {
        return 1.0;
    }
    OPENING + (1.0 - OPENING) * (elapsed / GRACE)
}

/// How much harder the opposition gets for every level the player takes.
///
/// Difficulty used to follow the clock alone, and a player who levelled
/// quickly simply outran it - playtesting described the result as becoming "a
/// hurricane of destruction", which is a compliment about the build and a
/// complaint about the game.
///
/// This is not rubber-banding. The growth per level is far below what a card
/// gives the player, so getting stronger still means *being* stronger; it just
/// stops the curve from being a formality. It also fits the central pillar:
/// levelling faster is one more way to speed the game up, and speeding the
/// game up is supposed to cost something.
#[must_use]
pub fn level_power(level: u32) -> f32 {
    1.0 + f32::from(u16::try_from(level.saturating_sub(1)).unwrap_or(u16::MAX)) * 0.085
}

// -- the wave cycle ---------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    /// Light trickle only. Build, repair, reposition.
    Prep,
    /// The wave is inbound and the director is spending its budget.
    Assault,
}

/// The main rhythm of a run.
///
/// Prep gives the player a window to act on the board without being punished
/// for it; calling the wave in early converts the unused window directly into
/// reward. That is the cleanest expression of the pacing pillar: the player is
/// always choosing between safety and payout.
#[derive(Debug, Resource)]
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
            cycle.budget =
                (18.0 + cycle.wave as f32 * 6.0) * threat.spawn_mult() * clock.time_power().sqrt();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_dial_starts_calm() {
        let t = Threat::default();
        assert_eq!(t.level, 1.0);
        assert_eq!(t.band(), "LULL");
        assert!(!t.surging());
        assert!(t.can_surge());
    }

    #[test]
    fn raising_and_lowering_respects_the_hard_limits() {
        let mut t = Threat::default();
        for _ in 0..200 {
            t.raise();
        }
        assert_eq!(t.intent, MAX_INTENT);
        for _ in 0..200 {
            t.lower();
        }
        assert_eq!(t.intent, MIN_INTENT);
    }

    #[test]
    fn the_dial_cannot_be_set_below_the_floor() {
        let mut t = Threat {
            floor: 3.0,
            intent: 3.0,
            ..Threat::default()
        };
        t.lower();
        assert_eq!(t.intent, 3.0, "the floor is what stops turtling");
    }

    #[test]
    fn rewards_grow_with_threat() {
        let low = Threat {
            level: 1.0,
            ..Threat::default()
        };
        let high = Threat {
            level: 6.0,
            ..Threat::default()
        };
        assert!(high.reward_mult() > low.reward_mult() * 4.0);
    }

    #[test]
    fn rewards_are_monotonic_across_the_whole_range() {
        let mut previous = 0.0;
        let mut t = Threat::default();
        let steps = ((MAX_INTENT - MIN_INTENT) / 0.25) as u32;
        for step in 0..=steps {
            let level = MIN_INTENT + step as f32 * 0.25;
            t.level = level;
            let r = t.reward_mult();
            assert!(r > previous, "reward went down at level {level}");
            previous = r;
        }
    }

    #[test]
    fn surging_pays_a_premium() {
        let mut t = Threat {
            level: 3.0,
            ..Threat::default()
        };
        let calm = t.reward_mult();
        t.surge = 5.0;
        assert!(t.reward_mult() > calm * 1.5);
    }

    #[test]
    fn spawn_and_power_scale_with_the_dial() {
        let mut t = Threat {
            level: 1.0,
            ..Threat::default()
        };
        let (s1, p1) = (t.spawn_mult(), t.power_mult());
        t.level = 5.0;
        assert!(t.spawn_mult() > s1);
        assert!(t.power_mult() > p1);
    }

    #[test]
    fn rarity_bonus_is_clamped() {
        let mut t = Threat {
            level: MIN_INTENT,
            ..Threat::default()
        };
        assert_eq!(t.rarity_bonus(), 0.0, "never negative");
        t.level = MAX_INTENT;
        t.territory = 10.0;
        assert!(t.rarity_bonus() <= 0.55);
    }

    #[test]
    fn a_surge_cannot_be_restarted_while_running() {
        let mut t = Threat::default();
        t.start_surge();
        assert!(t.surging());
        let remaining = t.surge;
        t.start_surge();
        assert_eq!(t.surge, remaining, "double-tapping O must not extend it");
        assert!(!t.can_surge());
    }

    #[test]
    fn the_surge_cooldown_outlasts_the_surge() {
        let mut t = Threat::default();
        t.start_surge();
        assert!(t.surge_cooldown > t.surge);
    }

    #[test]
    fn kill_pressure_saturates() {
        let mut t = Threat::default();
        for _ in 0..10_000 {
            t.note_kill();
        }
        assert!(t.streak <= 1.6, "streak ran away to {}", t.streak);
    }

    #[test]
    fn effective_threat_includes_streak_and_territory() {
        let t = Threat {
            level: 2.0,
            streak: 1.0,
            territory: 0.4,
            ..Threat::default()
        };
        assert!((t.effective() - 2.9).abs() < 1e-5);
    }

    #[test]
    fn bands_and_colours_cover_the_whole_range() {
        let mut t = Threat::default();
        let mut seen = Vec::new();
        let steps = ((MAX_INTENT + 2.5 - MIN_INTENT) / 0.1) as u32;
        for step in 0..=steps {
            t.level = MIN_INTENT + step as f32 * 0.1;
            let band = t.band();
            if seen.last() != Some(&band) {
                seen.push(band);
            }
            // Every level must produce a colour without panicking.
            let _ = t.band_color();
        }
        assert_eq!(
            seen,
            vec![
                "LULL",
                "STIRRING",
                "BUSY",
                "SWARMING",
                "OVERRUN",
                "CRITICAL",
                "APOCALYPTIC"
            ]
        );
    }

    #[test]
    fn reset_returns_to_the_default_state() {
        let mut t = Threat {
            level: 7.0,
            streak: 1.0,
            ..Threat::default()
        };
        t.start_surge();
        t.reset();
        assert_eq!(t.level, 1.0);
        assert_eq!(t.streak, 0.0);
        assert!(t.can_surge());
    }

    #[test]
    fn time_power_compounds() {
        let mut clock = RunClock {
            elapsed: 0.0,
            ..RunClock::default()
        };
        let start = clock.time_power();
        clock.elapsed = 600.0;
        let ten_minutes = clock.time_power();
        clock.elapsed = 1200.0;
        let twenty_minutes = clock.time_power();
        assert!(start >= 1.0);
        // Compounding means the second ten minutes adds more than the first.
        assert!(twenty_minutes - ten_minutes > ten_minutes - start);
    }

    #[test]
    fn stages_advance_every_ninety_seconds() {
        let mut clock = RunClock::default();
        assert_eq!(clock.stage(), 1);
        clock.elapsed = 89.0;
        assert_eq!(clock.stage(), 1);
        clock.elapsed = 90.0;
        assert_eq!(clock.stage(), 2);
    }

    #[test]
    fn enemy_power_combines_both_clocks() {
        let mut t = Threat::default();
        let mut clock = RunClock::default();
        let base = enemy_power(&t, &clock, 1);
        t.level = 6.0;
        assert!(enemy_power(&t, &clock, 1) > base);
        t.level = 1.0;
        clock.elapsed = 900.0;
        assert!(enemy_power(&t, &clock, 1) > base);
    }

    // -- wave cycle ---------------------------------------------------------

    #[test]
    fn a_run_opens_in_prep() {
        let c = WaveCycle::default();
        assert!(c.in_prep());
        assert_eq!(c.wave, 0);
        assert_eq!(c.label(), "PREP");
    }

    #[test]
    fn calling_early_banks_the_unused_window() {
        let mut c = WaveCycle {
            timer: 20.0,
            ..WaveCycle::default()
        };
        let expected = c.pending_bonus();
        assert!((expected - 0.5).abs() < 1e-6, "20s at 2.5%/s");
        c.call_early();
        assert_eq!(c.timer, 0.0);
        assert!((c.reward_mult() - 1.5).abs() < 1e-6);
    }

    #[test]
    fn calling_early_during_an_assault_does_nothing() {
        let mut c = WaveCycle {
            phase: Phase::Assault,
            timer: 10.0,
            ..WaveCycle::default()
        };
        assert_eq!(c.pending_bonus(), 0.0);
        c.call_early();
        assert_eq!(c.timer, 10.0);
    }

    #[test]
    fn a_full_wait_earns_no_bonus() {
        let c = WaveCycle {
            timer: 0.0,
            ..WaveCycle::default()
        };
        assert_eq!(c.pending_bonus(), 0.0);
        assert_eq!(c.reward_mult(), 1.0);
    }

    #[test]
    fn assault_length_grows_but_is_capped() {
        let mut c = WaveCycle {
            wave: 1,
            ..WaveCycle::default()
        };
        let early = c.assault_length();
        c.wave = 500;
        let late = c.assault_length();
        assert!(late > early);
        assert!(
            late <= 32.0 + 1e-6,
            "capped at base plus twelve, got {late}"
        );
    }

    #[test]
    fn prep_windows_shrink_but_never_vanish() {
        let mut c = WaveCycle::default();
        for wave in 0..200u32 {
            c.wave = wave;
            c.prep_length = (34.0 - wave as f32 * 0.9).max(12.0);
            assert!(c.prep_length >= 12.0, "prep dropped to {}", c.prep_length);
        }
    }
    #[test]
    fn difficulty_follows_the_player_up_the_level_curve() {
        // The complaint this exists to answer: level fast enough and the
        // opposition stops mattering.
        let t = Threat::default();
        let clock = RunClock::default();
        let fresh = enemy_power(&t, &clock, 1);
        let veteran = enemy_power(&t, &clock, 30);
        assert!(
            veteran > fresh * 3.0,
            "a level-30 player faces {veteran} against {fresh} at level 1"
        );
    }

    #[test]
    fn levelling_never_makes_the_game_easier() {
        let t = Threat::default();
        let clock = RunClock::default();
        let mut last = 0.0;
        for level in 1..=80 {
            let power = enemy_power(&t, &clock, level);
            assert!(power >= last, "power dipped at level {level}");
            last = power;
        }
    }

    #[test]
    fn the_level_term_is_weaker_than_the_upgrades_it_answers() {
        // If it matched the player's own growth it would be rubber-banding,
        // and getting stronger would stop meaning anything.
        let per_level = level_power(2) - level_power(1);
        assert!(
            per_level < 0.12,
            "{per_level} per level erases the player's progress"
        );
        assert!(per_level > 0.0, "the term does nothing at all");
    }

    #[test]
    fn a_hundred_levels_does_not_overflow_the_curve() {
        let t = Threat::default();
        let clock = RunClock::default();
        assert!(enemy_power(&t, &clock, u32::MAX).is_finite());
    }
    #[test]
    fn the_opening_is_survivable_and_the_grace_runs_out() {
        // The first ninety seconds are the only stretch with no tools at all.
        assert!(
            opening_grace(0.0) < 0.5,
            "the opening hits at full strength"
        );
        assert!(opening_grace(45.0) > opening_grace(0.0));
        assert!((opening_grace(90.0) - 1.0).abs() < 1e-6);
        assert!(
            (opening_grace(600.0) - 1.0).abs() < 1e-6,
            "grace never ends"
        );
    }

    #[test]
    fn the_grace_ramps_without_a_step() {
        // A visible gear change would read as a bug.
        let mut last = opening_grace(0.0);
        for i in 1..=180 {
            let now = opening_grace(i as f32);
            assert!(now >= last, "grace went backwards at {i}s");
            assert!(now - last < 0.02, "a step at {i}s");
            last = now;
        }
    }

    #[test]
    fn the_opening_is_easier_than_the_same_moment_later() {
        let t = Threat::default();
        let early = RunClock::default();
        let later = RunClock {
            elapsed: 120.0,
            ..RunClock::default()
        };
        assert!(enemy_power(&t, &early, 1) < enemy_power(&t, &later, 1));
    }
}
