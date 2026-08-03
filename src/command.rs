//! PLAN mode and every command-layer keybinding.
//!
//! This is the anti-reflex valve. `Space` drops the simulation to a crawl and
//! hands the arrow keys to a build cursor. There is no timer on it and no cost
//! to using it - the game should never punish thinking, only thinking wrong.

use bevy::prelude::*;

use crate::allies::{
    AllyKind, BuildRequest, Economy, RecruitRequest, Squad, Stance, TurretKind, Zone, ZoneOwner,
};
use crate::arena::ObstacleField;
use crate::art::{GameArt, Glow};
use crate::common::{Body, RunEntity, SfxEvent, to_world};
use crate::environments::EnvKind;
use crate::onboarding::{HintQueue, HintTone, Unlocks};
use crate::player::Player;
use crate::threat::{Threat, WaveCycle};
use crate::{AppState, GameSet, RunSetup};

/// Time scale while planning. Not zero: a little motion keeps the board legible
/// and stops the transition feeling like a freeze-frame bug.
const PLAN_TIME_SCALE: f32 = 0.12;
const CURSOR_SPEED: f32 = 17.0;

/// Furthest the build cursor can stray from the player.
///
/// The world is unbounded, so something has to stop the cursor wandering off
/// into unexplored ground. A leash is also the better rule: you place defences
/// around where you are standing, not anywhere on the map.
const CURSOR_LEASH: f32 = 26.0;

#[derive(Debug, Resource)]
pub struct PlanMode {
    pub active: bool,
    pub cursor: Vec2,
    /// Which structure the number keys will place.
    pub selected: usize,
    /// Whether the current cursor position is a legal build site.
    pub valid: bool,
    pub message: Option<(String, f32)>,
}

impl Default for PlanMode {
    fn default() -> Self {
        Self {
            active: false,
            cursor: Vec2::ZERO,
            selected: 0,
            valid: true,
            message: None,
        }
    }
}

impl PlanMode {
    pub fn selected_kind(&self) -> TurretKind {
        TurretKind::ALL[self.selected.min(TurretKind::ALL.len() - 1)]
    }

    fn say(&mut self, text: impl Into<String>) {
        self.message = Some((text.into(), 2.0));
    }
}

/// The translucent preview of what is about to be built.
#[derive(Debug, Component)]
pub struct BuildGhost;

#[derive(Debug)]
pub struct CommandPlugin;

impl Plugin for CommandPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlanMode>()
            .init_resource::<SimSpeed>()
            .add_systems(
                Update,
                (
                    toggle_plan_mode,
                    drive_time_scale,
                    plan_cursor,
                    plan_actions,
                    threat_input,
                    squad_input,
                )
                    .chain()
                    .in_set(GameSet::Input),
            )
            .add_systems(Update, update_ghost.in_set(GameSet::Present))
            .add_systems(OnExit(AppState::Menu), reset_plan.in_set(RunSetup::Reset));
    }
}

fn reset_plan(mut plan: ResMut<PlanMode>, base: Res<SimSpeed>, mut time: ResMut<Time<Virtual>>) {
    *plan = PlanMode::default();
    time.set_relative_speed(base.0);
}

fn toggle_plan_mode(
    keys: Res<ButtonInput<KeyCode>>,
    mut plan: ResMut<PlanMode>,
    mut hints: ResMut<HintQueue>,
    player: Query<&Body, With<Player>>,
) {
    // B as well as Space: the HUD advertises "B build", and a key the
    // interface promises has to exist.
    if !keys.just_pressed(KeyCode::Space) && !keys.just_pressed(KeyCode::KeyB) {
        return;
    }
    plan.active = !plan.active;
    if plan.active {
        // Start the cursor on the player so the common case - build right
        // here - is zero keystrokes of aiming.
        plan.cursor = player.iter().next().map_or(Vec2::ZERO, |b| b.pos);
        hints.push_once(
            "plan-keys",
            "PLANNING",
            "Arrows move the cursor. 1-5 pick a structure. Enter builds. Space resumes.",
            HintTone::Tip,
        );
    }
}

/// The baseline simulation rate that plan mode multiplies against. Normally
/// 1.0; the dev harness overrides it to fast-forward a run.
#[derive(Debug, Resource)]
pub struct SimSpeed(pub f32);

impl Default for SimSpeed {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Plan mode slows the *virtual* clock, which is what every gameplay system
/// reads. The camera and UI use real time, so they stay responsive.
fn drive_time_scale(plan: Res<PlanMode>, base: Res<SimSpeed>, mut time: ResMut<Time<Virtual>>) {
    let want = base.0 * if plan.active { PLAN_TIME_SCALE } else { 1.0 };
    if (time.relative_speed() - want).abs() > 1e-3 {
        time.set_relative_speed(want);
    }
}

fn plan_cursor(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    obstacles: Res<ObstacleField>,
    mut plan: ResMut<PlanMode>,
    player: Query<&Body, With<Player>>,
) {
    if !plan.active {
        return;
    }
    let dt = time.delta_secs();
    let anchor = player.iter().next().map_or(Vec2::ZERO, |b| b.pos);

    let mut dir = Vec2::ZERO;
    if keys.pressed(KeyCode::ArrowUp) {
        dir.y -= 1.0;
    }
    if keys.pressed(KeyCode::ArrowDown) {
        dir.y += 1.0;
    }
    if keys.pressed(KeyCode::ArrowLeft) {
        dir.x -= 1.0;
    }
    if keys.pressed(KeyCode::ArrowRight) {
        dir.x += 1.0;
    }

    let moved = plan.cursor + dir.normalize_or_zero() * CURSOR_SPEED * dt;
    // The cursor is leashed to the player rather than to an arena: you plan
    // where you are standing, not anywhere on an unbounded map.
    let leash = moved - anchor;
    plan.cursor = if leash.length() > CURSOR_LEASH {
        anchor + leash.normalize() * CURSOR_LEASH
    } else {
        moved
    };

    let kind = plan.selected_kind();
    let radius = if kind == TurretKind::Barricade {
        1.1
    } else {
        0.8
    };
    plan.valid = !obstacles.overlaps(plan.cursor, radius);

    if let Some((_, t)) = &mut plan.message {
        *t -= dt;
        if *t <= 0.0 {
            plan.message = None;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn plan_actions(
    keys: Res<ButtonInput<KeyCode>>,
    env: Res<EnvKind>,
    unlocks: Res<Unlocks>,
    economy: Res<Economy>,
    stats: Res<crate::player::PlayerStats>,
    mut plan: ResMut<PlanMode>,
    mut builds: MessageWriter<BuildRequest>,
    mut sfx: MessageWriter<SfxEvent>,
) {
    if !plan.active {
        return;
    }

    // Structure selection.
    for (i, key) in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
    ]
    .into_iter()
    .enumerate()
    {
        if keys.just_pressed(key) && i < TurretKind::ALL.len() {
            plan.selected = i;
            let kind = TurretKind::ALL[i];
            plan.say(format!(
                "{} - {} scrap",
                kind.name(*env),
                (kind.scrap_cost() * (1.0 - stats.build_discount)) as u32
            ));
            sfx.write(SfxEvent::at(crate::audio::Sfx::Tick, 0.5));
        }
    }

    if !keys.just_pressed(KeyCode::Enter) && !keys.just_pressed(KeyCode::NumpadEnter) {
        return;
    }

    if !unlocks.build {
        plan.say("Salvage comes online shortly.");
        return;
    }

    let kind = plan.selected_kind();
    let cost = kind.scrap_cost() * (1.0 - stats.build_discount);

    if !plan.valid {
        plan.say("Blocked - move the cursor.");
        sfx.write(SfxEvent::at(crate::audio::Sfx::Deny, 0.6));
        return;
    }
    if !economy.can_afford_scrap(cost) {
        plan.say(format!("Need {} scrap.", cost as u32));
        sfx.write(SfxEvent::at(crate::audio::Sfx::Deny, 0.6));
        return;
    }

    builds.write(BuildRequest {
        kind,
        pos: plan.cursor,
    });
}

/// The pacing dial, plus calling the wave in early.
#[allow(clippy::too_many_arguments)]
fn threat_input(
    keys: Res<ButtonInput<KeyCode>>,
    unlocks: Res<Unlocks>,
    plan: Res<PlanMode>,
    mut threat: ResMut<Threat>,
    mut cycle: ResMut<WaveCycle>,
    mut hints: ResMut<HintQueue>,
    mut sfx: MessageWriter<SfxEvent>,
) {
    // The dial stays automatic until it is unlocked, ramping on its own.
    if unlocks.threat_dial {
        if keys.just_pressed(KeyCode::Minus) || keys.just_pressed(KeyCode::NumpadSubtract) {
            threat.lower();
            sfx.write(SfxEvent::at(crate::audio::Sfx::Tick, 0.6));
        }
        if keys.just_pressed(KeyCode::Equal) || keys.just_pressed(KeyCode::NumpadAdd) {
            threat.raise();
            sfx.write(SfxEvent::at(crate::audio::Sfx::Tick, 0.8));
        }
        if keys.just_pressed(KeyCode::KeyO) {
            if threat.can_surge() {
                threat.start_surge();
                hints.push(
                    "OVERCLOCK",
                    "Threat spiked. Rewards up 60%. Survive it.",
                    HintTone::Tip,
                );
                sfx.write(SfxEvent::new(crate::audio::Sfx::Surge));
            } else {
                sfx.write(SfxEvent::at(crate::audio::Sfx::Deny, 0.5));
            }
        }
    } else {
        // Gentle automatic ramp during the tutorial window.
        threat.intent = threat.floor.max(1.0);
    }

    // Enter calls the wave early - but only outside plan mode, where Enter
    // means "build".
    if !plan.active
        && (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter))
        && cycle.in_prep()
    {
        let bonus = cycle.pending_bonus();
        if bonus > 0.01 {
            cycle.call_early();
            hints.push(
                format!("WAVE CALLED  +{}%", (bonus * 100.0) as u32),
                "Rewards boosted for this wave.",
                HintTone::Tip,
            );
            sfx.write(SfxEvent::new(crate::audio::Sfx::Surge));
        }
    }
}

/// Squad orders: rally, stance, recruit.
#[allow(clippy::too_many_arguments)]
fn squad_input(
    keys: Res<ButtonInput<KeyCode>>,
    unlocks: Res<Unlocks>,
    clock: Res<crate::threat::RunClock>,
    economy: Res<Economy>,
    mut squad: ResMut<Squad>,
    mut recruits: MessageWriter<RecruitRequest>,
    mut allies: Query<(&mut crate::allies::Ally, &Body)>,
    player: Query<&Body, (With<Player>, Without<crate::allies::Ally>)>,
    zones: Query<(&Zone, &Body), Without<crate::allies::Ally>>,
    mut hints: ResMut<HintQueue>,
    mut sfx: MessageWriter<SfxEvent>,
) {
    if !unlocks.allies {
        // Say so rather than swallowing the keystroke. Silence here reads as a
        // broken binding, which is precisely how it got reported.
        if keys.any_just_pressed([KeyCode::KeyF, KeyCode::KeyG, KeyCode::KeyR]) {
            crate::onboarding::locked_hint(
                &mut hints,
                "SQUAD",
                crate::onboarding::UNLOCK_ALLIES,
                clock.elapsed,
            );
            sfx.write(SfxEvent::at(crate::audio::Sfx::Deny, 0.5));
        }
        return;
    }

    // F: rally to the player and switch everyone to Follow.
    if keys.just_pressed(KeyCode::KeyF) {
        let anchor = player.iter().next().map_or(Vec2::ZERO, |b| b.pos);
        squad.stance = Stance::Follow;
        for (mut ally, _) in &mut allies {
            ally.stance = Stance::Follow;
            ally.anchor = anchor;
        }
        sfx.write(SfxEvent::at(crate::audio::Sfx::Order, 0.7));
    }

    // G: cycle the squad-wide stance.
    if keys.just_pressed(KeyCode::KeyG) {
        squad.stance = squad.stance.next();
        for (mut ally, body) in &mut allies {
            ally.stance = squad.stance;
            if squad.stance == Stance::Hold {
                ally.anchor = body.pos;
            }
        }
        hints.push(
            format!("SQUAD: {}", squad.stance.label()),
            match squad.stance {
                Stance::Follow => "They stay with you.",
                Stance::Hold => "They hold where they stand.",
                Stance::Guard => "They move to contested zones.",
            },
            HintTone::Tip,
        );
        sfx.write(SfxEvent::at(crate::audio::Sfx::Order, 0.7));
    }

    // R: recruit the best unit currently affordable, preferring variety.
    if keys.just_pressed(KeyCode::KeyR) {
        if squad.count >= squad.cap {
            hints.push("SQUAD FULL", "Cap reached.", HintTone::Tip);
            sfx.write(SfxEvent::at(crate::audio::Sfx::Deny, 0.5));
            return;
        }
        // Count what we already field so recruiting keeps the squad mixed.
        let mut have = [0u32; 4];
        for (ally, _) in &allies {
            have[ally.kind as usize] += 1;
        }
        let pick = AllyKind::ALL
            .iter()
            .copied()
            .filter(|k| economy.cores >= k.core_cost())
            .min_by_key(|k| (have[*k as usize], k.core_cost() as u32));

        if let Some(kind) = pick {
            recruits.write(RecruitRequest { kind });
        } else {
            hints.push(
                "NOT ENOUGH CORES",
                "Cores come from elites, bosses and holding zones.",
                HintTone::Tip,
            );
            sfx.write(SfxEvent::at(crate::audio::Sfx::Deny, 0.5));
        }
    }

    // Keep the squad count honest even if something died.
    squad.count = allies.iter().count() as u32;

    // Assign anchors for Guard so allies spread across zones rather than
    // all piling onto the same one.
    if keys.just_pressed(KeyCode::KeyG) && squad.stance == Stance::Guard {
        let targets: Vec<Vec2> = zones
            .iter()
            .filter(|(z, _)| z.owner != ZoneOwner::Player)
            .map(|(_, b)| b.pos)
            .collect();
        if !targets.is_empty() {
            for (i, (mut ally, _)) in allies.iter_mut().enumerate() {
                ally.anchor = targets[i % targets.len()];
            }
        }
    }
}

/// Draws the translucent build preview at the cursor.
fn update_ghost(
    mut commands: Commands,
    art: Res<GameArt>,
    plan: Res<PlanMode>,
    time: Res<Time<Real>>,
    mut ghosts: Query<
        (
            Entity,
            &mut Transform,
            &mut Mesh3d,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        With<BuildGhost>,
    >,
) {
    if !plan.active {
        for (e, _, _, _) in &mut ghosts {
            commands.entity(e).despawn();
        }
        return;
    }

    let kind = plan.selected_kind();
    let mesh = art.turrets[kind as usize].clone();
    let material = art.glow(if plan.valid {
        Glow::Ally
    } else {
        Glow::Warning
    });
    // A slow pulse keeps the ghost visually distinct from a real structure.
    let pulse = 1.0 + (time.elapsed_secs() * 4.0).sin() * 0.05;
    let transform =
        Transform::from_translation(to_world(plan.cursor, 0.05)).with_scale(Vec3::splat(pulse));

    if let Some((_, mut t, mut m, mut mat)) = ghosts.iter_mut().next() {
        *t = transform;
        m.0 = mesh;
        mat.0 = material;
    } else {
        commands.spawn((
            BuildGhost,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            transform,
            RunEntity,
        ));
    }
}
