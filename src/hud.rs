//! The in-run HUD.
//!
//! Laid out so the four things a commander checks constantly - health, the
//! clock, the dial, the bank - sit at four separate screen edges and never
//! move. Everything else is transient and appears where the eye already is.

use bevy::prelude::*;

use crate::allies::{Economy, Squad, Zone, ZoneOwner};
use crate::common::{Health, format_count, format_time};
use crate::enemy::{BossBarTarget, Director, Enemy};
use crate::environments::EnvKind;
use crate::onboarding::{HintQueue, HintTone, Unlocks};
use crate::palette as pal;
use crate::player::Player;
use crate::progress::Progression;
use crate::threat::{Phase, RunClock, Threat, WaveCycle};
use crate::weapons::{Loadout, MAX_LEVEL};
use crate::{AppState, GameSet};

/// `TextFont::font_size` takes a `FontSize` in Bevy 0.19; this keeps call sites
/// reading as plain pixel numbers.
pub fn px(size: f32) -> FontSize {
    FontSize::Px(size)
}

pub fn text(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(content.into()),
        TextFont {
            font_size: px(size),
            ..default()
        },
        TextColor(color),
    )
}

/// A standard translucent panel.
///
/// Bevy 0.19 folded `BorderRadius` into `Node` rather than keeping it a
/// separate component, so corner rounding is a field here and everywhere below.
fn panel() -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(Val::Px(12.0), Val::Px(9.0)),
            row_gap: Val::Px(4.0),
            border_radius: BorderRadius::all(Val::Px(7.0)),
            ..default()
        },
        BackgroundColor(pal::HUD_PANEL),
    )
}

// -- markers ----------------------------------------------------------------

#[derive(Debug, Component)]
pub struct HudRoot;

#[derive(Component)]
struct HealthFill;
#[derive(Component)]
struct HealthLabel;
#[derive(Component)]
struct XpFill;
#[derive(Component)]
struct LevelLabel;
#[derive(Component)]
struct ClockLabel;
#[derive(Component)]
struct PhaseLabel;
#[derive(Component)]
struct PhaseBarFill;
#[derive(Component)]
struct ThreatLabel;
#[derive(Component)]
struct ThreatBand;
#[derive(Component)]
struct RewardLabel;
#[derive(Component)]
struct SurgeLabel;
#[derive(Component)]
struct ScrapLabel;
#[derive(Component)]
struct CoreLabel;
#[derive(Component)]
struct SquadLabel;
#[derive(Component)]
struct ZoneLabel;
#[derive(Component)]
struct KillLabel;
#[derive(Component)]
struct WeaponList;
#[derive(Component)]
struct HintBanner;
#[derive(Component)]
struct HintHeadline;
#[derive(Component)]
struct HintDetail;
#[derive(Component)]
struct BossBanner;
#[derive(Component)]
struct BossFill;
#[derive(Component)]
struct BossName;
#[derive(Component)]
struct PlanBanner;
#[derive(Component)]
struct PlanText;
#[derive(Component)]
struct AnnounceRoot;
#[derive(Component)]
struct AnnounceText;
#[derive(Component)]
struct ControlsLabel;

#[derive(Debug)]
pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Playing), ensure_hud)
            .add_systems(
                Update,
                (
                    update_vitals,
                    update_clock,
                    update_threat,
                    update_economy,
                    update_weapons,
                    update_hint,
                    update_boss,
                    update_plan,
                )
                    .in_set(GameSet::Present),
            )
            .add_systems(Update, hud_visibility.run_if(state_changed::<AppState>))
            .add_systems(OnEnter(AppState::Menu), remove_hud);
    }
}

fn remove_hud(mut commands: Commands, q: Query<Entity, With<HudRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

/// Keep the HUD on screen for every in-run state, hidden only in the menu.
fn hud_visibility(state: Res<State<AppState>>, mut q: Query<&mut Node, With<HudRoot>>) {
    let show = state.get().run_alive();
    for mut node in &mut q {
        node.display = if show { Display::Flex } else { Display::None };
    }
}

fn ensure_hud(mut commands: Commands, existing: Query<Entity, With<HudRoot>>) {
    if !existing.is_empty() {
        return;
    }

    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            // The HUD must not eat clicks meant for the canvas.
            Pickable::IGNORE,
            GlobalZIndex(10),
        ))
        .with_children(|root| {
            top_left(root);
            top_center(root);
            top_right(root);
            bottom_left(root);
            bottom_right(root);
            centre_overlays(root);
        });
}

fn bar(
    parent: &mut ChildSpawnerCommands,
    width: f32,
    height: f32,
    color: Color,
    marker: impl Bundle,
) {
    parent
        .spawn((
            Node {
                width: Val::Px(width),
                height: Val::Px(height),
                border_radius: BorderRadius::all(Val::Px(height * 0.5)),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.12)),
        ))
        .with_children(|track| {
            track.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    border_radius: BorderRadius::all(Val::Px(height * 0.5)),
                    ..default()
                },
                BackgroundColor(color),
                marker,
            ));
        });
}

fn top_left(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(14.0),
            left: Val::Px(14.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        },
        Pickable::IGNORE,
    ))
    .with_children(|col| {
        col.spawn(panel()).with_children(|p| {
            p.spawn((
                Node {
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
                children![
                    (text("LV 1", 20.0, pal::ACCENT), LevelLabel),
                    (text("120 / 120", 15.0, pal::HUD_DIM), HealthLabel),
                ],
            ));
            bar(p, 210.0, 13.0, pal::HEAL_RED, HealthFill);
            bar(p, 210.0, 6.0, pal::XP_GREEN, XpFill);
        });
    });
}

fn top_center(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(14.0),
            left: Val::Percent(50.0),
            margin: UiRect::left(Val::Px(-130.0)),
            width: Val::Px(260.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(5.0),
            ..default()
        },
        Pickable::IGNORE,
    ))
    .with_children(|col| {
        col.spawn(panel()).with_children(|p| {
            p.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(3.0),
                    ..default()
                },
                children![
                    (text("0:00", 30.0, pal::HUD_TEXT), ClockLabel),
                    (text("PREP  40s", 14.0, pal::HUD_DIM), PhaseLabel),
                ],
            ));
            bar(p, 216.0, 5.0, pal::ACCENT, PhaseBarFill);
        });
    });
}

fn top_right(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(14.0),
            right: Val::Px(14.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::End,
            row_gap: Val::Px(6.0),
            ..default()
        },
        Pickable::IGNORE,
    ))
    .with_children(|col| {
        col.spawn(panel()).with_children(|p| {
            p.spawn((
                Node {
                    column_gap: Val::Px(9.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
                children![
                    (text("THREAT", 12.0, pal::HUD_DIM),),
                    (text("1.0", 24.0, pal::HUD_TEXT), ThreatLabel),
                    (text("LULL", 13.0, pal::HUD_DIM), ThreatBand),
                ],
            ));
            p.spawn((text("x1.00 rewards", 14.0, pal::XP_GREEN), RewardLabel));
            p.spawn((text("", 12.0, pal::ACCENT), SurgeLabel));
        });

        col.spawn(panel()).with_children(|p| {
            p.spawn((text("SCRAP 30", 17.0, pal::METAL), ScrapLabel));
            p.spawn((text("CORES 0", 17.0, pal::SCREEN_GLOW), CoreLabel));
        });
    });
}

fn bottom_left(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(14.0),
            left: Val::Px(14.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        },
        Pickable::IGNORE,
        WeaponList,
    ));
}

fn bottom_right(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(14.0),
            right: Val::Px(14.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::End,
            row_gap: Val::Px(6.0),
            ..default()
        },
        Pickable::IGNORE,
    ))
    .with_children(|col| {
        col.spawn(panel()).with_children(|p| {
            p.spawn((text("SQUAD 0/4  FOLLOW", 15.0, pal::HUD_TEXT), SquadLabel));
            p.spawn((text("ZONES 0", 15.0, pal::HUD_DIM), ZoneLabel));
            p.spawn((text("KILLS 0", 13.0, pal::HUD_DIM), KillLabel));
        });
        col.spawn((
            text(
                "SPACE plan   B build   R recruit   T research",
                12.0,
                pal::HUD_DIM,
            ),
            ControlsLabel,
        ));
    });
}

fn centre_overlays(root: &mut ChildSpawnerCommands) {
    // Boss bar, just under the clock.
    root.spawn((
        BossBanner,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(112.0),
            left: Val::Percent(50.0),
            margin: UiRect::left(Val::Px(-200.0)),
            width: Val::Px(400.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(4.0),
            display: Display::None,
            ..default()
        },
        Pickable::IGNORE,
    ))
    .with_children(|col| {
        col.spawn((text("BOSS", 17.0, pal::BOSS_TRIM), BossName));
        bar(col, 400.0, 11.0, pal::BOSS_TRIM, BossFill);
    });

    // Wave / boss announcements. The container and the text carry separate
    // markers so the update system can prove its queries are disjoint.
    root.spawn((
        AnnounceRoot,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(26.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            display: Display::None,
            ..default()
        },
        Pickable::IGNORE,
        children![(text("", 44.0, pal::BOSS_TRIM), AnnounceText)],
    ));

    // Hint banner.
    root.spawn((
        HintBanner,
        Node {
            position_type: PositionType::Absolute,
            // Low enough to clear the level-up card row, which occupies the
            // middle band whenever it is open.
            bottom: Val::Percent(8.0),
            left: Val::Percent(50.0),
            margin: UiRect::left(Val::Px(-260.0)),
            width: Val::Px(520.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::axes(Val::Px(16.0), Val::Px(11.0)),
            row_gap: Val::Px(3.0),
            display: Display::None,
            border_radius: BorderRadius::all(Val::Px(9.0)),
            ..default()
        },
        BackgroundColor(pal::HUD_PANEL),
        Pickable::IGNORE,
        children![
            (text("", 20.0, pal::ACCENT), HintHeadline),
            (text("", 14.0, pal::HUD_TEXT), HintDetail),
        ],
    ));

    // Plan-mode banner.
    root.spawn((
        PlanBanner,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(50.0),
            left: Val::Px(0.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            display: Display::None,
            ..default()
        },
        Pickable::IGNORE,
        children![(text("", 16.0, pal::ACCENT), PlanText)],
    ));
}

// -- update systems ---------------------------------------------------------

fn update_vitals(
    progression: Res<Progression>,
    player: Query<&Health, With<Player>>,
    mut fills: ParamSet<(
        Query<&mut Node, With<HealthFill>>,
        Query<&mut Node, With<XpFill>>,
    )>,
    mut labels: ParamSet<(
        Query<&mut Text, With<HealthLabel>>,
        Query<&mut Text, With<LevelLabel>>,
    )>,
) {
    if let Some(health) = player.iter().next() {
        for mut node in &mut fills.p0() {
            node.width = Val::Percent(health.fraction() * 100.0);
        }
        for mut t in &mut labels.p0() {
            t.0 = format!(
                "{} / {}",
                health.current.max(0.0).ceil() as i32,
                health.max.ceil() as i32
            );
        }
    }
    for mut node in &mut fills.p1() {
        node.width = Val::Percent(progression.fraction() * 100.0);
    }
    for mut t in &mut labels.p1() {
        t.0 = format!("LV {}", progression.level);
    }
}

/// Three of these queries write `&mut Text` and two write `&mut Node`; the
/// marker filters alone do not prove them disjoint to the scheduler, so each
/// group goes in its own `ParamSet`.
fn update_clock(
    clock: Res<RunClock>,
    cycle: Res<WaveCycle>,
    director: Res<Director>,
    mut texts: ParamSet<(
        Query<&mut Text, With<ClockLabel>>,
        Query<(&mut Text, &mut TextColor), With<PhaseLabel>>,
        Query<&mut Text, With<AnnounceText>>,
    )>,
    mut nodes: ParamSet<(
        Query<&mut Node, With<PhaseBarFill>>,
        Query<&mut Node, With<AnnounceRoot>>,
    )>,
) {
    for mut t in &mut texts.p0() {
        t.0 = format_time(clock.elapsed);
    }

    for (mut t, mut color) in &mut texts.p1() {
        let remaining = cycle.timer.max(0.0).ceil() as i32;
        t.0 = match cycle.phase {
            Phase::Prep => format!(
                "PREP {remaining}s   ENTER: call early +{}%",
                (cycle.pending_bonus() * 100.0) as i32
            ),
            Phase::Assault => format!("WAVE {}   {remaining}s", cycle.wave),
        };
        color.0 = match cycle.phase {
            Phase::Prep => pal::XP_GREEN,
            Phase::Assault => pal::DANGER,
        };
    }

    let total = match cycle.phase {
        Phase::Prep => cycle.prep_length,
        Phase::Assault => cycle.assault_length(),
    };
    for mut node in &mut nodes.p0() {
        node.width = Val::Percent((cycle.timer / total.max(0.01) * 100.0).clamp(0.0, 100.0));
    }

    // Boss / wave announcement.
    let message = director.announce.as_ref().map(|(m, _)| m.clone());
    for mut node in &mut nodes.p1() {
        node.display = if message.is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Some(msg) = message {
        for mut t in &mut texts.p2() {
            t.0.clone_from(&msg);
        }
    }
}

fn update_threat(
    threat: Res<Threat>,
    cycle: Res<WaveCycle>,
    unlocks: Res<Unlocks>,
    mut set: ParamSet<(
        Query<(&mut Text, &mut TextColor), With<ThreatLabel>>,
        Query<(&mut Text, &mut TextColor), With<ThreatBand>>,
        Query<&mut Text, With<RewardLabel>>,
        Query<(&mut Text, &mut TextColor), With<SurgeLabel>>,
    )>,
) {
    let effective = threat.effective();
    let color = threat.band_color();

    for (mut t, mut c) in &mut set.p0() {
        t.0 = format!("{effective:.1}");
        c.0 = color;
    }
    for (mut t, mut c) in &mut set.p1() {
        t.0 = threat.band().to_string();
        c.0 = color;
    }
    for mut t in &mut set.p2() {
        t.0 = format!("x{:.2} rewards", threat.reward_mult() * cycle.reward_mult());
    }
    for (mut t, mut c) in &mut set.p3() {
        if !unlocks.threat_dial {
            t.0 = "dial locked".into();
            c.0 = pal::HUD_DIM;
        } else if threat.surging() {
            t.0 = format!("OVERCLOCK {:.0}s", threat.surge);
            c.0 = pal::BOSS_TRIM;
        } else if threat.surge_cooldown > 0.0 {
            t.0 = format!("O ready in {:.0}s", threat.surge_cooldown);
            c.0 = pal::HUD_DIM;
        } else {
            t.0 = "O: OVERCLOCK READY".into();
            c.0 = pal::ACCENT;
        }
    }
}

fn update_economy(
    economy: Res<Economy>,
    squad: Res<Squad>,
    clock: Res<RunClock>,
    zones: Query<&Zone>,
    mut set: ParamSet<(
        Query<&mut Text, With<ScrapLabel>>,
        Query<&mut Text, With<CoreLabel>>,
        Query<&mut Text, With<SquadLabel>>,
        Query<&mut Text, With<ZoneLabel>>,
        Query<&mut Text, With<KillLabel>>,
    )>,
) {
    for mut t in &mut set.p0() {
        t.0 = if economy.scrap_rate > 0.0 {
            format!(
                "SCRAP {}  (+{:.1}/s)",
                economy.scrap as u32, economy.scrap_rate
            )
        } else {
            format!("SCRAP {}", economy.scrap as u32)
        };
    }
    for mut t in &mut set.p1() {
        t.0 = format!("CORES {}", economy.cores as u32);
    }
    for mut t in &mut set.p2() {
        t.0 = format!(
            "SQUAD {}/{}  {}",
            squad.count,
            squad.cap,
            squad.stance.label()
        );
    }
    let held = zones
        .iter()
        .filter(|z| z.owner == ZoneOwner::Player)
        .count();
    let total = zones.iter().count();
    for mut t in &mut set.p3() {
        t.0 = format!("ZONES {held}/{total}");
    }
    for mut t in &mut set.p4() {
        t.0 = format!("KILLS {}", format_count(clock.kills));
    }
}

/// Rebuilds the weapon list only when the loadout actually changes.
fn update_weapons(
    mut commands: Commands,
    loadout: Res<Loadout>,
    list: Query<Entity, With<WeaponList>>,
) {
    if !loadout.is_changed() {
        return;
    }
    let Ok(root) = list.single() else { return };

    commands.entity(root).despawn_related::<Children>();
    commands.entity(root).with_children(|col| {
        for slot in &loadout.slots {
            let maxed = slot.level >= MAX_LEVEL;
            col.spawn((
                Node {
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(pal::HUD_PANEL),
                children![
                    text(
                        slot.kind.name(),
                        14.0,
                        if maxed { pal::ACCENT } else { pal::HUD_TEXT }
                    ),
                    text(
                        if maxed {
                            "MAX".to_string()
                        } else {
                            format!("{}", slot.level)
                        },
                        14.0,
                        if maxed { pal::ACCENT } else { pal::HUD_DIM }
                    ),
                ],
            ));
        }
    });
}

fn update_hint(
    hints: Res<HintQueue>,
    mut banner: Query<&mut Node, With<HintBanner>>,
    mut headline: Query<(&mut Text, &mut TextColor), With<HintHeadline>>,
    mut detail: Query<&mut Text, (With<HintDetail>, Without<HintHeadline>)>,
) {
    let active = hints.active.clone();
    for mut node in &mut banner {
        node.display = if active.is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    let Some(hint) = active else { return };
    for (mut t, mut c) in &mut headline {
        t.0.clone_from(&hint.headline);
        c.0 = match hint.tone {
            HintTone::Unlock => pal::ACCENT,
            HintTone::Discovery => pal::ELITE_TRIM,
            HintTone::Tip => pal::SCREEN_GLOW,
        };
    }
    for mut t in &mut detail {
        t.0.clone_from(&hint.detail);
    }
}

fn update_boss(
    env: Res<EnvKind>,
    bosses: Query<(&Enemy, &Health), With<BossBarTarget>>,
    mut banner: Query<&mut Node, With<BossBanner>>,
    mut fill: Query<&mut Node, (With<BossFill>, Without<BossBanner>)>,
    mut name: Query<&mut Text, With<BossName>>,
) {
    // Track the most wounded boss, which is the one the player is fighting.
    let target = bosses
        .iter()
        .min_by(|a, b| a.1.fraction().total_cmp(&b.1.fraction()));

    for mut node in &mut banner {
        node.display = if target.is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    let Some((enemy, health)) = target else {
        return;
    };
    for mut node in &mut fill {
        node.width = Val::Percent(health.fraction() * 100.0);
    }
    for mut t in &mut name {
        t.0 = enemy.kind.name(*env).to_string();
    }
}

fn update_plan(
    plan: Res<crate::command::PlanMode>,
    economy: Res<Economy>,
    stats: Res<crate::player::PlayerStats>,
    mut banner: Query<&mut Node, With<PlanBanner>>,
    mut label: Query<(&mut Text, &mut TextColor), With<PlanText>>,
) {
    for mut node in &mut banner {
        node.display = if plan.active {
            Display::Flex
        } else {
            Display::None
        };
    }
    if !plan.active {
        return;
    }

    let kind = plan.selected_kind();
    let cost = (kind.scrap_cost() * (1.0 - stats.build_discount)) as u32;
    let affordable = economy.scrap >= f32::from(u16::try_from(cost).unwrap_or(u16::MAX));

    let body = plan.message.as_ref().map_or_else(
        || {
            format!(
                "PLAN  -  [{}] {}  {} scrap  -  ENTER to place, SPACE to resume",
                plan.selected + 1,
                kind.name(),
                cost
            )
        },
        |(m, _)| m.clone(),
    );

    for (mut t, mut c) in &mut label {
        t.0.clone_from(&body);
        c.0 = if plan.valid && affordable {
            pal::ACCENT
        } else {
            pal::DANGER
        };
    }
}
