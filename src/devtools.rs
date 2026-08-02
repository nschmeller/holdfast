//! Environment-variable driven development harness.
//!
//! Entirely inert unless one of the `DFFA_*` variables is set, so it costs a
//! branch at startup and nothing else. It exists because a game whose whole
//! surface is "press keys and look at it" is otherwise impossible to verify
//! without a human at the keyboard.
//!
//! | Variable | Effect |
//! | --- | --- |
//! | `DFFA_ARENA=forest` | Start in a named arena |
//! | `DFFA_AUTOSTART=1` | Skip the menu |
//! | `DFFA_SHOT=out.png@12` | Screenshot after 12 seconds |
//! | `DFFA_EXIT=20` | Quit after 20 seconds |
//! | `DFFA_SPEED=4` | Run the simulation at 4x |
//! | `DFFA_UNLOCK=1` | Turn on every subsystem immediately |
//! | `DFFA_AUTOPICK=1` | Auto-choose upgrade cards so a run never stalls |
//! | `DFFA_MONITOR=1` | Open the window centred on monitor 1 |

use std::env;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{MonitorSelection, WindowPosition};

use crate::environments::EnvKind;
use crate::AppState;

#[derive(Resource, Default, Clone)]
pub struct DevConfig {
    pub arena: Option<EnvKind>,
    pub autostart: bool,
    pub shot: Option<(String, f32)>,
    pub exit_after: Option<f32>,
    pub speed: Option<f32>,
    pub unlock_all: bool,
    pub autopick: bool,
    /// Monitor index to centre the window on. Useful when the game needs to be
    /// watched on one screen while a terminal stays visible on another.
    pub monitor: Option<usize>,
}

impl DevConfig {
    fn from_env() -> Self {
        let arena = env::var("DFFA_ARENA").ok().and_then(|v| {
            match v.to_ascii_lowercase().as_str() {
                "desk" => Some(EnvKind::Desk),
                "forest" => Some(EnvKind::Forest),
                "rooftop" | "urban" => Some(EnvKind::Rooftop),
                "grid" | "future" => Some(EnvKind::Grid),
                "arcane" | "magic" => Some(EnvKind::Arcane),
                other => {
                    warn!("DFFA_ARENA: unknown arena {other:?}");
                    None
                }
            }
        });

        // `name.png@seconds`, with the delay optional.
        let shot = env::var("DFFA_SHOT").ok().map(|v| {
            v.split_once('@').map_or_else(
                || (v.clone(), 6.0),
                |(path, secs)| (path.to_string(), secs.parse().unwrap_or(6.0)),
            )
        });

        Self {
            arena,
            autostart: truthy("DFFA_AUTOSTART"),
            shot,
            exit_after: env::var("DFFA_EXIT").ok().and_then(|v| v.parse().ok()),
            speed: env::var("DFFA_SPEED").ok().and_then(|v| v.parse().ok()),
            unlock_all: truthy("DFFA_UNLOCK"),
            autopick: truthy("DFFA_AUTOPICK"),
            monitor: env::var("DFFA_MONITOR").ok().and_then(|v| v.parse().ok()),
        }
    }

    fn any(&self) -> bool {
        self.arena.is_some()
            || self.autostart
            || self.shot.is_some()
            || self.exit_after.is_some()
            || self.speed.is_some()
            || self.unlock_all
            || self.autopick
            || self.monitor.is_some()
    }
}

fn truthy(key: &str) -> bool {
    env::var(key).is_ok_and(|v| matches!(v.as_str(), "1" | "true" | "yes" | "on"))
}

#[derive(Resource)]
struct DevTimers {
    elapsed: f32,
    shot_taken: bool,
}

pub struct DevToolsPlugin;

impl Plugin for DevToolsPlugin {
    fn build(&self, app: &mut App) {
        let config = DevConfig::from_env();
        if !config.any() {
            return;
        }
        info!("devtools active: {config:?}", config = DevSummary(&config));

        app.insert_resource(config)
            .insert_resource(DevTimers {
                elapsed: 0.0,
                shot_taken: false,
            })
            // PreStartup, because the initial `OnEnter(Menu)` transition -
            // which builds the world selector - runs before `Startup`.
            .add_systems(PreStartup, apply_startup_config)
            // Startup, not PreStartup: the primary window has to exist first.
            .add_systems(Startup, place_window)
            .add_systems(Update, (force_unlocks, tick_dev))
            .add_systems(
                Update,
                autopick_card.run_if(in_state(AppState::LevelUp)),
            );
    }
}

/// `DevConfig` is intentionally not `Debug` (it would leak into release logs);
/// this renders just enough for the startup line.
struct DevSummary<'a>(&'a DevConfig);

impl std::fmt::Debug for DevSummary<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "arena={:?} autostart={} shot={:?} exit={:?} speed={:?} unlock={}",
            self.0.arena,
            self.0.autostart,
            self.0.shot,
            self.0.exit_after,
            self.0.speed,
            self.0.unlock_all
        )
    }
}

fn apply_startup_config(
    config: Res<DevConfig>,
    mut env: ResMut<EnvKind>,
    mut next: ResMut<NextState<AppState>>,
    mut sim_speed: ResMut<crate::command::SimSpeed>,
) {
    if let Some(arena) = config.arena {
        *env = arena;
    }
    if let Some(speed) = config.speed {
        // Set the baseline rather than the clock directly: plan mode
        // multiplies against this, so the override survives.
        sim_speed.0 = speed.clamp(0.05, 20.0);
    }
    if config.autostart {
        next.set(AppState::Playing);
    }
}

fn place_window(
    config: Res<DevConfig>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
) {
    let Some(index) = config.monitor else { return };
    for mut window in &mut windows {
        window.position = WindowPosition::Centered(MonitorSelection::Index(index));
    }
}

/// Skip the onboarding schedule so late-game systems can be exercised without
/// waiting five minutes for them to unlock.
fn force_unlocks(
    config: Res<DevConfig>,
    mut unlocks: ResMut<crate::onboarding::Unlocks>,
    mut economy: ResMut<crate::allies::Economy>,
) {
    if !config.unlock_all || unlocks.build {
        return;
    }
    unlocks.build = true;
    unlocks.territory = true;
    unlocks.allies = true;
    unlocks.research = true;
    unlocks.threat_dial = true;
    economy.gain_scrap(400.0);
    economy.gain_cores(30.0);
}

/// Take the first card on offer and resume. Without this a scripted run stalls
/// on the first level-up, because nothing is there to press a number key.
#[allow(clippy::too_many_arguments)]
fn autopick_card(
    config: Res<DevConfig>,
    mut offer: ResMut<crate::progress::CardOffer>,
    mut progression: ResMut<crate::progress::Progression>,
    mut stats: ResMut<crate::player::PlayerStats>,
    mut loadout: ResMut<crate::weapons::Loadout>,
    mut economy: ResMut<crate::allies::Economy>,
    mut boosts: ResMut<crate::progress::AppliedBoosts>,
    mut next: ResMut<NextState<AppState>>,
    mut recompute: MessageWriter<crate::progress::RecomputeStats>,
) {
    if !config.autopick {
        return;
    }
    let Some(card) = offer.cards.first().cloned() else {
        return;
    };
    crate::progress::apply_card(&card, &mut stats, &mut loadout, &mut economy, &mut boosts);
    recompute.write(crate::progress::RecomputeStats);
    progression.pending_levels = progression.pending_levels.saturating_sub(1);
    offer.cards.clear();
    next.set(AppState::Playing);
}

fn tick_dev(
    time: Res<Time<Real>>,
    config: Res<DevConfig>,
    mut timers: ResMut<DevTimers>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    timers.elapsed += time.delta_secs();

    if let Some((path, at)) = &config.shot {
        if !timers.shot_taken && timers.elapsed >= *at {
            timers.shot_taken = true;
            info!("devtools: capturing {path}");
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path.clone()));
        }
    }

    if let Some(at) = config.exit_after {
        if timers.elapsed >= at {
            info!("devtools: exiting after {at}s");
            exit.write(AppExit::Success);
        }
    }
}
