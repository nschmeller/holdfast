//! THE ARCANE SANCTUM - a broken sanctum where the wards still hold. Barely.
//!
//! The only arena with friendly terrain: ley lines and the mana font heal
//! whatever stands in them. So does the enemy, which turns the healing tiles
//! into the objective rather than a gift.

use bevy::prelude::*;

use super::{HazardSpec, PropSpec, SceneData, Surface};
use crate::arena::{ColliderShape, Gust, HazardKind, Spotlight};
use crate::meshgen::{
    GroundCell, MeshWeld, at, at_rot_x, at_rot_z, cone, cube, cylinder, cylinder_hi, ground_grid,
    noise_soft, noise2, sphere, torus,
};
use crate::palette as pal;
use crate::rng::Rng;

const HALF_X: f32 = 22.0;
const HALF_Z: f32 = 15.0;

const STONE: Color = Color::srgb(0.19, 0.17, 0.26);
const STONE_LIGHT: Color = Color::srgb(0.27, 0.25, 0.36);
const STONE_DARK: Color = Color::srgb(0.12, 0.11, 0.18);
const RUNE_GOLD: Color = Color::srgb(1.0, 0.78, 0.32);
const CRYSTAL_VIOLET: Color = Color::srgb(0.68, 0.42, 1.0);
const CRYSTAL_CYAN: Color = Color::srgb(0.44, 0.86, 1.0);
const LEY: Color = Color::srgb(0.42, 1.0, 0.78);
const FLAME: Color = Color::srgb(0.6, 1.0, 0.7);
const MOSS_OLD: Color = Color::srgb(0.24, 0.34, 0.26);

pub fn build(rng: &mut Rng) -> SceneData {
    let mut s = SceneData::new(HALF_X, HALF_Z, floor(rng));

    s.sky = Color::srgb(0.022, 0.016, 0.045);
    s.ambient = Color::srgb(0.46, 0.38, 0.72);
    s.ambient_brightness = 250.0;
    s.sun_color = Color::srgb(0.78, 0.7, 1.0);
    s.sun_illuminance = 1900.0;
    s.sun_dir = Vec3::new(0.25, -1.0, -0.4);

    // Not wind - a pulse of raw mana that sweeps the sanctum.
    s.gust = Gust {
        interval: 14.0,
        duration: 3.2,
        cooldown: 12.0,
        remaining: 12.0,
        blowing: false,
        dir: Vec2::new(-0.7, 0.7).normalize(),
        lane_center_z: 0.0,
        lane_half_width: 8.0,
        strength: 10.0,
        enabled: true,
        label: "MANA SURGE",
    };

    s.spotlight = Spotlight {
        center: Vec2::new(0.0, -6.0),
        radius: 6.5,
        damage_bonus: 0.3,
        enabled: true,
        label: "WARD CIRCLE",
    };

    broken_edge(&mut s);

    // -- the obelisk -------------------------------------------------------
    s.prop(PropSpec::new(obelisk(), Vec2::new(0.0, -6.0)).solid(ColliderShape::Circle(1.6), 7.0));
    s.light(Vec3::new(0.0, 5.0, -6.0), CRYSTAL_VIOLET, 460_000.0, 30.0);

    // -- the portal arch ---------------------------------------------------
    s.prop(
        PropSpec::new(portal_arch(), Vec2::new(-16.0, 9.0))
            .rot(-34.0)
            .solid(ColliderShape::rect_rot(3.2, 0.7, -34.0), 5.5),
    );
    s.light(Vec3::new(-16.0, 3.0, 9.0), CRYSTAL_CYAN, 320_000.0, 24.0);

    // -- colonnade: a ring of pillars, half of them ruined -----------------
    let pillar_count = 10;
    for i in 0..pillar_count {
        let a = i as f32 / pillar_count as f32 * std::f32::consts::TAU;
        let p = Vec2::new(a.cos() * 15.5, a.sin() * 10.5);
        let intact = rng.chance(0.55);
        if intact {
            s.prop(
                PropSpec::new(pillar(rng.range(4.0, 6.5)), p)
                    .rot(rng.range(0.0, 360.0))
                    .solid(ColliderShape::Circle(0.95), 5.0),
            );
        } else {
            s.prop(
                PropSpec::new(broken_pillar(rng), p)
                    .rot(rng.range(0.0, 360.0))
                    .solid(ColliderShape::Circle(0.95), 1.6),
            );
            // The fallen drum lying beside it.
            let off = rng.unit_circle().truncate() * 2.4;
            s.prop(
                PropSpec::new(fallen_drum(), p + Vec2::new(off.x, off.y))
                    .rot(rng.range(0.0, 360.0))
                    .solid(ColliderShape::rect(1.6, 0.8), 0.9)
                    .passthrough(),
            );
        }
    }

    // -- crystal formations -------------------------------------------------
    for (p, scale, color) in [
        (Vec2::new(-9.0, -10.0), 1.5f32, CRYSTAL_VIOLET),
        (Vec2::new(11.0, 8.0), 1.8, CRYSTAL_CYAN),
        (Vec2::new(17.0, -9.0), 1.2, CRYSTAL_VIOLET),
        (Vec2::new(-19.0, -3.0), 1.0, CRYSTAL_CYAN),
        (Vec2::new(6.0, 12.0), 1.3, CRYSTAL_CYAN),
    ] {
        s.prop(
            PropSpec::new(crystal_cluster(scale, color, rng), p)
                .rot(rng.range(0.0, 360.0))
                .solid(ColliderShape::Circle(0.9 * scale), 2.5 * scale),
        );
        s.light(
            Vec3::new(p.x, 2.0 * scale, p.y),
            color,
            140_000.0 * scale,
            16.0,
        );
    }

    // -- braziers: the warm light ------------------------------------------
    for p in [
        Vec2::new(-5.0, -1.0),
        Vec2::new(5.0, -1.0),
        Vec2::new(-5.0, -11.0),
        Vec2::new(5.0, -11.0),
    ] {
        s.prop(PropSpec::new(brazier(), p).solid(ColliderShape::Circle(0.7), 1.9));
        s.light(Vec3::new(p.x, 2.2, p.y), FLAME, 130_000.0, 14.0);
    }

    // -- pedestals ----------------------------------------------------------
    for (p, deg) in [
        (Vec2::new(-12.0, 2.0), 20.0f32),
        (Vec2::new(13.0, 1.0), -30.0),
        (Vec2::new(2.0, 8.0), 55.0),
    ] {
        s.prop(
            PropSpec::new(pedestal(), p)
                .rot(deg)
                .solid(ColliderShape::Circle(0.6), 1.5)
                .passthrough(),
        );
    }

    // -- floating rubble, purely atmospheric --------------------------------
    for _ in 0..16 {
        let p = Vec2::new(rng.range(-HALF_X, HALF_X), rng.range(-HALF_Z, HALF_Z));
        s.prop(
            PropSpec::new(rubble(rng), p)
                .raised(rng.range(2.5, 8.0))
                .rot(rng.range(0.0, 360.0)),
        );
    }

    // -- ground rubble ------------------------------------------------------
    for _ in 0..14 {
        let p = Vec2::new(
            rng.range(-HALF_X + 2.0, HALF_X - 2.0),
            rng.range(-HALF_Z + 2.0, HALF_Z - 2.0),
        );
        if p.length() < 4.5 {
            continue;
        }
        s.prop(
            PropSpec::new(rubble(rng), p)
                .rot(rng.range(0.0, 360.0))
                .solid(ColliderShape::Circle(0.5), 0.7)
                .passthrough(),
        );
    }

    // -- ley lines: healing channels, the arena's signature -----------------
    // Four spokes radiating from the font. Whoever stands on them regenerates,
    // friend or foe, which makes them the real objective.
    for i in 0..4 {
        let a = i as f32 / 4.0 * std::f32::consts::TAU + 0.78;
        let dir = Vec2::new(a.cos(), a.sin());
        for k in 1..=6 {
            let p = dir * (k as f32 * 2.3) + Vec2::new(0.0, 4.0);
            s.hazards.push(HazardSpec {
                pos: p,
                radius: 1.4,
                kind: HazardKind::Font,
                // Negative dps: the damage pipeline reads this as healing.
                dps: -7.0,
                slow: 1.0,
                duty: None,
            });
        }
        s.prop(
            PropSpec::new(ley_line(14.0), Vec2::new(0.0, 4.0) + dir * 7.0)
                .rot(a.to_degrees())
                .raised(0.025)
                .surface(Surface::Solid),
        );
    }

    // The font at the hub.
    s.prop(
        PropSpec::new(mana_font(), Vec2::new(0.0, 4.0))
            .solid(ColliderShape::Circle(1.4), 1.2)
            .passthrough(),
    );
    s.hazards.push(HazardSpec {
        pos: Vec2::new(0.0, 4.0),
        radius: 2.6,
        kind: HazardKind::Font,
        dps: -14.0,
        slow: 1.0,
        duty: None,
    });
    s.light(Vec3::new(0.0, 2.0, 4.0), LEY, 220_000.0, 18.0);

    // -- one genuinely hostile tile: a void rift ---------------------------
    s.hazards.push(HazardSpec {
        pos: Vec2::new(-13.0, -12.0),
        radius: 3.0,
        kind: HazardKind::Scald,
        dps: 20.0,
        slow: 0.6,
        duty: Some((6.0, 0.45)),
    });
    s.prop(
        PropSpec::new(void_rift(3.0), Vec2::new(-13.0, -12.0))
            .raised(0.03)
            .surface(Surface::Solid),
    );

    s.zones = vec![
        Vec2::new(0.0, 4.0),
        Vec2::new(0.0, -6.0),
        Vec2::new(-16.0, 9.0),
        Vec2::new(15.0, -8.0),
        Vec2::new(-15.0, -8.0),
        Vec2::new(15.0, 9.0),
    ];

    s
}

pub(super) fn floor(rng: &mut Rng) -> Mesh {
    let seed = (rng.next_u64() & 0xFFFF) as u32;
    ground_grid(HALF_X, HALF_Z, 1.1, |ix, iz, c| {
        let n = noise_soft(c.x * 0.3, c.y * 0.3, seed);
        // Large flagstones with mortar seams.
        let seam = (c.x / 3.3).fract().abs() < 0.07 || (c.y / 3.3).fract().abs() < 0.07;
        let cracked = noise2(c.x * 0.13, c.y * 0.13, seed ^ 0x99) > 0.86;
        let mossy = noise2(c.x * 0.2, c.y * 0.2, seed ^ 0x1234) > 0.9;

        // A great inscribed circle around the obelisk.
        let r = (c - Vec2::new(0.0, -6.0)).length();
        let inscribed_ring = (r - 8.5).abs() < 0.35 || (r - 5.2).abs() < 0.22;
        // Radial rune ticks around it.
        let spoke = {
            let a = (c.y + 6.0).atan2(c.x);
            let ticks = (a / std::f32::consts::TAU * 24.0).fract().abs() < 0.08;
            ticks && r < 8.8 && r > 5.0
        };

        let color = if inscribed_ring || spoke {
            RUNE_GOLD
        } else if seam {
            STONE_DARK
        } else if mossy {
            MOSS_OLD
        } else if cracked {
            pal::shade(STONE_DARK, 1.2)
        } else if (ix / 3 + iz / 3) % 2 == 0 {
            pal::shade(STONE, 0.9 + n * 0.35)
        } else {
            pal::shade(STONE_LIGHT, 0.85 + n * 0.3)
        };

        GroundCell {
            color,
            height: if seam { -0.04 } else { (n - 0.5) * 0.04 },
        }
    })
}

/// A crumbling border instead of a clean rim: the sanctum is falling into the
/// void a piece at a time.
pub(super) fn broken_edge(scene: &mut SceneData) {
    let mut weld = MeshWeld::new();
    let mut rng = Rng::seeded(0xA2CA4E);
    for (cx, cz, sx, sz) in [
        (0.0f32, -HALF_Z, HALF_X, 0.4f32),
        (0.0, HALF_Z, HALF_X, 0.4),
        (-HALF_X, 0.0, 0.4, HALF_Z),
        (HALF_X, 0.0, 0.4, HALF_Z),
    ] {
        let along_x = sx > sz;
        let segments = if along_x { 26 } else { 18 };
        for index in 0..segments {
            let along = index as f32 / (segments - 1) as f32 - 0.5;
            // Random gaps where the wall has fallen away.
            if rng.chance(0.22) {
                continue;
            }
            let height = rng.range(0.5, 1.5);
            let (px, pz) = if along_x {
                (cx + along * sx * 2.0, cz)
            } else {
                (cx, cz + along * sz * 2.0)
            };
            weld.add(
                &cube(
                    if along_x { 1.5 } else { 0.8 },
                    height,
                    if along_x { 0.8 } else { 1.5 },
                ),
                at(px, height * 0.5, pz).with_rotation(Quat::from_rotation_y(rng.range(-0.1, 0.1))),
                if rng.chance(0.5) { STONE } else { STONE_LIGHT },
            );
        }
    }
    // Understructure so the platform reads as floating.
    weld.add(
        &cube(HALF_X * 2.0, 1.2, HALF_Z * 2.0),
        at(0.0, -0.7, 0.0),
        STONE_DARK,
    );
    for index in 0..10 {
        let a = index as f32 / 10.0 * std::f32::consts::TAU;
        weld.add(
            &cone(1.4, 3.5),
            at(a.cos() * HALF_X * 0.7, -2.6, a.sin() * HALF_Z * 0.7)
                .with_rotation(Quat::from_rotation_x(std::f32::consts::PI)),
            STONE_DARK,
        );
    }
    scene.prop(PropSpec::new(weld.build(), Vec2::ZERO));
}

// -- props ------------------------------------------------------------------

pub(super) fn obelisk() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(4.0, 0.5, 4.0), at(0.0, 0.25, 0.0), STONE_LIGHT);
    b.add(&cube(3.0, 0.4, 3.0), at(0.0, 0.7, 0.0), STONE);
    // Tapering shaft, faked with stacked shrinking blocks.
    for i in 0..7 {
        let t = i as f32 / 7.0;
        let w = 1.8 - t * 1.0;
        b.add(
            &cube(w, 0.8, w),
            at(0.0, 1.2 + i as f32 * 0.8, 0.0),
            if i % 2 == 0 { STONE } else { STONE_LIGHT },
        );
        // Glyph bands.
        b.add(
            &cube(w * 1.02, 0.12, w * 1.02),
            at(0.0, 1.2 + i as f32 * 0.8, 0.0),
            RUNE_GOLD,
        );
    }
    b.add(
        &Sphere::new(0.7).mesh().ico(0).unwrap(),
        at(0.0, 7.4, 0.0).with_scale(Vec3::new(1.0, 1.6, 1.0)),
        CRYSTAL_VIOLET,
    );
    b.build()
}

pub(super) fn portal_arch() -> Mesh {
    let mut b = MeshWeld::new();
    for side in [-1.0f32, 1.0] {
        b.add(&cube(1.0, 5.0, 1.0), at(side * 2.6, 2.5, 0.0), STONE);
        b.add(&cube(1.2, 0.3, 1.2), at(side * 2.6, 0.15, 0.0), STONE_LIGHT);
        for i in 0..5 {
            b.add(
                &cube(1.05, 0.1, 1.05),
                at(side * 2.6, 1.0 + i as f32 * 0.9, 0.0),
                RUNE_GOLD,
            );
        }
    }
    // Lintel, approximated as a stepped arch.
    for i in -3..=3 {
        let x = i as f32 * 0.85;
        let y = 5.2 + (1.0 - (i as f32 / 3.0).powi(2)) * 0.9;
        b.add(&cube(0.95, 0.7, 1.0), at(x, y, 0.0), STONE_LIGHT);
    }
    // The gate itself.
    b.add(&cube(4.4, 4.6, 0.12), at(0.0, 2.7, 0.0), CRYSTAL_CYAN);
    b.build()
}

pub(super) fn pillar(height: f32) -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cylinder_hi(1.1, 0.4), at(0.0, 0.2, 0.0), STONE_LIGHT);
    b.add(&cylinder_hi(0.9, 0.3), at(0.0, 0.5, 0.0), STONE);
    // Fluted shaft.
    b.add(
        &cylinder_hi(0.75, height),
        at(0.0, 0.65 + height * 0.5, 0.0),
        STONE,
    );
    for i in 0..10 {
        let a = i as f32 / 10.0 * std::f32::consts::TAU;
        b.add(
            &cube(0.14, height, 0.14),
            at(a.cos() * 0.74, 0.65 + height * 0.5, a.sin() * 0.74),
            STONE_LIGHT,
        );
    }
    let top = 0.65 + height;
    b.add(&cylinder_hi(0.95, 0.3), at(0.0, top + 0.15, 0.0), STONE);
    b.add(&cube(2.2, 0.4, 2.2), at(0.0, top + 0.5, 0.0), STONE_LIGHT);
    b.build()
}

pub(super) fn broken_pillar(rng: &mut Rng) -> Mesh {
    let mut b = MeshWeld::new();
    let h = rng.range(1.0, 2.2);
    b.add(&cylinder_hi(1.1, 0.4), at(0.0, 0.2, 0.0), STONE_LIGHT);
    b.add(&cylinder_hi(0.75, h), at(0.0, 0.4 + h * 0.5, 0.0), STONE);
    // Jagged top.
    for i in 0..6 {
        let a = i as f32 / 6.0 * std::f32::consts::TAU;
        b.add(
            &cube(0.4, rng.range(0.15, 0.5), 0.4),
            at(a.cos() * 0.4, 0.4 + h, a.sin() * 0.4),
            STONE_LIGHT,
        );
    }
    b.build()
}

pub(super) fn fallen_drum() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(
        &cylinder_hi(0.75, 1.6),
        at_rot_z(0.0, 0.75, 0.0, 90.0),
        STONE,
    );
    b.add(
        &cylinder_hi(0.8, 0.1),
        at_rot_z(0.82, 0.75, 0.0, 90.0),
        STONE_LIGHT,
    );
    b.add(
        &cylinder_hi(0.8, 0.1),
        at_rot_z(-0.82, 0.75, 0.0, 90.0),
        STONE_LIGHT,
    );
    b.build()
}

pub(super) fn crystal_cluster(scale: f32, color: Color, rng: &mut Rng) -> Mesh {
    let mut weld = MeshWeld::new();
    // A base rock the shards grow out of.
    weld.add(
        &sphere(0.9 * scale),
        at(0.0, 0.25 * scale, 0.0).with_scale(Vec3::new(1.0, 0.45, 1.0)),
        STONE_DARK,
    );
    let shards = 4 + rng.below(4);
    for index in 0..shards {
        let angle = index as f32 / shards as f32 * std::f32::consts::TAU + rng.range(-0.4, 0.4);
        let offset = rng.range(0.1, 0.6) * scale;
        let height = rng.range(1.2, 2.8) * scale;
        let lean = rng.range(-22.0, 22.0);
        weld.add(
            &Mesh::from(Cone::new(0.28 * scale, height).mesh().resolution(6)),
            at(angle.cos() * offset, height * 0.45, angle.sin() * offset).with_rotation(
                Quat::from_rotation_y(angle) * Quat::from_rotation_x(lean.to_radians()),
            ),
            pal::shade(color, rng.range(0.7, 1.3)),
        );
    }
    weld.build()
}

pub(super) fn brazier() -> Mesh {
    let mut b = MeshWeld::new();
    for i in 0..3 {
        let a = i as f32 / 3.0 * std::f32::consts::TAU;
        b.add(
            &cylinder(0.09, 1.3),
            at(a.cos() * 0.32, 0.65, a.sin() * 0.32)
                .with_rotation(Quat::from_rotation_z(a.cos() * 0.12)),
            STONE_DARK,
        );
    }
    b.add(&cone(0.68, 0.7), at_rot_x(0.0, 1.5, 0.0, 180.0), STONE);
    b.add(&torus(0.08, 0.62), at(0.0, 1.72, 0.0), RUNE_GOLD);
    // Flame: three tapering cones.
    let mut rng = Rng::seeded(0xF1A);
    for i in 0..3 {
        let a = i as f32 / 3.0 * std::f32::consts::TAU;
        b.add(
            &cone(rng.range(0.16, 0.3), rng.range(0.6, 1.1)),
            at(a.cos() * 0.14, 1.95, a.sin() * 0.14),
            FLAME,
        );
    }
    b.build()
}

pub(super) fn pedestal() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(1.2, 0.24, 1.2), at(0.0, 0.12, 0.0), STONE_LIGHT);
    b.add(&cylinder_hi(0.36, 1.1), at(0.0, 0.75, 0.0), STONE);
    b.add(&cube(1.0, 0.2, 1.0), at(0.0, 1.4, 0.0), STONE_LIGHT);
    // An open book, tilted.
    let t = at(0.0, 1.56, 0.0).with_rotation(Quat::from_rotation_x(-0.34));
    b.add(&cube(0.9, 0.1, 0.66), t, pal::PAPER);
    b.add(
        &cube(0.44, 0.14, 0.68),
        t * Transform::from_xyz(-0.24, -0.02, 0.0).with_rotation(Quat::from_rotation_z(0.16)),
        Color::srgb(0.4, 0.16, 0.2),
    );
    b.add(
        &cube(0.44, 0.14, 0.68),
        t * Transform::from_xyz(0.24, -0.02, 0.0).with_rotation(Quat::from_rotation_z(-0.16)),
        Color::srgb(0.4, 0.16, 0.2),
    );
    b.add(&sphere(0.14), at(0.0, 2.0, 0.0), RUNE_GOLD);
    b.build()
}

pub(super) fn rubble(rng: &mut Rng) -> Mesh {
    let mut b = MeshWeld::new();
    let n = 2 + rng.below(3);
    for _ in 0..n {
        let s = rng.range(0.25, 0.7);
        b.add(
            &cube(s, s * rng.range(0.6, 1.2), s * rng.range(0.7, 1.3)),
            Transform::from_translation(rng.in_disc(0.5) + Vec3::Y * rng.range(0.1, 0.4))
                .with_rotation(Quat::from_euler(
                    EulerRot::XYZ,
                    rng.range(0.0, 3.0),
                    rng.range(0.0, 3.0),
                    rng.range(0.0, 3.0),
                )),
            if rng.chance(0.5) { STONE } else { STONE_LIGHT },
        );
    }
    b.build()
}

pub(super) fn ley_line(len: f32) -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(len, 0.05, 1.5), Transform::IDENTITY, STONE_DARK);
    b.add(&cube(len, 0.07, 0.7), at(0.0, 0.02, 0.0), LEY);
    // Rune ticks along the channel.
    let n = (len / 1.6) as i32;
    for i in 0..n {
        let x = -len * 0.5 + (i as f32 + 0.5) * (len / n as f32);
        b.add(&cube(0.14, 0.09, 1.3), at(x, 0.03, 0.0), RUNE_GOLD);
    }
    b.build()
}

pub(super) fn mana_font() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cylinder_hi(2.4, 0.22), at(0.0, 0.11, 0.0), STONE_LIGHT);
    b.add(&torus(0.24, 1.9), at(0.0, 0.3, 0.0), STONE);
    b.add(&cylinder_hi(1.75, 0.16), at(0.0, 0.34, 0.0), LEY);
    // Floating runes above the basin.
    for i in 0..6 {
        let a = i as f32 / 6.0 * std::f32::consts::TAU;
        b.add(
            &cube(0.2, 0.2, 0.06),
            at(a.cos() * 1.2, 0.9, a.sin() * 1.2).with_rotation(Quat::from_rotation_y(-a)),
            RUNE_GOLD,
        );
    }
    b.add(
        &Sphere::new(0.34).mesh().ico(0).unwrap(),
        at(0.0, 1.3, 0.0),
        LEY,
    );
    b.build()
}

pub(super) fn void_rift(r: f32) -> Mesh {
    let mut b = MeshWeld::new();
    b.add(
        &Mesh::from(Cylinder::new(r, 0.05).mesh().resolution(20)),
        Transform::IDENTITY,
        Color::srgb(0.02, 0.0, 0.05),
    );
    b.add(
        &Mesh::from(Cylinder::new(r * 0.92, 0.06).mesh().resolution(20)),
        at(0.0, 0.01, 0.0),
        Color::srgb(0.35, 0.05, 0.5),
    );
    // Cracks radiating outwards.
    let mut rng = Rng::seeded(0x0111D);
    for i in 0..9 {
        let a = i as f32 / 9.0 * std::f32::consts::TAU + rng.range(-0.2, 0.2);
        b.add(
            &cube(rng.range(0.8, 1.8), 0.07, 0.14),
            at(a.cos() * r * 1.05, 0.02, a.sin() * r * 1.05)
                .with_rotation(Quat::from_rotation_y(-a)),
            Color::srgb(0.5, 0.1, 0.7),
        );
    }
    b.build()
}
