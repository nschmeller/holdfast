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
//! | `DFFA_MONITOR_NAME=DELL` | Pick the monitor by name instead of by index |
//! | `DFFA_TILE=0:2` | Take slot 0 of 2 side-by-side slots on that monitor |
//! | `DFFA_RES=960x600` | Override the window size |
//!
//! See `pilot` for the other half of the harness: a live command channel that
//! lets an outside process play the game rather than merely observe it.

use std::env;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{Monitor, MonitorSelection, PrimaryMonitor, WindowPosition, WindowResolution};

use crate::AppState;
use crate::environments::EnvKind;

#[derive(Debug, Resource, Default, Clone)]
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
    /// `(slot, of)` - which horizontal slice of the monitor to occupy, so
    /// several instances can be watched at once.
    pub tile: Option<(u32, u32)>,
    pub resolution: Option<(u32, u32)>,
    /// Case-insensitive fragment of the monitor's name, which survives
    /// replugging in a way the index does not.
    pub monitor_name: Option<String>,
}

impl DevConfig {
    fn from_env() -> Self {
        let arena =
            env::var("DFFA_ARENA")
                .ok()
                .and_then(|v| match v.to_ascii_lowercase().as_str() {
                    "desk" => Some(EnvKind::Desk),
                    "forest" => Some(EnvKind::Forest),
                    "rooftop" | "urban" => Some(EnvKind::Rooftop),
                    "grid" | "future" => Some(EnvKind::Grid),
                    "arcane" | "magic" => Some(EnvKind::Arcane),
                    other => {
                        warn!("DFFA_ARENA: unknown arena {other:?}");
                        None
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
            tile: env::var("DFFA_TILE").ok().and_then(|v| pair(&v, ':')),
            resolution: env::var("DFFA_RES").ok().and_then(|v| pair(&v, 'x')),
            monitor_name: env::var("DFFA_MONITOR_NAME").ok().filter(|v| !v.is_empty()),
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
            || self.tile.is_some()
            || self.resolution.is_some()
            || self.monitor_name.is_some()
    }
}

fn truthy(key: &str) -> bool {
    env::var(key).is_ok_and(|v| matches!(v.as_str(), "1" | "true" | "yes" | "on"))
}

/// Parse `a<sep>b` into a pair of positive integers.
fn pair(value: &str, sep: char) -> Option<(u32, u32)> {
    let (a, b) = value.split_once(sep)?;
    let (a, b) = (a.trim().parse().ok()?, b.trim().parse().ok()?);
    (b > 0).then_some((a, b))
}

#[derive(Resource)]
struct DevTimers {
    elapsed: f32,
    shot_taken: bool,
}

#[derive(Debug)]
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
            .insert_resource(WindowPlaced(false))
            // Startup, not PreStartup: the primary window has to exist first.
            .add_systems(Startup, place_window)
            .add_systems(Update, (tile_window, force_unlocks, tick_dev))
            .add_systems(Update, autopick_card.run_if(in_state(AppState::LevelUp)));
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
    for mut window in &mut windows {
        if let Some((width, height)) = config.resolution {
            window.resolution = WindowResolution::new(width.max(320), height.max(240));
        }
        if let Some(index) = config.monitor
            && config.tile.is_none()
        {
            window.position = WindowPosition::Centered(MonitorSelection::Index(index));
        }
    }
}

/// Set once the window has been tiled, so the placement is not fought over
/// every frame - the user should be able to drag the window afterwards.
#[derive(Resource)]
struct WindowPlaced(bool);

/// Lay the window into a horizontal slice of the chosen monitor.
///
/// This cannot run at `Startup`: monitor entities are created by the windowing
/// backend during the first frames, so the system retries until one shows up.
fn tile_window(
    config: Res<DevConfig>,
    mut placed: ResMut<WindowPlaced>,
    monitors: Query<(Entity, &Monitor, Option<&PrimaryMonitor>)>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
) {
    const SIDE: i32 = 32;
    /// Share of the monitor's height a tiled window may use.
    ///
    /// The remainder becomes slack above and below, because the window is
    /// positioned by its content area but drawn with a title bar above it, a
    /// desktop can reserve a menu bar and a dock, and the coordinate space the
    /// backend applies is not always the one asked for. Filling the screen
    /// exactly puts the bottom of the game off the bottom of the display; the
    /// window is centred vertically instead, so a placement that lands a
    /// hundred pixels out is still entirely on screen.
    const HEIGHT_SHARE: f32 = 0.62;

    let Some((slot, of)) = config.tile else {
        return;
    };
    if placed.0 || monitors.is_empty() {
        return;
    }

    // Sort by entity id to recover the order the backend enumerated them in.
    // Raw query order will not do: `PrimaryMonitor` puts one of them in a
    // different archetype, which floats it to the front or the back and makes
    // "monitor 1" mean the wrong screen.
    let mut all: Vec<(Entity, &Monitor, bool)> = monitors
        .iter()
        .map(|(entity, monitor, primary)| (entity, monitor, primary.is_some()))
        .collect();
    all.sort_unstable_by_key(|&(entity, ..)| entity);
    for (index, (_, monitor, primary)) in all.iter().enumerate() {
        info!(
            "devtools: monitor {index}{} {:?} {}x{} at {} scale {}",
            if *primary { " (primary)" } else { "" },
            monitor.name,
            monitor.physical_width,
            monitor.physical_height,
            monitor.physical_position,
            monitor.scale_factor
        );
    }

    // A name fragment beats an index: indices shuffle when a display is
    // plugged in, names do not.
    let by_name = config.monitor_name.as_ref().and_then(|wanted| {
        let wanted = wanted.to_ascii_lowercase();
        all.iter().find(|(_, monitor, _)| {
            monitor
                .name
                .as_ref()
                .is_some_and(|n| n.to_ascii_lowercase().contains(&wanted))
        })
    });
    let chosen = by_name
        .or_else(|| all.get(config.monitor.unwrap_or(0)))
        .or_else(|| all.first());
    let Some((_, monitor, _)) = chosen else {
        return;
    };
    let monitor = *monitor;

    let slot = i32::try_from(slot.min(of - 1)).unwrap_or(0);
    let of = i32::try_from(of).unwrap_or(1).max(1);
    let screen_w = i32::try_from(monitor.physical_width).unwrap_or(1920);
    let screen_h = i32::try_from(monitor.physical_height).unwrap_or(1080);

    let column = (screen_w - SIDE * 2) / of;
    let width = (column - SIDE).max(320);
    // Cap at 9:16 of the width too, so a wide slot does not become a letterbox
    // the fixed overlook camera looks silly in.
    let budget = (screen_h as f32 * HEIGHT_SHARE) as i32;
    let height = budget.min(width * 9 / 16).max(240);
    let top = monitor.physical_position.y + (screen_h - height) / 2;

    for mut window in &mut windows {
        window.resolution = WindowResolution::new(
            u32::try_from(width).unwrap_or(640),
            u32::try_from(height).unwrap_or(480),
        );
        window.position = WindowPosition::At(IVec2::new(
            monitor.physical_position.x + SIDE + column * slot,
            top,
        ));
    }
    placed.0 = true;
    info!(
        "devtools: tiled into slot {slot} of {of}: {width}x{height} at ({}, {top}) on a {screen_w}x{screen_h} monitor",
        monitor.physical_position.x + SIDE + column * slot
    );
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

    if let Some((path, at)) = &config.shot
        && !timers.shot_taken
        && timers.elapsed >= *at
    {
        timers.shot_taken = true;
        info!("devtools: capturing {path}");
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path.clone()));
    }

    if let Some(at) = config.exit_after
        && timers.elapsed >= at
    {
        info!("devtools: exiting after {at}s");
        exit.write(AppExit::Success);
    }
}
