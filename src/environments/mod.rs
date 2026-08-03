//! Environments: five worlds that share one rulebook.
//!
//! An environment is pure data. There is no arena - the world is unbounded, and
//! each module below is a function from `(chunk coordinate, seed)` to a
//! [`ChunkContent`]: some props with colliders, some lights, some hazards, and
//! the occasional landmark. `world` streams those chunks in and out around the
//! player.
//!
//! Nothing in here touches the ECS, which means a world can be unit-reasoned
//! about, swapped at runtime, and extended without going near the simulation.
//!
//! Two things are deliberately shared rather than left to each world:
//!
//! - the **look** (sky, sun, ambient, the prevailing wind), which applies once
//!   per run rather than per chunk, and
//! - the **strategic furniture** - territory, forts and spawner nests - which
//!   is placed by [`strategic_features`] so the war plays the same way in every
//!   world even though it looks completely different.

mod arcane;
mod desk;
mod forest;
mod grid;
mod rooftop;

use bevy::prelude::*;

use crate::arena::{ColliderShape, Gust, Hazard, HazardKind, ObstacleField};
use crate::art::{GameArt, Glow};
use crate::common::{Body, to_world};
use crate::rng::Rng;
use crate::world::{CHUNK_SIZE, Chasm, LightPool, WorldSeed, chunk_min};
use crate::{AppState, GameSet, RunSetup};

/// Radius around the world origin kept free of props and hostile furniture.
///
/// The player always starts at the origin. Without this a run can open inside a
/// coffee mug, or twenty feet from a fort.
pub const SPAWN_CLEARANCE: f32 = 7.0;

/// Radius around the origin in which no fort or nest is ever generated.
///
/// Much larger than the prop clearance, and deliberately larger than a fort's
/// assault range: a player who stays near where they landed fights waves and
/// nothing else. Walking out is what finds the war. At 46 units the nearest
/// stronghold was shelling the starting position inside half a minute, which
/// is not an opening, it is an ambush.
pub const HOME_PEACE: f32 = 130.0;

#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub enum EnvKind {
    #[default]
    Desk,
    Forest,
    Rooftop,
    Grid,
    Arcane,
}

impl EnvKind {
    pub const COUNT: usize = 5;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Desk,
        Self::Forest,
        Self::Rooftop,
        Self::Grid,
        Self::Arcane,
    ];

    #[must_use]
    pub fn next(self) -> Self {
        Self::ALL[(self as usize + 1) % Self::COUNT]
    }

    #[must_use]
    pub fn prev(self) -> Self {
        Self::ALL[(self as usize + Self::COUNT - 1) % Self::COUNT]
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Desk => "THE DESK",
            Self::Forest => "THE UNDERGROWTH",
            Self::Rooftop => "BLOCK 9 ROOFTOP",
            Self::Grid => "GRID ZERO",
            Self::Arcane => "THE ARCANE SANCTUM",
        }
    }

    /// Short label for the world selector chips.
    pub fn short_name(self) -> &'static str {
        match self {
            Self::Desk => "DESK",
            Self::Forest => "WILD",
            Self::Rooftop => "CITY",
            Self::Grid => "GRID",
            Self::Arcane => "ARCANE",
        }
    }

    /// The world's signature colour, used for its chip, its detail panel border
    /// and anywhere else the UI needs to say "you are here".
    pub fn accent(self) -> Color {
        match self {
            Self::Desk => Color::srgb(1.0, 0.745, 0.29),
            Self::Forest => Color::srgb(0.494, 0.863, 0.549),
            Self::Rooftop => Color::srgb(1.0, 0.251, 0.588),
            Self::Grid => Color::srgb(0.302, 0.949, 1.0),
            Self::Arcane => Color::srgb(0.682, 0.42, 1.0),
        }
    }

    pub fn tagline(self) -> &'static str {
        match self {
            Self::Desk => "2AM, and the desk goes on forever.",
            Self::Forest => "You are four inches tall and the moss has no edge.",
            Self::Rooftop => "Neon and rust, roof after roof after roof.",
            Self::Grid => "An endless test platform. Something is still testing.",
            Self::Arcane => "A sanctum that broke outward instead of falling down.",
        }
    }

    /// One line on what plays differently here, shown on the select screen.
    pub fn quirk(self) -> &'static str {
        match self {
            Self::Desk => "Tight and cluttered. The fan sweeps a lane every few seconds.",
            Self::Forest => "Open going, but mud slows everything that crosses it.",
            Self::Rooftop => "Long sightlines, and gaps between the roofs to fall down.",
            Self::Grid => "Almost no cover. Plasma conduits burn anything standing on them.",
            Self::Arcane => "Ley lines heal whoever holds them. So the enemy wants them too.",
        }
    }

    /// Lighting and weather, applied once per run rather than per chunk.
    pub fn look(self) -> EnvLook {
        match self {
            Self::Desk => desk::look(),
            Self::Forest => forest::look(),
            Self::Rooftop => rooftop::look(),
            Self::Grid => grid::look(),
            Self::Arcane => arcane::look(),
        }
    }

    /// Everything in one chunk of this world.
    pub fn generate_chunk(self, coord: IVec2, rng: &mut Rng) -> ChunkContent {
        let mut ctx = ChunkCtx::new(coord, rng);
        match self {
            Self::Desk => desk::chunk(&mut ctx),
            Self::Forest => forest::chunk(&mut ctx),
            Self::Rooftop => rooftop::chunk(&mut ctx),
            Self::Grid => grid::chunk(&mut ctx),
            Self::Arcane => arcane::chunk(&mut ctx),
        }
        strategic_features(&mut ctx);
        ctx.finish()
    }

    /// The floor mesh for one chunk, authored around its own centre.
    ///
    /// Takes the world seed rather than an `Rng` because floors must be
    /// *seamless*: neighbouring chunks have to agree about the ground they
    /// share, which means sampling world-space noise rather than rolling dice.
    pub fn chunk_floor(self, coord: IVec2, seed: WorldSeed) -> Mesh {
        let origin = chunk_min(coord) + Vec2::splat(CHUNK_SIZE * 0.5);
        // Truncated deliberately: the floor only needs a stable per-world
        // number to decorrelate its noise fields from any other world's.
        let salt = (seed.0 >> 32) as u32;
        match self {
            Self::Desk => desk::floor(origin, salt),
            Self::Forest => forest::floor(origin, salt),
            Self::Rooftop => rooftop::floor(origin, salt),
            Self::Grid => grid::floor(origin, salt),
            Self::Arcane => arcane::floor(origin, salt),
        }
    }
}

/// How a world is lit and what the weather does. One per run.
#[derive(Debug, Clone)]
pub struct EnvLook {
    pub sky: Color,
    pub ambient: Color,
    pub ambient_brightness: f32,
    pub sun_color: Color,
    pub sun_illuminance: f32,
    /// Direction the sun points, from the light towards the scene.
    pub sun_dir: Vec3,
    pub gust: Gust,
}

/// Which shared material a prop renders with.
#[derive(Debug, Clone, Copy)]
pub enum Surface {
    Solid,
    Matte,
    Metal,
    Glass,
    Glow(Glow),
}

#[derive(Debug)]
pub struct PropSpec {
    pub mesh: Mesh,
    pub pos: Vec2,
    pub y: f32,
    pub rot_y: f32,
    pub surface: Surface,
    /// `None` for decorative props that nothing collides with.
    pub collider: Option<ColliderShape>,
    pub blocks_shots: bool,
    pub height: f32,
}

impl PropSpec {
    pub fn new(mesh: Mesh, pos: Vec2) -> Self {
        Self {
            mesh,
            pos,
            y: 0.0,
            rot_y: 0.0,
            surface: Surface::Solid,
            collider: None,
            blocks_shots: false,
            height: 0.0,
        }
    }

    #[must_use]
    pub fn solid(mut self, shape: ColliderShape, height: f32) -> Self {
        self.collider = Some(shape);
        self.height = height;
        // Anything tall enough to hide behind should also stop shots; the
        // threshold matches roughly waist height on the player model.
        self.blocks_shots = height >= 0.75;
        self
    }

    #[must_use]
    pub fn passthrough(mut self) -> Self {
        self.blocks_shots = false;
        self
    }

    #[must_use]
    pub fn surface(mut self, surface: Surface) -> Self {
        self.surface = surface;
        self
    }

    #[must_use]
    pub fn rot(mut self, degrees: f32) -> Self {
        self.rot_y = degrees.to_radians();
        self
    }

    #[must_use]
    pub fn raised(mut self, y: f32) -> Self {
        self.y = y;
        self
    }
}

#[derive(Debug)]
pub struct LightSpec {
    pub pos: Vec3,
    pub color: Color,
    pub intensity: f32,
    pub range: f32,
    pub shadows: bool,
}

#[derive(Debug)]
pub struct HazardSpec {
    pub pos: Vec2,
    pub radius: f32,
    pub kind: HazardKind,
    pub dps: f32,
    pub slow: f32,
    /// Permanent features pulse on a cycle instead of expiring.
    pub duty: Option<(f32, f32)>,
}

/// Everything one chunk contributes to the world.
///
/// Positions are absolute world coordinates, not chunk-local: a generator that
/// wants to place something is already thinking in world space, and the
/// alternative is an offset applied in a dozen places and forgotten in one.
#[derive(Debug, Default)]
pub struct ChunkContent {
    pub props: Vec<PropSpec>,
    pub lights: Vec<LightSpec>,
    pub hazards: Vec<HazardSpec>,
    /// Territory markers to contest.
    pub zones: Vec<Vec2>,
    /// Standing in one of these trades attention for damage.
    pub light_pools: Vec<LightPool>,
    /// Holes to fall down.
    pub chasms: Vec<Chasm>,
    /// Hostile strongholds.
    pub forts: Vec<Vec2>,
    /// Hostile nests that trickle out monsters.
    pub spawners: Vec<Vec2>,
}

/// Scratch space handed to a world's chunk generator.
///
/// Exists so that each world's generator reads as a list of what is *in* that
/// world, with the arithmetic of chunk bounds, spawn clearance and determinism
/// handled once here instead of five times over.
#[derive(Debug)]
pub struct ChunkCtx<'a> {
    pub coord: IVec2,
    /// South-west corner of this chunk in world space.
    pub min: Vec2,
    pub rng: &'a mut Rng,
    out: ChunkContent,
}

impl<'a> ChunkCtx<'a> {
    fn new(coord: IVec2, rng: &'a mut Rng) -> Self {
        Self {
            coord,
            min: chunk_min(coord),
            rng,
            out: ChunkContent::default(),
        }
    }

    /// Centre of this chunk in world space.
    #[must_use]
    pub fn center(&self) -> Vec2 {
        self.min + Vec2::splat(CHUNK_SIZE * 0.5)
    }

    /// Distance from the world origin to this chunk's centre.
    #[must_use]
    pub fn from_home(&self) -> f32 {
        self.center().length()
    }

    /// A uniformly random point inside the chunk, kept `inset` from its edges.
    pub fn spot(&mut self, inset: f32) -> Vec2 {
        let lo = inset;
        let hi = CHUNK_SIZE - inset;
        self.min + Vec2::new(self.rng.range(lo, hi), self.rng.range(lo, hi))
    }

    /// A per-chunk coin flip. Use for landmarks, so that "one chunk in six has
    /// a water tower" is a single readable line.
    pub fn feature(&mut self, chance: f32) -> bool {
        self.rng.chance(chance)
    }

    pub fn prop(&mut self, prop: PropSpec) -> &mut Self {
        self.out.props.push(prop);
        self
    }

    pub fn light(&mut self, pos: Vec3, color: Color, intensity: f32, range: f32) -> &mut Self {
        self.out.lights.push(LightSpec {
            pos,
            color,
            intensity,
            range,
            shadows: false,
        });
        self
    }

    pub fn hazard(&mut self, hazard: HazardSpec) -> &mut Self {
        self.out.hazards.push(hazard);
        self
    }

    /// A patch of bright ground: more damage dealt, and more attention drawn.
    pub fn pool(&mut self, center: Vec2, radius: f32, damage_bonus: f32) -> &mut Self {
        self.out.light_pools.push(LightPool {
            center,
            radius,
            damage_bonus,
        });
        self
    }

    pub fn chasm(&mut self, center: Vec2, radius: f32) -> &mut Self {
        self.out.chasms.push(Chasm { center, radius });
        self
    }

    /// Drop anything that would bury the player's starting position, then hand
    /// the content over.
    fn finish(self) -> ChunkContent {
        let mut out = self.out;
        // Only the home chunk and its neighbours can possibly reach the origin,
        // so this is a cheap check that almost always passes trivially.
        if self.min.length() < CHUNK_SIZE * 2.0 {
            out.props
                .retain(|p| p.collider.is_none() || p.pos.length() > SPAWN_CLEARANCE);
            out.hazards
                .retain(|h| h.pos.length() > SPAWN_CLEARANCE + h.radius);
            out.chasms
                .retain(|c| c.center.length() > SPAWN_CLEARANCE + c.radius);
        }
        out
    }
}

/// Territory, forts and nests: the same in every world, so the war reads the
/// same wherever it is fought.
///
/// Placement is by chunk rather than by density over an area, which is what
/// keeps it deterministic and streaming-safe - a chunk decides its own contents
/// with no knowledge of its neighbours, and therefore generates identically
/// however the player wanders into it.
fn strategic_features(ctx: &mut ChunkCtx) {
    let from_home = ctx.from_home();

    // Territory is everywhere, including near home: it is the tutorial for the
    // whole holding-ground idea and should be found early.
    if ctx.feature(0.34) {
        let pos = ctx.spot(crate::allies::ZONE_RADIUS + 1.5);
        ctx.out.zones.push(pos);
    }

    if from_home < HOME_PEACE {
        return;
    }

    // Forts thin out near home and are common further out, so walking away from
    // the origin is legibly walking into danger.
    let fort_chance = (0.06 + from_home * 0.0009).min(0.16);
    if ctx.feature(fort_chance) {
        let pos = ctx.spot(6.0);
        ctx.out.forts.push(pos);
        // A fort is born with an escort of nests; it does not start from
        // nothing and slowly bootstrap while the player watches.
        let escort = if ctx.rng.chance(0.5) { 2 } else { 1 };
        for _ in 0..escort {
            let offset = ctx.rng.in_disc(9.0).truncate();
            ctx.out.spawners.push(pos + offset);
        }
    } else if ctx.feature(0.18) {
        // Loose nests between the forts, so the ground between strongholds is
        // not empty.
        let pos = ctx.spot(3.0);
        ctx.out.spawners.push(pos);
    }
}

/// Marker for everything spawned by the current environment's look, so a
/// rebuild is a single despawn query.
#[derive(Debug, Component)]
pub struct EnvEntity;

/// Set when the world's look needs applying (new run, or the player changed
/// world).
#[derive(Debug, Resource, Default)]
pub struct EnvDirty(pub bool);

/// A permanent hazard that cycles on and off, like a steam vent.
#[derive(Debug, Component)]
pub struct PulsingHazard {
    pub period: f32,
    pub on_fraction: f32,
    pub phase: f32,
    pub base_dps: f32,
}

#[derive(Debug)]
pub struct ArenaPlugin;

impl Plugin for ArenaPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EnvKind>()
            .init_resource::<ObstacleField>()
            .init_resource::<EnvDirty>()
            .init_resource::<Gust>()
            .add_systems(OnExit(AppState::Menu), mark_dirty.in_set(RunSetup::Reset))
            .add_systems(Update, apply_look.run_if(|d: Res<EnvDirty>| d.0))
            .add_systems(
                Update,
                (tick_gust, tick_pulsing_hazards).in_set(GameSet::Think),
            );
    }
}

fn mark_dirty(mut dirty: ResMut<EnvDirty>) {
    dirty.0 = true;
}

/// Install the sky, the sun and the prevailing wind for the chosen world.
fn apply_look(
    mut commands: Commands,
    mut dirty: ResMut<EnvDirty>,
    env: Res<EnvKind>,
    mut gust: ResMut<Gust>,
    mut clear: ResMut<ClearColor>,
    mut ambient: ResMut<GlobalAmbientLight>,
    existing: Query<Entity, With<EnvEntity>>,
) {
    dirty.0 = false;

    for e in &existing {
        commands.entity(e).despawn();
    }

    let look = env.look();
    *gust = look.gust;
    clear.0 = look.sky;
    ambient.color = look.ambient;
    ambient.brightness = look.ambient_brightness;

    commands.spawn((
        DirectionalLight {
            color: look.sun_color,
            illuminance: look.sun_illuminance,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_translation(-look.sun_dir.normalize() * 40.0)
            .looking_to(look.sun_dir, Vec3::Y),
        EnvEntity,
    ));
}

/// Spawn one hazard entity. Shared with `world`, which streams them in per
/// chunk, so the two cannot drift apart in how a hazard is assembled.
pub fn spawn_hazard_entity(
    commands: &mut Commands,
    art: &GameArt,
    spec: &HazardSpec,
    phase: f32,
) -> Entity {
    let tint = match spec.kind {
        HazardKind::Scald => Glow::Warning,
        HazardKind::Sticky => Glow::Scrap,
        HazardKind::Shock => Glow::Plasma,
        HazardKind::Font => Glow::ZoneHeld,
    };
    let mut entity = commands.spawn((
        Hazard {
            kind: spec.kind,
            radius: spec.radius,
            dps: spec.dps,
            slow: spec.slow,
            life: None,
            hurts_player: true,
            hurts_enemies: true,
        },
        Body::new(spec.pos, spec.radius),
        Mesh3d(art.disc.clone()),
        MeshMaterial3d(art.glow(tint)),
        Transform::from_translation(to_world(spec.pos, 0.03)).with_scale(Vec3::new(
            spec.radius,
            1.0,
            spec.radius,
        )),
    ));
    if let Some((period, on_fraction)) = spec.duty {
        entity.insert(PulsingHazard {
            period,
            on_fraction,
            phase,
            base_dps: spec.dps,
        });
    }
    entity.id()
}

fn tick_gust(time: Res<Time>, mut gust: ResMut<Gust>) {
    if !gust.enabled {
        gust.blowing = false;
        return;
    }
    let dt = time.delta_secs();
    gust.remaining -= dt;
    if gust.remaining <= 0.0 {
        gust.blowing = !gust.blowing;
        gust.remaining = if gust.blowing {
            gust.duration
        } else {
            gust.cooldown
        };
    }
}

fn tick_pulsing_hazards(
    time: Res<Time>,
    mut q: Query<(&mut PulsingHazard, &mut Hazard, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (mut pulse, mut hazard, mut transform) in &mut q {
        pulse.phase = (pulse.phase + dt / pulse.period).fract();
        let on = pulse.phase < pulse.on_fraction;
        hazard.dps = if on { pulse.base_dps } else { 0.0 };
        // Shrink to a faint ring when dormant so the telegraph is legible.
        let scale = if on {
            hazard.radius
        } else {
            hazard.radius * 0.35
        };
        transform.scale = Vec3::new(scale, 1.0, scale);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::chunk_rng;

    const SEED: WorldSeed = WorldSeed(0xA11CE);

    /// Generate a square block of chunks around the origin for one world.
    fn survey(kind: EnvKind, radius: i32) -> Vec<(IVec2, ChunkContent)> {
        let mut out = Vec::new();
        for z in -radius..=radius {
            for x in -radius..=radius {
                let coord = IVec2::new(x, z);
                let mut rng = chunk_rng(SEED, coord, 1);
                out.push((coord, kind.generate_chunk(coord, &mut rng)));
            }
        }
        out
    }

    #[test]
    fn every_world_generates_chunks_without_panicking() {
        for kind in EnvKind::ALL {
            let chunks = survey(kind, 2);
            assert_eq!(chunks.len(), 25, "{kind:?}");
        }
    }

    #[test]
    fn the_kind_list_matches_the_declared_count() {
        assert_eq!(EnvKind::ALL.len(), EnvKind::COUNT);
        // The name table in `enemy.rs` indexes by `env as usize`, so the
        // discriminants must line up with the array order.
        for (i, kind) in EnvKind::ALL.iter().enumerate() {
            assert_eq!(*kind as usize, i, "{kind:?} is out of order");
        }
    }

    #[test]
    fn cycling_worlds_wraps_in_both_directions() {
        for kind in EnvKind::ALL {
            assert_eq!(kind.next().prev(), kind);
            assert_eq!(kind.prev().next(), kind);
        }
        let mut cursor = EnvKind::Desk;
        let mut seen = vec![cursor];
        for _ in 1..EnvKind::COUNT {
            cursor = cursor.next();
            seen.push(cursor);
        }
        assert_eq!(cursor.next(), EnvKind::Desk, "the carousel must loop");
        seen.sort_by_key(|k| *k as usize);
        seen.dedup();
        assert_eq!(seen.len(), EnvKind::COUNT, "next() skipped a world");
    }

    #[test]
    fn every_world_is_fully_described() {
        for kind in EnvKind::ALL {
            assert!(!kind.title().is_empty(), "{kind:?} has no title");
            assert!(!kind.tagline().is_empty(), "{kind:?} has no tagline");
            assert!(!kind.quirk().is_empty(), "{kind:?} has no quirk");
            assert!(!kind.short_name().is_empty(), "{kind:?} has no chip label");
            assert!(
                kind.short_name().len() <= 7,
                "{kind:?} chip label will overflow its box"
            );
        }
    }

    #[test]
    fn world_titles_and_accents_are_distinct() {
        let mut titles: Vec<_> = EnvKind::ALL.iter().map(|k| k.title()).collect();
        titles.sort_unstable();
        titles.dedup();
        assert_eq!(titles.len(), EnvKind::COUNT);

        for a in EnvKind::ALL {
            for b in EnvKind::ALL {
                if a as usize >= b as usize {
                    continue;
                }
                let (x, y) = (a.accent().to_linear(), b.accent().to_linear());
                let delta =
                    (x.red - y.red).abs() + (x.green - y.green).abs() + (x.blue - y.blue).abs();
                assert!(delta > 0.2, "{a:?} and {b:?} look the same");
            }
        }
    }

    #[test]
    fn worlds_are_lit_distinctly() {
        // Two worlds that share a sky and a sun read as the same place.
        for a in EnvKind::ALL {
            for b in EnvKind::ALL {
                if a as usize >= b as usize {
                    continue;
                }
                let (la, lb) = (a.look(), b.look());
                let delta = |x: Color, y: Color| {
                    let (x, y) = (x.to_linear(), y.to_linear());
                    (x.red - y.red).abs() + (x.green - y.green).abs() + (x.blue - y.blue).abs()
                };
                assert!(
                    delta(la.sky, lb.sky) > 1e-4 || delta(la.ambient, lb.ambient) > 0.01,
                    "{a:?} and {b:?} are lit identically"
                );
            }
        }
    }

    #[test]
    fn every_world_has_a_coherent_prevailing_wind() {
        for kind in EnvKind::ALL {
            let g = kind.look().gust;
            assert!(!g.label.is_empty(), "{kind:?} gust has no label");
            assert!(g.duration > 0.0 && g.cooldown > 0.0, "{kind:?} gust timing");
            assert!(g.strength > 0.0, "{kind:?} gust does nothing");
            assert!(
                (g.dir.length() - 1.0).abs() < 1e-3,
                "{kind:?} gust direction is not normalised"
            );
            assert!(g.lane_half_width > 0.0);
        }
    }

    #[test]
    fn every_world_furnishes_the_ground_it_generates() {
        for kind in EnvKind::ALL {
            let props: usize = survey(kind, 2).iter().map(|(_, c)| c.props.len()).sum();
            assert!(
                props > 100,
                "{kind:?} generated only {props} props over 25 chunks"
            );
        }
    }

    #[test]
    fn chunks_stay_inside_their_own_square() {
        // A chunk that scatters outside itself would leave seams and double up
        // where two chunks overlap, and would be unloaded while still visible.
        const SLACK: f32 = 12.0;
        for kind in EnvKind::ALL {
            for (coord, content) in survey(kind, 2) {
                let min = chunk_min(coord) - SLACK;
                let max = chunk_min(coord) + CHUNK_SIZE + SLACK;
                for prop in &content.props {
                    assert!(
                        prop.pos.cmpge(min).all() && prop.pos.cmple(max).all(),
                        "{kind:?} chunk {coord} put a prop at {} ",
                        prop.pos
                    );
                }
            }
        }
    }

    #[test]
    fn the_same_seed_regenerates_a_chunk_identically() {
        // The whole streaming design rests on this: walk away, walk back, and
        // the world has to be where you left it.
        for kind in EnvKind::ALL {
            for coord in [IVec2::new(0, 0), IVec2::new(3, -2), IVec2::new(-7, 11)] {
                let a = kind.generate_chunk(coord, &mut chunk_rng(SEED, coord, 1));
                let b = kind.generate_chunk(coord, &mut chunk_rng(SEED, coord, 1));
                assert_eq!(a.props.len(), b.props.len(), "{kind:?} {coord}");
                assert_eq!(a.forts, b.forts, "{kind:?} {coord}");
                assert_eq!(a.zones, b.zones, "{kind:?} {coord}");
                for (x, y) in a.props.iter().zip(b.props.iter()) {
                    assert!((x.pos - y.pos).length() < 1e-6, "{kind:?} {coord}");
                }
            }
        }
    }

    #[test]
    fn neighbouring_chunks_do_not_generate_the_same_layout() {
        // Adjacent coordinates differ by one; a weak hash would make the world
        // visibly tiled.
        for kind in EnvKind::ALL {
            let a =
                kind.generate_chunk(IVec2::new(4, 4), &mut chunk_rng(SEED, IVec2::new(4, 4), 1));
            let b =
                kind.generate_chunk(IVec2::new(5, 4), &mut chunk_rng(SEED, IVec2::new(5, 4), 1));
            let same = a.props.len() == b.props.len()
                && a.props.iter().zip(b.props.iter()).all(|(x, y)| {
                    (x.pos - chunk_min(IVec2::new(4, 4)))
                        .distance(y.pos - chunk_min(IVec2::new(5, 4)))
                        < 1e-6
                });
            assert!(!same, "{kind:?} tiles the same chunk over and over");
        }
    }

    #[test]
    fn the_starting_position_is_never_walled_in() {
        // The player always starts at the origin; if a world buries it in
        // props, the run begins stuck inside a mug.
        for kind in EnvKind::ALL {
            let mut field = ObstacleField::default();
            for (_, content) in survey(kind, 1) {
                for prop in &content.props {
                    if let Some(shape) = prop.collider {
                        field.push(prop.pos, shape, prop.blocks_shots, prop.height);
                    }
                }
            }
            let resolved = field.resolve(Vec2::ZERO, crate::player::PLAYER_RADIUS);
            assert!(
                resolved.length() < 1e-3,
                "{kind:?} shoves the player {} units at spawn",
                resolved.length()
            );
        }
    }

    #[test]
    fn worlds_leave_room_to_move() {
        // Sample a lattice and require that most of it is walkable, or the
        // world is a maze rather than a battlefield.
        for kind in EnvKind::ALL {
            let mut field = ObstacleField::default();
            for (_, content) in survey(kind, 2) {
                for prop in &content.props {
                    if let Some(shape) = prop.collider {
                        field.push(prop.pos, shape, prop.blocks_shots, prop.height);
                    }
                }
            }
            let (mut open, mut total) = (0, 0);
            let reach = (CHUNK_SIZE * 2.0) as i32;
            for ix in -reach..=reach {
                for iz in -reach..=reach {
                    let p = Vec2::new(ix as f32, iz as f32);
                    total += 1;
                    if !field.overlaps(p, crate::player::PLAYER_RADIUS) {
                        open += 1;
                    }
                }
            }
            let share = f64::from(open) / f64::from(total);
            assert!(share > 0.6, "{kind:?} is only {share:.2} walkable");
        }
    }

    #[test]
    fn hazards_are_sane_wherever_they_land() {
        for kind in EnvKind::ALL {
            for (_, content) in survey(kind, 2) {
                for h in &content.hazards {
                    assert!(h.radius > 0.0, "{kind:?} has a zero-radius hazard");
                    assert!(h.slow > 0.0 && h.slow <= 1.0, "{kind:?} slow {}", h.slow);
                    if let Some((period, on)) = h.duty {
                        assert!(period > 1.0, "{kind:?} pulses too fast to read");
                        assert!(
                            (0.1..0.75).contains(&on),
                            "{kind:?} duty {on} leaves no safe window"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn healing_terrain_stays_a_sanctum_signature() {
        for kind in EnvKind::ALL {
            let healing = survey(kind, 2).iter().any(|(_, c)| {
                c.hazards
                    .iter()
                    .any(|h| h.kind == HazardKind::Font && h.dps < 0.0)
            });
            assert_eq!(healing, kind == EnvKind::Arcane, "{kind:?} healing terrain");
        }
    }

    #[test]
    fn tall_props_block_shots_and_flat_ones_do_not() {
        for kind in EnvKind::ALL {
            for (_, content) in survey(kind, 2) {
                for prop in &content.props {
                    if prop.collider.is_some() && prop.blocks_shots {
                        assert!(
                            prop.height >= 0.75,
                            "{kind:?} has a {}-high prop stopping shots",
                            prop.height
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn territory_appears_regularly_but_not_everywhere() {
        for kind in EnvKind::ALL {
            let chunks = survey(kind, 3);
            let with_zones = chunks.iter().filter(|(_, c)| !c.zones.is_empty()).count();
            assert!(
                with_zones >= 8,
                "{kind:?}: only {with_zones} of {} chunks have territory",
                chunks.len()
            );
            assert!(
                with_zones < chunks.len(),
                "{kind:?}: territory in every single chunk is not a choice"
            );
        }
    }

    #[test]
    fn home_is_peaceful_and_the_far_country_is_not() {
        for kind in EnvKind::ALL {
            let mut near_hostiles = 0;
            let mut far_hostiles = 0;
            for (coord, content) in survey(kind, 6) {
                let hostile = content.forts.len() + content.spawners.len();
                if (chunk_min(coord) + CHUNK_SIZE * 0.5).length() < HOME_PEACE {
                    near_hostiles += hostile;
                } else {
                    far_hostiles += hostile;
                }
            }
            assert_eq!(near_hostiles, 0, "{kind:?} put a nest on the doorstep");
            assert!(
                far_hostiles > 4,
                "{kind:?} generated only {far_hostiles} hostile sites in the far country"
            );
        }
    }

    #[test]
    fn forts_arrive_with_an_escort_of_nests() {
        // A fort that starts alone spends the first minute bootstrapping while
        // the player watches, which is not a fight.
        for kind in EnvKind::ALL {
            let chunks = survey(kind, 6);
            let with_forts: Vec<_> = chunks.iter().filter(|(_, c)| !c.forts.is_empty()).collect();
            assert!(!with_forts.is_empty(), "{kind:?} generated no forts at all");
            for (coord, content) in with_forts {
                assert!(
                    content.spawners.len() >= content.forts.len(),
                    "{kind:?} chunk {coord} has a fort with no nests"
                );
            }
        }
    }

    #[test]
    fn prop_spec_builder_defaults_are_sane() {
        let p = PropSpec::new(crate::meshgen::cube(1.0, 1.0, 1.0), Vec2::ZERO);
        assert!(p.collider.is_none());
        assert!(!p.blocks_shots);

        let solid = PropSpec::new(crate::meshgen::cube(1.0, 1.0, 1.0), Vec2::ZERO)
            .solid(ColliderShape::Circle(1.0), 2.0);
        assert!(solid.blocks_shots, "tall props should stop shots");

        let low = PropSpec::new(crate::meshgen::cube(1.0, 1.0, 1.0), Vec2::ZERO)
            .solid(ColliderShape::Circle(1.0), 0.2);
        assert!(!low.blocks_shots, "flat props should not");

        assert!(!solid.passthrough().blocks_shots);
    }
}
