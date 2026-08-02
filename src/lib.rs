//! HOLDFAST - five small worlds, one rule: hold your ground.
//!
//! A keyboard-only 3D survival command game. You hold ground against an endless
//! escalation, and you set the pace yourself. See `DESIGN.md` for the pillars.

pub mod allies;
pub mod arena;
pub mod art;
pub mod audio;
pub mod camera;
pub mod combat;
pub mod command;
pub mod common;
pub mod devtools;
pub mod enemy;
pub mod environments;
pub mod fx;
pub mod hud;
pub mod meshgen;
pub mod models;
pub mod onboarding;
pub mod palette;
pub mod pickups;
pub mod player;
pub mod progress;
pub mod rng;
pub mod screens;
pub mod threat;
pub mod weapons;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};

/// Top-level screen. Gameplay systems only run in `Playing`, so every overlay
/// pauses the world simply by being a different state - there is no separate
/// paused flag to keep in sync.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Menu,
    Playing,
    LevelUp,
    SkillTree,
    Paused,
    GameOver,
}

impl AppState {
    /// True for states where a run exists and should stay rendered.
    pub fn run_alive(self) -> bool {
        !matches!(self, Self::Menu)
    }
}

/// Ordered phases of starting a run, all running on `OnExit(Menu)`.
///
/// Without this the clear pass races the spawn pass and can delete the hero it
/// was supposed to make room for - which is exactly what happened the first
/// time the game ran.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RunSetup {
    /// Despawn everything left over from the previous run.
    Clear,
    /// Return resources to their starting values.
    Reset,
    /// Create the hero and the arena.
    Spawn,
}

/// Ordered phases within a gameplay frame.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameSet {
    /// Read the keyboard, refresh the broad phase.
    Input,
    /// Decide where things want to go.
    Think,
    /// Integrate motion and resolve collisions.
    Move,
    /// Fire weapons, apply damage.
    Combat,
    /// Deaths, drops, economy.
    Resolve,
    /// Cameras, particles, HUD.
    Present,
    /// Despawn everything marked `Doomed`.
    ///
    /// Strictly last. Several systems can condemn the same entity in one frame
    /// - a projectile, a hazard tick and a fall off the edge all racing - and
    /// reaping mid-frame would leave the others writing to a dead entity.
    Reap,
}

/// Build and run the game.
///
/// Lives in the library rather than the binary so that integration tests
/// and the iOS static library can both drive the same app definition.
pub fn run() {
    let mut app = App::new();

    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "HOLDFAST".into(),
                    resolution: WindowResolution::new(1280, 720),
                    present_mode: PresentMode::AutoVsync,
                    // Stop the browser from stealing the arrow keys and space
                    // bar, both of which are core controls.
                    prevent_default_event_handling: true,
                    fit_canvas_to_parent: true,
                    canvas: Some("#game-canvas".into()),
                    ..default()
                }),
                ..default()
            })
            .set(ImagePlugin::default_nearest()),
    );

    app.init_state::<AppState>()
        .init_resource::<rng::Rng>()
        .init_resource::<threat::Threat>()
        .init_resource::<threat::RunClock>()
        .init_resource::<threat::WaveCycle>()
        .insert_resource(ClearColor(Color::srgb(0.016, 0.018, 0.028)))
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.42, 0.5, 0.78),
            brightness: 260.0,
            ..default()
        })
        // Bigger shadow map: the arenas are small and prop shadows are most of
        // what sells the third-person overlook as a real space.
        .insert_resource(bevy::light::DirectionalLightShadowMap { size: 2048 });

    app.configure_sets(
        OnExit(AppState::Menu),
        (RunSetup::Clear, RunSetup::Reset, RunSetup::Spawn).chain(),
    );

    app.configure_sets(
        Update,
        (
            GameSet::Input,
            GameSet::Think,
            GameSet::Move,
            GameSet::Combat,
            GameSet::Resolve,
            GameSet::Present,
            GameSet::Reap,
        )
            .chain()
            .run_if(in_state(AppState::Playing)),
    );

    app.add_systems(
        Update,
        (threat::tick_threat, threat::tick_waves)
            .chain()
            .in_set(GameSet::Input),
    );

    // Split in two: `Plugins` is only implemented for tuples up to 15 wide.
    app.add_plugins((
        art::ArtPlugin,
        audio::AudioFxPlugin,
        camera::CameraPlugin,
        environments::ArenaPlugin,
        player::PlayerPlugin,
        enemy::EnemyPlugin,
        weapons::WeaponPlugin,
        combat::CombatPlugin,
    ));
    app.add_plugins((
        allies::AlliesPlugin,
        pickups::PickupPlugin,
        progress::ProgressPlugin,
        command::CommandPlugin,
        onboarding::OnboardingPlugin,
        fx::FxPlugin,
        hud::HudPlugin,
        screens::ScreensPlugin,
        devtools::DevToolsPlugin,
    ));

    app.run();
}
