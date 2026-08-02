//! BLOCK 9 ROOFTOP - neon, rust, and something in the vents.
//!
//! Long sightlines down the service alleys between the plant units, broken by
//! hard cover. The arena that rewards turret lanes.

use bevy::prelude::*;

use super::{HazardSpec, PropSpec, SceneData, Surface};
use crate::arena::{ColliderShape, Gust, HazardKind, Spotlight};
use crate::art::Glow;
use crate::meshgen::*;
use crate::palette as pal;
use crate::rng::Rng;

const HALF_X: f32 = 22.0;
const HALF_Z: f32 = 15.0;

const TAR: Color = Color::srgb(0.14, 0.145, 0.16);
const TAR_LIGHT: Color = Color::srgb(0.2, 0.205, 0.225);
const CONCRETE: Color = Color::srgb(0.42, 0.42, 0.44);
const CONCRETE_DARK: Color = Color::srgb(0.29, 0.29, 0.32);
const RUST: Color = Color::srgb(0.48, 0.26, 0.15);
const DUCT: Color = Color::srgb(0.56, 0.58, 0.6);
const NEON_PINK: Color = Color::srgb(1.0, 0.25, 0.6);
const NEON_CYAN: Color = Color::srgb(0.3, 0.9, 1.0);
const PUDDLE: Color = Color::srgb(0.15, 0.2, 0.26);

pub fn build(rng: &mut Rng) -> SceneData {
    let mut s = SceneData::new(HALF_X, HALF_Z, floor(rng));

    s.sky = Color::srgb(0.03, 0.028, 0.05);
    s.ambient = Color::srgb(0.4, 0.42, 0.62);
    s.ambient_brightness = 230.0;
    s.sun_color = Color::srgb(0.6, 0.68, 1.0);
    s.sun_illuminance = 1800.0;
    s.sun_dir = Vec3::new(0.3, -1.0, -0.6);

    // A downdraft that runs the length of the roof.
    s.gust = Gust {
        interval: 13.0,
        duration: 3.4,
        cooldown: 11.0,
        remaining: 11.0,
        blowing: false,
        dir: Vec2::new(0.0, 1.0),
        lane_center_z: 0.0,
        lane_half_width: 6.0,
        strength: 11.0,
        enabled: true,
        label: "DOWNDRAFT",
    };

    s.spotlight = Spotlight {
        center: Vec2::new(-15.0, -10.0),
        radius: 6.2,
        damage_bonus: 0.25,
        enabled: true,
        label: "SIGN GLOW",
    };

    parapet(&mut s);

    // -- water tower: the landmark ----------------------------------------
    s.prop(
        PropSpec::new(water_tower(), Vec2::new(13.0, -9.0))
            .solid(ColliderShape::Circle(3.0), 8.0),
    );

    // -- HVAC plant: the cover ---------------------------------------------
    for (p, w, d, deg) in [
        (Vec2::new(-6.0, 6.0), 3.2f32, 2.4f32, 0.0f32),
        (Vec2::new(2.0, 9.5), 2.6, 2.0, 18.0),
        (Vec2::new(-13.0, 2.0), 2.8, 2.2, -12.0),
        (Vec2::new(9.0, 5.0), 2.2, 2.2, 42.0),
    ] {
        s.prop(
            PropSpec::new(ac_unit(w, d), p)
                .rot(deg)
                .solid(ColliderShape::rect_rot(w, d, deg), 2.2),
        );
    }

    // -- ducting snaking between them --------------------------------------
    for (p, len, deg) in [
        (Vec2::new(-2.0, 7.6), 7.0f32, 22.0f32),
        (Vec2::new(-9.5, 4.0), 6.0, -55.0),
        (Vec2::new(16.0, 4.0), 8.0, 78.0),
    ] {
        s.prop(
            PropSpec::new(duct_run(len), p)
                .rot(deg)
                .solid(ColliderShape::rect_rot(len * 0.5, 0.8, deg), 1.4),
        );
    }

    // -- roof access and chimney -------------------------------------------
    s.prop(
        PropSpec::new(roof_door(), Vec2::new(-17.0, 8.0))
            .rot(-8.0)
            .solid(ColliderShape::rect_rot(1.9, 1.6, -8.0), 3.4),
    );
    s.prop(
        PropSpec::new(chimney(), Vec2::new(6.0, -12.0))
            .solid(ColliderShape::rect(1.5, 1.5), 5.0),
    );

    // -- satellite dishes ---------------------------------------------------
    for (p, deg) in [
        (Vec2::new(19.0, 11.0), -140.0f32),
        (Vec2::new(-20.0, -4.0), 40.0),
    ] {
        s.prop(
            PropSpec::new(satellite_dish(), p)
                .rot(deg)
                .solid(ColliderShape::Circle(1.5), 3.0),
        );
    }

    // -- the neon sign: this roof's light source ---------------------------
    s.prop(
        PropSpec::new(neon_sign(), Vec2::new(-16.0, -11.0))
            .rot(24.0)
            .solid(ColliderShape::rect_rot(3.6, 0.5, 24.0), 4.5)
            .surface(Surface::Solid),
    );
    s.light(Vec3::new(-15.0, 4.0, -10.0), NEON_PINK, 420_000.0, 26.0);
    s.light(Vec3::new(12.0, 6.0, 8.0), NEON_CYAN, 200_000.0, 24.0);

    // -- skylights: fragile-looking, actually solid ------------------------
    for p in [Vec2::new(-3.0, -6.0), Vec2::new(4.0, -2.0)] {
        s.prop(
            PropSpec::new(skylight(), p)
                .solid(ColliderShape::rect(1.8, 1.4), 0.7)
                .passthrough(),
        );
        s.light(Vec3::new(p.x, 1.2, p.y), Color::srgb(1.0, 0.85, 0.5), 60_000.0, 10.0);
    }

    // -- clutter ------------------------------------------------------------
    for _ in 0..12 {
        let p = Vec2::new(
            rng.range(-HALF_X + 2.5, HALF_X - 2.5),
            rng.range(-HALF_Z + 2.5, HALF_Z - 2.5),
        );
        if p.length() < 5.0 {
            continue;
        }
        if rng.chance(0.5) {
            s.prop(
                PropSpec::new(vent_pipe(rng), p)
                    .solid(ColliderShape::Circle(0.45), 1.3)
                    .passthrough(),
            );
        } else {
            s.prop(
                PropSpec::new(crate_stack(rng), p)
                    .rot(rng.range(0.0, 360.0))
                    .solid(ColliderShape::rect(0.8, 0.8), 1.2),
            );
        }
    }

    // -- steam vents: the hazard that defines this roof --------------------
    for (i, p) in [
        Vec2::new(-8.0, -3.0),
        Vec2::new(7.0, 1.0),
        Vec2::new(0.0, 12.0),
        Vec2::new(17.0, -2.0),
    ]
    .into_iter()
    .enumerate()
    {
        s.hazards.push(HazardSpec {
            pos: p,
            radius: 2.6,
            kind: HazardKind::Scald,
            dps: 22.0,
            slow: 1.0,
            // Staggered phases so the roof is never entirely safe or entirely
            // lethal; there is always a route through.
            duty: Some((5.0 + i as f32 * 0.7, 0.32)),
        });
        s.prop(
            PropSpec::new(vent_grate(), p)
                .raised(0.03)
                .surface(Surface::Metal),
        );
    }

    // -- puddles: decorative, but they read as slick -----------------------
    for _ in 0..7 {
        let p = Vec2::new(
            rng.range(-HALF_X + 3.0, HALF_X - 3.0),
            rng.range(-HALF_Z + 3.0, HALF_Z - 3.0),
        );
        s.prop(
            PropSpec::new(puddle(rng.range(1.0, 2.4)), p)
                .raised(0.02)
                .surface(Surface::Glass),
        );
    }

    s.zones = vec![
        Vec2::new(-15.0, -10.0),
        Vec2::new(12.0, -8.0),
        Vec2::new(-4.0, 8.0),
        Vec2::new(18.0, 8.0),
        Vec2::new(0.0, -2.0),
    ];

    s
}

fn floor(rng: &mut Rng) -> Mesh {
    let seed = (rng.next_u64() & 0xFFFF) as u32;
    ground_grid(HALF_X, HALF_Z, 1.1, |ix, iz, c| {
        let n = noise_soft(c.x * 0.4, c.y * 0.4, seed);
        // Tar paper laid in overlapping strips.
        let strip = (c.y / 4.0).floor() as i32;
        let seam = (c.y / 4.0).fract().abs() < 0.08;
        let patch = noise2(c.x * 0.09, c.y * 0.09, seed ^ 0x33) > 0.78;

        let color = if seam {
            CONCRETE_DARK
        } else if patch {
            RUST
        } else if (strip + ix as i32 + iz as i32) % 2 == 0 {
            pal::shade(TAR, 0.9 + n * 0.3)
        } else {
            pal::shade(TAR_LIGHT, 0.85 + n * 0.3)
        };
        crate::meshgen::GroundCell {
            color,
            height: 0.0,
        }
    })
}

/// The roof edge wall. Solid, so it genuinely shapes the fight.
fn parapet(s: &mut SceneData) {
    let mut b = MeshWeld::new();
    let t = 0.6;
    let h = 1.5;
    for (cx, cz, sx, sz) in [
        (0.0f32, -HALF_Z - t * 0.5, HALF_X + t, t),
        (0.0, HALF_Z + t * 0.5, HALF_X + t, t),
        (-HALF_X - t * 0.5, 0.0, t, HALF_Z + t),
        (HALF_X + t * 0.5, 0.0, t, HALF_Z + t),
    ] {
        b.add(&cube(sx * 2.0, h, sz * 2.0), at(cx, h * 0.5, cz), CONCRETE);
        b.add(
            &cube(sx * 2.0 + 0.2, 0.16, sz * 2.0 + 0.2),
            at(cx, h, cz),
            CONCRETE_DARK,
        );
    }
    s.prop(PropSpec::new(b.build(), Vec2::ZERO));
}

// -- props ------------------------------------------------------------------

fn water_tower() -> Mesh {
    let mut b = MeshWeld::new();
    // Legs.
    for i in 0..4 {
        let a = i as f32 / 4.0 * std::f32::consts::TAU + 0.78;
        b.add(
            &cylinder(0.16, 4.4),
            at(a.cos() * 2.1, 2.2, a.sin() * 2.1)
                .with_rotation(Quat::from_rotation_x(0.06) * Quat::from_rotation_z(0.06)),
            RUST,
        );
    }
    // Cross bracing.
    for y in [1.4f32, 3.0] {
        b.add(&torus(0.07, 2.1), at(0.0, y, 0.0), RUST);
    }
    // Tank.
    b.add(&cylinder_hi(2.5, 3.4), at(0.0, 6.2, 0.0), Color::srgb(0.36, 0.28, 0.22));
    for i in 0..14 {
        let a = i as f32 / 14.0 * std::f32::consts::TAU;
        b.add(
            &cube(0.18, 3.4, 0.2),
            at(a.cos() * 2.5, 6.2, a.sin() * 2.5)
                .with_rotation(Quat::from_rotation_y(-a)),
            Color::srgb(0.3, 0.23, 0.18),
        );
    }
    b.add(&torus(0.12, 2.55), at(0.0, 5.0, 0.0), RUST);
    b.add(&torus(0.12, 2.55), at(0.0, 7.4, 0.0), RUST);
    b.add(&cone(2.7, 1.4), at(0.0, 8.5, 0.0), CONCRETE_DARK);
    b.build()
}

fn ac_unit(w: f32, d: f32) -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(w * 2.0, 2.0, d * 2.0), at(0.0, 1.0, 0.0), DUCT);
    b.add(&cube(w * 2.1, 0.2, d * 2.1), at(0.0, 2.05, 0.0), CONCRETE_DARK);
    // Fan grille on top.
    b.add(&cylinder(w * 0.62, 0.14), at(0.0, 2.2, 0.0), CONCRETE_DARK);
    for i in 0..4 {
        let a = i as f32 / 4.0 * std::f32::consts::PI;
        b.add(
            &cube(w * 1.2, 0.06, 0.12),
            at(0.0, 2.28, 0.0).with_rotation(Quat::from_rotation_y(a)),
            DUCT,
        );
    }
    // Side louvres.
    for i in 0..5 {
        b.add(
            &cube(0.08, 0.16, d * 1.8),
            at(w, 0.5 + i as f32 * 0.3, 0.0),
            CONCRETE_DARK,
        );
    }
    b.build()
}

fn duct_run(len: f32) -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cylinder_hi(0.7, len), at_rot_z(0.0, 0.9, 0.0, 90.0), DUCT);
    // Segment bands.
    let segs = (len / 1.4).ceil() as i32;
    for i in 0..=segs {
        let x = -len * 0.5 + i as f32 * (len / segs as f32);
        b.add(&torus(0.08, 0.74), at_rot_z(x, 0.9, 0.0, 90.0), CONCRETE_DARK);
    }
    // Feet.
    for x in [-len * 0.4, len * 0.4] {
        b.add(&cube(0.3, 0.4, 0.9), at(x, 0.2, 0.0), CONCRETE_DARK);
    }
    b.build()
}

fn roof_door() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(3.6, 3.4, 3.0), at(0.0, 1.7, 0.0), CONCRETE);
    b.add(&cube(3.8, 0.24, 3.2), at(0.0, 3.5, 0.0), CONCRETE_DARK);
    b.add(&cube(1.6, 2.4, 0.14), at(0.0, 1.2, 1.55), RUST);
    b.add(&sphere(0.12), at(0.5, 1.2, 1.66), DUCT);
    b.build()
}

fn chimney() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(2.8, 5.0, 2.8), at(0.0, 2.5, 0.0), Color::srgb(0.36, 0.22, 0.18));
    // Brick courses.
    for i in 0..10 {
        b.add(
            &cube(2.9, 0.08, 2.9),
            at(0.0, 0.3 + i as f32 * 0.5, 0.0),
            Color::srgb(0.28, 0.17, 0.14),
        );
    }
    b.add(&cube(3.2, 0.3, 3.2), at(0.0, 5.1, 0.0), CONCRETE_DARK);
    b.add(&cylinder(0.9, 0.7), at(0.0, 5.5, 0.0), CONCRETE_DARK);
    b.build()
}

fn satellite_dish() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cylinder(0.9, 0.3), at(0.0, 0.15, 0.0), CONCRETE_DARK);
    b.add(&cylinder(0.18, 2.4), at(0.0, 1.2, 0.0), DUCT);
    // Dish: a shallow cone tipped skyward.
    b.add(
        &cone(1.5, 0.9),
        at(0.0, 2.4, 0.3).with_rotation(Quat::from_rotation_x(2.3)),
        Color::srgb(0.72, 0.72, 0.7),
    );
    b.add(&cylinder(0.06, 1.2), at(0.0, 2.9, 1.0), DUCT);
    b.add(&sphere(0.16), at(0.0, 3.2, 1.35), CONCRETE_DARK);
    b.build()
}

fn neon_sign() -> Mesh {
    let mut b = MeshWeld::new();
    // Scaffold.
    for x in [-3.0f32, 3.0] {
        b.add(&cylinder(0.14, 4.6), at(x, 2.3, 0.0), RUST);
    }
    b.add(&cube(7.0, 0.16, 0.3), at(0.0, 4.5, 0.0), RUST);
    b.add(&cube(7.0, 0.16, 0.3), at(0.0, 1.6, 0.0), RUST);
    // Tubes: an abstract glyph, because a legible word would date instantly.
    b.add(&cube(0.9, 2.2, 0.16), at(-2.1, 3.1, 0.2), NEON_PINK);
    b.add(&cube(2.6, 0.36, 0.16), at(-0.6, 3.9, 0.2), NEON_PINK);
    b.add(&cube(0.36, 2.2, 0.16), at(0.9, 3.1, 0.2), NEON_CYAN);
    b.add(&cube(1.8, 0.36, 0.16), at(1.9, 2.3, 0.2), NEON_CYAN);
    b.add(&torus(0.14, 0.6), at_rot_x(2.4, 3.6, 0.2, 90.0), NEON_PINK);
    b.build()
}

fn skylight() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(3.8, 0.36, 3.0), at(0.0, 0.18, 0.0), CONCRETE_DARK);
    b.add(&cube(3.4, 0.14, 2.6), at(0.0, 0.42, 0.0), Color::srgb(0.8, 0.7, 0.45));
    // Mullions.
    b.add(&cube(3.5, 0.18, 0.14), at(0.0, 0.44, 0.0), CONCRETE_DARK);
    b.add(&cube(0.14, 0.18, 2.7), at(0.0, 0.44, 0.0), CONCRETE_DARK);
    b.build()
}

fn vent_pipe(rng: &mut Rng) -> Mesh {
    let mut b = MeshWeld::new();
    let h = rng.range(0.9, 1.6);
    b.add(&cylinder(0.34, h), at(0.0, h * 0.5, 0.0), DUCT);
    b.add(&torus(0.07, 0.38), at(0.0, h, 0.0), CONCRETE_DARK);
    // The classic bent cap.
    b.add(
        &cylinder(0.3, 0.6),
        at(0.0, h + 0.2, 0.2).with_rotation(Quat::from_rotation_x(1.0)),
        DUCT,
    );
    b.build()
}

fn crate_stack(rng: &mut Rng) -> Mesh {
    let mut b = MeshWeld::new();
    let n = 1 + rng.below(3);
    for i in 0..n {
        let s = 1.4 - i as f32 * 0.16;
        b.add(
            &cube(s, 0.8, s),
            at(rng.range(-0.12, 0.12), 0.4 + i as f32 * 0.8, rng.range(-0.12, 0.12))
                .with_rotation(Quat::from_rotation_y(rng.range(-0.3, 0.3))),
            if rng.chance(0.5) { RUST } else { CONCRETE_DARK },
        );
    }
    b.build()
}

fn vent_grate() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cylinder(1.5, 0.12), Transform::IDENTITY, CONCRETE_DARK);
    for i in 0..6 {
        b.add(
            &cube(2.6, 0.1, 0.18),
            at(0.0, 0.08, -1.0 + i as f32 * 0.4),
            DUCT,
        );
    }
    b.build()
}

fn puddle(r: f32) -> Mesh {
    let mut b = MeshWeld::new();
    b.add(
        &Mesh::from(Cylinder::new(r, 0.04).mesh().resolution(16)),
        Transform::IDENTITY,
        PUDDLE,
    );
    b.build()
}
