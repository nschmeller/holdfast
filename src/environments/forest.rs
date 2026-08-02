//! THE UNDERGROWTH - you are four inches tall and the moss is hostile.
//!
//! The widest arena. Long approaches and few hard walls, so squad positioning
//! and mobile defence beat static turret nests here.

use bevy::prelude::*;

use super::{HazardSpec, PropSpec, SceneData, Surface};
use crate::arena::{ColliderShape, Gust, HazardKind, Spotlight};
use crate::art::Glow;
use crate::meshgen::*;
use crate::palette as pal;
use crate::rng::Rng;

const HALF_X: f32 = 24.0;
const HALF_Z: f32 = 16.0;

const SOIL: Color = Color::srgb(0.21, 0.16, 0.11);
const SOIL_DARK: Color = Color::srgb(0.14, 0.11, 0.08);
const MOSS: Color = Color::srgb(0.22, 0.4, 0.2);
const MOSS_LIGHT: Color = Color::srgb(0.32, 0.53, 0.26);
const BARK: Color = Color::srgb(0.31, 0.23, 0.16);
const BARK_LIGHT: Color = Color::srgb(0.42, 0.32, 0.22);
const MUSHROOM_CAP: Color = Color::srgb(0.78, 0.28, 0.3);
const MUSHROOM_GLOW: Color = Color::srgb(0.5, 0.92, 0.86);
const STEM: Color = Color::srgb(0.9, 0.87, 0.78);
const WATER: Color = Color::srgb(0.16, 0.34, 0.42);
const PETAL: Color = Color::srgb(0.86, 0.72, 0.92);

pub fn build(rng: &mut Rng) -> SceneData {
    let mut s = SceneData::new(HALF_X, HALF_Z, floor(rng));

    s.sky = Color::srgb(0.02, 0.035, 0.032);
    s.ambient = Color::srgb(0.34, 0.52, 0.46);
    s.ambient_brightness = 240.0;
    s.sun_color = Color::srgb(0.66, 0.82, 1.0);
    s.sun_illuminance = 2400.0;
    s.sun_dir = Vec3::new(0.4, -1.0, 0.5);

    // Wind through the canopy, wider and gentler than the desk fan.
    s.gust = Gust {
        interval: 15.0,
        duration: 4.2,
        cooldown: 12.0,
        remaining: 12.0,
        blowing: false,
        dir: Vec2::new(0.75, -0.66).normalize(),
        lane_center_z: -2.0,
        lane_half_width: 7.5,
        strength: 8.5,
        enabled: true,
        label: "CANOPY WIND",
    };

    // A shaft of moonlight through the leaves.
    s.spotlight = Spotlight {
        center: Vec2::new(-8.0, 3.0),
        radius: 6.5,
        damage_bonus: 0.25,
        enabled: true,
        label: "MOONSHAFT",
    };

    // -- the fallen log: the arena's one real wall -------------------------
    s.prop(
        PropSpec::new(fallen_log(), Vec2::new(4.0, -8.0))
            .rot(-14.0)
            .solid(ColliderShape::rect_rot(9.0, 1.6, -14.0), 3.0),
    );

    // -- stumps ------------------------------------------------------------
    for (p, r, deg) in [
        (Vec2::new(-15.0, -9.0), 2.4f32, 0.0f32),
        (Vec2::new(16.0, 7.5), 2.0, 30.0),
        (Vec2::new(-4.0, 11.0), 1.7, -20.0),
    ] {
        s.prop(
            PropSpec::new(stump(r), p)
                .rot(deg)
                .solid(ColliderShape::Circle(r), 2.2),
        );
    }

    // -- glowing mushrooms: the light sources ------------------------------
    let shrooms = [
        (Vec2::new(-10.0, -3.0), 2.6f32),
        (Vec2::new(9.0, 9.0), 2.0),
        (Vec2::new(-18.0, 6.0), 2.2),
        (Vec2::new(18.0, -6.0), 1.8),
        (Vec2::new(0.5, 2.0), 1.5),
    ];
    for (p, scale) in shrooms {
        s.prop(
            PropSpec::new(glow_mushroom(scale), p)
                .solid(ColliderShape::Circle(0.55 * scale), 1.4 * scale),
        );
        s.light(
            Vec3::new(p.x, 2.0 * scale, p.y),
            MUSHROOM_GLOW,
            120_000.0 * scale,
            16.0,
        );
    }

    // -- roots arching out of the soil -------------------------------------
    for (p, deg, len) in [
        (Vec2::new(-13.0, 12.0), 24.0f32, 5.0f32),
        (Vec2::new(12.0, -13.0), -50.0, 6.0),
        (Vec2::new(20.0, 2.0), 80.0, 4.5),
        (Vec2::new(-20.0, -2.0), -70.0, 4.0),
    ] {
        s.prop(
            PropSpec::new(root_arch(len), p)
                .rot(deg)
                .solid(ColliderShape::rect_rot(len * 0.5, 0.7, deg), 1.6),
        );
    }

    // -- pebbles and acorns ------------------------------------------------
    for _ in 0..18 {
        let p = Vec2::new(
            rng.range(-HALF_X + 2.0, HALF_X - 2.0),
            rng.range(-HALF_Z + 2.0, HALF_Z - 2.0),
        );
        if p.length() < 5.5 {
            continue;
        }
        if rng.chance(0.55) {
            let r = rng.range(0.5, 1.1);
            s.prop(
                PropSpec::new(pebble(r, rng), p)
                    .rot(rng.range(0.0, 360.0))
                    .solid(ColliderShape::Circle(r * 0.9), r * 1.2),
            );
        } else {
            s.prop(
                PropSpec::new(acorn(), p)
                    .rot(rng.range(0.0, 360.0))
                    .solid(ColliderShape::Circle(0.4), 0.7)
                    .passthrough(),
            );
        }
    }

    // -- ferns and flowers, purely decorative ------------------------------
    for _ in 0..26 {
        let p = Vec2::new(
            rng.range(-HALF_X + 1.5, HALF_X - 1.5),
            rng.range(-HALF_Z + 1.5, HALF_Z - 1.5),
        );
        if rng.chance(0.6) {
            s.prop(PropSpec::new(fern(rng), p).rot(rng.range(0.0, 360.0)));
        } else {
            s.prop(PropSpec::new(flower(rng), p).rot(rng.range(0.0, 360.0)));
        }
    }

    // -- hazards: mud patches ----------------------------------------------
    for (p, r) in [
        (Vec2::new(-6.0, -12.0), 3.4f32),
        (Vec2::new(13.0, 12.0), 3.0),
        (Vec2::new(-19.0, -1.0), 2.6),
    ] {
        s.hazards.push(HazardSpec {
            pos: p,
            radius: r,
            kind: HazardKind::Sticky,
            dps: 0.0,
            slow: 0.45,
            duty: None,
        });
        s.prop(
            PropSpec::new(mud_patch(r), p)
                .raised(0.02)
                .surface(Surface::Matte),
        );
    }

    // A stagnant pool that actually stings.
    s.hazards.push(HazardSpec {
        pos: Vec2::new(17.0, -13.0),
        radius: 3.2,
        kind: HazardKind::Scald,
        dps: 5.0,
        slow: 0.7,
        duty: None,
    });
    s.prop(
        PropSpec::new(pool(3.2), Vec2::new(17.0, -13.0))
            .raised(0.015)
            .surface(Surface::Glass),
    );

    s.zones = vec![
        Vec2::new(-8.0, 3.0),
        Vec2::new(10.0, -3.0),
        Vec2::new(-16.0, -13.0),
        Vec2::new(18.0, 12.0),
        Vec2::new(0.0, 13.0),
    ];

    s
}

fn floor(rng: &mut Rng) -> Mesh {
    let seed = (rng.next_u64() & 0xFFFF) as u32;
    ground_grid(HALF_X, HALF_Z, 1.0, |_, _, c| {
        let n = noise_soft(c.x * 0.28, c.y * 0.28, seed);
        let patch = noise2(c.x * 0.11, c.y * 0.11, seed ^ 0x5A5A);
        let color = if patch > 0.62 {
            // Moss clumps.
            if n > 0.55 { MOSS_LIGHT } else { MOSS }
        } else if n > 0.72 {
            SOIL_DARK
        } else {
            pal::shade(SOIL, 0.85 + n * 0.35)
        };
        crate::meshgen::GroundCell {
            color,
            // A little unevenness so the ground does not read as a table.
            height: (n - 0.5) * 0.09,
        }
    })
}

// -- props ------------------------------------------------------------------

fn fallen_log() -> Mesh {
    let mut b = MeshWeld::new();
    // Trunk lying along X.
    b.add(
        &cylinder_hi(1.5, 17.0),
        at_rot_z(0.0, 1.5, 0.0, 90.0),
        BARK,
    );
    // Bark ridges.
    let mut rng = Rng::seeded(0xB4B4);
    for _ in 0..22 {
        let x = rng.range(-8.0, 8.0);
        let a = rng.range(0.0, std::f32::consts::TAU);
        b.add(
            &cube(rng.range(0.7, 2.0), 0.16, 0.4),
            at(x, 1.5 + a.sin() * 1.45, a.cos() * 1.45)
                .with_rotation(Quat::from_rotation_x(-a)),
            if rng.chance(0.5) { BARK_LIGHT } else { SOIL_DARK },
        );
    }
    // Cut end with rings.
    for (i, r) in [1.45f32, 1.05, 0.65, 0.3].iter().enumerate() {
        b.add(
            &cylinder_hi(*r, 0.06),
            at_rot_z(8.52 + i as f32 * 0.01, 1.5, 0.0, 90.0),
            if i % 2 == 0 { pal::CORK } else { BARK_LIGHT },
        );
    }
    // Moss on top.
    for _ in 0..14 {
        let x = rng.range(-8.0, 8.0);
        let z = rng.range(-1.0, 1.0);
        b.add(
            &sphere(rng.range(0.3, 0.6)),
            at(x, 2.85, z).with_scale(Vec3::new(1.0, 0.35, 1.0)),
            if rng.chance(0.5) { MOSS } else { MOSS_LIGHT },
        );
    }
    b.build()
}

fn stump(r: f32) -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cylinder_hi(r, 2.2), at(0.0, 1.1, 0.0), BARK);
    for (i, rr) in [0.9f32, 0.62, 0.35].iter().enumerate() {
        b.add(
            &cylinder_hi(r * rr, 0.05),
            at(0.0, 2.2 + i as f32 * 0.01, 0.0),
            if i % 2 == 0 { pal::CORK } else { BARK_LIGHT },
        );
    }
    // Roots splaying at the base.
    let mut rng = Rng::seeded(0x57);
    for i in 0..6 {
        let a = i as f32 / 6.0 * std::f32::consts::TAU + rng.range(-0.2, 0.2);
        b.add(
            &cone(0.3, r * 1.2),
            at(a.cos() * r * 0.8, 0.2, a.sin() * r * 0.8).with_rotation(
                Quat::from_rotation_y(-a) * Quat::from_rotation_x(1.4),
            ),
            BARK,
        );
    }
    b.build()
}

fn glow_mushroom(scale: f32) -> Mesh {
    let mut b = MeshWeld::new();
    let mut rng = Rng::seeded(0x5480 ^ (scale * 100.0) as u64);
    // One tall cap plus a cluster of small ones.
    b.add(
        &cylinder_hi(0.22 * scale, 1.5 * scale),
        at(0.0, 0.75 * scale, 0.0),
        STEM,
    );
    b.add(
        &sphere_hi(0.85 * scale),
        at(0.0, 1.5 * scale, 0.0).with_scale(Vec3::new(1.0, 0.62, 1.0)),
        MUSHROOM_CAP,
    );
    // Underside gills glow.
    b.add(
        &cylinder_hi(0.78 * scale, 0.1),
        at(0.0, 1.3 * scale, 0.0),
        MUSHROOM_GLOW,
    );
    // Spots.
    for _ in 0..7 {
        let a = rng.range(0.0, std::f32::consts::TAU);
        let r = rng.range(0.2, 0.7) * scale;
        b.add(
            &sphere(rng.range(0.08, 0.16) * scale),
            at(a.cos() * r, 1.75 * scale, a.sin() * r),
            STEM,
        );
    }
    for _ in 0..4 {
        let a = rng.range(0.0, std::f32::consts::TAU);
        let d = rng.range(0.7, 1.3) * scale;
        let ss = rng.range(0.3, 0.55) * scale;
        b.add(
            &cylinder(0.09 * scale, 0.6 * ss * 2.0),
            at(a.cos() * d, 0.3 * ss * 2.0, a.sin() * d),
            STEM,
        );
        b.add(
            &sphere(0.42 * ss * 2.0),
            at(a.cos() * d, 0.62 * ss * 2.0, a.sin() * d)
                .with_scale(Vec3::new(1.0, 0.6, 1.0)),
            MUSHROOM_GLOW,
        );
    }
    b.build()
}

fn root_arch(len: f32) -> Mesh {
    let mut b = MeshWeld::new();
    // Three segments forming a shallow arch out of and back into the soil.
    let steps = 7;
    for i in 0..steps {
        let t = i as f32 / (steps - 1) as f32;
        let x = (t - 0.5) * len;
        let y = (t * std::f32::consts::PI).sin() * 1.4;
        let r = 0.34 * (1.0 - (t - 0.5).abs() * 0.7);
        b.add(&sphere(r.max(0.12)), at(x, y.max(0.05), 0.0), BARK);
    }
    b.build()
}

fn pebble(r: f32, rng: &mut Rng) -> Mesh {
    let mut b = MeshWeld::new();
    let grey = 0.34 + rng.f32() * 0.2;
    b.add(
        &sphere(r),
        at(0.0, r * 0.55, 0.0).with_scale(Vec3::new(1.0, 0.7, rng.range(0.8, 1.2))),
        Color::srgb(grey, grey * 1.02, grey * 1.06),
    );
    b.build()
}

fn acorn() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(
        &sphere(0.34),
        at(0.0, 0.32, 0.0).with_scale(Vec3::new(1.0, 1.25, 1.0)),
        Color::srgb(0.66, 0.47, 0.24),
    );
    b.add(
        &sphere(0.36),
        at(0.0, 0.52, 0.0).with_scale(Vec3::new(1.0, 0.55, 1.0)),
        BARK,
    );
    b.add(&cylinder(0.05, 0.24), at(0.0, 0.72, 0.0), BARK);
    b.build()
}

fn fern(rng: &mut Rng) -> Mesh {
    let mut b = MeshWeld::new();
    let fronds = 5 + rng.below(4);
    for i in 0..fronds {
        let a = i as f32 / fronds as f32 * std::f32::consts::TAU;
        let len = rng.range(1.1, 2.0);
        let lean = rng.range(45.0, 72.0);
        b.add(
            &cone(0.16, len),
            at(a.cos() * 0.15, len * 0.3, a.sin() * 0.15).with_rotation(
                Quat::from_rotation_y(a) * Quat::from_rotation_x(lean.to_radians()),
            ),
            pal::shade(MOSS_LIGHT, rng.range(0.7, 1.25)),
        );
    }
    b.build()
}

fn flower(rng: &mut Rng) -> Mesh {
    let mut b = MeshWeld::new();
    let h = rng.range(0.8, 1.5);
    b.add(&cylinder(0.05, h), at(0.0, h * 0.5, 0.0), MOSS_LIGHT);
    let petals = 5;
    for i in 0..petals {
        let a = i as f32 / petals as f32 * std::f32::consts::TAU;
        b.add(
            &sphere(0.22),
            at(a.cos() * 0.22, h, a.sin() * 0.22).with_scale(Vec3::new(1.0, 0.34, 1.0)),
            pal::shade(PETAL, rng.range(0.8, 1.2)),
        );
    }
    b.add(&sphere(0.14), at(0.0, h + 0.05, 0.0), pal::PENCIL_YELLOW);
    b.build()
}

fn mud_patch(r: f32) -> Mesh {
    let mut b = MeshWeld::new();
    let mut rng = Rng::seeded((r * 977.0) as u64);
    b.add(
        &Mesh::from(Cylinder::new(r, 0.04).mesh().resolution(18)),
        Transform::IDENTITY,
        SOIL_DARK,
    );
    for _ in 0..8 {
        let p = rng.in_disc(r * 0.9);
        b.add(
            &Mesh::from(Cylinder::new(rng.range(0.3, 0.8), 0.05).mesh().resolution(10)),
            Transform::from_translation(p + Vec3::Y * 0.01),
            pal::shade(SOIL_DARK, rng.range(0.7, 1.3)),
        );
    }
    b.build()
}

fn pool(r: f32) -> Mesh {
    let mut b = MeshWeld::new();
    b.add(
        &Mesh::from(Cylinder::new(r, 0.06).mesh().resolution(22)),
        Transform::IDENTITY,
        WATER,
    );
    b.add(
        &Mesh::from(Cylinder::new(r * 1.1, 0.03).mesh().resolution(22)),
        Transform::from_xyz(0.0, -0.02, 0.0),
        SOIL_DARK,
    );
    b.build()
}
