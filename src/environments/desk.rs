//! THE DESK - 2AM, one lamp, and the stationery has opinions.
//!
//! Tight and cluttered: the shortest sightlines of any arena, so barricades and
//! chokepoints matter more here than anywhere else.

use bevy::prelude::*;

use super::{HazardSpec, PropSpec, SceneData, Surface};
use crate::arena::{ColliderShape, Gust, HazardKind, Spotlight};
use crate::meshgen::{
    GroundCell, MeshWeld, at, at_rot_x, at_rot_y, at_rot_z, cone, cube, cylinder, cylinder_hi,
    ground_grid, noise_soft, sphere, torus,
};
use crate::palette as pal;
use crate::rng::Rng;

const HALF_X: f32 = 20.0;
const HALF_Z: f32 = 13.0;

pub fn build(rng: &mut Rng) -> SceneData {
    let mut s = SceneData::new(HALF_X, HALF_Z, floor());

    s.sky = Color::srgb(0.012, 0.012, 0.02);
    s.ambient = Color::srgb(0.36, 0.42, 0.7);
    s.ambient_brightness = 210.0;
    s.sun_color = Color::srgb(1.0, 0.86, 0.66);
    s.sun_illuminance = 2100.0;
    s.sun_dir = Vec3::new(-0.55, -1.0, 0.3);

    // The USB fan sweeps the near lane, shoving everything towards -X.
    s.gust = Gust {
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
    };

    s.spotlight = Spotlight {
        center: Vec2::new(12.5, -8.0),
        radius: 6.0,
        damage_bonus: 0.25,
        enabled: true,
        label: "LAMPLIGHT",
    };

    rim(&mut s);

    // -- the big furniture -------------------------------------------------
    s.prop(
        PropSpec::new(monitor(), Vec2::new(-5.0, -11.0)).solid(ColliderShape::rect(3.6, 1.0), 3.0),
    );
    s.light(
        Vec3::new(-5.0, 3.2, -9.0),
        pal::SCREEN_GLOW,
        260_000.0,
        26.0,
    );

    s.prop(
        PropSpec::new(desk_lamp(), Vec2::new(14.0, -10.0)).solid(ColliderShape::Circle(1.3), 2.4),
    );
    s.light(Vec3::new(13.2, 5.0, -9.2), pal::LAMP_GLOW, 700_000.0, 34.0);

    s.prop(
        PropSpec::new(keyboard(), Vec2::new(-1.0, 8.4)).solid(ColliderShape::rect(6.6, 2.3), 0.55),
    );

    // Mousepad: pure decoration, nothing collides with it.
    s.prop(
        PropSpec::new(mousepad(), Vec2::new(12.0, 6.0))
            .raised(0.012)
            .surface(Surface::Matte),
    );

    // -- mid clutter -------------------------------------------------------
    s.prop(
        PropSpec::new(coffee_mug(), Vec2::new(-13.5, 2.0)).solid(ColliderShape::Circle(1.15), 1.7),
    );
    s.prop(
        PropSpec::new(pen_holder(), Vec2::new(7.0, -6.5)).solid(ColliderShape::Circle(0.95), 2.2),
    );
    s.prop(
        PropSpec::new(book_stack(), Vec2::new(-16.0, -6.0))
            .rot(12.0)
            .solid(ColliderShape::rect_rot(2.4, 1.7, 12.0), 1.1),
    );
    s.prop(
        PropSpec::new(headphones(), Vec2::new(-11.5, -10.0))
            .rot(-24.0)
            .solid(ColliderShape::Circle(1.9), 1.0),
    );
    s.prop(
        PropSpec::new(plant_pot(), Vec2::new(17.5, -2.0)).solid(ColliderShape::Circle(1.25), 2.6),
    );
    s.prop(
        PropSpec::new(tape_dispenser(), Vec2::new(-17.0, 9.0))
            .rot(-18.0)
            .solid(ColliderShape::rect_rot(1.5, 0.9, -18.0), 1.2),
    );
    s.prop(
        PropSpec::new(stapler_prop(), Vec2::new(-7.5, 4.0))
            .rot(64.0)
            .solid(ColliderShape::rect_rot(1.7, 0.6, 64.0), 0.8),
    );
    s.prop(
        PropSpec::new(cable_coil(), Vec2::new(17.0, 8.5))
            .solid(ColliderShape::Circle(1.7), 0.35)
            .passthrough(),
    );
    s.prop(
        PropSpec::new(rubiks_cube(), Vec2::new(2.5, -6.0))
            .rot(22.0)
            .solid(ColliderShape::rect_rot(0.72, 0.72, 22.0), 1.4),
    );
    s.prop(
        PropSpec::new(calculator(), Vec2::new(-19.0, -0.5))
            .rot(84.0)
            .solid(ColliderShape::rect_rot(1.5, 0.9, 84.0), 0.3)
            .passthrough(),
    );

    // -- small scatter -----------------------------------------------------
    s.prop(PropSpec::new(sticky_pad(pal::STICKY_YELLOW), Vec2::new(4.0, -2.5)).rot(-9.0));
    s.prop(PropSpec::new(sticky_pad(pal::STICKY_PINK), Vec2::new(-3.0, -8.0)).rot(31.0));
    s.prop(PropSpec::new(sticky_pad(pal::STICKY_CYAN), Vec2::new(9.5, 11.0)).rot(-40.0));
    s.prop(
        PropSpec::new(eraser(), Vec2::new(-2.5, 4.5))
            .rot(41.0)
            .solid(ColliderShape::rect_rot(0.55, 0.32, 41.0), 0.3)
            .passthrough(),
    );
    s.prop(
        PropSpec::new(usb_stick(), Vec2::new(10.5, 1.5))
            .rot(-63.0)
            .solid(ColliderShape::rect_rot(0.55, 0.24, -63.0), 0.25)
            .passthrough(),
    );

    for _ in 0..14 {
        let p = Vec2::new(
            rng.range(-HALF_X + 2.0, HALF_X - 2.0),
            rng.range(-HALF_Z + 2.0, HALF_Z - 2.0),
        );
        // Keep the middle clear so the opening minute is never a maze.
        if p.length() < 5.0 {
            continue;
        }
        s.prop(
            PropSpec::new(loose_paperclip(), p)
                .rot(rng.range(0.0, 360.0))
                .surface(Surface::Metal),
        );
    }

    // -- hazards -----------------------------------------------------------
    // A cold coffee ring by the mug: slows, does not burn.
    s.hazards.push(HazardSpec {
        pos: Vec2::new(-11.0, 4.6),
        radius: 2.6,
        kind: HazardKind::Sticky,
        dps: 0.0,
        slow: 0.5,
        duty: None,
    });
    // Eraser shavings.
    s.hazards.push(HazardSpec {
        pos: Vec2::new(-1.0, 5.6),
        radius: 1.9,
        kind: HazardKind::Sticky,
        dps: 0.0,
        slow: 0.65,
        duty: None,
    });

    // -- territory ---------------------------------------------------------
    // Placed at the genuinely contested spots: under the lamp, behind the
    // monitor, in the keyboard's shadow, and out on the exposed flank.
    s.zones = vec![
        Vec2::new(12.0, -7.0),
        Vec2::new(-6.0, -7.5),
        Vec2::new(-1.0, 4.0),
        Vec2::new(-15.5, 8.0),
    ];

    s
}

/// Wood: planks running along X, with grain streaks and a warm falloff towards
/// the edges so the middle of the desk reads as the lit area.
fn floor() -> Mesh {
    ground_grid(HALF_X, HALF_Z, 0.9, |_, _, c| {
        let plank = (c.y / 3.2).floor();
        let grain = noise_soft(c.x * 0.7, plank * 9.3, 11);
        let seam = ((c.y / 3.2).fract().abs() - 0.5).abs() < 0.06;

        let base = if seam {
            pal::DESK_EDGE
        } else if grain > 0.62 {
            pal::DESK_WOOD_DARK
        } else {
            pal::DESK_WOOD
        };

        // Subtle radial warmth towards the lamp corner.
        let warmth = 1.0 - (c - Vec2::new(12.0, -8.0)).length() / 46.0;
        GroundCell {
            color: pal::shade(base, 0.82 + grain * 0.2 + warmth * 0.18),
            height: 0.0,
        }
    })
}

/// The raised lip around the desk. Visual only: the player is already held
/// inside by the arena bounds, and giving it a collider would just create a
/// sticky one-unit gutter around the whole playfield.
fn rim(s: &mut SceneData) {
    let mut b = MeshWeld::new();
    let t = 0.7;
    let h = 0.5;
    for (cx, cz, sx, sz) in [
        (0.0, -HALF_Z - t * 0.5, HALF_X + t, t),
        (0.0, HALF_Z + t * 0.5, HALF_X + t, t),
        (-HALF_X - t * 0.5, 0.0, t, HALF_Z + t),
        (HALF_X + t * 0.5, 0.0, t, HALF_Z + t),
    ] {
        b.add(
            &cube(sx * 2.0, h, sz * 2.0),
            at(cx, h * 0.5 - 0.05, cz),
            pal::DESK_EDGE,
        );
    }
    s.prop(PropSpec::new(b.build(), Vec2::ZERO));
}

// -- props ------------------------------------------------------------------

fn monitor() -> Mesh {
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

fn desk_lamp() -> Mesh {
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

fn keyboard() -> Mesh {
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

fn mousepad() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(9.0, 0.05, 7.0), at(0.0, 0.0, 0.0), pal::MOUSEPAD);
    b.add(
        &cube(9.4, 0.03, 7.4),
        at(0.0, -0.01, 0.0),
        pal::MOUSEPAD_TRIM,
    );
    b.build()
}

fn coffee_mug() -> Mesh {
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

fn pen_holder() -> Mesh {
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

fn book_stack() -> Mesh {
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

fn headphones() -> Mesh {
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

fn plant_pot() -> Mesh {
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

fn tape_dispenser() -> Mesh {
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

fn stapler_prop() -> Mesh {
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

fn cable_coil() -> Mesh {
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

fn rubiks_cube() -> Mesh {
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

fn calculator() -> Mesh {
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

fn sticky_pad(color: Color) -> Mesh {
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

fn eraser() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(1.1, 0.42, 0.62), at(0.0, 0.21, 0.0), pal::ERASER_PINK);
    b.add(&cube(0.5, 0.44, 0.64), at(0.0, 0.21, 0.0), pal::PAPER);
    b.build()
}

fn usb_stick() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(
        &cube(1.1, 0.26, 0.44),
        at(0.0, 0.13, 0.0),
        pal::PLASTIC_DARK,
    );
    b.add(&cube(0.5, 0.16, 0.34), at(0.7, 0.13, 0.0), pal::METAL);
    b.build()
}

fn loose_paperclip() -> Mesh {
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
