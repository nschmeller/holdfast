//! BLOCK 9 ROOFTOP - neon, rust, and something in the vents.
//!
//! Long sightlines down the service alleys between the plant units, broken by
//! hard cover. The arena that rewards turret lanes.

use bevy::prelude::*;

use super::{ChunkCtx, EnvLook, HazardSpec, PropSpec, Surface};
use crate::arena::{ColliderShape, Gust, HazardKind};
use crate::meshgen::{
    GroundCell, MeshWeld, at, at_rot_x, at_rot_z, cone, cube, cylinder, cylinder_hi, ground_grid,
    noise_soft, noise2, sphere, torus,
};
use crate::palette as pal;
use crate::rng::Rng;

use crate::world::CHUNK_SIZE;

/// Half a chunk, which is the extent every floor mesh is authored over.
const HALF: f32 = CHUNK_SIZE * 0.5;

const TAR: Color = Color::srgb(0.14, 0.145, 0.16);
const TAR_LIGHT: Color = Color::srgb(0.2, 0.205, 0.225);
const CONCRETE: Color = Color::srgb(0.42, 0.42, 0.44);
const CONCRETE_DARK: Color = Color::srgb(0.29, 0.29, 0.32);
const RUST: Color = Color::srgb(0.48, 0.26, 0.15);
const DUCT: Color = Color::srgb(0.56, 0.58, 0.6);
const NEON_PINK: Color = Color::srgb(1.0, 0.25, 0.6);
const NEON_CYAN: Color = Color::srgb(0.3, 0.9, 1.0);
const PUDDLE: Color = Color::srgb(0.15, 0.2, 0.26);

pub(super) fn look() -> EnvLook {
    EnvLook {
        sky: Color::srgb(0.03, 0.028, 0.05),
        ambient: Color::srgb(0.4, 0.42, 0.62),
        ambient_brightness: 230.0,
        sun_color: Color::srgb(0.6, 0.68, 1.0),
        sun_illuminance: 1800.0,
        sun_dir: Vec3::new(0.3, -1.0, -0.6),
        // A downdraft that runs the length of the roofs.
        gust: Gust {
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
        },
    }
}

pub(super) fn chunk(c: &mut ChunkCtx) {
    // The gap to the next roof. This world's signature: long sightlines, and
    // a long way down if something shoves you.
    if c.feature(0.2) {
        let p = c.spot(7.0);
        let r = c.rng.range(2.8, 4.6);
        c.chasm(p, r);
    }

    // -- water tower: the landmark you navigate by --------------------------
    if c.feature(0.1) {
        let p = c.spot(6.0);
        c.prop(PropSpec::new(water_tower(), p).solid(ColliderShape::Circle(3.0), 8.0));
    }

    // -- HVAC plant: the cover ----------------------------------------------
    for _ in 0..=c.rng.below(3) {
        let p = c.spot(4.0);
        let w = c.rng.range(2.2, 3.2);
        let d = c.rng.range(2.0, 2.4);
        let deg = c.rng.range(-90.0, 90.0);
        let mesh = ac_unit(w, d);
        c.prop(
            PropSpec::new(mesh, p)
                .rot(deg)
                .solid(ColliderShape::rect_rot(w, d, deg), 2.2),
        );
    }

    // -- ducting snaking between them ---------------------------------------
    for _ in 0..c.rng.below(3) {
        let p = c.spot(5.0);
        let len = c.rng.range(6.0, 8.0);
        let deg = c.rng.range(-90.0, 90.0);
        let mesh = duct_run(len);
        c.prop(
            PropSpec::new(mesh, p)
                .rot(deg)
                .solid(ColliderShape::rect_rot(len * 0.5, 0.8, deg), 1.4),
        );
    }

    // -- roof access, chimneys, dishes ---------------------------------------
    if c.feature(0.18) {
        let p = c.spot(3.0);
        let deg = c.rng.range(0.0, 360.0);
        c.prop(
            PropSpec::new(roof_door(), p)
                .rot(deg)
                .solid(ColliderShape::rect_rot(1.2, 0.8, deg), 2.4),
        );
    }
    if c.feature(0.2) {
        let p = c.spot(3.0);
        c.prop(PropSpec::new(chimney(), p).solid(ColliderShape::Circle(0.9), 3.2));
    }
    if c.feature(0.14) {
        let p = c.spot(3.5);
        let deg = c.rng.range(0.0, 360.0);
        c.prop(
            PropSpec::new(satellite_dish(), p)
                .rot(deg)
                .solid(ColliderShape::Circle(1.4), 2.6),
        );
    }

    // -- neon: the light, and the bright ground under it ---------------------
    if c.feature(0.2) {
        let p = c.spot(4.0);
        let deg = c.rng.range(0.0, 360.0);
        c.prop(
            PropSpec::new(neon_sign(), p)
                .rot(deg)
                .solid(ColliderShape::rect_rot(2.6, 0.4, deg), 3.4),
        );
        let tint = if c.rng.chance(0.5) {
            NEON_PINK
        } else {
            NEON_CYAN
        };
        c.light(Vec3::new(p.x, 3.0, p.y), tint, 300_000.0, 22.0);
        c.pool(p, 6.2, 0.25);
    }

    if c.feature(0.22) {
        let p = c.spot(3.0);
        let deg = c.rng.range(0.0, 360.0);
        c.prop(
            PropSpec::new(skylight(), p)
                .rot(deg)
                .raised(0.05)
                .surface(Surface::Glass),
        );
    }

    // -- vent pipes and crates ------------------------------------------------
    for _ in 0..=c.rng.below(4) {
        let p = c.spot(2.0);
        let mesh = vent_pipe(c.rng);
        c.prop(
            PropSpec::new(mesh, p)
                .solid(ColliderShape::Circle(0.4), 1.3)
                .passthrough(),
        );
    }
    for _ in 0..c.rng.below(3) {
        let p = c.spot(3.0);
        let deg = c.rng.range(0.0, 360.0);
        let mesh = crate_stack(c.rng);
        c.prop(
            PropSpec::new(mesh, p)
                .rot(deg)
                .solid(ColliderShape::rect_rot(1.1, 1.1, deg), 1.8),
        );
    }

    // -- steam vents: the timed hazard ----------------------------------------
    for _ in 0..c.rng.below(3) {
        let p = c.spot(3.0);
        let period = c.rng.range(4.0, 7.0);
        c.prop(
            PropSpec::new(vent_grate(), p)
                .raised(0.03)
                .surface(Surface::Metal),
        );
        c.hazard(HazardSpec {
            pos: p,
            radius: 2.2,
            kind: HazardKind::Scald,
            dps: 18.0,
            slow: 1.0,
            duty: Some((period, 0.35)),
        });
    }

    // -- puddles ---------------------------------------------------------------
    for _ in 0..=c.rng.below(3) {
        let p = c.spot(2.5);
        let r = c.rng.range(1.4, 2.6);
        let mesh = puddle(r);
        c.prop(PropSpec::new(mesh, p).raised(0.012).surface(Surface::Glass));
    }
}

pub(super) fn floor(origin: Vec2, salt: u32) -> Mesh {
    let seed = 0x70FF ^ salt;
    ground_grid(HALF, HALF, 1.1, |_, _, local| {
        let c = origin + local;
        let n = noise_soft(c.x * 0.4, c.y * 0.4, seed);
        // Tar paper laid in overlapping strips.
        let strip = (c.y / 4.0).floor() as i32;
        let seam = (c.y / 4.0).fract().abs() < 0.08;
        let patch = noise2(c.x * 0.09, c.y * 0.09, seed ^ 0x33) > 0.78;
        // Checker indices come from world space, not the chunk's own cell
        // numbering, or the pattern would restart at every boundary.
        let wx = (c.x / 1.1).floor() as i32;
        let wz = (c.y / 1.1).floor() as i32;

        let color = if seam {
            CONCRETE_DARK
        } else if patch {
            RUST
        } else if (strip + wx + wz).rem_euclid(2) == 0 {
            pal::shade(TAR, 0.9 + n * 0.3)
        } else {
            pal::shade(TAR_LIGHT, 0.85 + n * 0.3)
        };
        GroundCell { color, height: 0.0 }
    })
}

// -- props ------------------------------------------------------------------

pub(super) fn water_tower() -> Mesh {
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
    b.add(
        &cylinder_hi(2.5, 3.4),
        at(0.0, 6.2, 0.0),
        Color::srgb(0.36, 0.28, 0.22),
    );
    for i in 0..14 {
        let a = i as f32 / 14.0 * std::f32::consts::TAU;
        b.add(
            &cube(0.18, 3.4, 0.2),
            at(a.cos() * 2.5, 6.2, a.sin() * 2.5).with_rotation(Quat::from_rotation_y(-a)),
            Color::srgb(0.3, 0.23, 0.18),
        );
    }
    b.add(&torus(0.12, 2.55), at(0.0, 5.0, 0.0), RUST);
    b.add(&torus(0.12, 2.55), at(0.0, 7.4, 0.0), RUST);
    b.add(&cone(2.7, 1.4), at(0.0, 8.5, 0.0), CONCRETE_DARK);
    b.build()
}

pub(super) fn ac_unit(w: f32, d: f32) -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(w * 2.0, 2.0, d * 2.0), at(0.0, 1.0, 0.0), DUCT);
    b.add(
        &cube(w * 2.1, 0.2, d * 2.1),
        at(0.0, 2.05, 0.0),
        CONCRETE_DARK,
    );
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

pub(super) fn duct_run(len: f32) -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cylinder_hi(0.7, len), at_rot_z(0.0, 0.9, 0.0, 90.0), DUCT);
    // Segment bands.
    let segs = (len / 1.4).ceil() as i32;
    for i in 0..=segs {
        let x = -len * 0.5 + i as f32 * (len / segs as f32);
        b.add(
            &torus(0.08, 0.74),
            at_rot_z(x, 0.9, 0.0, 90.0),
            CONCRETE_DARK,
        );
    }
    // Feet.
    for x in [-len * 0.4, len * 0.4] {
        b.add(&cube(0.3, 0.4, 0.9), at(x, 0.2, 0.0), CONCRETE_DARK);
    }
    b.build()
}

pub(super) fn roof_door() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(3.6, 3.4, 3.0), at(0.0, 1.7, 0.0), CONCRETE);
    b.add(&cube(3.8, 0.24, 3.2), at(0.0, 3.5, 0.0), CONCRETE_DARK);
    b.add(&cube(1.6, 2.4, 0.14), at(0.0, 1.2, 1.55), RUST);
    b.add(&sphere(0.12), at(0.5, 1.2, 1.66), DUCT);
    b.build()
}

pub(super) fn chimney() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(
        &cube(2.8, 5.0, 2.8),
        at(0.0, 2.5, 0.0),
        Color::srgb(0.36, 0.22, 0.18),
    );
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

pub(super) fn satellite_dish() -> Mesh {
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

pub(super) fn neon_sign() -> Mesh {
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

pub(super) fn skylight() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(3.8, 0.36, 3.0), at(0.0, 0.18, 0.0), CONCRETE_DARK);
    b.add(
        &cube(3.4, 0.14, 2.6),
        at(0.0, 0.42, 0.0),
        Color::srgb(0.8, 0.7, 0.45),
    );
    // Mullions.
    b.add(&cube(3.5, 0.18, 0.14), at(0.0, 0.44, 0.0), CONCRETE_DARK);
    b.add(&cube(0.14, 0.18, 2.7), at(0.0, 0.44, 0.0), CONCRETE_DARK);
    b.build()
}

pub(super) fn vent_pipe(rng: &mut Rng) -> Mesh {
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

pub(super) fn crate_stack(rng: &mut Rng) -> Mesh {
    let mut b = MeshWeld::new();
    let n = 1 + rng.below(3);
    for i in 0..n {
        let s = 1.4 - i as f32 * 0.16;
        b.add(
            &cube(s, 0.8, s),
            at(
                rng.range(-0.12, 0.12),
                0.4 + i as f32 * 0.8,
                rng.range(-0.12, 0.12),
            )
            .with_rotation(Quat::from_rotation_y(rng.range(-0.3, 0.3))),
            if rng.chance(0.5) { RUST } else { CONCRETE_DARK },
        );
    }
    b.build()
}

pub(super) fn vent_grate() -> Mesh {
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

pub(super) fn puddle(r: f32) -> Mesh {
    let mut b = MeshWeld::new();
    b.add(
        &Mesh::from(Cylinder::new(r, 0.04).mesh().resolution(16)),
        Transform::IDENTITY,
        PUDDLE,
    );
    b.build()
}
