//! Infinite worlds, streamed in chunks.
//!
//! There is no arena any more. The world is an unbounded grid of chunks, each
//! generated deterministically from `(world seed, chunk coordinate)`, so the
//! same seed always produces the same landscape and a chunk can be unloaded and
//! regenerated identically when the player walks back.
//!
//! Only the chunks near the player exist as entities. Everything further out is
//! a coordinate and a hash.

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;

use crate::arena::{Hazard, ObstacleField, PlacedObstacle};
use crate::art::{GameArt, Glow};
use crate::common::{Body, to_world};
use crate::environments::{ChunkContent, EnvKind, Surface};
use crate::rng::Rng;
use crate::{AppState, GameSet, RunSetup};

/// Side length of one chunk, in world units.
///
/// Sized so a chunk is a little wider than the camera's near field: big enough
/// that streaming happens a few times a minute rather than constantly, small
/// enough that the working set stays modest.
pub const CHUNK_SIZE: f32 = 24.0;

/// Chunks kept live in each direction from the player's chunk. A radius of 3
/// is a 7x7 window, roughly 168 units across - comfortably beyond what the
/// camera can see even in plan mode.
pub const STREAM_RADIUS: i32 = 3;
/// Chunks are only unloaded once they fall outside this larger radius, so
/// walking back and forth across a boundary does not thrash.
pub const UNLOAD_RADIUS: i32 = 5;

/// The seed for the entire world. One per run.
#[derive(Resource, Debug, Clone, Copy)]
pub struct WorldSeed(pub u64);

impl Default for WorldSeed {
    fn default() -> Self {
        Self(0x5EED_0FC0_FFEE)
    }
}

/// Chunk coordinate containing a world position.
#[must_use]
pub fn chunk_of(pos: Vec2) -> IVec2 {
    IVec2::new(
        (pos.x / CHUNK_SIZE).floor() as i32,
        (pos.y / CHUNK_SIZE).floor() as i32,
    )
}

/// Minimum (south-west) corner of a chunk in world space.
#[must_use]
pub fn chunk_min(coord: IVec2) -> Vec2 {
    Vec2::new(coord.x as f32, coord.y as f32) * CHUNK_SIZE
}

#[must_use]
pub fn chunk_center(coord: IVec2) -> Vec2 {
    chunk_min(coord) + Vec2::splat(CHUNK_SIZE * 0.5)
}

/// A deterministic RNG for one chunk.
///
/// The inputs are mixed *sequentially* rather than combined with xor. Xor-ing
/// three multiples together looks like it separates them and does not: the
/// operation is commutative, so coordinates collide in pairs and neighbouring
/// chunks come out visibly correlated. Folding each value through an avalanche
/// step in turn makes the order matter and every input bit reach every output
/// bit.
#[must_use]
pub fn chunk_rng(seed: WorldSeed, coord: IVec2, salt: u64) -> Rng {
    let mut hash = seed.0;
    for value in [i64::from(coord.x) as u64, i64::from(coord.y) as u64, salt] {
        hash ^= value.wrapping_add(0x9E37_79B9_7F4A_7C15);
        hash = hash.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
        hash ^= hash >> 33;
        hash = hash.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
        hash ^= hash >> 29;
    }
    Rng::seeded(hash)
}

/// A chunk that currently exists in the world.
///
/// The obstacle, light and chasm lists are kept here rather than regenerated on
/// demand: the flat resources they feed are rebuilt on every streaming change,
/// and re-running the generator for the whole window each time would make
/// walking across a boundary quadratically expensive.
#[derive(Debug)]
struct LoadedChunk {
    entities: Vec<Entity>,
    obstacles: Vec<PlacedObstacle>,
    light_pools: Vec<LightPool>,
    chasms: Vec<Chasm>,
}

/// Marks every entity belonging to a streamed chunk.
#[derive(Component, Debug)]
pub struct ChunkEntity(pub IVec2);

/// Tracks which chunks are live.
#[derive(Resource, Debug, Default)]
pub struct ChunkManager {
    loaded: HashMap<IVec2, LoadedChunk>,
    /// The player's chunk as of the last streaming pass.
    pub center: IVec2,
    /// Total chunks generated this run, purely for the stats ledger.
    pub generated: u32,
}

impl ChunkManager {
    pub fn clear(&mut self) {
        self.loaded.clear();
        self.center = IVec2::ZERO;
        self.generated = 0;
    }

    #[must_use]
    pub fn is_loaded(&self, coord: IVec2) -> bool {
        self.loaded.contains_key(&coord)
    }

    #[must_use]
    pub fn loaded_count(&self) -> usize {
        self.loaded.len()
    }
}

/// Light pools scattered across the world, replacing the old single spotlight.
///
/// Standing in one is still the same trade - more damage, more attention - but
/// now they are places you find rather than a fixed feature.
#[derive(Resource, Debug, Default)]
pub struct LightPools {
    pub pools: Vec<LightPool>,
}

#[derive(Debug, Clone, Copy)]
pub struct LightPool {
    pub center: Vec2,
    pub radius: f32,
    pub damage_bonus: f32,
}

impl LightPools {
    /// Damage multiplier at a position: 1.0 outside every pool.
    #[must_use]
    pub fn bonus_at(&self, pos: Vec2) -> f32 {
        self.pools
            .iter()
            .find(|p| pos.distance_squared(p.center) <= p.radius * p.radius)
            .map_or(1.0, |p| 1.0 + p.damage_bonus)
    }

    #[must_use]
    pub fn contains(&self, pos: Vec2) -> bool {
        self.bonus_at(pos) > 1.0
    }
}

/// A void that things fall into. Keeps knockback lethal now that arenas have no
/// edge: the gap between two desks, a hole in the platform, a chasm.
#[derive(Resource, Debug, Default)]
pub struct Chasms {
    pub holes: Vec<Chasm>,
}

#[derive(Debug, Clone, Copy)]
pub struct Chasm {
    pub center: Vec2,
    pub radius: f32,
}

impl Chasms {
    #[must_use]
    pub fn contains(&self, pos: Vec2) -> bool {
        self.holes
            .iter()
            .any(|h| pos.distance_squared(h.center) <= h.radius * h.radius)
    }

    /// Push a position out of any chasm it is inside, for keeping the player
    /// and their structures on solid ground.
    #[must_use]
    pub fn push_out(&self, pos: Vec2, radius: f32) -> Vec2 {
        let mut out = pos;
        for hole in &self.holes {
            let delta = out - hole.center;
            let min = hole.radius + radius;
            let dist = delta.length();
            if dist < min {
                let dir = if dist > 1e-4 { delta / dist } else { Vec2::X };
                out = hole.center + dir * min;
            }
        }
        out
    }
}

#[derive(Debug)]
pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldSeed>()
            .init_resource::<ChunkManager>()
            .init_resource::<LightPools>()
            .init_resource::<Chasms>()
            .add_systems(OnExit(AppState::Menu), reset_world.in_set(RunSetup::Reset))
            // Streaming runs before anything reads the obstacle field.
            .add_systems(Update, stream_chunks.in_set(GameSet::Input));
    }
}

fn reset_world(
    mut commands: Commands,
    mut manager: ResMut<ChunkManager>,
    mut obstacles: ResMut<ObstacleField>,
    mut pools: ResMut<LightPools>,
    mut chasms: ResMut<Chasms>,
    mut seed: ResMut<WorldSeed>,
    clock: Res<crate::threat::RunClock>,
    existing: Query<Entity, With<ChunkEntity>>,
) {
    for e in &existing {
        commands.entity(e).despawn();
    }
    manager.clear();
    obstacles.clear();
    pools.pools.clear();
    chasms.holes.clear();

    // Vary the world per run without needing a wall clock: the previous run's
    // length is a perfectly good source of entropy, and a fresh process starts
    // from the default seed.
    seed.0 = seed
        .0
        .wrapping_mul(0x2545_F491_4F6C_DD1D)
        .wrapping_add((clock.elapsed * 1000.0) as u64 | 1);
}

/// Load chunks near the player, unload those far away.
#[allow(clippy::too_many_arguments)]
fn stream_chunks(
    mut commands: Commands,
    env: Res<EnvKind>,
    seed: Res<WorldSeed>,
    art: Res<GameArt>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut manager: ResMut<ChunkManager>,
    mut obstacles: ResMut<ObstacleField>,
    mut pools: ResMut<LightPools>,
    mut chasms: ResMut<Chasms>,
    mut zone_spawns: MessageWriter<crate::allies::SpawnZone>,
    player: Query<&Body, With<crate::player::Player>>,
) {
    let Some(body) = player.iter().next() else {
        return;
    };
    let center = chunk_of(body.pos);

    // Which chunks should exist right now?
    let mut wanted: HashSet<IVec2> = HashSet::default();
    for dz in -STREAM_RADIUS..=STREAM_RADIUS {
        for dx in -STREAM_RADIUS..=STREAM_RADIUS {
            wanted.insert(center + IVec2::new(dx, dz));
        }
    }

    let mut changed = false;

    // Unload anything well outside the window.
    let stale: Vec<IVec2> = manager
        .loaded
        .keys()
        .copied()
        .filter(|c| {
            let d = *c - center;
            d.x.abs() > UNLOAD_RADIUS || d.y.abs() > UNLOAD_RADIUS
        })
        .collect();
    for coord in stale {
        if let Some(chunk) = manager.loaded.remove(&coord) {
            for e in chunk.entities {
                commands.entity(e).try_despawn();
            }
            changed = true;
        }
    }

    // Load anything missing. Budgeted per frame so a teleport or a fast dash
    // cannot stall a frame generating two dozen chunks at once.
    let mut budget = 3;
    for coord in wanted {
        if budget == 0 {
            break;
        }
        if manager.is_loaded(coord) {
            continue;
        }
        budget -= 1;
        changed = true;
        manager.generated += 1;

        let chunk = build_chunk(
            &mut commands,
            &art,
            &mut meshes,
            *env,
            *seed,
            coord,
            &mut zone_spawns,
        );
        manager.loaded.insert(coord, chunk);
    }

    manager.center = center;

    if changed {
        // Light pools, chasms and obstacles live in flat lists rebuilt whenever
        // the loaded set changes, so the hot-path queries stay linear scans
        // over a few hundred entries rather than ECS queries per actor.
        obstacles.items.clear();
        pools.pools.clear();
        chasms.holes.clear();
        for chunk in manager.loaded.values() {
            obstacles.items.extend(chunk.obstacles.iter().copied());
            pools.pools.extend(chunk.light_pools.iter().copied());
            chasms.holes.extend(chunk.chasms.iter().copied());
        }
    }
}

/// Generate and spawn one chunk.
fn build_chunk(
    commands: &mut Commands,
    art: &GameArt,
    meshes: &mut Assets<Mesh>,
    env: EnvKind,
    seed: WorldSeed,
    coord: IVec2,
    zone_spawns: &mut MessageWriter<crate::allies::SpawnZone>,
) -> LoadedChunk {
    let mut rng = chunk_rng(seed, coord, 1);
    let content: ChunkContent = env.generate_chunk(coord, &mut rng);

    let mut entities = Vec::with_capacity(content.props.len() + 2);
    let mut obstacles = Vec::new();
    let light_pools = content.light_pools;
    let chasms = content.chasms;

    // Floor.
    let ground = env.chunk_floor(coord, seed);
    entities.push(
        commands
            .spawn((
                ChunkEntity(coord),
                Mesh3d(meshes.add(ground)),
                MeshMaterial3d(art.ground.clone()),
                Transform::from_translation(to_world(chunk_center(coord), 0.0)),
                crate::fog::FogOccluded::default(),
            ))
            .id(),
    );

    // Props.
    for prop in content.props {
        let material = match prop.surface {
            Surface::Solid => art.solid.clone(),
            Surface::Matte => art.matte.clone(),
            Surface::Metal => art.metal.clone(),
            Surface::Glass => art.glass.clone(),
            Surface::Glow(g) => art.glow(g),
        };
        if let Some(shape) = prop.collider {
            obstacles.push(PlacedObstacle {
                pos: prop.pos,
                shape,
                blocks_shots: prop.blocks_shots,
                height: prop.height,
            });
        }
        entities.push(
            commands
                .spawn((
                    ChunkEntity(coord),
                    Mesh3d(meshes.add(prop.mesh)),
                    MeshMaterial3d(material),
                    Transform::from_translation(to_world(prop.pos, prop.y))
                        .with_rotation(Quat::from_rotation_y(prop.rot_y)),
                    crate::fog::FogOccluded::default(),
                ))
                .id(),
        );
    }

    // Point lights.
    for l in content.lights {
        entities.push(
            commands
                .spawn((
                    ChunkEntity(coord),
                    PointLight {
                        color: l.color,
                        intensity: l.intensity,
                        range: l.range,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_translation(l.pos),
                ))
                .id(),
        );
    }

    // Hazards.
    for h in content.hazards {
        let tint = match h.kind {
            crate::arena::HazardKind::Scald => Glow::Warning,
            crate::arena::HazardKind::Sticky => Glow::Scrap,
            crate::arena::HazardKind::Shock => Glow::Plasma,
            crate::arena::HazardKind::Font => Glow::ZoneHeld,
        };
        let mut e = commands.spawn((
            ChunkEntity(coord),
            Hazard {
                kind: h.kind,
                radius: h.radius,
                dps: h.dps,
                slow: h.slow,
                life: None,
                hurts_player: true,
                hurts_enemies: true,
            },
            Body::new(h.pos, h.radius),
            Mesh3d(art.disc.clone()),
            MeshMaterial3d(art.glow(tint)),
            Transform::from_translation(to_world(h.pos, 0.03))
                .with_scale(Vec3::new(h.radius, 1.0, h.radius)),
            crate::fog::FogOccluded::default(),
        ));
        if let Some((period, on_fraction)) = h.duty {
            e.insert(crate::environments::PulsingHazard {
                period,
                on_fraction,
                // Offset by chunk so vents across the world do not fire in
                // lockstep, which would read as a global heartbeat.
                phase: (coord.x * 7 + coord.y * 13).rem_euclid(10) as f32 / 10.0,
                base_dps: h.dps,
            });
        }
        entities.push(e.id());
    }

    // Territory markers.
    for pos in content.zones {
        zone_spawns.write(crate::allies::SpawnZone { pos });
    }

    LoadedChunk {
        entities,
        obstacles,
        light_pools,
        chasms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_map_to_the_chunk_that_contains_them() {
        assert_eq!(chunk_of(Vec2::ZERO), IVec2::ZERO);
        assert_eq!(chunk_of(Vec2::new(CHUNK_SIZE * 0.5, 0.0)), IVec2::ZERO);
        assert_eq!(chunk_of(Vec2::new(CHUNK_SIZE + 0.1, 0.0)), IVec2::new(1, 0));
        // Negative coordinates floor rather than truncate, or everything just
        // left of the origin would claim to be in chunk zero.
        assert_eq!(chunk_of(Vec2::new(-0.1, -0.1)), IVec2::new(-1, -1));
        assert_eq!(
            chunk_of(Vec2::new(-CHUNK_SIZE, -CHUNK_SIZE)),
            IVec2::new(-1, -1)
        );
    }

    #[test]
    fn a_chunks_corner_and_centre_agree_with_its_coordinate() {
        for coord in [IVec2::ZERO, IVec2::new(3, -4), IVec2::new(-9, 12)] {
            assert_eq!(chunk_of(chunk_center(coord)), coord);
            assert_eq!(chunk_of(chunk_min(coord) + 0.01), coord);
        }
    }

    #[test]
    fn chunk_seeds_differ_between_neighbours() {
        // Adjacent chunks differ by one; a weak mix would correlate them and
        // the world would look tiled.
        let world = WorldSeed(1234);
        let mut drawn = Vec::new();
        for x in -2..=2 {
            for z in -2..=2 {
                drawn.push(chunk_rng(world, IVec2::new(x, z), 1).next_u64());
            }
        }
        let unique = {
            let mut v = drawn.clone();
            v.sort_unstable();
            v.dedup();
            v.len()
        };
        assert_eq!(unique, drawn.len(), "two chunks drew the same seed");
    }

    #[test]
    fn the_same_chunk_seed_is_reproducible() {
        let seed = WorldSeed(99);
        let a = chunk_rng(seed, IVec2::new(5, -3), 7).next_u64();
        let b = chunk_rng(seed, IVec2::new(5, -3), 7).next_u64();
        assert_eq!(a, b, "walking back must find the same world");
    }

    #[test]
    fn a_different_salt_gives_a_different_stream() {
        let seed = WorldSeed(99);
        let a = chunk_rng(seed, IVec2::new(5, -3), 1).next_u64();
        let b = chunk_rng(seed, IVec2::new(5, -3), 2).next_u64();
        assert_ne!(a, b);
    }

    #[test]
    fn light_pools_only_pay_out_inside_themselves() {
        let pools = LightPools {
            pools: vec![LightPool {
                center: Vec2::new(2.0, 2.0),
                radius: 3.0,
                damage_bonus: 0.25,
            }],
        };
        assert!((pools.bonus_at(Vec2::new(2.0, 4.0)) - 1.25).abs() < 1e-6);
        assert!((pools.bonus_at(Vec2::new(2.0, 9.0)) - 1.0).abs() < 1e-6);
        assert!(pools.contains(Vec2::new(2.0, 2.0)));
        assert!(!pools.contains(Vec2::new(20.0, 2.0)));
    }

    #[test]
    fn nothing_is_inside_a_world_with_no_pools() {
        let pools = LightPools::default();
        assert!(!pools.contains(Vec2::ZERO));
        assert!((pools.bonus_at(Vec2::ZERO) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_chasm_swallows_only_what_is_over_it() {
        let chasms = Chasms {
            holes: vec![Chasm {
                center: Vec2::ZERO,
                radius: 4.0,
            }],
        };
        assert!(chasms.contains(Vec2::new(1.0, 1.0)));
        assert!(!chasms.contains(Vec2::new(5.0, 0.0)));
    }

    #[test]
    fn pushing_out_of_a_chasm_clears_it_completely() {
        let chasms = Chasms {
            holes: vec![Chasm {
                center: Vec2::ZERO,
                radius: 4.0,
            }],
        };
        let out = chasms.push_out(Vec2::new(0.5, 0.0), 0.6);
        assert!(
            out.length() >= 4.6 - 1e-4,
            "still overhanging the hole at {out:?}"
        );
        // Somewhere already clear must not be moved at all.
        let clear = Vec2::new(30.0, 0.0);
        assert_eq!(chasms.push_out(clear, 0.6), clear);
    }

    #[test]
    fn a_body_exactly_on_a_chasm_centre_still_escapes() {
        // No direction to flee along; without a fallback it would sit there.
        let chasms = Chasms {
            holes: vec![Chasm {
                center: Vec2::ZERO,
                radius: 3.0,
            }],
        };
        let out = chasms.push_out(Vec2::ZERO, 0.5);
        assert!(out.length() >= 3.5 - 1e-4, "{out:?}");
    }
}
