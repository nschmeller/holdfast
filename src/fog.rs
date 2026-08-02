//! Fog of war.
//!
//! Three states per cell, and each one means something different:
//!
//! - **Unexplored** - never seen. Terrain is blacked out and nothing in it
//!   renders at all.
//! - **Explored** - seen once. The landscape and any structures stay on screen,
//!   dimmed, because you remember the ground. Living things do not: an enemy
//!   you cannot currently see is not drawn.
//! - **In sight** - within the player's sight radius right now. Fully lit.
//!
//! That split is what makes exploring worth doing and what makes a scouting
//! ally worth recruiting.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::platform::collections::HashSet;
use bevy::prelude::*;

use crate::common::Body;
use crate::player::Player;
use crate::world::{CHUNK_SIZE, STREAM_RADIUS};
use crate::{AppState, GameSet, RunSetup};

/// Side length of one fog cell. Small enough that the revealed area reads as a
/// circle rather than a staircase, large enough that the overlay mesh stays a
/// few thousand quads.
pub const FOG_CELL: f32 = 3.0;

/// How far the player can see. Comfortably inside the streaming window, so the
/// fog always hides the boundary where chunks pop in.
pub const SIGHT_RADIUS: f32 = 21.0;

/// How dark explored-but-unseen ground is drawn.
const DIM_ALPHA: f32 = 0.55;

/// Cell coordinate containing a world position.
#[must_use]
pub fn cell_of(pos: Vec2) -> IVec2 {
    IVec2::new(
        (pos.x / FOG_CELL).floor() as i32,
        (pos.y / FOG_CELL).floor() as i32,
    )
}

#[must_use]
fn cell_min(cell: IVec2) -> Vec2 {
    Vec2::new(cell.x as f32, cell.y as f32) * FOG_CELL
}

/// What the player has seen, and what they can see now.
#[derive(Resource, Debug, Default)]
pub struct FogMap {
    explored: HashSet<IVec2>,
    visible: HashSet<IVec2>,
    /// Set when the overlay needs rebuilding.
    dirty: bool,
    /// The cell the player occupied at the last rebuild.
    anchor: IVec2,
    /// Cells revealed this run, for the results screen and achievements.
    pub revealed: u32,
}

impl FogMap {
    pub fn reset(&mut self) {
        self.explored.clear();
        self.visible.clear();
        self.dirty = true;
        self.anchor = IVec2::ZERO;
        self.revealed = 0;
    }

    #[must_use]
    pub fn is_explored(&self, pos: Vec2) -> bool {
        self.explored.contains(&cell_of(pos))
    }

    #[must_use]
    pub fn is_visible(&self, pos: Vec2) -> bool {
        self.visible.contains(&cell_of(pos))
    }

    /// Area explored, in square world units. Reads better than a cell count.
    #[must_use]
    pub fn explored_area(&self) -> f32 {
        self.explored.len() as f32 * FOG_CELL * FOG_CELL
    }
}

/// Attach to anything the fog should hide.
#[derive(Component, Debug, Default)]
pub struct FogOccluded {
    /// When true the entity also disappears once it leaves the sight radius,
    /// not merely when its ground is unexplored. Living things set this;
    /// terrain does not.
    pub require_sight: bool,
}

impl FogOccluded {
    #[must_use]
    pub fn living() -> Self {
        Self {
            require_sight: true,
        }
    }
}

/// The single overlay mesh drawn over unexplored and dimmed ground.
#[derive(Component, Debug)]
struct FogOverlay;

#[derive(Debug)]
pub struct FogPlugin;

impl Plugin for FogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FogMap>()
            .add_systems(OnExit(AppState::Menu), reset_fog.in_set(RunSetup::Reset))
            .add_systems(Update, reveal_around_player.in_set(GameSet::Input))
            .add_systems(
                Update,
                (rebuild_overlay, apply_fog_visibility).in_set(GameSet::Present),
            );
    }
}

fn reset_fog(mut fog: ResMut<FogMap>, mut commands: Commands, q: Query<Entity, With<FogOverlay>>) {
    fog.reset();
    for e in &q {
        commands.entity(e).despawn();
    }
}

/// Reveal everything inside the sight radius, and remember it.
fn reveal_around_player(
    mut fog: ResMut<FogMap>,
    player: Query<&Body, With<Player>>,
    scouts: Query<(&Body, &crate::allies::Ally), Without<Player>>,
) {
    let Some(body) = player.iter().next() else {
        return;
    };

    fog.visible.clear();

    // The player, plus every Scout - which is what makes that ally worth its
    // Cores on a map you cannot see.
    let mut eyes = vec![(body.pos, SIGHT_RADIUS)];
    for (ally_body, ally) in &scouts {
        if ally.kind == crate::allies::AllyKind::Scout {
            eyes.push((ally_body.pos, SIGHT_RADIUS * 0.75));
        }
    }

    let mut newly = 0u32;
    for (centre, radius) in eyes {
        let reach = (radius / FOG_CELL).ceil() as i32;
        let origin = cell_of(centre);
        let r_sq = radius * radius;
        for dz in -reach..=reach {
            for dx in -reach..=reach {
                let cell = origin + IVec2::new(dx, dz);
                // Test the cell centre so the boundary is a circle, not a box.
                let cell_centre = cell_min(cell) + Vec2::splat(FOG_CELL * 0.5);
                if cell_centre.distance_squared(centre) > r_sq {
                    continue;
                }
                fog.visible.insert(cell);
                if fog.explored.insert(cell) {
                    newly += 1;
                }
            }
        }
    }

    fog.revealed += newly;

    // Rebuild when new ground appears, or when the player crosses a cell - the
    // dimmed band moves with them, so it has to follow.
    let here = cell_of(body.pos);
    if newly > 0 || here != fog.anchor {
        fog.anchor = here;
        fog.dirty = true;
    }
}

/// Regenerate the overlay quad mesh.
///
/// One mesh for the whole visible window rather than an entity per cell: a few
/// thousand cells would be a few thousand draw calls otherwise.
fn rebuild_overlay(
    mut commands: Commands,
    mut fog: ResMut<FogMap>,
    art: Res<crate::art::GameArt>,
    mut meshes: ResMut<Assets<Mesh>>,
    player: Query<&Body, With<Player>>,
    existing: Query<(Entity, &Mesh3d), With<FogOverlay>>,
) {
    if !fog.dirty {
        return;
    }
    fog.dirty = false;

    let Some(body) = player.iter().next() else {
        return;
    };

    // Cover the whole streamed window so the fog reaches past anything the
    // camera can frame.
    let half = (STREAM_RADIUS as f32 + 0.5) * CHUNK_SIZE;
    let reach = (half / FOG_CELL).ceil() as i32;
    let origin = cell_of(body.pos);

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for dz in -reach..=reach {
        for dx in -reach..=reach {
            let cell = origin + IVec2::new(dx, dz);
            let explored = fog.explored.contains(&cell);
            if explored && fog.visible.contains(&cell) {
                continue; // Fully lit: no overlay at all.
            }
            let alpha = if explored { DIM_ALPHA } else { 1.0 };

            let min = cell_min(cell);
            let max = min + Vec2::splat(FOG_CELL);
            let base = positions.len() as u32;
            // A hair of overlap stops seams showing between adjacent quads.
            let (x0, z0, x1, z1) = (min.x - 0.01, min.y - 0.01, max.x + 0.01, max.y + 0.01);

            for (x, z, u, v) in [
                (x0, z0, 0.0, 0.0),
                (x1, z0, 1.0, 0.0),
                (x1, z1, 1.0, 1.0),
                (x0, z1, 0.0, 1.0),
            ] {
                positions.push([x, 0.0, z]);
                normals.push([0.0, 1.0, 0.0]);
                uvs.push([u, v]);
                colors.push([0.0, 0.0, 0.015, alpha]);
            }
            indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));

    let handle = meshes.add(mesh);

    if let Ok((entity, current)) = existing.single() {
        // Drop the previous mesh explicitly; the overlay is rebuilt several
        // times a second and orphaned meshes would accumulate fast.
        meshes.remove(&current.0);
        commands.entity(entity).insert(Mesh3d(handle));
    } else {
        commands.spawn((
            FogOverlay,
            Mesh3d(handle),
            MeshMaterial3d(art.unlit.clone()),
            // Above the ground and any decal, below every standing prop.
            Transform::from_xyz(0.0, 0.09, 0.0),
            crate::common::RunEntity,
        ));
    }
}

/// Hide whatever the fog covers.
fn apply_fog_visibility(fog: Res<FogMap>, mut q: Query<(&Body, &FogOccluded, &mut Visibility)>) {
    for (body, occlusion, mut visibility) in &mut q {
        let shown = if occlusion.require_sight {
            fog.is_visible(body.pos)
        } else {
            fog.is_explored(body.pos)
        };
        let want = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        // Only write when it changes: `Visibility` is change-detected and
        // touching it every frame on every entity would defeat that.
        if *visibility != want {
            *visibility = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_tile_the_plane_without_gaps_or_overlap() {
        assert_eq!(cell_of(Vec2::ZERO), IVec2::ZERO);
        assert_eq!(cell_of(Vec2::splat(FOG_CELL * 0.99)), IVec2::ZERO);
        assert_eq!(cell_of(Vec2::splat(FOG_CELL)), IVec2::ONE);
        assert_eq!(cell_of(Vec2::splat(-0.01)), IVec2::splat(-1));
    }

    #[test]
    fn cell_min_is_the_inverse_of_cell_of() {
        for (x, z) in [(0, 0), (3, -7), (-11, 4), (100, -100)] {
            let cell = IVec2::new(x, z);
            assert_eq!(cell_of(cell_min(cell) + Vec2::splat(0.1)), cell);
        }
    }

    #[test]
    fn a_fresh_map_knows_nothing() {
        let fog = FogMap::default();
        assert!(!fog.is_explored(Vec2::ZERO));
        assert!(!fog.is_visible(Vec2::ZERO));
        assert_eq!(fog.explored_area(), 0.0);
    }

    #[test]
    fn exploring_is_permanent_but_sight_is_not() {
        let mut fog = FogMap::default();
        let cell = cell_of(Vec2::new(5.0, 5.0));
        fog.explored.insert(cell);
        fog.visible.insert(cell);
        assert!(fog.is_explored(Vec2::new(5.0, 5.0)));
        assert!(fog.is_visible(Vec2::new(5.0, 5.0)));

        // A new frame clears sight; memory survives.
        fog.visible.clear();
        assert!(fog.is_explored(Vec2::new(5.0, 5.0)));
        assert!(!fog.is_visible(Vec2::new(5.0, 5.0)));
    }

    #[test]
    fn explored_area_counts_whole_cells() {
        let mut fog = FogMap::default();
        fog.explored.insert(IVec2::ZERO);
        fog.explored.insert(IVec2::ONE);
        assert!((fog.explored_area() - 2.0 * FOG_CELL * FOG_CELL).abs() < 1e-4);
    }

    #[test]
    fn reset_forgets_everything_and_asks_for_a_rebuild() {
        let mut fog = FogMap::default();
        fog.explored.insert(IVec2::ZERO);
        fog.revealed = 12;
        fog.reset();
        assert!(!fog.is_explored(Vec2::ZERO));
        assert_eq!(fog.revealed, 0);
        assert!(fog.dirty);
    }

    #[test]
    fn living_things_need_line_of_sight() {
        assert!(FogOccluded::living().require_sight);
        assert!(!FogOccluded::default().require_sight);
    }

    #[test]
    fn the_sight_radius_stays_inside_the_streamed_window() {
        // If sight reached past the loaded chunks the player would see the
        // edge of the world pop in.
        let window = STREAM_RADIUS as f32 * CHUNK_SIZE;
        assert!(
            SIGHT_RADIUS < window,
            "sight {SIGHT_RADIUS} exceeds the {window}-unit streaming window"
        );
    }
}
