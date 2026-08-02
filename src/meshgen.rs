//! Procedural mesh construction.
//!
//! Every visible object in the game is built here at startup by welding Bevy's
//! primitive shapes together into a single mesh that carries per-vertex colour.
//! That buys us three things at once: no asset files to ship, one draw call per
//! prop instead of one per part, and a single shared material for the whole
//! scene.

use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};

/// Accumulates transformed primitives into one mesh.
#[derive(Default)]
pub struct MeshWeld {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl MeshWeld {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append `mesh`, transformed by `xform`, tinted `color`.
    pub fn add(&mut self, mesh: &Mesh, xform: Transform, color: Color) -> &mut Self {
        let base = self.positions.len() as u32;

        let Some(src_pos) = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(VertexAttributeValues::as_float3)
        else {
            return self;
        };

        let affine = xform.compute_affine();
        // Normals need the inverse-transpose of the linear part, otherwise any
        // non-uniform scale (which we use constantly for squashed props) skews
        // the lighting.
        let normal_matrix = Mat3::from(affine.matrix3).inverse().transpose();

        let src_nrm = mesh
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .and_then(VertexAttributeValues::as_float3);
        let src_uv = match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
            Some(VertexAttributeValues::Float32x2(v)) => Some(v),
            _ => None,
        };

        let rgba = color.to_linear().to_f32_array();

        for (i, p) in src_pos.iter().enumerate() {
            let world = affine.transform_point3(Vec3::from_array(*p));
            self.positions.push(world.to_array());

            let n = src_nrm
                .map(|n| Vec3::from_array(n[i]))
                .unwrap_or(Vec3::Y);
            self.normals
                .push((normal_matrix * n).normalize_or_zero().to_array());

            self.uvs.push(src_uv.map(|u| u[i]).unwrap_or([0.0, 0.0]));
            self.colors.push(rgba);
        }

        match mesh.indices() {
            Some(Indices::U32(idx)) => self.indices.extend(idx.iter().map(|i| i + base)),
            Some(Indices::U16(idx)) => {
                self.indices.extend(idx.iter().map(|i| *i as u32 + base))
            }
            None => self.indices.extend(0..src_pos.len() as u32),
        }

        self
    }

    /// Convenience: append a primitive shape directly.
    pub fn shape<T: Into<Mesh>>(&mut self, shape: T, xform: Transform, color: Color) -> &mut Self {
        let mesh: Mesh = shape.into();
        self.add(&mesh, xform, color)
    }

    pub fn build(self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}

// -- transform shorthands, to keep the prop definitions readable ------------

pub fn at(x: f32, y: f32, z: f32) -> Transform {
    Transform::from_xyz(x, y, z)
}

pub fn at_scaled(x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32) -> Transform {
    Transform::from_xyz(x, y, z).with_scale(Vec3::new(sx, sy, sz))
}

/// Rotation about X, in degrees, at a position.
pub fn at_rot_x(x: f32, y: f32, z: f32, deg: f32) -> Transform {
    Transform::from_xyz(x, y, z).with_rotation(Quat::from_rotation_x(deg.to_radians()))
}

pub fn at_rot_y(x: f32, y: f32, z: f32, deg: f32) -> Transform {
    Transform::from_xyz(x, y, z).with_rotation(Quat::from_rotation_y(deg.to_radians()))
}

pub fn at_rot_z(x: f32, y: f32, z: f32, deg: f32) -> Transform {
    Transform::from_xyz(x, y, z).with_rotation(Quat::from_rotation_z(deg.to_radians()))
}

/// A box spanning arbitrary corners, which reads better than centre+extent for
/// things like desk rims and keyboard frames.
pub fn boxed(min: Vec3, max: Vec3) -> (Cuboid, Transform) {
    let size = max - min;
    let center = (max + min) * 0.5;
    (
        Cuboid::new(size.x.abs(), size.y.abs(), size.z.abs()),
        Transform::from_translation(center),
    )
}

/// One cell of a procedural floor.
pub struct GroundCell {
    pub color: Color,
    /// Vertical offset, for cobbles, floorboards and hex plates.
    pub height: f32,
}

/// Build a floor as a grid of independent quads.
///
/// Unshared vertices are the point: each cell gets a flat colour and its own
/// height, which is what lets a floor read as planks, grass clumps, concrete
/// slabs or hex plating without a single texture byte.
pub fn ground_grid(
    half_x: f32,
    half_z: f32,
    cell: f32,
    mut f: impl FnMut(usize, usize, Vec2) -> GroundCell,
) -> Mesh {
    let nx = ((half_x * 2.0) / cell).ceil() as usize;
    let nz = ((half_z * 2.0) / cell).ceil() as usize;

    let mut positions = Vec::with_capacity(nx * nz * 4);
    let mut normals = Vec::with_capacity(nx * nz * 4);
    let mut uvs = Vec::with_capacity(nx * nz * 4);
    let mut colors = Vec::with_capacity(nx * nz * 4);
    let mut indices = Vec::with_capacity(nx * nz * 6);

    for ix in 0..nx {
        for iz in 0..nz {
            let x0 = -half_x + ix as f32 * cell;
            let z0 = -half_z + iz as f32 * cell;
            let x1 = (x0 + cell).min(half_x);
            let z1 = (z0 + cell).min(half_z);
            let center = Vec2::new((x0 + x1) * 0.5, (z0 + z1) * 0.5);

            let GroundCell { color, height } = f(ix, iz, center);
            let rgba = color.to_linear().to_f32_array();
            let base = positions.len() as u32;

            for (x, z, u, v) in [
                (x0, z0, 0.0, 0.0),
                (x1, z0, 1.0, 0.0),
                (x1, z1, 1.0, 1.0),
                (x0, z1, 0.0, 1.0),
            ] {
                positions.push([x, height, z]);
                normals.push([0.0, 1.0, 0.0]);
                uvs.push([u, v]);
                colors.push(rgba);
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
    mesh
}

/// Cheap value noise in `[0, 1)`. Deterministic for a given `(x, z, seed)`.
///
/// Good enough for scattering grain and grass; not good enough for terrain, but
/// we never ask it to be.
pub fn noise2(x: f32, z: f32, seed: u32) -> f32 {
    let mut h = seed
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add((x * 374_761_393.0) as i32 as u32)
        .wrapping_add((z * 668_265_263.0) as i32 as u32);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h & 0xFFFF) as f32 / 65536.0
}

/// Smooth-ish noise by averaging four offset samples. Still cheap.
pub fn noise_soft(x: f32, z: f32, seed: u32) -> f32 {
    (noise2(x, z, seed)
        + noise2(x + 0.5, z, seed)
        + noise2(x, z + 0.5, seed)
        + noise2(x + 0.5, z + 0.5, seed))
        * 0.25
}

// -- shared primitive presets ----------------------------------------------

/// Low-poly sphere. Desk props are small on screen; 2 subdivisions is plenty
/// and keeps the merged meshes cheap enough to spawn hundreds of enemies.
pub fn sphere(r: f32) -> Mesh {
    Sphere::new(r).mesh().ico(2).unwrap()
}

pub fn sphere_hi(r: f32) -> Mesh {
    Sphere::new(r).mesh().ico(3).unwrap()
}

pub fn cylinder(r: f32, h: f32) -> Mesh {
    Mesh::from(Cylinder::new(r, h).mesh().resolution(12))
}

pub fn cylinder_hi(r: f32, h: f32) -> Mesh {
    Mesh::from(Cylinder::new(r, h).mesh().resolution(20))
}

pub fn cone(r: f32, h: f32) -> Mesh {
    Mesh::from(Cone::new(r, h).mesh().resolution(10))
}

pub fn torus(inner: f32, outer: f32) -> Mesh {
    Mesh::from(Torus::new(inner, outer).mesh().major_resolution(16).minor_resolution(8))
}

pub fn cube(x: f32, y: f32, z: f32) -> Mesh {
    Mesh::from(Cuboid::new(x, y, z))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count_vertices(mesh: &Mesh) -> usize {
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(VertexAttributeValues::as_float3)
            .map_or(0, <[[f32; 3]]>::len)
    }

    #[test]
    fn an_empty_weld_produces_an_empty_mesh() {
        let mesh = MeshWeld::new().build();
        assert_eq!(count_vertices(&mesh), 0);
    }

    #[test]
    fn welding_preserves_the_vertex_count() {
        let cube_mesh = cube(1.0, 1.0, 1.0);
        let n = count_vertices(&cube_mesh);
        let mut w = MeshWeld::new();
        w.add(&cube_mesh, Transform::IDENTITY, Color::WHITE);
        w.add(&cube_mesh, at(2.0, 0.0, 0.0), Color::BLACK);
        assert_eq!(count_vertices(&w.build()), n * 2);
    }

    #[test]
    fn welding_carries_every_attribute() {
        let mut w = MeshWeld::new();
        w.add(&cube(1.0, 1.0, 1.0), Transform::IDENTITY, Color::WHITE);
        let mesh = w.build();
        for attr in [
            Mesh::ATTRIBUTE_POSITION,
            Mesh::ATTRIBUTE_NORMAL,
            Mesh::ATTRIBUTE_UV_0,
            Mesh::ATTRIBUTE_COLOR,
        ] {
            assert!(mesh.attribute(attr).is_some(), "missing an attribute");
        }
        assert!(mesh.indices().is_some());
    }

    #[test]
    fn welded_indices_are_rebased_into_range() {
        // The classic bug: appending a second mesh without offsetting its
        // indices, which silently draws garbage triangles.
        let mut w = MeshWeld::new();
        w.add(&cube(1.0, 1.0, 1.0), Transform::IDENTITY, Color::WHITE);
        w.add(&cube(1.0, 1.0, 1.0), at(3.0, 0.0, 0.0), Color::WHITE);
        let mesh = w.build();
        let count = count_vertices(&mesh) as u32;
        let Some(Indices::U32(idx)) = mesh.indices() else {
            panic!("expected u32 indices");
        };
        assert!(idx.iter().all(|i| *i < count), "index out of range");
        assert!(idx.iter().any(|i| *i >= count / 2), "second mesh not offset");
    }

    #[test]
    fn transforms_are_applied_to_positions() {
        let mut w = MeshWeld::new();
        w.add(&cube(1.0, 1.0, 1.0), at(10.0, 0.0, 0.0), Color::WHITE);
        let mesh = w.build();
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(VertexAttributeValues::as_float3)
            .unwrap();
        assert!(positions.iter().all(|p| p[0] > 9.0));
    }

    #[test]
    fn normals_survive_non_uniform_scale() {
        // A squashed sphere's normals must stay unit length, or the lighting
        // on every flattened prop in the game goes wrong.
        let mut w = MeshWeld::new();
        w.add(
            &sphere(1.0),
            Transform::from_scale(Vec3::new(1.0, 0.2, 1.0)),
            Color::WHITE,
        );
        let mesh = w.build();
        let normals = mesh
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .and_then(VertexAttributeValues::as_float3)
            .unwrap();
        for n in normals {
            let len = Vec3::from_array(*n).length();
            assert!((len - 1.0).abs() < 1e-3, "normal length {len}");
        }
    }

    #[test]
    fn the_shape_helper_matches_adding_a_mesh() {
        let mut a = MeshWeld::new();
        a.shape(Cuboid::new(1.0, 1.0, 1.0), Transform::IDENTITY, Color::WHITE);
        let mut b = MeshWeld::new();
        b.add(&cube(1.0, 1.0, 1.0), Transform::IDENTITY, Color::WHITE);
        assert_eq!(count_vertices(&a.build()), count_vertices(&b.build()));
    }

    #[test]
    fn primitive_presets_all_produce_geometry() {
        for mesh in [
            sphere(1.0),
            sphere_hi(1.0),
            cylinder(1.0, 2.0),
            cylinder_hi(1.0, 2.0),
            cone(1.0, 2.0),
            torus(0.2, 1.0),
            cube(1.0, 2.0, 3.0),
        ] {
            assert!(count_vertices(&mesh) > 0);
        }
    }

    #[test]
    fn ground_grid_covers_the_requested_area() {
        let mesh = ground_grid(10.0, 5.0, 1.0, |_, _, _| GroundCell {
            color: Color::WHITE,
            height: 0.0,
        });
        // 20x10 cells, four unshared vertices each.
        assert_eq!(count_vertices(&mesh), 20 * 10 * 4);
    }

    #[test]
    fn ground_grid_stays_inside_its_bounds() {
        let mesh = ground_grid(7.0, 3.0, 0.9, |_, _, _| GroundCell {
            color: Color::WHITE,
            height: 0.0,
        });
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(VertexAttributeValues::as_float3)
            .unwrap();
        for p in positions {
            assert!(p[0] >= -7.0 - 1e-4 && p[0] <= 7.0 + 1e-4, "x {}", p[0]);
            assert!(p[2] >= -3.0 - 1e-4 && p[2] <= 3.0 + 1e-4, "z {}", p[2]);
        }
    }

    #[test]
    fn ground_grid_passes_cell_centres_to_the_callback() {
        let mut centres = Vec::new();
        let _ = ground_grid(2.0, 2.0, 1.0, |_, _, c| {
            centres.push(c);
            GroundCell {
                color: Color::WHITE,
                height: 0.0,
            }
        });
        assert_eq!(centres.len(), 16);
        assert!(centres.iter().all(|c| c.x.abs() <= 2.0 && c.y.abs() <= 2.0));
    }

    #[test]
    fn ground_grid_applies_per_cell_height() {
        let mesh = ground_grid(2.0, 2.0, 1.0, |ix, _, _| GroundCell {
            color: Color::WHITE,
            height: if ix == 0 { 1.0 } else { 0.0 },
        });
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(VertexAttributeValues::as_float3)
            .unwrap();
        assert!(positions.iter().any(|p| (p[1] - 1.0).abs() < 1e-5));
        assert!(positions.iter().any(|p| p[1].abs() < 1e-5));
    }

    #[test]
    fn noise_stays_in_the_unit_interval() {
        for i in 0..2000 {
            let x = i as f32 * 0.37 - 300.0;
            let z = i as f32 * -0.71 + 100.0;
            for f in [noise2(x, z, 7), noise_soft(x, z, 7)] {
                assert!((0.0..1.0).contains(&f), "noise gave {f}");
            }
        }
    }

    #[test]
    fn noise_is_deterministic_and_seed_sensitive() {
        assert_eq!(noise2(1.5, -2.5, 3), noise2(1.5, -2.5, 3));
        assert_ne!(noise2(1.5, -2.5, 3), noise2(1.5, -2.5, 4));
    }

    #[test]
    fn transform_helpers_place_and_rotate() {
        assert_eq!(at(1.0, 2.0, 3.0).translation, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(
            at_scaled(0.0, 0.0, 0.0, 2.0, 3.0, 4.0).scale,
            Vec3::new(2.0, 3.0, 4.0)
        );
        // A 90-degree Y rotation should send +Z to +X.
        let rotated = at_rot_y(0.0, 0.0, 0.0, 90.0).rotation * Vec3::Z;
        assert!((rotated - Vec3::X).length() < 1e-5);
        let x = at_rot_x(0.0, 0.0, 0.0, 90.0).rotation * Vec3::Y;
        assert!((x - Vec3::Z).length() < 1e-5);
        let z = at_rot_z(0.0, 0.0, 0.0, 90.0).rotation * Vec3::X;
        assert!((z - Vec3::Y).length() < 1e-5);
    }

    #[test]
    fn boxed_spans_the_given_corners() {
        let (shape, transform) = boxed(Vec3::new(-1.0, 0.0, -2.0), Vec3::new(3.0, 4.0, 2.0));
        assert_eq!(transform.translation, Vec3::new(1.0, 2.0, 0.0));
        assert!((shape.half_size.x - 2.0).abs() < 1e-5);
        assert!((shape.half_size.y - 2.0).abs() < 1e-5);
        assert!((shape.half_size.z - 2.0).abs() < 1e-5);
    }
}
