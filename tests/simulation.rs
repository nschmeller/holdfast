//! Integration tests that drive real Bevy schedules.
//!
//! These build a headless `App` - no window, no renderer - and step it, so the
//! pacing systems are exercised through the same `Time` plumbing the game uses
//! rather than by calling their bodies directly.

use std::time::Duration;

use bevy::prelude::*;

use holdfast::threat::{Phase, RunClock, Threat, WaveCycle, tick_threat, tick_waves};

/// A minimal app with just a clock and the pacing systems.
///
/// `TimePlugin` is deliberately absent: it drives the clock from the wall
/// clock, which would overwrite the deterministic steps these tests take.
fn pacing_app() -> App {
    let mut app = App::new();
    app.init_resource::<Time>()
        .init_resource::<Threat>()
        .init_resource::<RunClock>()
        .init_resource::<WaveCycle>()
        .add_systems(Update, (tick_threat, tick_waves).chain());
    app
}

/// Step the app forward by `seconds` in fixed slices, so the systems see a
/// plausible frame cadence rather than one enormous delta.
fn advance(app: &mut App, seconds: f32) {
    const STEP: f32 = 1.0 / 60.0;
    let frames = (seconds / STEP).round() as u32;
    let step = Duration::from_secs_f32(STEP);
    for _ in 0..frames {
        app.world_mut().resource_mut::<Time>().advance_by(step);
        app.update();
    }
}

#[test]
fn the_run_clock_tracks_elapsed_time() {
    let mut app = pacing_app();
    advance(&mut app, 10.0);
    let elapsed = app.world().resource::<RunClock>().elapsed;
    assert!(
        (elapsed - 10.0).abs() < 0.5,
        "clock read {elapsed} after ten seconds"
    );
}

#[test]
fn the_threat_floor_climbs_on_its_own() {
    let mut app = pacing_app();
    let start = app.world().resource::<Threat>().floor;
    advance(&mut app, 120.0);
    let later = app.world().resource::<Threat>().floor;
    assert!(
        later > start,
        "floor stayed at {start}; turtling would be viable forever"
    );
}

#[test]
fn threat_chases_the_players_intent() {
    let mut app = pacing_app();
    app.world_mut().resource_mut::<Threat>().intent = 5.0;
    advance(&mut app, 20.0);
    let level = app.world().resource::<Threat>().level;
    assert!((level - 5.0).abs() < 0.3, "level settled at {level}");
}

#[test]
fn threat_rises_faster_than_it_falls() {
    // Escalation should be easy and de-escalation a commitment.
    //
    // The window is short on purpose: given enough time both directions reach
    // the target and the asymmetry disappears. What matters is the slope.
    const WINDOW: f32 = 0.3;

    let mut up = pacing_app();
    up.world_mut().resource_mut::<Threat>().intent = 4.0;
    advance(&mut up, WINDOW);
    let gained = up.world().resource::<Threat>().level - 1.0;

    let mut down = pacing_app();
    {
        let mut t = down.world_mut().resource_mut::<Threat>();
        t.level = 4.0;
        t.intent = 1.0;
    }
    advance(&mut down, WINDOW);
    let lost = 4.0 - down.world().resource::<Threat>().level;

    assert!(gained > 0.0 && lost > 0.0, "nothing moved at all");
    assert!(gained > lost * 1.5, "gained {gained} but lost {lost}");
}

#[test]
fn a_surge_expires_and_leaves_a_cooldown() {
    let mut app = pacing_app();
    app.world_mut().resource_mut::<Threat>().start_surge();
    assert!(app.world().resource::<Threat>().surging());

    advance(&mut app, holdfast::threat::SURGE_DURATION + 2.0);
    let t = app.world().resource::<Threat>();
    assert!(!t.surging(), "the surge never ended");
    assert!(!t.can_surge(), "it should still be on cooldown");
}

#[test]
fn kill_pressure_bleeds_off_when_the_player_stops_fighting() {
    let mut app = pacing_app();
    {
        let mut t = app.world_mut().resource_mut::<Threat>();
        for _ in 0..100 {
            t.note_kill();
        }
    }
    let peak = app.world().resource::<Threat>().streak;
    assert!(peak > 0.0);
    advance(&mut app, 20.0);
    assert_eq!(
        app.world().resource::<Threat>().streak,
        0.0,
        "streak from {peak} should decay to nothing"
    );
}

#[test]
fn the_wave_cycle_alternates_prep_and_assault() {
    let mut app = pacing_app();
    assert!(app.world().resource::<WaveCycle>().in_prep());

    // Long enough to cover the opening prep window and the first assault.
    advance(&mut app, 45.0);
    assert_eq!(
        app.world().resource::<WaveCycle>().phase,
        Phase::Assault,
        "the first wave never arrived"
    );
    assert_eq!(app.world().resource::<WaveCycle>().wave, 1);

    advance(&mut app, 40.0);
    assert!(
        app.world().resource::<WaveCycle>().in_prep(),
        "the assault never ended"
    );
}

#[test]
fn waves_keep_coming_and_the_budget_grows() {
    let mut app = pacing_app();
    advance(&mut app, 45.0);
    let first = app.world().resource::<WaveCycle>().budget;

    // Several full cycles later the director should have more to spend.
    advance(&mut app, 300.0);
    let cycle = app.world().resource::<WaveCycle>();
    assert!(cycle.wave >= 4, "only reached wave {}", cycle.wave);
    assert!(
        cycle.budget > first,
        "budget went from {first} to {}",
        cycle.budget
    );
}

#[test]
fn calling_a_wave_early_shortens_the_prep_and_pays_a_bonus() {
    let mut app = pacing_app();
    advance(&mut app, 1.0);

    let bonus = {
        let mut cycle = app.world_mut().resource_mut::<WaveCycle>();
        let pending = cycle.pending_bonus();
        cycle.call_early();
        pending
    };
    assert!(
        bonus > 0.5,
        "a near-full window should pay well, got {bonus}"
    );

    // The very next tick should flip into the assault.
    advance(&mut app, 0.2);
    let cycle = app.world().resource::<WaveCycle>();
    assert_eq!(cycle.phase, Phase::Assault);
    assert!((cycle.reward_mult() - (1.0 + bonus)).abs() < 1e-4);
}

#[test]
fn the_early_bonus_is_cleared_when_the_assault_ends() {
    let mut app = pacing_app();
    advance(&mut app, 1.0);
    app.world_mut().resource_mut::<WaveCycle>().call_early();
    advance(&mut app, 0.2);
    assert!(app.world().resource::<WaveCycle>().early_bonus > 0.0);

    advance(&mut app, 40.0);
    let cycle = app.world().resource::<WaveCycle>();
    assert!(cycle.in_prep());
    assert_eq!(cycle.early_bonus, 0.0, "the bonus must not carry over");
}

#[test]
fn prep_windows_shrink_as_the_run_goes_on() {
    let mut app = pacing_app();
    advance(&mut app, 45.0);
    advance(&mut app, 40.0);
    let early = app.world().resource::<WaveCycle>().prep_length;

    advance(&mut app, 600.0);
    let late = app.world().resource::<WaveCycle>().prep_length;
    assert!(late < early, "prep stayed at {early}");
    assert!(late >= 12.0, "prep collapsed to {late}");
}

#[test]
fn enemy_power_compounds_over_a_long_run() {
    let mut app = pacing_app();
    app.world_mut().resource_mut::<Threat>().intent = 3.0;

    advance(&mut app, 60.0);
    let early = {
        let t = app.world().resource::<Threat>();
        let c = app.world().resource::<RunClock>();
        holdfast::threat::enemy_power(t, c)
    };

    advance(&mut app, 600.0);
    let late = {
        let t = app.world().resource::<Threat>();
        let c = app.world().resource::<RunClock>();
        holdfast::threat::enemy_power(t, c)
    };

    assert!(
        late > early * 2.0,
        "power only went from {early} to {late}; the curve is flat"
    );
}

#[test]
fn a_long_run_never_produces_a_non_finite_value() {
    // An endless game must not drift into NaN or infinity an hour in.
    let mut app = pacing_app();
    app.world_mut().resource_mut::<Threat>().intent = 8.0;
    advance(&mut app, 3600.0);

    let threat = app.world().resource::<Threat>();
    let clock = app.world().resource::<RunClock>();
    let cycle = app.world().resource::<WaveCycle>();

    assert!(threat.effective().is_finite());
    assert!(threat.reward_mult().is_finite());
    assert!(clock.time_power().is_finite());
    assert!(cycle.budget.is_finite());
    assert!(holdfast::threat::enemy_power(threat, clock).is_finite());
    assert!(threat.level <= holdfast::threat::MAX_INTENT + 3.0);
}
