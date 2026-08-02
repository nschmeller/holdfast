//! THE ARCANE SANCTUM - a broken sanctum where the wards still hold. Barely.
//!
//! The only arena with friendly terrain: ley lines and the mana font heal
//! whatever stands in them. So does the enemy, which turns the healing tiles
//! into the objective rather than a gift.

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

const STONE: Color = Color::srgb(0.19, 0.17, 0.26);
const STONE_LIGHT: Color = Color::srgb(0.27, 0.25, 0.36);
const STONE_DARK: Color = Color::srgb(0.12, 0.11, 0.18);
const RUNE_GOLD: Color = Color::srgb(1.0, 0.78, 0.32);
const CRYSTAL_VIOLET: Color = Color::srgb(0.68, 0.42, 1.0);
const CRYSTAL_CYAN: Color = Color::srgb(0.44, 0.86, 1.0);
const LEY: Color = Color::srgb(0.42, 1.0, 0.78);
const FLAME: Color = Color::srgb(0.6, 1.0, 0.7);
const MOSS_OLD: Color = Color::srgb(0.24, 0.34, 0.26);

pub(super) fn look() -> EnvLook {
    EnvLook {
        sky: Color::srgb(0.022, 0.016, 0.045),
        ambient: Color::srgb(0.46, 0.38, 0.72),
        ambient_brightness: 250.0,
        sun_color: Color::srgb(0.78, 0.7, 1.0),
        sun_illuminance: 1900.0,
        sun_dir: Vec3::new(0.25, -1.0, -0.4),
        // Not wind - a pulse of raw mana that sweeps the sanctum.
        gust: Gust {
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
        },
    }
}

pub(super) fn chunk(c: &mut ChunkCtx) {
    // -- the obelisk ---------------------------------------------------------
    if c.feature(0.09) {
        let p = c.spot(6.0);
        c.prop(PropSpec::new(obelisk(), p).solid(ColliderShape::Circle(1.6), 7.0));
        c.light(Vec3::new(p.x, 5.0, p.y), CRYSTAL_VIOLET, 460_000.0, 30.0);
        c.pool(p, 6.5, 0.3);
    }

    // -- a portal arch -------------------------------------------------------
    if c.feature(0.1) {
        let p = c.spot(5.0);
        let deg = c.rng.range(0.0, 360.0);
        c.prop(
            PropSpec::new(portal_arch(), p)
                .rot(deg)
                .solid(ColliderShape::rect_rot(3.2, 0.7, deg), 5.5),
        );
        c.light(Vec3::new(p.x, 3.0, p.y), CRYSTAL_CYAN, 320_000.0, 24.0);
    }

    // -- a colonnade, half of it ruined --------------------------------------
    if c.feature(0.14) {
        let hub = c.center();
        for i in 0..10 {
            let a = i as f32 / 10.0 * std::f32::consts::TAU;
            let p = hub + Vec2::new(a.cos() * 9.0, a.sin() * 9.0);
            let deg = c.rng.range(0.0, 360.0);
            if c.rng.chance(0.55) {
                let h = c.rng.range(4.0, 6.5);
                let mesh = pillar(h);
                c.prop(
                    PropSpec::new(mesh, p)
                        .rot(deg)
                        .solid(ColliderShape::Circle(0.95), 5.0),
                );
            } else {
                let mesh = broken_pillar(c.rng);
                c.prop(
                    PropSpec::new(mesh, p)
                        .rot(deg)
                        .solid(ColliderShape::Circle(0.95), 1.6),
                );
                // The fallen drum lying beside it.
                let off = c.rng.unit_circle().truncate() * 2.4;
                let drum_deg = c.rng.range(0.0, 360.0);
                c.prop(
                    PropSpec::new(fallen_drum(), p + Vec2::new(off.x, off.y))
                        .rot(drum_deg)
                        .solid(ColliderShape::rect(1.6, 0.8), 0.9)
                        .passthrough(),
                );
            }
        }
    }

    // -- loose pillars between the set pieces ---------------------------------
    for _ in 0..c.rng.below(3) {
        let p = c.spot(3.0);
        let deg = c.rng.range(0.0, 360.0);
        let h = c.rng.range(4.0, 6.5);
        let mesh = pillar(h);
        c.prop(
            PropSpec::new(mesh, p)
                .rot(deg)
                .solid(ColliderShape::Circle(0.95), 5.0),
        );
    }

    // -- crystal formations ----------------------------------------------------
    for _ in 0..c.rng.below(3) {
        let p = c.spot(3.0);
        let scale = c.rng.range(1.0, 1.8);
        let deg = c.rng.range(0.0, 360.0);
        let color = if c.rng.chance(0.5) {
            CRYSTAL_VIOLET
        } else {
            CRYSTAL_CYAN
        };
        let mesh = crystal_cluster(scale, color, c.rng);
        c.prop(
            PropSpec::new(mesh, p)
                .rot(deg)
                .solid(ColliderShape::Circle(0.9 * scale), 2.5 * scale),
        );
        c.light(
            Vec3::new(p.x, 2.0 * scale, p.y),
            color,
            140_000.0 * scale,
            16.0,
        );
    }

    // -- braziers: the warm light ------------------------------------------------
    for _ in 0..c.rng.below(4) {
        let p = c.spot(2.5);
        c.prop(PropSpec::new(brazier(), p).solid(ColliderShape::Circle(0.7), 1.9));
        c.light(Vec3::new(p.x, 2.2, p.y), FLAME, 130_000.0, 14.0);
    }

    // -- pedestals ----------------------------------------------------------------
    for _ in 0..c.rng.below(3) {
        let p = c.spot(2.5);
        let deg = c.rng.range(0.0, 360.0);
        c.prop(
            PropSpec::new(pedestal(), p)
                .rot(deg)
                .solid(ColliderShape::Circle(0.6), 1.5)
                .passthrough(),
        );
    }

    // -- rubble, floating and grounded ---------------------------------------------
    for _ in 0..c.rng.below(9) + 5 {
        let p = c.spot(0.5);
        let y = c.rng.range(2.5, 8.0);
        let deg = c.rng.range(0.0, 360.0);
        let mesh = rubble(c.rng);
        c.prop(PropSpec::new(mesh, p).raised(y).rot(deg));
    }
    for _ in 0..c.rng.below(8) + 4 {
        let p = c.spot(2.0);
        let deg = c.rng.range(0.0, 360.0);
        let mesh = rubble(c.rng);
        c.prop(
            PropSpec::new(mesh, p)
                .rot(deg)
                .solid(ColliderShape::Circle(0.5), 0.7)
                .passthrough(),
        );
    }

    // -- a ley hub: healing channels, this world's signature -------------------
    // Four spokes radiating from a font. Whoever stands on them regenerates,
    // friend or foe, which makes them the real objective.
    if c.feature(0.16) {
        let hub = c.spot(9.0);
        for i in 0..4 {
            let a = i as f32 / 4.0 * std::f32::consts::TAU + 0.78;
            let dir = Vec2::new(a.cos(), a.sin());
            for k in 1..=5 {
                c.hazard(HazardSpec {
                    pos: hub + dir * (k as f32 * 2.3),
                    radius: 1.4,
                    kind: HazardKind::Font,
                    // Negative dps: the damage pipeline reads this as healing.
                    dps: -7.0,
                    slow: 1.0,
                    duty: None,
                });
            }
            let mesh = ley_line(12.0);
            c.prop(
                PropSpec::new(mesh, hub + dir * 6.0)
                    .rot(a.to_degrees())
                    .raised(0.025)
                    .surface(Surface::Solid),
            );
        }

        c.prop(
            PropSpec::new(mana_font(), hub)
                .solid(ColliderShape::Circle(1.4), 1.2)
                .passthrough(),
        );
        c.hazard(HazardSpec {
            pos: hub,
            radius: 2.6,
            kind: HazardKind::Font,
            dps: -14.0,
            slow: 1.0,
            duty: None,
        });
        c.light(Vec3::new(hub.x, 2.0, hub.y), LEY, 220_000.0, 18.0);
    }

    // -- void rifts: the one genuinely hostile tile ------------------------------
    if c.feature(0.14) {
        let p = c.spot(4.0);
        let r = c.rng.range(2.4, 3.2);
        let period = c.rng.range(5.0, 7.0);
        c.hazard(HazardSpec {
            pos: p,
            radius: r,
            kind: HazardKind::Scald,
            dps: 20.0,
            slow: 0.6,
            duty: Some((period, 0.45)),
        });
        let mesh = void_rift(r);
        c.prop(PropSpec::new(mesh, p).raised(0.03).surface(Surface::Solid));
    }

    // Where the sanctum floor gave way entirely.
    if c.feature(0.08) {
        let p = c.spot(6.0);
        let r = c.rng.range(2.6, 4.2);
        c.chasm(p, r);
    }
}

pub(super) fn floor(origin: Vec2, salt: u32) -> Mesh {
    let seed = 0xA2CA ^ salt;
    ground_grid(HALF, HALF, 1.1, |_, _, local| {
        let c = origin + local;
        let n = noise_soft(c.x * 0.3, c.y * 0.3, seed);
        // Large flagstones with mortar seams.
        let seam = (c.x / 3.3).fract().abs() < 0.07 || (c.y / 3.3).fract().abs() < 0.07;
        let cracked = noise2(c.x * 0.13, c.y * 0.13, seed ^ 0x99) > 0.86;
        let mossy = noise2(c.x * 0.2, c.y * 0.2, seed ^ 0x1234) > 0.9;
        // Checker indices from world space, so the flagstones run continuously
        // across chunk boundaries.
        let wx = (c.x / 1.1).floor() as i32;
        let wz = (c.y / 1.1).floor() as i32;

        // Great inscribed circles, repeating on a slow lattice so the endless
        // sanctum still reads as a built place rather than a texture.
        let hub = Vec2::new(
            (c.x / 96.0).round() * 96.0,
            (c.y / 96.0).round() * 96.0 - 6.0,
        );
        let r = (c - hub).length();
        let inscribed_ring = (r - 8.5).abs() < 0.35 || (r - 5.2).abs() < 0.22;
        let spoke = {
            let a = (c.y - hub.y).atan2(c.x - hub.x);
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
        } else if (wx.div_euclid(3) + wz.div_euclid(3)).rem_euclid(2) == 0 {
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
