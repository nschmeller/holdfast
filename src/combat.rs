//! Damage, projectiles, hazards, and the broad-phase everything queries.

use bevy::prelude::*;

use crate::arena::{ArenaBounds, Hazard, HazardKind, ObstacleField};
use crate::art::{GameArt, Glow};
use crate::common::{
    Altitude, Body, BurstEvent, DamageEvent, DamageSource, DeathEvent, Doomed, Ephemeral,
    FloatingTextEvent, Health, RunEntity, SfxEvent, ShakeEvent, VisualScale, damp, damp_vec2,
    to_world, yaw_towards,
};
use crate::enemy::{Enemy, StatusEffects};
use crate::player::{Player, PlayerStats};
use crate::rng::Rng;
use crate::{AppState, GameSet};

/// Marks anything enemies are willing to attack.
#[derive(Debug, Component)]
pub struct Damageable {
    pub hostile_target: bool,
}

/// Non-player actors that integrate through the shared movement pass.
#[derive(Debug, Component)]
pub struct Actor {
    /// Whether the actor is stopped by obstacles. Flyers are not.
    pub collides: bool,
    /// Whether the actor is confined to the arena. Enemies are not, so they
    /// can be knocked off the edge.
    pub confined: bool,
}

impl Default for Actor {
    fn default() -> Self {
        Self {
            collides: true,
            confined: true,
        }
    }
}

// -- broad phase ------------------------------------------------------------

/// A uniform grid over enemy positions, rebuilt once per frame.
///
/// Weapons, turrets and allies all need "what is near this point" many times
/// per frame; doing that against a flat list is the one thing that would
/// actually fall over at the enemy counts this game reaches.
#[derive(Debug, Resource)]
pub struct EnemyGrid {
    cell: f32,
    cols: usize,
    rows: usize,
    min: Vec2,
    buckets: Vec<Vec<GridEntry>>,
}

#[derive(Debug, Clone, Copy)]
pub struct GridEntry {
    pub entity: Entity,
    pub pos: Vec2,
    pub radius: f32,
    pub is_boss: bool,
}

impl Default for EnemyGrid {
    fn default() -> Self {
        Self {
            cell: 4.0,
            cols: 0,
            rows: 0,
            min: Vec2::ZERO,
            buckets: Vec::new(),
        }
    }
}

impl EnemyGrid {
    fn rebuild(&mut self, bounds: ArenaBounds) {
        self.min = Vec2::new(-bounds.half_x - 8.0, -bounds.half_z - 8.0);
        let span = Vec2::new(bounds.half_x + 8.0, bounds.half_z + 8.0) - self.min;
        self.cols = (span.x / self.cell).ceil() as usize + 1;
        self.rows = (span.y / self.cell).ceil() as usize + 1;
        let needed = self.cols * self.rows;
        if self.buckets.len() == needed {
            for b in &mut self.buckets {
                b.clear();
            }
        } else {
            self.buckets = vec![Vec::new(); needed];
        }
    }

    fn index(&self, pos: Vec2) -> Option<usize> {
        let rel = (pos - self.min) / self.cell;
        if rel.x < 0.0 || rel.y < 0.0 {
            return None;
        }
        let (cx, cy) = (rel.x as usize, rel.y as usize);
        if cx >= self.cols || cy >= self.rows {
            return None;
        }
        Some(cy * self.cols + cx)
    }

    fn insert(&mut self, entry: GridEntry) {
        if let Some(i) = self.index(entry.pos) {
            self.buckets[i].push(entry);
        }
    }

    /// Visit every enemy whose centre lies within `radius` of `pos`.
    pub fn for_each_near(&self, pos: Vec2, radius: f32, mut f: impl FnMut(&GridEntry)) {
        let Some(centre) = self.index(pos) else {
            return;
        };
        let reach = (radius / self.cell).ceil() as usize + 1;
        let (cx, cy) = (centre % self.cols, centre / self.cols);

        // Clamping the window in unsigned space avoids the signed round-trip
        // entirely; the saturating_sub is the whole edge case.
        let x0 = cx.saturating_sub(reach);
        let x1 = (cx + reach).min(self.cols - 1);
        let y0 = cy.saturating_sub(reach);
        let y1 = (cy + reach).min(self.rows - 1);
        let r_sq = radius * radius;

        for y in y0..=y1 {
            for x in x0..=x1 {
                for e in &self.buckets[y * self.cols + x] {
                    if e.pos.distance_squared(pos) <= r_sq {
                        f(e);
                    }
                }
            }
        }
    }

    /// Nearest enemy to `pos` within `radius`, if any.
    pub fn nearest(&self, pos: Vec2, radius: f32) -> Option<GridEntry> {
        let mut best: Option<(f32, GridEntry)> = None;
        self.for_each_near(pos, radius, |e| {
            let d = e.pos.distance_squared(pos);
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, *e));
            }
        });
        best.map(|(_, e)| e)
    }

    /// Prefers bosses, then whatever is closest. Turrets and the laser use this
    /// so high-value targets do not get ignored in favour of chaff.
    pub fn best_target(&self, pos: Vec2, radius: f32) -> Option<GridEntry> {
        let mut best: Option<(bool, f32, GridEntry)> = None;
        self.for_each_near(pos, radius, |e| {
            let d = e.pos.distance_squared(pos);
            let better = match best {
                None => true,
                Some((was_boss, bd, _)) => match (e.is_boss, was_boss) {
                    (true, false) => true,
                    (false, true) => false,
                    _ => d < bd,
                },
            };
            if better {
                best = Some((e.is_boss, d, *e));
            }
        });
        best.map(|(_, _, e)| e)
    }
}

// -- projectiles ------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShotVisual {
    Dart,
    Staple,
    Tack,
    Band,
    Pellet,
    Plasma,
    Beam,
}

impl ShotVisual {
    fn mesh(self, art: &GameArt) -> Handle<Mesh> {
        match self {
            Self::Dart => art.dart.clone(),
            Self::Staple => art.staple.clone(),
            Self::Tack => art.tack.clone(),
            Self::Band => art.band.clone(),
            Self::Pellet | Self::Plasma => art.pellet.clone(),
            Self::Beam => art.beam_seg.clone(),
        }
    }

    fn material(self, art: &GameArt, friendly: bool) -> Handle<StandardMaterial> {
        match self {
            Self::Dart | Self::Staple | Self::Tack | Self::Band => {
                if friendly {
                    art.solid.clone()
                } else {
                    art.glow(Glow::EnemyShot)
                }
            }
            Self::Plasma => art.glow(Glow::Plasma),
            Self::Beam => art.glow(Glow::Beam),
            Self::Pellet => art.glow(if friendly {
                Glow::PlayerShot
            } else {
                Glow::EnemyShot
            }),
        }
    }
}

#[derive(Debug, Component)]
pub struct Projectile {
    pub vel: Vec2,
    pub damage: f32,
    pub friendly: bool,
    pub crit: bool,
    pub life: f32,
    /// Remaining extra targets after the first.
    pub pierce: i32,
    /// Remaining wall bounces.
    pub bounces: i32,
    pub knockback: f32,
    /// Splash radius on impact; zero for single-target.
    pub aoe: f32,
    pub slow: f32,
    pub burn: f32,
    pub radius: f32,
    /// Entities already hit, so a piercing shot does not tick the same target
    /// every frame it overlaps.
    pub hit: Vec<Entity>,
    pub spin: bool,
}

/// Request a projectile. A message rather than a direct spawn so weapons,
/// enemies, allies and turrets all funnel through one construction path.
#[derive(Debug, Message, Clone)]
pub struct SpawnShot {
    pub pos: Vec2,
    pub dir: Vec2,
    pub speed: f32,
    pub damage: f32,
    pub friendly: bool,
    pub crit: bool,
    pub visual: ShotVisual,
    pub life: f32,
    pub pierce: i32,
    pub bounces: i32,
    pub knockback: f32,
    pub aoe: f32,
    pub slow: f32,
    pub burn: f32,
    pub radius: f32,
    pub height: f32,
    pub scale: f32,
    pub spin: bool,
}

impl SpawnShot {
    pub fn friendly(pos: Vec2, dir: Vec2, speed: f32, damage: f32, visual: ShotVisual) -> Self {
        Self {
            pos,
            dir: dir.normalize_or_zero(),
            speed,
            damage,
            friendly: true,
            crit: false,
            visual,
            life: 2.6,
            pierce: 0,
            bounces: 0,
            knockback: 1.0,
            aoe: 0.0,
            slow: 0.0,
            burn: 0.0,
            radius: 0.32,
            height: 0.55,
            scale: 1.0,
            spin: false,
        }
    }

    pub fn enemy(pos: Vec2, dir: Vec2, speed: f32, damage: f32, visual: ShotVisual) -> Self {
        Self {
            friendly: false,
            ..Self::friendly(pos, dir, speed, damage, visual)
        }
    }
}

#[derive(Debug, Message, Clone, Copy)]
pub struct SpawnHazard {
    pub pos: Vec2,
    pub radius: f32,
    pub dps: f32,
    pub life: f32,
    pub kind: HazardKind,
    pub hurts_player: bool,
    pub hurts_enemies: bool,
}

#[derive(Debug)]
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EnemyGrid>()
            .add_message::<SpawnShot>()
            .add_message::<SpawnHazard>()
            .add_message::<DamageEvent>()
            .add_message::<DeathEvent>()
            .add_message::<ShakeEvent>()
            .add_message::<FloatingTextEvent>()
            .add_message::<BurstEvent>()
            .add_message::<SfxEvent>()
            .add_systems(Update, rebuild_grid.in_set(GameSet::Input))
            .add_systems(
                Update,
                (integrate_actors, separate_bodies)
                    .chain()
                    .in_set(GameSet::Move),
            )
            .add_systems(
                Update,
                (
                    spawn_shots,
                    spawn_hazards,
                    move_projectiles,
                    hazard_ticks,
                    apply_damage,
                )
                    .chain()
                    .in_set(GameSet::Combat),
            )
            .add_systems(Update, expire_hazards.in_set(GameSet::Resolve))
            .add_systems(Update, reap_doomed.in_set(GameSet::Reap))
            .add_systems(Update, sync_transforms.in_set(GameSet::Present))
            // Overlays freeze gameplay but the world should still be drawn in
            // the right place, so transform sync also runs outside Playing.
            .add_systems(
                PostUpdate,
                sync_transforms.run_if(not(in_state(AppState::Playing))),
            );
    }
}

fn rebuild_grid(
    bounds: Res<ArenaBounds>,
    mut grid: ResMut<EnemyGrid>,
    q: Query<(Entity, &Body, &Enemy)>,
) {
    grid.rebuild(*bounds);
    for (entity, body, enemy) in &q {
        grid.insert(GridEntry {
            entity,
            pos: body.pos,
            radius: body.radius,
            is_boss: enemy.kind.is_boss(),
        });
    }
}

/// Shared movement for every non-player actor.
fn integrate_actors(
    time: Res<Time>,
    bounds: Res<ArenaBounds>,
    obstacles: Res<ObstacleField>,
    mut q: Query<(&mut Body, &Actor), Without<Player>>,
) {
    let dt = time.delta_secs();
    for (mut body, actor) in &mut q {
        let step = (body.vel + body.impulse) * dt;
        body.pos += step;
        body.impulse = damp_vec2(body.impulse, Vec2::ZERO, 7.0, dt);

        if actor.collides {
            let r = body.radius;
            body.pos = obstacles.resolve(body.pos, r);
        }
        if actor.confined {
            let r = body.radius;
            body.pos = bounds.clamp(body.pos, r);
        }
    }
}

/// Most an overlap can move a body in one frame.
///
/// Separation is a relaxation, not a constraint solver: capping the step keeps
/// a deep overlap - two things spawned on the same tile - from flinging one of
/// them across the arena, at the cost of taking a few frames to resolve.
const MAX_SEPARATION_STEP: f32 = 0.9;

/// How far apart to look for neighbours, beyond the body's own radius. Larger
/// than any enemy so nothing is missed, small enough to stay in nearby cells.
const NEIGHBOUR_REACH: f32 = 3.0;

/// The displacement that pushes a body at `pos` clear of one at `other`.
///
/// `share` is how much of the overlap this body absorbs: 1.0 when the other
/// body will not move at all, 0.5 when both are pushing each other apart.
/// Returns zero when they are not touching.
fn separation(
    pos: Vec2,
    radius: f32,
    other: Vec2,
    other_radius: f32,
    share: f32,
    tie: u64,
) -> Vec2 {
    let delta = pos - other;
    let distance = delta.length();
    let overlap = radius + other_radius - distance;
    if overlap <= 0.0 {
        return Vec2::ZERO;
    }
    // Exactly coincident centres have no direction to escape along, which
    // happens whenever two things spawn on the same point. Pick one from the
    // entity's own bits so the choice is stable frame to frame and two stacked
    // bodies do not pick the same way out and stay stacked.
    let away = if distance > 1e-4 {
        delta / distance
    } else {
        let angle = (tie % 628) as f32 * 0.01;
        Vec2::new(angle.cos(), angle.sin())
    };
    away * (overlap * share).min(MAX_SEPARATION_STEP)
}

/// Keep bodies out of each other's space.
///
/// Without this a swarm converges to a single point on top of the hero and
/// reads as one enemy with a great deal of health - and the player cannot see
/// what is hitting them. Enemies absorb the whole overlap against the player
/// rather than sharing it: being shoved around by contact would take control
/// away from the player in a game that is otherwise about deciding things.
fn separate_bodies(
    grid: Res<EnemyGrid>,
    obstacles: Res<ObstacleField>,
    player: Query<&Body, With<Player>>,
    mut enemies: Query<(Entity, &mut Body, &Enemy), Without<Player>>,
) {
    let hero = player.iter().next().map(|body| (body.pos, body.radius));

    for (entity, mut body, enemy) in &mut enemies {
        let (pos, radius) = (body.pos, body.radius);
        let tie = u64::from(entity.index().index());
        let flies = enemy.kind.flies();
        let mut push = Vec2::ZERO;

        // Flyers are drawn above the fight, so sharing ground with the hero
        // and with walkers is correct for them.
        if let Some((hero_pos, hero_radius)) = hero
            && !flies
        {
            push += separation(pos, radius, hero_pos, hero_radius, 1.0, tie);
        }

        grid.for_each_near(pos, radius + NEIGHBOUR_REACH, |other| {
            if other.entity == entity {
                return;
            }
            push += separation(pos, radius, other.pos, other.radius, 0.5, tie);
        });

        if push == Vec2::ZERO {
            continue;
        }
        body.pos = pos + push.clamp_length_max(MAX_SEPARATION_STEP);
        // Being pushed clear of a crowd must not push anything into a wall.
        if !flies {
            body.pos = obstacles.resolve(body.pos, radius);
        }
    }
}

fn spawn_shots(mut commands: Commands, art: Res<GameArt>, mut shots: MessageReader<SpawnShot>) {
    for s in shots.read() {
        let yaw = yaw_towards(s.dir);
        commands.spawn((
            Projectile {
                vel: s.dir * s.speed,
                damage: s.damage,
                friendly: s.friendly,
                crit: s.crit,
                life: s.life,
                pierce: s.pierce,
                bounces: s.bounces,
                knockback: s.knockback,
                aoe: s.aoe,
                slow: s.slow,
                burn: s.burn,
                radius: s.radius,
                hit: Vec::new(),
                spin: s.spin,
            },
            Body::new(s.pos, s.radius),
            Mesh3d(s.visual.mesh(&art)),
            MeshMaterial3d(s.visual.material(&art, s.friendly)),
            Transform::from_translation(to_world(s.pos, s.height))
                .with_rotation(Quat::from_rotation_y(yaw))
                .with_scale(Vec3::splat(s.scale)),
            RunEntity,
        ));
    }
}

fn spawn_hazards(
    mut commands: Commands,
    art: Res<GameArt>,
    mut requests: MessageReader<SpawnHazard>,
) {
    for h in requests.read() {
        let tint = match h.kind {
            HazardKind::Scald => Glow::Warning,
            HazardKind::Sticky => Glow::Scrap,
            HazardKind::Shock => Glow::Plasma,
            HazardKind::Font => Glow::ZoneHeld,
        };
        commands.spawn((
            Hazard {
                kind: h.kind,
                radius: h.radius,
                dps: h.dps,
                slow: if h.kind == HazardKind::Sticky {
                    0.5
                } else {
                    1.0
                },
                life: Some(h.life),
                hurts_player: h.hurts_player,
                hurts_enemies: h.hurts_enemies,
            },
            Body::new(h.pos, h.radius),
            Ephemeral::new(h.life),
            Mesh3d(art.disc.clone()),
            MeshMaterial3d(art.glow(tint)),
            Transform::from_translation(to_world(h.pos, 0.04))
                .with_scale(Vec3::new(h.radius, 1.0, h.radius)),
            RunEntity,
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn move_projectiles(
    time: Res<Time>,
    bounds: Res<ArenaBounds>,
    obstacles: Res<ObstacleField>,
    grid: Res<EnemyGrid>,
    mut commands: Commands,
    mut projectiles: Query<(Entity, &mut Projectile, &mut Body, &mut Transform)>,
    targets: Query<(Entity, &Body, &Damageable), Without<Projectile>>,
    mut damage: MessageWriter<DamageEvent>,
    mut bursts: MessageWriter<BurstEvent>,
) {
    let dt = time.delta_secs();

    for (entity, mut proj, mut body, mut transform) in &mut projectiles {
        proj.life -= dt;
        if proj.life <= 0.0 {
            commands.entity(entity).try_insert(Doomed);
            continue;
        }

        let prev = body.pos;
        let step = proj.vel * dt;
        body.pos += step;

        // Walls: bounce if the shot has bounces left, otherwise expire.
        let hit_wall = !bounds.contains(body.pos) || obstacles.blocks_segment(prev, body.pos, 0.55);
        if hit_wall {
            if proj.bounces > 0 {
                proj.bounces -= 1;
                // Reflect off whichever axis was crossed. Approximate, but for
                // a bouncing rubber band "approximate" is indistinguishable
                // from correct and costs nothing.
                if body.pos.x.abs() > bounds.half_x {
                    proj.vel.x = -proj.vel.x;
                } else if body.pos.y.abs() > bounds.half_z {
                    proj.vel.y = -proj.vel.y;
                } else {
                    proj.vel = -proj.vel;
                }
                body.pos = prev;
                proj.hit.clear();
            } else {
                commands.entity(entity).try_insert(Doomed);
                continue;
            }
        }

        transform.translation = to_world(body.pos, transform.translation.y);
        if proj.spin {
            transform.rotate_y(dt * 16.0);
        } else {
            transform.rotation = Quat::from_rotation_y(yaw_towards(proj.vel));
        }

        // Collision. Friendly shots use the grid; enemy shots test the much
        // smaller set of friendly targets directly.
        let mut struck: Option<(Entity, Vec2)> = None;
        if proj.friendly {
            let reach = proj.radius + 1.2;
            grid.for_each_near(body.pos, reach, |e| {
                if struck.is_some() || proj.hit.contains(&e.entity) {
                    return;
                }
                let r = proj.radius + e.radius;
                if e.pos.distance_squared(body.pos) <= r * r {
                    struck = Some((e.entity, e.pos));
                }
            });
        } else {
            for (te, tbody, dmg) in &targets {
                if !dmg.hostile_target || proj.hit.contains(&te) {
                    continue;
                }
                let r = proj.radius + tbody.radius;
                if tbody.pos.distance_squared(body.pos) <= r * r {
                    struck = Some((te, tbody.pos));
                    break;
                }
            }
        }

        let Some((target, target_pos)) = struck else {
            continue;
        };

        proj.hit.push(target);
        let dir = (target_pos - prev).normalize_or_zero();

        damage.write(DamageEvent {
            target,
            amount: proj.damage,
            crit: proj.crit,
            knockback: dir,
            knockback_force: proj.knockback * 8.0,
            source: if proj.friendly {
                DamageSource::Player
            } else {
                DamageSource::Enemy
            },
        });

        // Splash.
        if proj.aoe > 0.0 && proj.friendly {
            let aoe = proj.aoe;
            let dmg = proj.damage * 0.65;
            let centre = body.pos;
            grid.for_each_near(centre, aoe, |e| {
                if e.entity == target {
                    return;
                }
                damage.write(DamageEvent {
                    target: e.entity,
                    amount: dmg,
                    crit: false,
                    knockback: (e.pos - centre).normalize_or_zero(),
                    knockback_force: proj.knockback * 5.0,
                    source: DamageSource::Player,
                });
            });
            bursts.write(BurstEvent {
                pos: centre,
                height: 0.5,
                color: crate::palette::ACCENT,
                count: 10,
                speed: 5.0,
                size: 0.9,
            });
        }

        if proj.pierce > 0 {
            proj.pierce -= 1;
        } else {
            commands.entity(entity).try_insert(Doomed);
        }
    }
}

/// Status application from projectiles is separate from the damage event so
/// slow/burn land even on targets that survive.
fn hazard_ticks(
    time: Res<Time>,
    hazards: Query<(&Hazard, &Body)>,
    mut enemies: Query<(Entity, &Body, &mut StatusEffects), With<Enemy>>,
    mut healths: Query<&mut Health, With<Enemy>>,
    mut damage: MessageWriter<DamageEvent>,
) {
    let dt = time.delta_secs();
    for (entity, body, mut status) in &mut enemies {
        for (hazard, hbody) in &hazards {
            if !hazard.hurts_enemies {
                continue;
            }
            let reach = hazard.radius + body.radius;
            if body.pos.distance_squared(hbody.pos) > reach * reach {
                continue;
            }
            if hazard.dps > 0.0 {
                damage.write(DamageEvent {
                    target: entity,
                    amount: hazard.dps * dt,
                    crit: false,
                    knockback: Vec2::ZERO,
                    knockback_force: 0.0,
                    source: DamageSource::Hazard,
                });
            } else if hazard.dps < 0.0 {
                // Ley lines heal the enemy too. That is the whole point of
                // contesting them.
                if let Ok(mut health) = healths.get_mut(entity) {
                    health.heal(-hazard.dps * dt);
                }
            }
            if hazard.slow < 1.0 {
                status.apply_slow(1.0 - hazard.slow, 0.25);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_damage(
    mut events: MessageReader<DamageEvent>,
    stats: Res<PlayerStats>,
    mut rng: ResMut<Rng>,
    mut targets: Query<(
        &mut Health,
        &mut Body,
        Option<&Player>,
        Option<&mut VisualScale>,
    )>,
    mut deaths: MessageWriter<DeathEvent>,
    mut floats: MessageWriter<FloatingTextEvent>,
    mut shakes: MessageWriter<ShakeEvent>,
    mut sfx: MessageWriter<SfxEvent>,
) {
    for ev in events.read() {
        let Ok((mut health, mut body, is_player, visual)) = targets.get_mut(ev.target) else {
            continue;
        };
        if health.is_dead() {
            continue;
        }

        let mut amount = ev.amount;

        if is_player.is_some() {
            if health.invuln > 0.0 {
                continue;
            }
            amount = stats.mitigate(amount);
            // A short grace window after every hit, so standing in a crowd is
            // survivable long enough to walk out of it.
            health.invuln = 0.35;
            shakes.write(ShakeEvent {
                amount: (amount / health.max).clamp(0.05, 0.4),
            });
            sfx.write(SfxEvent::new(crate::audio::Sfx::PlayerHurt));
        }

        health.current -= amount;

        if let Some(mut vs) = visual {
            vs.pulse = 1.0;
        }

        // Knockback.
        if ev.knockback_force > 0.0 {
            let force = ev.knockback_force
                * if is_player.is_some() {
                    0.35
                } else {
                    stats.knockback
                };
            body.push(ev.knockback, force);
        }

        // Damage numbers, but only for meaningful hits: a burn tick every frame
        // would bury the screen in noise.
        if ev.source != DamageSource::Hazard && amount >= 1.0 && is_player.is_none() {
            floats.write(FloatingTextEvent {
                pos: body.pos + rng.in_disc(0.3).truncate(),
                height: 1.2,
                text: format!("{}", amount.round() as i32),
                color: if ev.crit {
                    crate::palette::ACCENT
                } else {
                    crate::palette::HUD_TEXT
                },
                size: if ev.crit { 24.0 } else { 17.0 },
            });
        }

        if health.is_dead() {
            deaths.write(DeathEvent {
                entity: ev.target,
                pos: body.pos,
                by_player: ev.source == DamageSource::Player,
            });
        }
    }
}

fn expire_hazards(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Ephemeral, &mut Transform), With<Hazard>>,
) {
    let dt = time.delta_secs();
    for (entity, mut eph, mut transform) in &mut q {
        eph.life -= dt;
        if eph.life <= 0.0 {
            commands.entity(entity).try_insert(Doomed);
            continue;
        }
        // Shrink out rather than popping.
        let t = 1.0 - eph.t();
        let s = transform.scale;
        transform.scale = Vec3::new(s.x, 1.0, s.z) * (0.6 + t * 0.4).max(0.05);
    }
}

fn reap_doomed(mut commands: Commands, q: Query<Entity, With<Doomed>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

/// One place writes `Transform` from the simulation state.
fn sync_transforms(
    time: Res<Time>,
    mut q: Query<
        (
            &Body,
            Option<&Altitude>,
            Option<&mut VisualScale>,
            Option<&crate::player::Facing>,
            &mut Transform,
        ),
        Without<Projectile>,
    >,
) {
    let dt = time.delta_secs();
    for (body, alt, visual, facing, mut transform) in &mut q {
        let y = alt.map_or(0.0, |a| a.y);
        transform.translation = to_world(body.pos, y);

        if let Some(f) = facing {
            transform.rotation = Quat::from_rotation_y(f.yaw);
        } else if body.vel.length_squared() > 0.5 {
            // Everything else just faces where it is going.
            transform.rotation = Quat::from_rotation_y(yaw_towards(body.vel));
        }

        if let Some(mut vs) = visual {
            vs.pulse = damp(vs.pulse, 0.0, 9.0, dt);
            // A quick squash on hit reads at any distance, unlike a colour
            // flash which gets lost against bright ground.
            let squash = 1.0 + vs.pulse * 0.35;
            transform.scale = Vec3::new(
                vs.base * squash,
                vs.base / squash.max(0.01),
                vs.base * squash,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(i: u32) -> Entity {
        Entity::from_raw_u32(i).expect("valid test entity index")
    }

    fn grid_with(points: &[(u32, Vec2)]) -> EnemyGrid {
        let mut grid = EnemyGrid::default();
        grid.rebuild(ArenaBounds {
            half_x: 20.0,
            half_z: 13.0,
        });
        for (i, pos) in points {
            grid.insert(GridEntry {
                entity: entity(*i),
                pos: *pos,
                radius: 0.5,
                is_boss: false,
            });
        }
        grid
    }

    #[test]
    fn an_empty_grid_finds_nothing() {
        let grid = grid_with(&[]);
        assert!(grid.nearest(Vec2::ZERO, 50.0).is_none());
        assert!(grid.best_target(Vec2::ZERO, 50.0).is_none());
    }

    #[test]
    fn nearest_picks_the_closest_entry() {
        let grid = grid_with(&[(1, Vec2::new(5.0, 0.0)), (2, Vec2::new(1.0, 0.0))]);
        assert_eq!(grid.nearest(Vec2::ZERO, 20.0).unwrap().entity, entity(2));
    }

    #[test]
    fn nearest_respects_its_radius() {
        let grid = grid_with(&[(1, Vec2::new(9.0, 0.0))]);
        assert!(grid.nearest(Vec2::ZERO, 5.0).is_none());
        assert!(grid.nearest(Vec2::ZERO, 10.0).is_some());
    }

    #[test]
    fn for_each_near_matches_a_brute_force_scan() {
        // The grid is an optimisation; it must return exactly what a linear
        // scan would, or weapons will silently miss targets near cell edges.
        let mut rng = Rng::seeded(4242);
        let points: Vec<(u32, Vec2)> = (0..400)
            .map(|i| (i, Vec2::new(rng.range(-20.0, 20.0), rng.range(-13.0, 13.0))))
            .collect();
        let grid = grid_with(&points);

        for _ in 0..200 {
            let origin = Vec2::new(rng.range(-20.0, 20.0), rng.range(-13.0, 13.0));
            let radius = rng.range(0.5, 14.0);

            let mut expected: Vec<Entity> = points
                .iter()
                .filter(|(_, p)| p.distance(origin) <= radius)
                .map(|(i, _)| entity(*i))
                .collect();
            expected.sort_unstable();

            let mut found = Vec::new();
            grid.for_each_near(origin, radius, |e| found.push(e.entity));
            found.sort_unstable();

            assert_eq!(found, expected, "origin {origin:?} radius {radius}");
        }
    }

    #[test]
    fn queries_far_outside_the_grid_are_safe() {
        let grid = grid_with(&[(1, Vec2::ZERO)]);
        // Must not panic or index out of bounds.
        assert!(grid.nearest(Vec2::new(10_000.0, 10_000.0), 5.0).is_none());
        let mut hits = 0;
        grid.for_each_near(Vec2::new(-9999.0, 0.0), 3.0, |_| hits += 1);
        assert_eq!(hits, 0);
    }

    #[test]
    fn entries_outside_the_grid_are_dropped_not_panicked_on() {
        let mut grid = EnemyGrid::default();
        grid.rebuild(ArenaBounds {
            half_x: 5.0,
            half_z: 5.0,
        });
        grid.insert(GridEntry {
            entity: entity(1),
            pos: Vec2::new(10_000.0, 10_000.0),
            radius: 0.5,
            is_boss: false,
        });
        assert!(grid.nearest(Vec2::ZERO, 100.0).is_none());
    }

    #[test]
    fn best_target_prefers_a_boss_over_closer_chaff() {
        let mut grid = EnemyGrid::default();
        grid.rebuild(ArenaBounds::default());
        grid.insert(GridEntry {
            entity: entity(1),
            pos: Vec2::new(1.0, 0.0),
            radius: 0.5,
            is_boss: false,
        });
        grid.insert(GridEntry {
            entity: entity(2),
            pos: Vec2::new(9.0, 0.0),
            radius: 2.0,
            is_boss: true,
        });
        assert_eq!(
            grid.best_target(Vec2::ZERO, 20.0).unwrap().entity,
            entity(2),
            "the laser should ignore chaff while a boss is in range"
        );
    }

    #[test]
    fn best_target_falls_back_to_nearest_without_a_boss() {
        let grid = grid_with(&[(1, Vec2::new(8.0, 0.0)), (2, Vec2::new(2.0, 0.0))]);
        assert_eq!(
            grid.best_target(Vec2::ZERO, 20.0).unwrap().entity,
            entity(2)
        );
    }

    #[test]
    fn rebuilding_clears_the_previous_frame() {
        let mut grid = grid_with(&[(1, Vec2::ZERO)]);
        assert!(grid.nearest(Vec2::ZERO, 5.0).is_some());
        grid.rebuild(ArenaBounds::default());
        assert!(
            grid.nearest(Vec2::ZERO, 5.0).is_none(),
            "stale entries would let weapons shoot dead enemies"
        );
    }

    #[test]
    fn rebuilding_to_a_different_arena_size_is_safe() {
        let mut grid = EnemyGrid::default();
        grid.rebuild(ArenaBounds {
            half_x: 5.0,
            half_z: 5.0,
        });
        grid.rebuild(ArenaBounds {
            half_x: 40.0,
            half_z: 30.0,
        });
        grid.insert(GridEntry {
            entity: entity(1),
            pos: Vec2::new(35.0, 25.0),
            radius: 0.5,
            is_boss: false,
        });
        assert!(grid.nearest(Vec2::new(35.0, 25.0), 2.0).is_some());
    }

    #[test]
    fn shot_builders_set_the_right_faction() {
        let f = SpawnShot::friendly(Vec2::ZERO, Vec2::X, 10.0, 5.0, ShotVisual::Dart);
        assert!(f.friendly);
        let e = SpawnShot::enemy(Vec2::ZERO, Vec2::X, 10.0, 5.0, ShotVisual::Tack);
        assert!(!e.friendly);
    }

    #[test]
    fn shot_directions_are_normalised() {
        let s = SpawnShot::friendly(
            Vec2::ZERO,
            Vec2::new(30.0, 40.0),
            10.0,
            5.0,
            ShotVisual::Dart,
        );
        assert!((s.dir.length() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn a_zero_direction_shot_does_not_produce_nan() {
        let s = SpawnShot::friendly(Vec2::ZERO, Vec2::ZERO, 10.0, 5.0, ShotVisual::Dart);
        assert!(s.dir.is_finite());
    }

    #[test]
    fn actors_default_to_colliding_and_confined() {
        let a = Actor::default();
        assert!(a.collides);
        assert!(a.confined);
    }
    #[test]
    fn bodies_that_do_not_touch_are_left_alone() {
        let push = separation(Vec2::new(5.0, 0.0), 0.5, Vec2::ZERO, 0.5, 1.0, 3);
        assert_eq!(push, Vec2::ZERO);
    }

    #[test]
    fn a_full_share_pushes_clear_of_the_other_body() {
        // A monster standing on the hero must end up outside them, not merely
        // nudged: overlapping the player is what this whole pass exists to stop.
        let (pos, radius) = (Vec2::new(0.3, 0.0), 0.5);
        let (other, other_radius) = (Vec2::ZERO, 0.6);
        let push = separation(pos, radius, other, other_radius, 1.0, 0);
        let settled = pos + push;
        assert!(
            settled.distance(other) >= radius + other_radius - 1e-4,
            "still overlapping at {settled:?}"
        );
    }

    #[test]
    fn separation_pushes_directly_away() {
        let push = separation(Vec2::new(1.0, 0.0), 1.0, Vec2::ZERO, 1.0, 1.0, 0);
        assert!(push.x > 0.0 && push.y.abs() < 1e-6);
    }

    #[test]
    fn a_half_share_leaves_the_rest_for_the_other_body() {
        // A shallow overlap, so the per-frame cap does not mask the ratio.
        let full = separation(Vec2::new(1.0, 0.0), 0.6, Vec2::ZERO, 0.6, 1.0, 0);
        let half = separation(Vec2::new(1.0, 0.0), 0.6, Vec2::ZERO, 0.6, 0.5, 0);
        assert!((full.length() - 0.2).abs() < 1e-5, "{full:?}");
        assert!((half.length() - full.length() * 0.5).abs() < 1e-5);
    }

    #[test]
    fn coincident_bodies_still_find_a_way_out() {
        // Two things spawned on the same point have no direction to separate
        // along; without a fallback they stay welded together forever.
        let push = separation(Vec2::ZERO, 0.5, Vec2::ZERO, 0.5, 1.0, 17);
        assert!(push.length() > 0.0, "coincident bodies never separated");
    }

    #[test]
    fn coincident_bodies_pick_different_directions_by_identity() {
        let a = separation(Vec2::ZERO, 0.5, Vec2::ZERO, 0.5, 0.5, 3);
        let b = separation(Vec2::ZERO, 0.5, Vec2::ZERO, 0.5, 0.5, 400);
        assert!(
            a.distance(b) > 0.1,
            "two stacked bodies both fled the same way and stayed stacked"
        );
    }

    #[test]
    fn one_frame_of_separation_is_bounded() {
        // A deep overlap resolves over several frames rather than teleporting.
        let push = separation(Vec2::new(0.01, 0.0), 40.0, Vec2::ZERO, 40.0, 1.0, 0);
        assert!(push.length() <= MAX_SEPARATION_STEP + 1e-6, "{push:?}");
    }

    #[test]
    fn repeated_separation_converges_rather_than_oscillating() {
        // Relaxation has to settle: a pair that keeps overshooting would jitter
        // on screen forever.
        let mut a = Vec2::new(0.05, 0.0);
        let mut b = Vec2::new(-0.05, 0.0);
        for _ in 0..64 {
            let pa = separation(a, 0.6, b, 0.6, 0.5, 1);
            let pb = separation(b, 0.6, a, 0.6, 0.5, 2);
            a += pa;
            b += pb;
        }
        assert!(
            a.distance(b) >= 1.2 - 1e-3,
            "settled only {} apart",
            a.distance(b)
        );
    }
}
