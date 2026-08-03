//! GRID ZERO - a test platform in hard vacuum. Something is testing back.
//!
//! Almost no cover by design. This is the arena where the squad *is* the wall:
//! barricades you build are the only terrain you get.

use bevy::prelude::*;

use super::{ChunkCtx, EnvLook, HazardSpec, PropSpec, Surface};
use crate::arena::{ColliderShape, Gust, HazardKind};
use crate::meshgen::{
    GroundCell, MeshWeld, at, cube, cylinder, cylinder_hi, ground_grid, noise_soft, noise2, sphere,
    torus,
};
use crate::palette as pal;
use crate::rng::Rng;

use crate::world::CHUNK_SIZE;

/// Half a chunk, which is the extent every floor mesh is authored over.
const HALF: f32 = CHUNK_SIZE * 0.5;

const PLATE: Color = Color::srgb(0.09, 0.11, 0.14);
const PLATE_LIT: Color = Color::srgb(0.13, 0.17, 0.22);
const SEAM: Color = Color::srgb(0.05, 0.06, 0.08);
const CYAN: Color = Color::srgb(0.3, 0.95, 1.0);
const MAGENTA: Color = Color::srgb(1.0, 0.28, 0.72);
const STEEL: Color = Color::srgb(0.5, 0.54, 0.6);
const STEEL_DARK: Color = Color::srgb(0.24, 0.27, 0.32);

pub(super) fn look() -> EnvLook {
    EnvLook {
        // Deep space: almost no ambient, so the emissives carry the whole read.
        sky: Color::srgb(0.004, 0.005, 0.012),
        ambient: Color::srgb(0.28, 0.42, 0.6),
        ambient_brightness: 150.0,
        sun_color: Color::srgb(0.7, 0.85, 1.0),
        sun_illuminance: 1500.0,
        sun_dir: Vec3::new(-0.35, -1.0, -0.5),
        // Gravity shear instead of wind.
        gust: Gust {
            interval: 10.0,
            duration: 3.0,
            cooldown: 9.0,
            remaining: 9.0,
            blowing: false,
            dir: Vec2::new(1.0, 0.0),
            lane_center_z: 0.0,
            lane_half_width: 5.0,
            strength: 14.0,
            enabled: true,
            label: "GRAV SHEAR",
        },
    }
}

pub(super) fn chunk(c: &mut ChunkCtx) {
    // Missing plates. The platform was never finished, and in a world with
    // almost no cover the holes are most of the terrain you get.
    if c.feature(0.16) {
        let p = c.spot(7.0);
        let r = c.rng.range(3.0, 5.0);
        c.chasm(p, r);
    }

    // -- core pylon: the landmark and the bright ground ---------------------
    if c.feature(0.08) {
        let p = c.spot(6.0);
        c.prop(PropSpec::new(core_pylon(), p).solid(ColliderShape::Circle(1.5), 5.0));
        c.light(Vec3::new(p.x, 4.0, p.y), CYAN, 500_000.0, 30.0);
        c.pool(p, 5.5, 0.3);
    }

    // -- energy pylons -------------------------------------------------------
    for i in 0..c.rng.below(3) {
        let p = c.spot(4.0);
        let color = if i % 2 == 0 { CYAN } else { MAGENTA };
        let mesh = energy_pylon(color);
        c.prop(PropSpec::new(mesh, p).solid(ColliderShape::Circle(1.0), 4.2));
        c.light(Vec3::new(p.x, 3.4, p.y), color, 260_000.0, 22.0);
    }

    // -- server monoliths: the only real cover -------------------------------
    for _ in 0..=c.rng.below(3) {
        let p = c.spot(3.0);
        let deg = c.rng.range(0.0, 360.0);
        c.prop(
            PropSpec::new(monolith(), p)
                .rot(deg)
                .solid(ColliderShape::rect_rot(1.4, 0.7, deg), 3.6),
        );
    }

    // -- holographic barriers: block shots, not movement ---------------------
    // A deliberate inversion of the usual rule, and the thing that makes this
    // world tactically distinct.
    for _ in 0..=c.rng.below(3) {
        let p = c.spot(3.0);
        let deg = c.rng.range(0.0, 360.0);
        c.prop(
            PropSpec::new(holo_barrier(), p)
                .rot(deg)
                .surface(Surface::Glass),
        );
    }

    // -- floating debris cubes, purely atmospheric ---------------------------
    for _ in 0..c.rng.below(9) + 5 {
        let p = c.spot(1.0);
        let y = c.rng.range(3.0, 9.0);
        let deg = c.rng.range(0.0, 360.0);
        let mesh = float_cube(c.rng);
        c.prop(
            PropSpec::new(mesh, p)
                .raised(y)
                .rot(deg)
                .surface(Surface::Metal),
        );
    }

    // -- low bollards, light cover --------------------------------------------
    for _ in 0..c.rng.below(6) + 2 {
        let p = c.spot(3.0);
        c.prop(
            PropSpec::new(bollard(), p)
                .solid(ColliderShape::Circle(0.45), 1.1)
                .passthrough(),
        );
    }

    // -- plasma conduits: the signature hazard --------------------------------
    // Long lines rather than pools, so they cut the ground into lanes that open
    // and close on a rhythm.
    for i in 0..c.rng.below(3) {
        let p = c.spot(6.0);
        let deg = c.rng.range(0.0, 360.0);
        let len = c.rng.range(7.0, 9.0);
        let period = c.rng.range(4.5, 6.5);
        // Approximate a line hazard with overlapping discs; cheap, and the
        // damage falloff at the ends actually reads better than a hard edge.
        let dir = Vec2::new(deg.to_radians().cos(), deg.to_radians().sin());
        let steps = (len / 1.6).ceil() as i32;
        for k in 0..=steps {
            let t = k as f32 / steps as f32 - 0.5;
            c.hazard(HazardSpec {
                pos: p + dir * (t * len),
                radius: 1.5,
                kind: HazardKind::Shock,
                dps: 26.0,
                slow: 1.0,
                duty: Some((period + i as f32 * 0.6, 0.4)),
            });
        }
        let mesh = conduit(len);
        c.prop(
            PropSpec::new(mesh, p)
                .rot(deg)
                .raised(0.02)
                .surface(Surface::Solid),
        );
    }
}

pub(super) fn floor(origin: Vec2, salt: u32) -> Mesh {
    let seed = 0x6D1D ^ salt;
    ground_grid(HALF, HALF, 1.0, |_, _, local| {
        let c = origin + local;
        // Hex-ish plating faked with an offset brick pattern, plus glowing
        // circuit traces on a sparse subset of cells. Indices are taken from
        // world space so the plating runs unbroken between chunks.
        let wx = (c.x).floor() as i32;
        let wz = (c.y).floor() as i32;
        let row_offset = if wz.rem_euclid(2) == 0 { 0.0 } else { 0.5 };
        let u = (c.x / 3.0 + row_offset).fract().abs();
        let seam = u < 0.06 || (c.y / 3.0).fract().abs() < 0.06;
        let trace = noise2(wx as f32 * 0.7, wz as f32 * 0.7, seed) > 0.93;
        let lit = noise_soft(c.x * 0.2, c.y * 0.2, seed ^ 0x77) > 0.7;

        let color = if trace {
            pal::shade(CYAN, 0.35)
        } else if seam {
            SEAM
        } else if lit {
            PLATE_LIT
        } else {
            PLATE
        };
        GroundCell {
            color,
            height: if seam { -0.03 } else { 0.0 },
        }
    })
}

// -- props ------------------------------------------------------------------

pub(super) fn core_pylon() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cylinder_hi(1.5, 0.5), at(0.0, 0.25, 0.0), STEEL_DARK);
    b.add(&cylinder_hi(1.1, 0.4), at(0.0, 0.6, 0.0), STEEL);
    // Three struts rising to a suspended core.
    for i in 0..3 {
        let a = i as f32 / 3.0 * std::f32::consts::TAU;
        b.add(
            &cube(0.26, 4.0, 0.26),
            at(a.cos() * 0.9, 2.6, a.sin() * 0.9)
                .with_rotation(Quat::from_rotation_y(-a) * Quat::from_rotation_x(0.16)),
            STEEL,
        );
    }
    b.add(
        &Sphere::new(0.9).mesh().ico(1).unwrap(),
        at(0.0, 4.4, 0.0),
        CYAN,
    );
    b.add(&torus(0.09, 1.4), at(0.0, 4.4, 0.0), MAGENTA);
    b.add(
        &torus(0.09, 1.4),
        at(0.0, 4.4, 0.0).with_rotation(Quat::from_rotation_x(1.57)),
        CYAN,
    );
    b.build()
}

pub(super) fn energy_pylon(color: Color) -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cylinder_hi(1.0, 0.4), at(0.0, 0.2, 0.0), STEEL_DARK);
    b.add(&cube(0.7, 3.4, 0.7), at(0.0, 1.9, 0.0), STEEL);
    // Emitter rings climbing the shaft.
    for i in 0..4 {
        b.add(&torus(0.06, 0.6), at(0.0, 0.9 + i as f32 * 0.8, 0.0), color);
    }
    b.add(
        &Sphere::new(0.55).mesh().ico(1).unwrap(),
        at(0.0, 3.9, 0.0),
        color,
    );
    b.build()
}

pub(super) fn monolith() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(2.6, 3.6, 1.2), at(0.0, 1.8, 0.0), STEEL_DARK);
    b.add(&cube(2.2, 3.2, 0.1), at(0.0, 1.8, 0.62), PLATE);
    // Status LEDs.
    let mut rng = Rng::seeded(0x11AA);
    for row in 0..10 {
        for col in 0..4 {
            if !rng.chance(0.55) {
                continue;
            }
            b.add(
                &cube(0.16, 0.1, 0.06),
                at(-0.75 + col as f32 * 0.5, 0.5 + row as f32 * 0.3, 0.68),
                if rng.chance(0.7) { CYAN } else { MAGENTA },
            );
        }
    }
    b.add(&cube(2.8, 0.2, 1.4), at(0.0, 3.7, 0.0), STEEL);
    b.build()
}

pub(super) fn holo_barrier() -> Mesh {
    let mut b = MeshWeld::new();
    for x in [-3.0f32, 3.0] {
        b.add(&cylinder(0.16, 2.6), at(x, 1.3, 0.0), STEEL);
        b.add(&sphere(0.24), at(x, 2.6, 0.0), CYAN);
    }
    // The field itself.
    b.add(&cube(6.0, 2.4, 0.08), at(0.0, 1.4, 0.0), CYAN);
    for i in 0..5 {
        b.add(
            &cube(6.0, 0.05, 0.12),
            at(0.0, 0.5 + i as f32 * 0.5, 0.0),
            CYAN,
        );
    }
    b.build()
}

pub(super) fn float_cube(rng: &mut Rng) -> Mesh {
    let mut b = MeshWeld::new();
    let s = rng.range(0.4, 1.3);
    b.add(&cube(s, s, s), Transform::IDENTITY, STEEL_DARK);
    b.add(&cube(s * 1.02, s * 0.1, s * 0.1), Transform::IDENTITY, CYAN);
    b.build()
}

pub(super) fn bollard() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cylinder(0.42, 1.0), at(0.0, 0.5, 0.0), STEEL_DARK);
    b.add(&torus(0.05, 0.44), at(0.0, 0.85, 0.0), MAGENTA);
    b.add(&cylinder(0.3, 0.1), at(0.0, 1.05, 0.0), STEEL);
    b.build()
}

pub(super) fn conduit(len: f32) -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(len, 0.06, 2.0), Transform::IDENTITY, SEAM);
    b.add(
        &cube(len, 0.08, 1.2),
        at(0.0, 0.02, 0.0),
        pal::shade(MAGENTA, 0.5),
    );
    // Rails either side.
    for z in [-1.05f32, 1.05] {
        b.add(&cube(len, 0.14, 0.16), at(0.0, 0.05, z), STEEL_DARK);
    }
    b.build()
}
