//! Drops: experience, scrap, cores, health and gear.
//!
//! Everything an enemy leaves behind is routed through one death handler so the
//! threat dial's reward multiplier can be applied in exactly one place.

use bevy::prelude::*;

use crate::allies::Economy;
use crate::art::{GameArt, Glow};
use crate::common::{
    Altitude, Body, BurstEvent, DeathEvent, Doomed, FloatingTextEvent, Health, RunEntity, SfxEvent,
    ShakeEvent, Spin, to_world,
};
use crate::enemy::{Director, Enemy, Rank};
use crate::palette as pal;
use crate::player::{Player, PlayerStats};
use crate::progress::{Equipped, Progression, RecomputeStats, roll_gear};
use crate::rng::Rng;
use crate::threat::{RunClock, Threat, WaveCycle};
use crate::{AppState, GameSet};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PickupKind {
    Xp,
    BigXp,
    Scrap,
    Core,
    Health,
    Gear,
}

#[derive(Debug, Component)]
pub struct Pickup {
    pub kind: PickupKind,
    pub value: f32,
    /// Small delay before magnetism engages, so drops visibly pop out first.
    pub settle: f32,
    pub attracted: bool,
}

#[derive(Debug)]
pub struct PickupPlugin;

impl Plugin for PickupPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (handle_deaths, magnetise, collect)
                .chain()
                .in_set(GameSet::Resolve),
        );
    }
}

/// Everything that happens when something dies: drops, bookkeeping, feedback.
#[allow(clippy::too_many_arguments)]
fn handle_deaths(
    mut commands: Commands,
    art: Res<GameArt>,
    stats: Res<PlayerStats>,
    // Taken mutably once: this system both reads the reward multiplier and
    // feeds the kill streak back into it.
    mut threat: ResMut<Threat>,
    cycle: Res<WaveCycle>,
    mut clock: ResMut<RunClock>,
    mut director: ResMut<Director>,
    mut rng: ResMut<Rng>,
    mut deaths: MessageReader<DeathEvent>,
    enemies: Query<&Enemy>,
    mut bursts: MessageWriter<BurstEvent>,
    mut sfx: MessageWriter<SfxEvent>,
    mut shakes: MessageWriter<ShakeEvent>,
    mut next_state: ResMut<NextState<AppState>>,
    mut records: MessageWriter<crate::stats::Record>,
    players: Query<Entity, With<Player>>,
) {
    // One multiplier, computed once, applied to every reward this frame.
    let reward = threat.reward_mult() * cycle.reward_mult();

    for death in deaths.read() {
        // The player dying ends the run and nothing else.
        if players.contains(death.entity) {
            next_state.set(AppState::GameOver);
            sfx.write(SfxEvent::new(crate::audio::Sfx::Death));
            shakes.write(ShakeEvent { amount: 1.2 });
            continue;
        }

        let Ok(enemy) = enemies.get(death.entity) else {
            // Allies and structures die quietly; only enemies pay out.
            commands.entity(death.entity).try_insert(Doomed);
            continue;
        };

        director.alive = director.alive.saturating_sub(1);
        clock.kills += 1;
        threat.note_kill();

        let is_boss = enemy.rank == Rank::Boss;
        let is_elite = enemy.rank == Rank::Elite;

        records.write(crate::stats::Record::add(crate::stats::stat::KILLS, 1.0));
        if is_boss {
            records.write(crate::stats::Record::add(crate::stats::stat::BOSSES, 1.0));
        }
        if is_elite {
            records.write(crate::stats::Record::add(crate::stats::stat::ELITES, 1.0));
        }

        // -- experience -----------------------------------------------------
        let xp_total = enemy.xp * reward * stats.xp_mult;
        let orbs = if is_boss {
            14
        } else if is_elite {
            4
        } else {
            1
        };
        for _ in 0..orbs {
            spawn_pickup(
                &mut commands,
                &art,
                death.pos + rng.in_disc(if is_boss { 2.5 } else { 0.6 }).truncate(),
                if is_boss || is_elite {
                    PickupKind::BigXp
                } else {
                    PickupKind::Xp
                },
                xp_total / orbs as f32,
                &mut rng,
            );
        }

        // -- scrap ----------------------------------------------------------
        let scrap_chance = if is_boss {
            1.0
        } else if is_elite {
            0.9
        } else {
            0.22
        };
        if rng.chance(scrap_chance) {
            let n = if is_boss { 8 } else { 1 };
            for _ in 0..n {
                spawn_pickup(
                    &mut commands,
                    &art,
                    death.pos + rng.in_disc(1.4).truncate(),
                    PickupKind::Scrap,
                    (3.0 + enemy.xp * 0.4) * reward * stats.scrap_mult,
                    &mut rng,
                );
            }
        }

        // -- cores: only from meaningful kills ------------------------------
        if is_boss || is_elite || rng.chance(0.015) {
            let n = if is_boss { 5 } else { 1 };
            for _ in 0..n {
                spawn_pickup(
                    &mut commands,
                    &art,
                    death.pos + rng.in_disc(1.2).truncate(),
                    PickupKind::Core,
                    1.0 * stats.core_mult,
                    &mut rng,
                );
            }
        }

        // -- health ---------------------------------------------------------
        if rng.chance(0.02 + if is_elite { 0.15 } else { 0.0 }) {
            spawn_pickup(
                &mut commands,
                &art,
                death.pos,
                PickupKind::Health,
                stats.max_hp * 0.18,
                &mut rng,
            );
        }

        // -- gear -----------------------------------------------------------
        if is_boss || (is_elite && rng.chance(0.35)) {
            spawn_pickup(
                &mut commands,
                &art,
                death.pos + rng.in_disc(1.0).truncate(),
                PickupKind::Gear,
                0.0,
                &mut rng,
            );
        }

        // -- feedback --------------------------------------------------------
        bursts.write(BurstEvent {
            pos: death.pos,
            height: 0.5,
            color: if is_boss {
                pal::BOSS_TRIM
            } else if is_elite {
                pal::ELITE_TRIM
            } else {
                pal::DUST_GREY
            },
            count: if is_boss { 60 } else { 8 },
            speed: if is_boss { 14.0 } else { 5.0 },
            size: if is_boss { 1.6 } else { 0.6 },
        });

        if is_boss {
            shakes.write(ShakeEvent { amount: 0.9 });
            sfx.write(SfxEvent::new(crate::audio::Sfx::BossDown));
        } else if is_elite {
            shakes.write(ShakeEvent { amount: 0.2 });
            sfx.write(SfxEvent::at(crate::audio::Sfx::Kill, 0.9));
        } else {
            sfx.write(SfxEvent::at(crate::audio::Sfx::Kill, 0.35));
        }

        commands.entity(death.entity).try_insert(Doomed);
    }
}

fn spawn_pickup(
    commands: &mut Commands,
    art: &GameArt,
    pos: Vec2,
    kind: PickupKind,
    value: f32,
    rng: &mut Rng,
) {
    let (mesh, glow, scale) = match kind {
        PickupKind::Xp => (art.xp_orb.clone(), Glow::Xp, 0.7),
        PickupKind::BigXp => (art.xp_gem.clone(), Glow::Xp, 1.1),
        PickupKind::Scrap => (art.scrap.clone(), Glow::Scrap, 1.0),
        PickupKind::Core => (art.xp_gem.clone(), Glow::Plasma, 1.0),
        PickupKind::Health => (art.heart.clone(), Glow::Heal, 1.0),
        PickupKind::Gear => (art.crate_mesh.clone(), Glow::Gear, 1.2),
    };

    commands.spawn((
        Pickup {
            kind,
            value,
            settle: 0.35,
            attracted: false,
        },
        Body::new(pos, 0.4),
        Altitude {
            y: 0.5,
            vy: rng.range(1.5, 3.0),
            gravity: 14.0,
        },
        Spin {
            speed: rng.range(1.4, 3.0),
            axis: Vec3::Y,
        },
        Mesh3d(mesh),
        MeshMaterial3d(art.glow(glow)),
        Transform::from_translation(to_world(pos, 0.5)).with_scale(Vec3::splat(scale)),
        RunEntity,
    ));
}

/// Drops arc out, land, then home in once the player is close enough.
fn magnetise(
    time: Res<Time>,
    stats: Res<PlayerStats>,
    player: Query<&Body, With<Player>>,
    mut pickups: Query<
        (&mut Pickup, &mut Body, &mut Altitude, &Spin, &mut Transform),
        Without<Player>,
    >,
) {
    let dt = time.delta_secs();
    let Some(player_body) = player.iter().next() else {
        return;
    };

    let radius_sq = stats.pickup_radius * stats.pickup_radius;

    for (mut pickup, mut body, mut alt, spin, mut transform) in &mut pickups {
        pickup.settle = (pickup.settle - dt).max(0.0);

        // Ballistic pop-out.
        if alt.y > 0.0 || alt.vy > 0.0 {
            alt.vy -= alt.gravity * dt;
            alt.y += alt.vy * dt;
            if alt.y <= 0.28 {
                alt.y = 0.28;
                alt.vy = 0.0;
            }
        }

        let to_player = player_body.pos - body.pos;
        if pickup.settle <= 0.0 && (pickup.attracted || to_player.length_squared() <= radius_sq) {
            pickup.attracted = true;
            // Accelerating attraction feels far better than constant speed.
            let dist = to_player.length().max(0.001);
            let speed = 9.0 + (stats.pickup_radius - dist).max(0.0) * 4.5;
            body.pos += to_player / dist * speed * dt;
        }

        transform.translation = to_world(body.pos, alt.y);
        transform.rotate_axis(Dir3::new(spin.axis).unwrap_or(Dir3::Y), spin.speed * dt);
    }
}

#[allow(clippy::too_many_arguments)]
fn collect(
    mut commands: Commands,
    mut progression: ResMut<Progression>,
    mut economy: ResMut<Economy>,
    mut equipped: ResMut<Equipped>,
    stats: Res<PlayerStats>,
    threat: Res<Threat>,
    mut rng: ResMut<Rng>,
    mut player: Query<(&Body, &mut Health), With<Player>>,
    pickups: Query<(Entity, &Pickup, &Body), Without<Player>>,
    mut floats: MessageWriter<FloatingTextEvent>,
    mut sfx: MessageWriter<SfxEvent>,
    mut recompute: MessageWriter<RecomputeStats>,
) {
    let Ok((player_body, mut health)) = player.single_mut() else {
        return;
    };

    for (entity, pickup, body) in &pickups {
        let reach = player_body.radius + body.radius + 0.3;
        if player_body.pos.distance_squared(body.pos) > reach * reach {
            continue;
        }

        match pickup.kind {
            PickupKind::Xp | PickupKind::BigXp => {
                progression.gain(pickup.value);
                sfx.write(SfxEvent::at(crate::audio::Sfx::Pickup, 0.25));
            }
            PickupKind::Scrap => {
                economy.gain_scrap(pickup.value);
                sfx.write(SfxEvent::at(crate::audio::Sfx::Pickup, 0.3));
            }
            PickupKind::Core => {
                economy.gain_cores(pickup.value);
                floats.write(FloatingTextEvent {
                    pos: body.pos,
                    height: 1.4,
                    text: "+CORE".into(),
                    color: pal::SCREEN_GLOW,
                    size: 19.0,
                });
                sfx.write(SfxEvent::new(crate::audio::Sfx::Core));
            }
            PickupKind::Health => {
                health.heal(pickup.value);
                floats.write(FloatingTextEvent {
                    pos: body.pos,
                    height: 1.4,
                    text: format!("+{}", pickup.value.round() as i32),
                    color: pal::HEAL_RED,
                    size: 20.0,
                });
                sfx.write(SfxEvent::new(crate::audio::Sfx::Heal));
            }
            PickupKind::Gear => {
                let piece = roll_gear(&mut rng, stats.luck, threat.rarity_bonus());
                let better = equipped
                    .get(piece.slot)
                    .is_none_or(|cur| piece.score() >= cur.score());

                if better {
                    floats.write(FloatingTextEvent {
                        pos: body.pos,
                        height: 1.8,
                        text: format!("{} EQUIPPED", piece.name.to_uppercase()),
                        color: pal::RARITY[piece.rarity],
                        size: 21.0,
                    });
                    // Equip is automatic and always an upgrade, so gear never
                    // becomes an inventory-management chore mid-fight.
                    equipped.set(piece);
                    recompute.write(RecomputeStats);
                    sfx.write(SfxEvent::new(crate::audio::Sfx::Gear));
                } else {
                    // Salvage the dud rather than dropping it on the floor.
                    economy.gain_scrap(35.0);
                    floats.write(FloatingTextEvent {
                        pos: body.pos,
                        height: 1.6,
                        text: "SALVAGED +35".into(),
                        color: pal::METAL,
                        size: 18.0,
                    });
                    sfx.write(SfxEvent::at(crate::audio::Sfx::Pickup, 0.5));
                }
            }
        }

        commands.entity(entity).try_insert(Doomed);
    }
}
