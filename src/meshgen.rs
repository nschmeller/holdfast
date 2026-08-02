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
