//! GRID ZERO - a test platform in hard vacuum. Something is testing back.
//!
//! Almost no cover by design. This is the arena where the squad *is* the wall:
//! barricades you build are the only terrain you get.

use bevy::prelude::*;

use super::{HazardSpec, PropSpec, SceneData, Surface};
use crate::arena::{ColliderShape, Gust, HazardKind, Spotlight};
use crate::meshgen::{
    GroundCell, MeshWeld, at, cube, cylinder, cylinder_hi, ground_grid, noise_soft, noise2, sphere,
    torus,
};
use crate::palette as pal;
use crate::rng::Rng;

const HALF_X: f32 = 21.0;
const HALF_Z: f32 = 14.0;

const PLATE: Color = Color::srgb(0.09, 0.11, 0.14);
const PLATE_LIT: Color = Color::srgb(0.13, 0.17, 0.22);
const SEAM: Color = Color::srgb(0.05, 0.06, 0.08);
const CYAN: Color = Color::srgb(0.3, 0.95, 1.0);
const MAGENTA: Color = Color::srgb(1.0, 0.28, 0.72);
const STEEL: Color = Color::srgb(0.5, 0.54, 0.6);
const STEEL_DARK: Color = Color::srgb(0.24, 0.27, 0.32);

pub fn build(rng: &mut Rng) -> SceneData {
    let mut s = SceneData::new(HALF_X, HALF_Z, floor(rng));

    // Deep space: almost no ambient, so the emissives carry the whole read.
    s.sky = Color::srgb(0.004, 0.005, 0.012);
    s.ambient = Color::srgb(0.28, 0.42, 0.6);
    s.ambient_brightness = 150.0;
    s.sun_color = Color::srgb(0.7, 0.85, 1.0);
    s.sun_illuminance = 1500.0;
    s.sun_dir = Vec3::new(-0.35, -1.0, -0.5);

    // Gravity shear instead of wind.
    s.gust = Gust {
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
    };

    s.spotlight = Spotlight {
        center: Vec2::ZERO,
        radius: 5.5,
        damage_bonus: 0.3,
        enabled: true,
        label: "CORE FIELD",
    };

    edge_rail(&mut s);

    // -- the core pylon at the centre --------------------------------------
    s.prop(PropSpec::new(core_pylon(), Vec2::ZERO).solid(ColliderShape::Circle(1.5), 5.0));
    s.light(Vec3::new(0.0, 4.0, 0.0), CYAN, 500_000.0, 30.0);

    // -- corner pylons ------------------------------------------------------
    for (i, p) in [
        Vec2::new(-15.0, -9.0),
        Vec2::new(15.0, -9.0),
        Vec2::new(-15.0, 9.0),
        Vec2::new(15.0, 9.0),
    ]
    .into_iter()
    .enumerate()
    {
        let color = if i % 2 == 0 { CYAN } else { MAGENTA };
        s.prop(PropSpec::new(energy_pylon(color), p).solid(ColliderShape::Circle(1.0), 4.2));
        s.light(Vec3::new(p.x, 3.4, p.y), color, 260_000.0, 22.0);
    }

    // -- server monoliths: the only real cover -----------------------------
    for (p, deg) in [
        (Vec2::new(-8.0, 4.0), 18.0f32),
        (Vec2::new(8.5, -4.5), -22.0),
        (Vec2::new(-3.0, -10.0), 66.0),
        (Vec2::new(4.0, 10.5), -70.0),
    ] {
        s.prop(
            PropSpec::new(monolith(), p)
                .rot(deg)
                .solid(ColliderShape::rect_rot(1.4, 0.7, deg), 3.6),
        );
    }

    // -- holographic barriers: block shots, not movement -------------------
    // A deliberate inversion of the usual rule, and the thing that makes this
    // arena tactically distinct.
    for (p, deg) in [
        (Vec2::new(-12.0, 0.0), 90.0f32),
        (Vec2::new(12.0, 2.0), 74.0),
        (Vec2::new(0.0, 7.5), 10.0),
    ] {
        s.prop(
            PropSpec::new(holo_barrier(), p)
                .rot(deg)
                .surface(Surface::Glass),
        );
    }

    // -- floating debris cubes, purely atmospheric -------------------------
    for _ in 0..14 {
        let p = Vec2::new(
            rng.range(-HALF_X + 1.0, HALF_X - 1.0),
            rng.range(-HALF_Z + 1.0, HALF_Z - 1.0),
        );
        s.prop(
            PropSpec::new(float_cube(rng), p)
                .raised(rng.range(3.0, 9.0))
                .rot(rng.range(0.0, 360.0))
                .surface(Surface::Metal),
        );
    }

    // -- low bollards, light cover ------------------------------------------
    for _ in 0..8 {
        let p = Vec2::new(
            rng.range(-HALF_X + 3.0, HALF_X - 3.0),
            rng.range(-HALF_Z + 3.0, HALF_Z - 3.0),
        );
        if p.length() < 4.0 {
            continue;
        }
        s.prop(
            PropSpec::new(bollard(), p)
                .solid(ColliderShape::Circle(0.45), 1.1)
                .passthrough(),
        );
    }

    // -- plasma conduits: the signature hazard -----------------------------
    // Long lines rather than pools, so they cut the arena into lanes that open
    // and close on a rhythm.
    for (i, (p, deg, len)) in [
        (Vec2::new(-6.0, -6.0), 34.0f32, 9.0f32),
        (Vec2::new(7.0, 6.0), -28.0, 8.0),
        (Vec2::new(-14.0, 5.0), 100.0, 7.0),
        (Vec2::new(14.0, -5.0), 80.0, 7.0),
    ]
    .into_iter()
    .enumerate()
    {
        // Approximate a line hazard with overlapping discs; cheap and the
        // damage falloff at the ends actually reads better than a hard edge.
        let dir = Vec2::new(deg.to_radians().cos(), deg.to_radians().sin());
        let steps = (len / 1.6).ceil() as i32;
        for k in 0..=steps {
            let t = k as f32 / steps as f32 - 0.5;
            s.hazards.push(HazardSpec {
                pos: p + dir * (t * len),
                radius: 1.5,
                kind: HazardKind::Scald,
                dps: 26.0,
                slow: 1.0,
                duty: Some((4.5 + i as f32 * 0.6, 0.4)),
            });
        }
        s.prop(
            PropSpec::new(conduit(len), p)
                .rot(deg)
                .raised(0.02)
                .surface(Surface::Solid),
        );
    }

    s.zones = vec![
        Vec2::new(0.0, 0.0),
        Vec2::new(-15.0, -9.0),
        Vec2::new(15.0, 9.0),
        Vec2::new(15.0, -9.0),
        Vec2::new(-15.0, 9.0),
    ];

    s
}

pub(super) fn floor(rng: &mut Rng) -> Mesh {
    let seed = (rng.next_u64() & 0xFFFF) as u32;
    ground_grid(HALF_X, HALF_Z, 1.0, |ix, iz, c| {
        // Hex-ish plating faked with an offset brick pattern, plus glowing
        // circuit traces on a sparse subset of cells.
        let row_offset = if iz % 2 == 0 { 0.0 } else { 0.5 };
        let u = (c.x / 3.0 + row_offset).fract().abs();
        let seam = u < 0.06 || (c.y / 3.0).fract().abs() < 0.06;
        let trace = noise2(ix as f32 * 0.7, iz as f32 * 0.7, seed) > 0.93;
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

/// A glowing rail marking the drop, so the edge is unmistakable.
pub(super) fn edge_rail(s: &mut SceneData) {
    let mut b = MeshWeld::new();
    let t = 0.25;
    for (cx, cz, sx, sz) in [
        (0.0f32, -HALF_Z, HALF_X, t),
        (0.0, HALF_Z, HALF_X, t),
        (-HALF_X, 0.0, t, HALF_Z),
        (HALF_X, 0.0, t, HALF_Z),
    ] {
        b.add(
            &cube(sx * 2.0, 0.12, sz * 2.0),
            at(cx, 0.06, cz),
            STEEL_DARK,
        );
        b.add(
            &cube(sx * 2.0, 0.06, sz * 2.0 * 0.5),
            at(cx, 0.16, cz),
            CYAN,
        );
    }
    // Under-platform structure, visible past the edge.
    b.add(
        &cube(HALF_X * 2.0, 0.8, HALF_Z * 2.0),
        at(0.0, -0.5, 0.0),
        STEEL_DARK,
    );
    for i in 0..12 {
        let a = i as f32 / 12.0 * std::f32::consts::TAU;
        b.add(
            &cube(1.0, 2.4, 1.0),
            at(a.cos() * HALF_X * 0.8, -1.6, a.sin() * HALF_Z * 0.8),
            SEAM,
        );
    }
    s.prop(PropSpec::new(b.build(), Vec2::ZERO));
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
