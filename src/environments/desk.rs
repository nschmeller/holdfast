//! THE DESK - 2AM, one lamp, and the stationery has opinions.
//!
//! Tight and cluttered: the shortest sightlines of any arena, so barricades and
//! chokepoints matter more here than anywhere else.

use bevy::prelude::*;

use super::{ChunkCtx, EnvLook, HazardSpec, PropSpec, Surface};
use crate::arena::{ColliderShape, Gust, HazardKind};
use crate::meshgen::{
    GroundCell, MeshWeld, at, at_rot_x, at_rot_y, at_rot_z, cone, cube, cylinder, cylinder_hi,
    ground_grid, noise_soft, sphere, torus,
};
use crate::palette as pal;
use crate::rng::Rng;
use crate::world::CHUNK_SIZE;

/// Half a chunk, which is the extent every floor mesh is authored over.
const HALF: f32 = CHUNK_SIZE * 0.5;

pub(super) fn look() -> EnvLook {
    EnvLook {
        sky: Color::srgb(0.012, 0.012, 0.02),
        ambient: Color::srgb(0.36, 0.42, 0.7),
        // Low, so the lamps actually do something.
        //
        // This file opens with "2AM, one lamp" and `palette.rs` with "one warm
        // desk lamp in a dark office", and a UX pass measured that floor two
        // metres from a lamp was no brighter than floor twenty metres away. The
        // lamps were never the problem - each one emits a 700k point light. The
        // problem was 210 of ambient plus 2100 of sun washing it out, so the
        // scene was uniformly lit and the art direction existed only in comments.
        ambient_brightness: 115.0,
        sun_color: Color::srgb(1.0, 0.86, 0.66),
        sun_illuminance: 1050.0,
        sun_dir: Vec3::new(-0.55, -1.0, 0.3),
        // The USB fan sweeps a lane, shoving everything towards -X.
        gust: Gust {
            interval: 12.0,
            duration: 2.8,
            cooldown: 10.0,
            remaining: 10.0,
            blowing: false,
            dir: Vec2::new(-1.0, 0.0),
            lane_center_z: 6.5,
            lane_half_width: 4.0,
            strength: 12.0,
            enabled: true,
            label: "USB FAN",
        },
    }
}

pub(super) fn chunk(c: &mut ChunkCtx) {
    // -- landmarks: rare, large, and worth walking towards -----------------
    if c.feature(0.10) {
        let p = c.spot(6.0);
        c.prop(PropSpec::new(monitor(), p).solid(ColliderShape::rect(3.6, 1.0), 3.0));
        c.light(
            Vec3::new(p.x, 3.2, p.y + 2.0),
            pal::SCREEN_GLOW,
            260_000.0,
            26.0,
        );
    }

    if c.feature(0.12) {
        let p = c.spot(5.0);
        c.prop(PropSpec::new(desk_lamp(), p).solid(ColliderShape::Circle(1.3), 2.4));
        c.light(
            Vec3::new(p.x - 0.8, 5.0, p.y + 0.8),
            pal::LAMP_GLOW,
            700_000.0,
            34.0,
        );
        // Lamplight is the world's damage-bonus ground: bright, useful, and
        // exactly where everything else can see you.
        c.pool(p + Vec2::new(-0.5, 1.0), 6.0, 0.25);
    }

    if c.feature(0.11) {
        let p = c.spot(7.0);
        c.prop(PropSpec::new(keyboard(), p).solid(ColliderShape::rect(6.6, 2.3), 0.55));
        // A mousepad usually sits alongside. Pure decoration.
        let pad = p + Vec2::new(c.rng.range(8.0, 11.0), c.rng.range(-2.0, 2.0));
        c.prop(
            PropSpec::new(mousepad(), pad)
                .raised(0.012)
                .surface(Surface::Matte),
        );
    }

    // The gap between two desks. Everything that falls in is gone, which is
    // what keeps knockback lethal now that the world has no edge.
    if c.feature(0.09) {
        let p = c.spot(7.0);
        let r = c.rng.range(2.6, 4.4);
        c.chasm(p, r);
    }

    // -- mid clutter: the working furniture --------------------------------
    for _ in 0..c.rng.below(4) + 2 {
        let p = c.spot(3.0);
        let deg = c.rng.range(0.0, 360.0);
        match c.rng.below(9) {
            0 => c.prop(PropSpec::new(coffee_mug(), p).solid(ColliderShape::Circle(1.15), 1.7)),
            1 => c.prop(PropSpec::new(pen_holder(), p).solid(ColliderShape::Circle(0.95), 2.2)),
            2 => c.prop(
                PropSpec::new(book_stack(), p)
                    .rot(deg)
                    .solid(ColliderShape::rect_rot(2.4, 1.7, deg), 1.1),
            ),
            3 => c.prop(
                PropSpec::new(headphones(), p)
                    .rot(deg)
                    .solid(ColliderShape::Circle(1.9), 1.0),
            ),
            4 => c.prop(PropSpec::new(plant_pot(), p).solid(ColliderShape::Circle(1.25), 2.6)),
            5 => c.prop(
                PropSpec::new(tape_dispenser(), p)
                    .rot(deg)
                    .solid(ColliderShape::rect_rot(1.5, 0.9, deg), 1.2),
            ),
            6 => c.prop(
                PropSpec::new(stapler_prop(), p)
                    .rot(deg)
                    .solid(ColliderShape::rect_rot(1.7, 0.6, deg), 0.8),
            ),
            7 => c.prop(
                PropSpec::new(cable_coil(), p)
                    .solid(ColliderShape::Circle(1.7), 0.35)
                    .passthrough(),
            ),
            _ => c.prop(
                PropSpec::new(rubiks_cube(), p)
                    .rot(deg)
                    .solid(ColliderShape::rect_rot(0.72, 0.72, deg), 1.4),
            ),
        };
    }

    if c.feature(0.3) {
        let p = c.spot(3.0);
        let deg = c.rng.range(0.0, 360.0);
        c.prop(
            PropSpec::new(calculator(), p)
                .rot(deg)
                .solid(ColliderShape::rect_rot(1.5, 0.9, deg), 0.3)
                .passthrough(),
        );
    }

    // -- small scatter, all of it walkable ---------------------------------
    for _ in 0..c.rng.below(7) + 5 {
        let p = c.spot(1.0);
        let deg = c.rng.range(0.0, 360.0);
        match c.rng.below(6) {
            0 => c.prop(PropSpec::new(sticky_pad(pal::STICKY_YELLOW), p).rot(deg)),
            1 => c.prop(PropSpec::new(sticky_pad(pal::STICKY_PINK), p).rot(deg)),
            2 => c.prop(PropSpec::new(sticky_pad(pal::STICKY_CYAN), p).rot(deg)),
            3 => c.prop(
                PropSpec::new(eraser(), p)
                    .rot(deg)
                    .solid(ColliderShape::rect_rot(0.55, 0.32, deg), 0.3)
                    .passthrough(),
            ),
            4 => c.prop(
                PropSpec::new(usb_stick(), p)
                    .rot(deg)
                    .solid(ColliderShape::rect_rot(0.55, 0.24, deg), 0.25)
                    .passthrough(),
            ),
            _ => c.prop(
                PropSpec::new(loose_paperclip(), p)
                    .rot(deg)
                    .surface(Surface::Metal),
            ),
        };
    }

    // -- hazards: spills and shavings, sticky rather than lethal -----------
    if c.feature(0.26) {
        let p = c.spot(4.0);
        let radius = c.rng.range(1.8, 2.8);
        let slow = c.rng.range(0.45, 0.65);
        c.hazard(HazardSpec {
            pos: p,
            radius,
            kind: HazardKind::Sticky,
            dps: 0.0,
            slow,
            duty: None,
        });
    }
}

/// Wood: planks running along X, with grain streaks and a warm falloff towards
/// the edges so the middle of the desk reads as the lit area.
pub(super) fn floor(origin: Vec2, salt: u32) -> Mesh {
    ground_grid(HALF, HALF, 0.9, |_, _, local| {
        // Sample in world space, not chunk space: two chunks meeting at a seam
        // have to agree about the plank they share.
        let c = origin + local;
        let plank = (c.y / 3.2).floor();
        let grain = noise_soft(c.x * 0.7, plank * 9.3, 0x0B ^ salt);
        let seam = ((c.y / 3.2).fract().abs() - 0.5).abs() < 0.06;

        let base = if seam {
            pal::DESK_EDGE
        } else if grain > 0.62 {
            pal::DESK_WOOD_DARK
        } else {
            pal::DESK_WOOD
        };

        // Slow warmth variation, so the endless desk is not endlessly uniform.
        let warmth = noise_soft(c.x * 0.02, c.y * 0.02, 0x4D ^ salt);
        GroundCell {
            color: pal::shade(base, 0.82 + grain * 0.2 + warmth * 0.18),
            height: 0.0,
        }
    })
}

// -- props ------------------------------------------------------------------

pub(super) fn monitor() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(4.4, 0.22, 2.0), at(0.0, 0.11, 0.4), pal::PLASTIC_DARK);
    b.add(&cylinder(0.34, 1.6), at(0.0, 0.9, 0.2), pal::PLASTIC_MID);
    // Panel, leaning back a touch.
    let lean = Transform::from_xyz(0.0, 2.6, 0.0)
        .with_rotation(Quat::from_rotation_x((-7.0f32).to_radians()));
    b.add(&cube(9.0, 5.2, 0.34), lean, pal::PLASTIC_DARK);
    b.add(
        &cube(8.4, 4.6, 0.06),
        lean * Transform::from_xyz(0.0, 0.1, 0.22),
        pal::SCREEN_DIM,
    );
    // A few glowing "windows" on the screen, because it is 2am and the build
    // is still running.
    for (x, y, w, h) in [
        (-2.4f32, 1.1f32, 3.0f32, 1.6f32),
        (1.2, 0.2, 4.2, 2.6),
        (-2.0, -1.6, 2.4, 1.0),
    ] {
        b.add(
            &cube(w, h, 0.04),
            lean * Transform::from_xyz(x, y, 0.26),
            pal::SCREEN_GLOW,
        );
    }
    b.build()
}

pub(super) fn desk_lamp() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cylinder(1.2, 0.24), at(0.0, 0.12, 0.0), pal::METAL_DARK);
    b.add(
        &cylinder(0.1, 3.4),
        at(0.0, 1.8, 0.15).with_rotation(Quat::from_rotation_x(0.14)),
        pal::METAL,
    );
    b.add(
        &cylinder(0.1, 2.2),
        at(0.0, 3.4, -0.9).with_rotation(Quat::from_rotation_x(-1.05)),
        pal::METAL,
    );
    b.add(
        &cone(1.15, 1.5),
        at(0.0, 3.7, -1.9).with_rotation(Quat::from_rotation_x(2.5)),
        pal::LAMP_SHADE,
    );
    b.add(&sphere(0.42), at(0.0, 3.15, -1.75), pal::LAMP_GLOW);
    b.build()
}

pub(super) fn keyboard() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(13.0, 0.4, 4.4), at(0.0, 0.2, 0.0), pal::PLASTIC_DARK);
    // Keycaps. A 14x4 grid with a spacebar row reads unmistakably as a
    // keyboard at any camera distance.
    for row in 0..4 {
        let z = -1.5 + row as f32 * 0.95;
        if row == 3 {
            b.add(&cube(5.4, 0.22, 0.7), at(0.4, 0.5, z), pal::KEYCAP);
            for (x, w) in [(-4.6f32, 1.1f32), (-3.3, 1.1), (4.0, 1.1), (5.3, 1.1)] {
                b.add(&cube(w, 0.22, 0.7), at(x, 0.5, z), pal::KEYCAP);
            }
            continue;
        }
        for col in 0..14 {
            let x = -5.85 + col as f32 * 0.9;
            b.add(&cube(0.72, 0.22, 0.72), at(x, 0.5, z), pal::KEYCAP);
        }
    }
    b.build()
}

pub(super) fn mousepad() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(9.0, 0.05, 7.0), at(0.0, 0.0, 0.0), pal::MOUSEPAD);
    b.add(
        &cube(9.4, 0.03, 7.4),
        at(0.0, -0.01, 0.0),
        pal::MOUSEPAD_TRIM,
    );
    b.build()
}

pub(super) fn coffee_mug() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cylinder_hi(1.0, 1.7), at(0.0, 0.85, 0.0), pal::CERAMIC);
    b.add(&cylinder_hi(0.88, 0.1), at(0.0, 1.62, 0.0), pal::COFFEE);
    b.add(&cylinder_hi(1.02, 0.2), at(0.0, 1.1, 0.0), pal::SCREEN_GLOW);
    b.add(
        &torus(0.11, 0.42),
        at_rot_y(1.15, 0.9, 0.0, 90.0),
        pal::CERAMIC,
    );
    b.build()
}

pub(super) fn pen_holder() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(
        &cylinder_hi(0.85, 1.7),
        at(0.0, 0.85, 0.0),
        pal::PLASTIC_MID,
    );
    b.add(
        &cylinder_hi(0.72, 0.1),
        at(0.0, 1.6, 0.0),
        pal::PLASTIC_DARK,
    );
    // Pens and pencils leaning at different angles.
    let pens = [
        (0.2f32, 0.1f32, 8.0f32, pal::PENCIL_YELLOW),
        (-0.3, 0.25, -13.0, pal::DANGER),
        (0.05, -0.3, 6.0, pal::SCREEN_GLOW),
        (0.35, -0.15, 16.0, pal::GRAPHITE),
    ];
    for (dx, dz, tilt, color) in pens {
        let t = at(dx, 2.2, dz).with_rotation(
            Quat::from_rotation_z(tilt.to_radians()) * Quat::from_rotation_x(dz * 0.4),
        );
        b.add(&cylinder(0.09, 2.6), t, color);
        b.add(
            &cone(0.09, 0.24),
            t * Transform::from_xyz(0.0, 1.4, 0.0),
            pal::CORK,
        );
    }
    b.build()
}

pub(super) fn book_stack() -> Mesh {
    let mut b = MeshWeld::new();
    let covers = [pal::DANGER, pal::LEAF, pal::SCREEN_GLOW];
    for (i, color) in covers.iter().enumerate() {
        let y = 0.18 + i as f32 * 0.36;
        let skew = (i as f32 - 1.0) * 5.0;
        let t = at(0.0, y, 0.0).with_rotation(Quat::from_rotation_y(skew.to_radians()));
        b.add(&cube(4.4, 0.34, 3.0), t, *color);
        b.add(
            &cube(4.1, 0.24, 2.8),
            t * Transform::from_xyz(0.1, 0.02, 0.0),
            pal::PAPER,
        );
    }
    b.build()
}

pub(super) fn headphones() -> Mesh {
    let mut b = MeshWeld::new();
    // Band lying flat, two cups.
    b.add(&torus(0.16, 1.5), at(0.0, 0.16, 0.0), pal::PLASTIC_DARK);
    for side in [-1.0f32, 1.0] {
        b.add(
            &cylinder_hi(0.72, 0.5),
            at(side * 1.5, 0.26, 0.0),
            pal::PLASTIC_MID,
        );
        b.add(
            &cylinder_hi(0.58, 0.14),
            at(side * 1.5, 0.54, 0.0),
            pal::PLASTIC_DARK,
        );
    }
    b.build()
}

pub(super) fn plant_pot() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(
        &cone(1.1, 1.5),
        at_rot_x(0.0, 0.75, 0.0, 180.0),
        pal::TERRACOTTA,
    );
    b.add(
        &cylinder_hi(1.0, 0.2),
        at(0.0, 1.4, 0.0),
        pal::shade(pal::TERRACOTTA, 0.8),
    );
    // A small succulent: overlapping leaf blades.
    let mut rng = Rng::seeded(0x9F1A);
    for i in 0..11 {
        let a = i as f32 / 11.0 * std::f32::consts::TAU;
        let lean = rng.range(35.0, 65.0);
        b.add(
            &cone(0.2, rng.range(0.8, 1.4)),
            at(a.cos() * 0.3, 1.8, a.sin() * 0.3)
                .with_rotation(Quat::from_rotation_y(a) * Quat::from_rotation_x(lean.to_radians())),
            pal::shade(pal::LEAF, rng.range(0.75, 1.2)),
        );
    }
    b.build()
}

pub(super) fn tape_dispenser() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(2.6, 0.5, 1.4), at(0.0, 0.25, 0.0), pal::PLASTIC_DARK);
    b.add(&cube(1.2, 1.0, 1.2), at(-0.6, 0.7, 0.0), pal::PLASTIC_MID);
    b.add(
        &torus(0.34, 0.66),
        at_rot_y(0.5, 0.95, 0.0, 90.0),
        pal::CERAMIC,
    );
    b.add(&cube(0.7, 0.1, 1.2), at(1.2, 0.5, 0.0), pal::METAL);
    b.build()
}

pub(super) fn stapler_prop() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(3.2, 0.36, 1.1), at(0.0, 0.18, 0.0), pal::PLASTIC_DARK);
    b.add(
        &cube(3.0, 0.42, 1.0),
        at(0.1, 0.6, 0.0).with_rotation(Quat::from_rotation_z(0.06)),
        pal::DANGER,
    );
    b.add(
        &cylinder(0.22, 1.0),
        at_rot_z(-1.5, 0.5, 0.0, 90.0),
        pal::METAL,
    );
    b.build()
}

pub(super) fn cable_coil() -> Mesh {
    let mut b = MeshWeld::new();
    // A loose coil: three offset rings.
    for (i, r) in [1.5f32, 1.15, 0.8].iter().enumerate() {
        b.add(
            &torus(0.13, *r),
            at(i as f32 * 0.12, 0.14 + i as f32 * 0.02, i as f32 * -0.1),
            pal::PLASTIC_DARK,
        );
    }
    b.add(&cube(0.5, 0.3, 0.7), at(1.7, 0.16, 0.6), pal::METAL);
    b.build()
}

pub(super) fn rubiks_cube() -> Mesh {
    let mut b = MeshWeld::new();
    let faces = [
        pal::DANGER,
        pal::LEAF,
        pal::SCREEN_GLOW,
        pal::PENCIL_YELLOW,
        pal::DUCK_BEAK,
        pal::PAPER,
    ];
    b.add(
        &cube(1.44, 1.44, 1.44),
        at(0.0, 0.72, 0.0),
        pal::PLASTIC_DARK,
    );
    // Sticker grid on each face.
    for (fi, normal) in [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z]
        .iter()
        .enumerate()
    {
        for u in -1..=1 {
            for v in -1..=1 {
                let (a, bb) = if normal.x != 0.0 {
                    (Vec3::Y, Vec3::Z)
                } else if normal.y != 0.0 {
                    (Vec3::X, Vec3::Z)
                } else {
                    (Vec3::X, Vec3::Y)
                };
                let p = *normal * 0.735
                    + a * (u as f32 * 0.46)
                    + bb * (v as f32 * 0.46)
                    + Vec3::new(0.0, 0.72, 0.0);
                b.add(
                    &cube(0.4, 0.4, 0.4),
                    Transform::from_translation(p).with_scale(Vec3::ONE - normal.abs() * 0.88),
                    faces[fi],
                );
            }
        }
    }
    b.build()
}

pub(super) fn calculator() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(2.8, 0.24, 1.7), at(0.0, 0.12, 0.0), pal::PLASTIC_MID);
    b.add(&cube(2.2, 0.06, 0.5), at(0.0, 0.26, -0.5), pal::SCREEN_DIM);
    for row in 0..3 {
        for col in 0..5 {
            b.add(
                &cube(0.3, 0.08, 0.22),
                at(-1.0 + col as f32 * 0.5, 0.26, 0.05 + row as f32 * 0.32),
                pal::PLASTIC_DARK,
            );
        }
    }
    b.build()
}

pub(super) fn sticky_pad(color: Color) -> Mesh {
    let mut b = MeshWeld::new();
    for i in 0..4 {
        b.add(
            &cube(2.2, 0.04, 2.2),
            at(i as f32 * 0.02, 0.02 + i as f32 * 0.04, i as f32 * 0.015),
            pal::shade(color, 1.0 - i as f32 * 0.04),
        );
    }
    b.build()
}

pub(super) fn eraser() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(1.1, 0.42, 0.62), at(0.0, 0.21, 0.0), pal::ERASER_PINK);
    b.add(&cube(0.5, 0.44, 0.64), at(0.0, 0.21, 0.0), pal::PAPER);
    b.build()
}

pub(super) fn usb_stick() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(
        &cube(1.1, 0.26, 0.44),
        at(0.0, 0.13, 0.0),
        pal::PLASTIC_DARK,
    );
    b.add(&cube(0.5, 0.16, 0.34), at(0.7, 0.13, 0.0), pal::METAL);
    b.build()
}

pub(super) fn loose_paperclip() -> Mesh {
    let mut b = MeshWeld::new();
    for (i, r) in [0.3f32, 0.19].iter().enumerate() {
        b.add(
            &torus(0.035, *r),
            at(i as f32 * 0.06, 0.04, 0.0).with_scale(Vec3::new(0.55, 1.0, 1.0)),
            pal::CLIP_STEEL,
        );
    }
    b.build()
}
