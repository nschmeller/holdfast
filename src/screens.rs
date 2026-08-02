//! Full-screen states: menu, level-up, research, pause, results.
//!
//! Every one of them is driven entirely by the keyboard, and every one lists
//! its own keys on screen. Nothing here assumes a mouse exists.

use bevy::prelude::*;

use crate::allies::Economy;
use crate::common::*;
use crate::environments::{EnvDirty, EnvKind};
use crate::hud::text;
use crate::onboarding::Unlocks;
use crate::palette as pal;
use crate::player::PlayerStats;
use crate::progress::{
    AppliedBoosts, Branch, CardOffer, Equipped, GearSlot, Progression, RecomputeStats, Research,
    apply_card, build_offer, card_color,
};
use crate::rng::Rng;
use crate::threat::{RunClock, Threat, WaveCycle};
use crate::weapons::Loadout;
use crate::{AppState, RunSetup};

#[derive(Component)]
struct ScreenRoot;

/// Which branch the research cursor is in.
#[derive(Resource, Default)]
struct ResearchCursor {
    branch: usize,
    row: usize,
}

pub struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ResearchCursor>()
            .add_systems(OnEnter(AppState::Menu), build_menu)
            .add_systems(OnEnter(AppState::LevelUp), build_levelup)
            .add_systems(OnEnter(AppState::SkillTree), build_research)
            .add_systems(OnEnter(AppState::Paused), build_pause)
            .add_systems(OnEnter(AppState::GameOver), build_gameover)
            .add_systems(
                OnExit(AppState::Menu),
                (clear_screens, start_run).chain().in_set(RunSetup::Clear),
            )
            .add_systems(OnExit(AppState::LevelUp), clear_screens)
            .add_systems(OnExit(AppState::SkillTree), clear_screens)
            .add_systems(OnExit(AppState::Paused), clear_screens)
            .add_systems(OnExit(AppState::GameOver), clear_screens)
            .add_systems(
                Update,
                (
                    menu_input.run_if(in_state(AppState::Menu)),
                    levelup_input.run_if(in_state(AppState::LevelUp)),
                    research_input.run_if(in_state(AppState::SkillTree)),
                    pause_input.run_if(in_state(AppState::Paused)),
                    gameover_input.run_if(in_state(AppState::GameOver)),
                    global_hotkeys.run_if(in_state(AppState::Playing)),
                ),
            );
    }
}

fn clear_screens(mut commands: Commands, q: Query<Entity, With<ScreenRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

fn overlay(dim: f32) -> impl Bundle {
    (
        ScreenRoot,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(16.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.022, 0.035, dim)),
        GlobalZIndex(50),
    )
}

fn card_panel(accent: Color) -> impl Bundle {
    (
        Node {
            width: Val::Px(268.0),
            min_height: Val::Px(190.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::all(Val::Px(18.0)),
            row_gap: Val::Px(10.0),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(11.0)),
            ..default()
        },
        BackgroundColor(pal::HUD_PANEL_SOLID),
        BorderColor::all(accent),
    )
}

// -- menu -------------------------------------------------------------------

/// The wordmark's tagline. Deliberately world-agnostic: the desk is one of
/// five, not the premise.
const TAGLINE: &str = "FIVE SMALL WORLDS.  ONE RULE:  HOLD YOUR GROUND.";

#[derive(Component)]
struct EnvTitle;
#[derive(Component)]
struct EnvTagline;
#[derive(Component)]
struct EnvQuirk;
#[derive(Component)]
struct EnvDetailPanel;
/// One selector chip, tagged with the world it selects.
#[derive(Component)]
struct WorldChip(EnvKind);

fn build_menu(mut commands: Commands, env: Res<EnvKind>) {
    let selected = *env;

    commands.spawn(overlay(1.0)).with_children(|root| {
        // -- wordmark ------------------------------------------------------
        root.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(2.0),
                ..default()
            },
            children![
                text("HOLDFAST", 78.0, pal::HUD_TEXT),
                text(TAGLINE, 14.0, pal::ACCENT),
                (
                    Node {
                        margin: UiRect::top(Val::Px(6.0)),
                        ..default()
                    },
                    text("a survival command roguelite", 13.0, pal::HUD_DIM),
                ),
            ],
        ));

        // -- world selector -------------------------------------------------
        root.spawn((
            Node {
                margin: UiRect::top(Val::Px(26.0)),
                column_gap: Val::Px(10.0),
                ..default()
            },
        ))
        .with_children(|row| {
            for world in EnvKind::ALL {
                let active = world == selected;
                row.spawn((
                    WorldChip(world),
                    Node {
                        width: Val::Px(118.0),
                        height: Val::Px(58.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(chip_background(world, active)),
                    BorderColor::all(chip_border(world, active)),
                    children![text(
                        world.short_name(),
                        18.0,
                        if active { world.accent() } else { pal::HUD_DIM }
                    )],
                ));
            }
        });

        root.spawn((
            Node {
                margin: UiRect::top(Val::Px(7.0)),
                ..default()
            },
            text("< LEFT / RIGHT to choose a world >", 12.0, pal::HUD_DIM),
        ));

        // -- detail panel ---------------------------------------------------
        root.spawn((
            EnvDetailPanel,
            Node {
                margin: UiRect::top(Val::Px(14.0)),
                width: Val::Px(640.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(28.0), Val::Px(16.0)),
                row_gap: Val::Px(6.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(11.0)),
                ..default()
            },
            BackgroundColor(pal::HUD_PANEL_SOLID),
            BorderColor::all(selected.accent()),
            children![
                (text(selected.title(), 32.0, pal::HUD_TEXT), EnvTitle),
                (text(selected.tagline(), 15.0, pal::HUD_DIM), EnvTagline),
                (
                    Node {
                        margin: UiRect::top(Val::Px(4.0)),
                        ..default()
                    },
                    text(selected.quirk(), 14.0, selected.accent()),
                    EnvQuirk,
                ),
            ],
        ));

        root.spawn((
            Node {
                margin: UiRect::top(Val::Px(18.0)),
                ..default()
            },
            text("PRESS ENTER TO DEPLOY", 24.0, pal::XP_GREEN),
        ));

        // -- controls -------------------------------------------------------
        root.spawn((
            Node {
                margin: UiRect::top(Val::Px(24.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(3.0),
                ..default()
            },
            children![
                text("WASD move    SPACE plan (slows time to a crawl)    Q/E rotate", 14.0, pal::HUD_DIM),
                text("B build    R recruit    F rally    G stance    T research", 14.0, pal::HUD_DIM),
                text("- / =  threat dial    O overclock    ENTER call the wave early", 14.0, pal::HUD_DIM),
                (
                    Node {
                        margin: UiRect::top(Val::Px(7.0)),
                        ..default()
                    },
                    text(
                        "Everything aims itself. You decide where to stand, what to build, and how fast this goes.",
                        13.0,
                        pal::HUD_DIM,
                    ),
                ),
            ],
        ));
    });
}

fn chip_background(world: EnvKind, active: bool) -> Color {
    if active {
        world.accent().with_alpha(0.16)
    } else {
        Color::srgba(1.0, 1.0, 1.0, 0.04)
    }
}

fn chip_border(world: EnvKind, active: bool) -> Color {
    if active {
        world.accent()
    } else {
        Color::srgba(1.0, 1.0, 1.0, 0.12)
    }
}

#[allow(clippy::too_many_arguments)]
fn menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut env: ResMut<EnvKind>,
    mut next: ResMut<NextState<AppState>>,
    mut sfx: MessageWriter<SfxEvent>,
    mut labels: ParamSet<(
        Query<&mut Text, With<EnvTitle>>,
        Query<&mut Text, With<EnvTagline>>,
        Query<(&mut Text, &mut TextColor), With<EnvQuirk>>,
    )>,
    mut panel: Query<&mut BorderColor, (With<EnvDetailPanel>, Without<WorldChip>)>,
    mut chips: Query<
        (&WorldChip, &mut BackgroundColor, &mut BorderColor, &Children),
        Without<EnvDetailPanel>,
    >,
    mut chip_text: Query<&mut TextColor, (Without<EnvQuirk>, Without<EnvTitle>)>,
) {
    let mut changed = false;
    if keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::KeyA) {
        *env = env.prev();
        changed = true;
    }
    if keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::KeyD) {
        *env = env.next();
        changed = true;
    }

    if changed {
        let selected = *env;
        sfx.write(SfxEvent::at(crate::audio::Sfx::Tick, 0.8));

        for mut t in &mut labels.p0() {
            t.0 = selected.title().to_string();
        }
        for mut t in &mut labels.p1() {
            t.0 = selected.tagline().to_string();
        }
        for (mut t, mut c) in &mut labels.p2() {
            t.0 = selected.quirk().to_string();
            c.0 = selected.accent();
        }
        for mut border in &mut panel {
            *border = BorderColor::all(selected.accent());
        }
        for (chip, mut bg, mut border, children) in &mut chips {
            let active = chip.0 == selected;
            bg.0 = chip_background(chip.0, active);
            *border = BorderColor::all(chip_border(chip.0, active));
            for child in children.iter() {
                if let Ok(mut color) = chip_text.get_mut(child) {
                    color.0 = if active { chip.0.accent() } else { pal::HUD_DIM };
                }
            }
        }
    }

    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
        next.set(AppState::Playing);
    }
}

/// Wipe the previous run and rebuild the arena.
#[allow(clippy::too_many_arguments)]
fn start_run(
    mut commands: Commands,
    mut clock: ResMut<RunClock>,
    mut threat: ResMut<Threat>,
    mut cycle: ResMut<WaveCycle>,
    mut dirty: ResMut<EnvDirty>,
    mut recompute: MessageWriter<RecomputeStats>,
    old: Query<Entity, With<RunEntity>>,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    *clock = RunClock::default();
    threat.reset();
    cycle.reset();
    dirty.0 = true;
    recompute.write(RecomputeStats);
}

// -- level up ---------------------------------------------------------------

fn build_levelup(mut commands: Commands, offer: Res<CardOffer>, mut sfx: MessageWriter<SfxEvent>) {
    sfx.write(SfxEvent::new(crate::audio::Sfx::LevelUp));

    commands.spawn(overlay(0.72)).with_children(|root| {
        root.spawn(text("LEVEL UP", 44.0, pal::ACCENT));
        root.spawn(text("Pick one. Press the number.", 15.0, pal::HUD_DIM));

        root.spawn((
            Node {
                margin: UiRect::top(Val::Px(10.0)),
                column_gap: Val::Px(18.0),
                ..default()
            },
        ))
        .with_children(|row| {
            for (i, card) in offer.cards.iter().enumerate() {
                let accent = card_color(card.rarity);
                row.spawn(card_panel(accent)).with_children(|p| {
                    p.spawn(text(format!("{}", i + 1), 26.0, accent));
                    p.spawn((
                        Node {
                            margin: UiRect::top(Val::Px(2.0)),
                            ..default()
                        },
                        text(card.title.clone(), 21.0, pal::HUD_TEXT),
                    ));
                    p.spawn(text(
                        pal::rarity_name(card.rarity),
                        11.0,
                        accent,
                    ));
                    p.spawn((
                        Node {
                            margin: UiRect::top(Val::Px(6.0)),
                            max_width: Val::Px(228.0),
                            ..default()
                        },
                        text(card.detail.clone(), 14.0, pal::HUD_DIM),
                    ));
                });
            }
        });

        root.spawn((
            Node {
                margin: UiRect::top(Val::Px(12.0)),
                ..default()
            },
            text("R to reroll (once per level)", 14.0, pal::SCREEN_GLOW),
        ));
    });
}

#[allow(clippy::too_many_arguments)]
fn levelup_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut offer: ResMut<CardOffer>,
    mut progression: ResMut<Progression>,
    mut stats: ResMut<PlayerStats>,
    mut loadout: ResMut<Loadout>,
    mut economy: ResMut<Economy>,
    mut boosts: ResMut<AppliedBoosts>,
    mut rng: ResMut<Rng>,
    mut next: ResMut<NextState<AppState>>,
    mut recompute: MessageWriter<RecomputeStats>,
    mut sfx: MessageWriter<SfxEvent>,
    roots: Query<Entity, With<ScreenRoot>>,
) {
    // Reroll.
    if keys.just_pressed(KeyCode::KeyR) && offer.reroll_available {
        offer.cards = build_offer(&mut rng, &loadout, &boosts);
        offer.reroll_available = false;
        sfx.write(SfxEvent::at(crate::audio::Sfx::Tick, 1.0));
        // Rebuild the screen with the new offer.
        for e in &roots {
            commands.entity(e).despawn();
        }
        let cards = offer.cards.clone();
        commands.spawn(overlay(0.72)).with_children(|root| {
            root.spawn(text("LEVEL UP", 44.0, pal::ACCENT));
            root.spawn(text("Pick one. Press the number.", 15.0, pal::HUD_DIM));
            root.spawn((Node {
                margin: UiRect::top(Val::Px(10.0)),
                column_gap: Val::Px(18.0),
                ..default()
            },))
                .with_children(|row| {
                    for (i, card) in cards.iter().enumerate() {
                        let accent = card_color(card.rarity);
                        row.spawn(card_panel(accent)).with_children(|p| {
                            p.spawn(text(format!("{}", i + 1), 26.0, accent));
                            p.spawn(text(card.title.clone(), 21.0, pal::HUD_TEXT));
                            p.spawn(text(pal::rarity_name(card.rarity), 11.0, accent));
                            p.spawn((
                                Node {
                                    max_width: Val::Px(228.0),
                                    ..default()
                                },
                                text(card.detail.clone(), 14.0, pal::HUD_DIM),
                            ));
                        });
                    }
                });
        });
        return;
    }

    let picked = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3]
        .iter()
        .position(|k| keys.just_pressed(*k));

    let Some(index) = picked else { return };
    let Some(card) = offer.cards.get(index).cloned() else {
        return;
    };

    apply_card(&card, &mut stats, &mut loadout, &mut economy, &mut boosts);
    recompute.write(RecomputeStats);

    progression.pending_levels = progression.pending_levels.saturating_sub(1);
    offer.cards.clear();
    sfx.write(SfxEvent::new(crate::audio::Sfx::Gear));

    // More levels banked: the check system will immediately re-open.
    next.set(AppState::Playing);
}

// -- research ---------------------------------------------------------------

fn build_research(
    mut commands: Commands,
    research: Res<Research>,
    progression: Res<Progression>,
    economy: Res<Economy>,
    cursor: Res<ResearchCursor>,
) {
    commands.spawn(overlay(0.82)).with_children(|root| {
        root.spawn(text("RESEARCH", 40.0, pal::ACCENT));
        root.spawn(text(
            format!(
                "{} Cores    {} Skill Points    arrows to move, ENTER to buy, T to close",
                economy.cores as u32, progression.skill_points
            ),
            15.0,
            pal::HUD_DIM,
        ));

        root.spawn((Node {
            margin: UiRect::top(Val::Px(14.0)),
            column_gap: Val::Px(14.0),
            align_items: AlignItems::Start,
            ..default()
        },))
            .with_children(|row| {
                for (bi, branch) in Branch::ALL.iter().enumerate() {
                    let selected_branch = bi == cursor.branch;
                    row.spawn((
                        Node {
                            width: Val::Px(232.0),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            padding: UiRect::all(Val::Px(12.0)),
                            row_gap: Val::Px(7.0),
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: BorderRadius::all(Val::Px(9.0)),
                            ..default()
                        },
                        BackgroundColor(pal::HUD_PANEL_SOLID),
                        BorderColor::all(if selected_branch {
                            branch.color()
                        } else {
                            Color::srgba(1.0, 1.0, 1.0, 0.1)
                        }),
                    ))
                    .with_children(|col| {
                        col.spawn(text(branch.title(), 20.0, branch.color()));

                        for (ri, ni) in research.in_branch(*branch).into_iter().enumerate() {
                            let node = &research.nodes[ni];
                            let here = selected_branch && ri == cursor.row;
                            let maxed = node.maxed();
                            let label = if node.endless {
                                format!("{}  [{}]", node.title, node.rank)
                            } else {
                                format!("{}  {}/{}", node.title, node.rank, node.max_rank)
                            };
                            col.spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Column,
                                    padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                                    row_gap: Val::Px(1.0),
                                    border_radius: BorderRadius::all(Val::Px(5.0)),
                                    ..default()
                                },
                                BackgroundColor(if here {
                                    Color::srgba(1.0, 1.0, 1.0, 0.12)
                                } else {
                                    Color::NONE
                                }),
                                children![
                                    text(
                                        label,
                                        14.0,
                                        if maxed { pal::HUD_DIM } else { pal::HUD_TEXT }
                                    ),
                                    text(node.detail, 12.0, pal::HUD_DIM),
                                    text(
                                        if maxed {
                                            "MAXED".to_string()
                                        } else {
                                            format!("{} cores", node.current_cost().ceil() as u32)
                                        },
                                        12.0,
                                        if maxed { pal::HUD_DIM } else { branch.color() }
                                    ),
                                ],
                            ));
                        }
                    });
                }
            });
    });
}

#[allow(clippy::too_many_arguments)]
fn research_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut cursor: ResMut<ResearchCursor>,
    mut research: ResMut<Research>,
    mut economy: ResMut<Economy>,
    progression: Res<Progression>,
    mut next: ResMut<NextState<AppState>>,
    mut recompute: MessageWriter<RecomputeStats>,
    mut sfx: MessageWriter<SfxEvent>,
    roots: Query<Entity, With<ScreenRoot>>,
) {
    let mut dirty = false;

    if keys.just_pressed(KeyCode::ArrowLeft) {
        cursor.branch = (cursor.branch + Branch::ALL.len() - 1) % Branch::ALL.len();
        cursor.row = 0;
        dirty = true;
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        cursor.branch = (cursor.branch + 1) % Branch::ALL.len();
        cursor.row = 0;
        dirty = true;
    }

    let rows = research.in_branch(Branch::ALL[cursor.branch]).len();
    if keys.just_pressed(KeyCode::ArrowUp) && rows > 0 {
        cursor.row = (cursor.row + rows - 1) % rows;
        dirty = true;
    }
    if keys.just_pressed(KeyCode::ArrowDown) && rows > 0 {
        cursor.row = (cursor.row + 1) % rows;
        dirty = true;
    }

    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
        let indices = research.in_branch(Branch::ALL[cursor.branch]);
        if let Some(&ni) = indices.get(cursor.row) {
            let cost = research.nodes[ni].current_cost();
            let maxed = research.nodes[ni].maxed();
            if !maxed && economy.spend_cores(cost) {
                research.nodes[ni].rank += 1;
                recompute.write(RecomputeStats);
                sfx.write(SfxEvent::new(crate::audio::Sfx::Gear));
            } else {
                sfx.write(SfxEvent::at(crate::audio::Sfx::Deny, 0.6));
            }
            dirty = true;
        }
    }

    if keys.just_pressed(KeyCode::KeyT) || keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::Playing);
        return;
    }

    if dirty {
        for e in &roots {
            commands.entity(e).despawn();
        }
        // Rebuilding the whole panel is cheap and keeps the draw code in one
        // place rather than duplicating it as an update path.
        build_research(commands, research.into(), progression, economy.into(), cursor.into());
    }
}

// -- pause ------------------------------------------------------------------

fn build_pause(
    mut commands: Commands,
    clock: Res<RunClock>,
    progression: Res<Progression>,
    equipped: Res<Equipped>,
    loadout: Res<Loadout>,
) {
    commands.spawn(overlay(0.86)).with_children(|root| {
        root.spawn(text("PAUSED", 46.0, pal::ACCENT));
        root.spawn(text(
            format!(
                "{}   Level {}   {} kills",
                format_time(clock.elapsed),
                progression.level,
                format_count(clock.kills)
            ),
            17.0,
            pal::HUD_DIM,
        ));

        root.spawn((Node {
            margin: UiRect::top(Val::Px(16.0)),
            column_gap: Val::Px(22.0),
            align_items: AlignItems::Start,
            ..default()
        },))
            .with_children(|row| {
                row.spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        ..default()
                    },
                ))
                .with_children(|col| {
                    col.spawn(text("WEAPONS", 16.0, pal::ACCENT));
                    for slot in &loadout.slots {
                        col.spawn(text(
                            format!("{}  lv {}", slot.kind.name(), slot.level),
                            14.0,
                            pal::HUD_TEXT,
                        ));
                    }
                });

                row.spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        ..default()
                    },
                ))
                .with_children(|col| {
                    col.spawn(text("GEAR", 16.0, pal::ACCENT));
                    for slot in GearSlot::ALL {
                        let line = equipped.get(slot).map_or_else(
                            || format!("{}: -", slot.label()),
                            |g| format!("{}: {} ({})", slot.label(), g.name, g.describe()),
                        );
                        let color = equipped
                            .get(slot)
                            .map_or(pal::HUD_DIM, |g| pal::RARITY[g.rarity]);
                        col.spawn(text(line, 13.0, color));
                    }
                });
            });

        root.spawn((
            Node {
                margin: UiRect::top(Val::Px(20.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(3.0),
                ..default()
            },
            children![
                text("ESC to resume", 18.0, pal::XP_GREEN),
                text("BACKSPACE to abandon the run", 14.0, pal::HUD_DIM),
            ],
        ));
    });
}

fn pause_input(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::Playing);
    }
    if keys.just_pressed(KeyCode::Backspace) {
        next.set(AppState::Menu);
    }
}

// -- results ----------------------------------------------------------------

fn build_gameover(
    mut commands: Commands,
    clock: Res<RunClock>,
    progression: Res<Progression>,
    economy: Res<Economy>,
    threat: Res<Threat>,
    cycle: Res<WaveCycle>,
    env: Res<EnvKind>,
    unlocks: Res<Unlocks>,
) {
    commands.spawn(overlay(0.9)).with_children(|root| {
        root.spawn(text("OVERRUN", 58.0, pal::DANGER));
        root.spawn(text(env.title(), 18.0, pal::HUD_DIM));

        root.spawn((
            Node {
                margin: UiRect::top(Val::Px(18.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(34.0), Val::Px(18.0)),
                row_gap: Val::Px(6.0),
                border_radius: BorderRadius::all(Val::Px(11.0)),
                ..default()
            },
            BackgroundColor(pal::HUD_PANEL_SOLID),
            children![
                text(format!("SURVIVED   {}", format_time(clock.elapsed)), 30.0, pal::ACCENT),
                text(format!("Wave {}   Level {}", cycle.wave, progression.level), 17.0, pal::HUD_TEXT),
                text(format!("{} kills", format_count(clock.kills)), 16.0, pal::HUD_DIM),
                text(
                    format!(
                        "Peak threat {:.1}   {} scrap   {} cores earned",
                        threat.effective(),
                        economy.lifetime_scrap as u32,
                        economy.lifetime_cores as u32
                    ),
                    15.0,
                    pal::HUD_DIM
                ),
                text(
                    format!("{}/5 systems brought online", unlocks.online()),
                    14.0,
                    pal::SCREEN_GLOW
                ),
            ],
        ));

        root.spawn((
            Node {
                margin: UiRect::top(Val::Px(22.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                ..default()
            },
            children![
                text("ENTER to run it back", 22.0, pal::XP_GREEN),
                text("ESC for the menu", 15.0, pal::HUD_DIM),
            ],
        ));
    });
}

fn gameover_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<AppState>>,
    mut dirty: ResMut<EnvDirty>,
    mut commands: Commands,
    mut clock: ResMut<RunClock>,
    mut threat: ResMut<Threat>,
    mut cycle: ResMut<WaveCycle>,
    old: Query<Entity, With<RunEntity>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::Menu);
        return;
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
        // Restart in place: same arena, fresh run.
        for e in &old {
            commands.entity(e).despawn();
        }
        *clock = RunClock::default();
        threat.reset();
        cycle.reset();
        dirty.0 = true;
        // Bouncing through Menu re-fires every OnExit(Menu) reset system, so
        // there is exactly one code path that starts a run.
        next.set(AppState::Menu);
    }
}

/// Keys that open overlays from gameplay.
fn global_hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    unlocks: Res<Unlocks>,
    mut next: ResMut<NextState<AppState>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::Paused);
    }
    if keys.just_pressed(KeyCode::KeyT) && unlocks.research {
        next.set(AppState::SkillTree);
    }
}
