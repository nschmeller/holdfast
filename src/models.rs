//! Every creature, projectile and pickup mesh, welded together from primitives.
//!
//! These are deliberately chunky and low-poly. The camera sits well back, the
//! silhouette is what the player actually reads at speed, and a low triangle
//! budget lets a few hundred enemies share one draw call per kind.

use bevy::prelude::*;

use crate::allies::{AllyKind, TurretKind};
use crate::enemy::EnemyKind;
use crate::meshgen::{
    MeshWeld, at, at_rot_x, at_rot_z, cone, cube, cylinder, sphere, sphere_hi, torus,
};
use crate::palette as pal;
use crate::rng::Rng;

/// The player: a rubber duck, the patron saint of desks everywhere.
/// Faces `+Z`.
pub fn player_duck() -> Mesh {
    let mut b = MeshWeld::new();

    // Body: a squashed, slightly elongated sphere.
    b.add(
        &sphere_hi(0.5),
        at(0.0, 0.42, 0.0).with_scale(Vec3::new(1.0, 0.86, 1.22)),
        pal::DUCK_BODY,
    );
    // Chest highlight so the front reads at a glance.
    b.add(
        &sphere(0.34),
        at(0.0, 0.34, 0.3).with_scale(Vec3::new(0.9, 0.8, 0.7)),
        pal::DUCK_SHADE,
    );
    // Head.
    b.add(&sphere_hi(0.31), at(0.0, 0.82, 0.24), pal::DUCK_BODY);
    // Beak, pointing forward.
    b.add(
        &cone(0.15, 0.32),
        at_rot_x(0.0, 0.78, 0.52, 90.0),
        pal::DUCK_BEAK,
    );
    // Eyes.
    for side in [-1.0f32, 1.0] {
        b.add(&sphere(0.075), at(side * 0.15, 0.92, 0.44), pal::DUCK_EYE);
        // A tiny specular dot sells the cartoon look better than a shader would.
        b.add(
            &sphere(0.032),
            at(side * 0.17, 0.96, 0.48),
            Color::srgb(1.0, 1.0, 1.0),
        );
    }
    // Wings.
    for side in [-1.0f32, 1.0] {
        b.add(
            &sphere(0.26),
            at(side * 0.42, 0.44, -0.02).with_scale(Vec3::new(0.4, 0.8, 1.1)),
            pal::DUCK_SHADE,
        );
    }
    // Tail flip.
    b.add(
        &cone(0.2, 0.3),
        at(0.0, 0.6, -0.6).with_rotation(Quat::from_rotation_x((-40.0f32).to_radians())),
        pal::DUCK_BODY,
    );

    b.build()
}

pub fn enemy_mesh(kind: EnemyKind) -> Mesh {
    match kind {
        EnemyKind::DustBunny => dust_bunny(),
        EnemyKind::Ant => ant(),
        EnemyKind::ClipCrawler => clip_crawler(),
        EnemyKind::StapleSkitter => staple_skitter(),
        EnemyKind::CrumbBlob => crumb_blob(),
        EnemyKind::TackLobber => tack_lobber(),
        EnemyKind::StainSlime => stain_slime(),
        EnemyKind::Moth => moth(),
        EnemyKind::Gremlin => gremlin(),
        EnemyKind::BossStapler => boss_stapler(),
        EnemyKind::BossHolePunch => boss_hole_punch(),
        EnemyKind::BossLamp => boss_lamp(),
    }
}

/// Two beady eyes, the universal "this thing is alive" signal.
fn eyes(b: &mut MeshWeld, y: f32, z: f32, spread: f32, r: f32) {
    for side in [-1.0f32, 1.0] {
        b.add(&sphere(r), at(side * spread, y, z), pal::DUCK_EYE);
        b.add(
            &sphere(r * 0.42),
            at(side * spread + 0.01, y + r * 0.5, z + r * 0.55),
            Color::WHITE,
        );
    }
}

fn dust_bunny() -> Mesh {
    let mut b = MeshWeld::new();
    let mut rng = Rng::seeded(0xD057);
    b.add(&sphere(0.44), at(0.0, 0.42, 0.0), pal::DUST_GREY);
    // Tufts poking out in every direction give the fuzzy silhouette.
    for _ in 0..14 {
        let dir = Vec3::new(
            rng.range(-1.0, 1.0),
            rng.range(-0.35, 1.0),
            rng.range(-1.0, 1.0),
        )
        .normalize_or_zero();
        let p = dir * rng.range(0.36, 0.52) + Vec3::new(0.0, 0.42, 0.0);
        b.add(
            &cone(rng.range(0.07, 0.13), rng.range(0.18, 0.34)),
            Transform::from_translation(p).with_rotation(Quat::from_rotation_arc(Vec3::Y, dir)),
            if rng.chance(0.5) {
                pal::DUST_GREY
            } else {
                pal::DUST_DARK
            },
        );
    }
    eyes(&mut b, 0.5, 0.38, 0.14, 0.075);
    b.build()
}

fn ant() -> Mesh {
    let mut b = MeshWeld::new();
    // Three segments along +Z.
    b.add(
        &sphere(0.2),
        at(0.0, 0.24, 0.32).with_scale(Vec3::new(1.0, 0.9, 1.0)),
        pal::ANT_BODY,
    );
    b.add(
        &sphere(0.17),
        at(0.0, 0.24, 0.02).with_scale(Vec3::new(0.9, 0.85, 1.0)),
        pal::ANT_BODY,
    );
    b.add(
        &sphere(0.26),
        at(0.0, 0.26, -0.4).with_scale(Vec3::new(1.0, 0.9, 1.3)),
        pal::ANT_BODY,
    );
    // Legs: three per side, splayed.
    for side in [-1.0f32, 1.0] {
        for (i, z) in [0.24f32, 0.02, -0.2].iter().enumerate() {
            let lean = -18.0 + i as f32 * 18.0;
            b.add(
                &cylinder(0.035, 0.42),
                at(side * 0.2, 0.16, *z).with_rotation(
                    Quat::from_rotation_z(side * 62.0f32.to_radians())
                        * Quat::from_rotation_x(lean.to_radians()),
                ),
                pal::GRAPHITE,
            );
        }
        // Antennae.
        b.add(
            &cylinder(0.024, 0.3),
            at(side * 0.09, 0.4, 0.44).with_rotation(
                Quat::from_rotation_z(side * 26.0f32.to_radians())
                    * Quat::from_rotation_x(52.0f32.to_radians()),
            ),
            pal::GRAPHITE,
        );
    }
    eyes(&mut b, 0.3, 0.46, 0.1, 0.055);
    b.build()
}

fn clip_crawler() -> Mesh {
    let mut b = MeshWeld::new();
    // The clip itself: nested arcs of bent wire.
    for (i, scale) in [1.0f32, 0.66].iter().enumerate() {
        let z = -0.04 + i as f32 * 0.08;
        b.add(
            &torus(0.045, 0.3 * scale),
            at_rot_x(0.0, 0.34, z, 90.0).with_scale(Vec3::new(0.62, 1.0, 1.0)),
            pal::CLIP_STEEL,
        );
    }
    // Legs.
    for side in [-1.0f32, 1.0] {
        for z in [0.18f32, -0.18] {
            b.add(
                &cylinder(0.032, 0.34),
                at(side * 0.16, 0.14, z)
                    .with_rotation(Quat::from_rotation_z(side * 55.0f32.to_radians())),
                pal::METAL_DARK,
            );
        }
    }
    eyes(&mut b, 0.46, 0.2, 0.11, 0.06);
    b.build()
}

fn staple_skitter() -> Mesh {
    let mut b = MeshWeld::new();
    // A staple: a flat crown with two legs bent down.
    b.add(
        &cube(0.62, 0.09, 0.11),
        at(0.0, 0.4, 0.0),
        pal::STAPLE_STEEL,
    );
    for side in [-1.0f32, 1.0] {
        b.add(
            &cube(0.09, 0.36, 0.11),
            at(side * 0.27, 0.22, 0.0),
            pal::STAPLE_STEEL,
        );
        // Skittering feet.
        b.add(
            &cube(0.1, 0.07, 0.24),
            at(side * 0.27, 0.05, 0.05),
            pal::METAL_DARK,
        );
    }
    eyes(&mut b, 0.44, 0.1, 0.14, 0.055);
    b.build()
}

fn crumb_blob() -> Mesh {
    let mut b = MeshWeld::new();
    let mut rng = Rng::seeded(0xC2003);
    // A lumpy mass of overlapping spheres.
    for i in 0..7 {
        let p = if i == 0 {
            Vec3::new(0.0, 0.46, 0.0)
        } else {
            rng.in_disc(0.34) + Vec3::new(0.0, rng.range(0.28, 0.66), 0.0)
        };
        let r = if i == 0 { 0.46 } else { rng.range(0.2, 0.34) };
        b.add(
            &sphere(r),
            Transform::from_translation(p),
            if rng.chance(0.4) {
                pal::CRUMB_TAN
            } else {
                pal::CORK
            },
        );
    }
    eyes(&mut b, 0.56, 0.42, 0.16, 0.085);
    b.build()
}

fn tack_lobber() -> Mesh {
    let mut b = MeshWeld::new();
    // Domed head.
    b.add(
        &sphere(0.34),
        at(0.0, 0.44, 0.0).with_scale(Vec3::new(1.0, 0.62, 1.0)),
        pal::TACK_RED,
    );
    b.add(&cylinder(0.33, 0.1), at(0.0, 0.3, 0.0), pal::TACK_RED);
    // The pin, pointing down and back like a stinger.
    b.add(
        &cone(0.07, 0.42),
        at(0.0, 0.16, -0.3).with_rotation(Quat::from_rotation_x(150.0f32.to_radians())),
        pal::METAL,
    );
    // Stubby legs.
    for side in [-1.0f32, 1.0] {
        b.add(
            &cylinder(0.045, 0.26),
            at(side * 0.18, 0.13, 0.06),
            pal::METAL_DARK,
        );
    }
    eyes(&mut b, 0.5, 0.28, 0.13, 0.07);
    b.build()
}

fn stain_slime() -> Mesh {
    let mut b = MeshWeld::new();
    let mut rng = Rng::seeded(0x51117);
    // A low dome with a spreading skirt.
    b.add(
        &sphere_hi(0.5),
        at(0.0, 0.2, 0.0).with_scale(Vec3::new(1.05, 0.66, 1.05)),
        pal::SLIME_BROWN,
    );
    b.add(
        &cylinder(0.6, 0.07),
        at(0.0, 0.05, 0.0),
        pal::shade(pal::SLIME_BROWN, 0.7),
    );
    // Drips around the rim.
    for i in 0..9 {
        let a = i as f32 / 9.0 * std::f32::consts::TAU;
        let r = rng.range(0.45, 0.66);
        b.add(
            &sphere(rng.range(0.07, 0.14)),
            at(a.cos() * r, 0.06, a.sin() * r),
            pal::SLIME_BROWN,
        );
    }
    eyes(&mut b, 0.3, 0.4, 0.15, 0.08);
    b.build()
}

fn moth() -> Mesh {
    let mut b = MeshWeld::new();
    // Fuzzy body.
    b.add(
        &sphere(0.2),
        at(0.0, 0.5, 0.0).with_scale(Vec3::new(1.0, 1.0, 1.7)),
        pal::MOTH_WING,
    );
    b.add(&sphere(0.15), at(0.0, 0.58, 0.26), pal::DUST_GREY);
    // Two wing pairs, swept back.
    for side in [-1.0f32, 1.0] {
        b.add(
            &sphere(0.4),
            at(side * 0.34, 0.54, -0.02).with_scale(Vec3::new(1.1, 0.07, 0.72))
                * Transform::from_rotation(Quat::from_rotation_y(side * -0.35)),
            pal::MOTH_WING,
        );
        b.add(
            &sphere(0.28),
            at(side * 0.26, 0.5, -0.34).with_scale(Vec3::new(1.0, 0.07, 0.7)),
            pal::shade(pal::MOTH_WING, 0.75),
        );
        // Feathery antennae.
        b.add(
            &cylinder(0.02, 0.3),
            at(side * 0.08, 0.72, 0.34).with_rotation(
                Quat::from_rotation_z(side * 30.0f32.to_radians())
                    * Quat::from_rotation_x(45.0f32.to_radians()),
            ),
            pal::GRAPHITE,
        );
    }
    eyes(&mut b, 0.6, 0.36, 0.09, 0.055);
    b.build()
}

fn gremlin() -> Mesh {
    let mut b = MeshWeld::new();
    // A USB stick that grew limbs.
    b.add(&cube(0.44, 0.5, 0.7), at(0.0, 0.45, 0.0), pal::GREMLIN_TEAL);
    // The metal connector as a snout.
    b.add(&cube(0.3, 0.22, 0.3), at(0.0, 0.44, 0.44), pal::METAL);
    b.add(
        &cube(0.22, 0.12, 0.06),
        at(0.0, 0.44, 0.58),
        pal::PLASTIC_DARK,
    );
    // Arms and legs.
    for side in [-1.0f32, 1.0] {
        b.add(
            &cylinder(0.05, 0.34),
            at(side * 0.26, 0.5, 0.0)
                .with_rotation(Quat::from_rotation_z(side * 50.0f32.to_radians())),
            pal::PLASTIC_DARK,
        );
        b.add(
            &cylinder(0.055, 0.24),
            at(side * 0.14, 0.12, 0.0),
            pal::PLASTIC_DARK,
        );
    }
    eyes(&mut b, 0.6, 0.32, 0.12, 0.07);
    b.build()
}

fn boss_stapler() -> Mesh {
    let mut b = MeshWeld::new();
    // Base.
    b.add(&cube(1.5, 0.36, 3.4), at(0.0, 0.2, 0.0), pal::METAL_DARK);
    b.add(
        &cube(1.35, 0.2, 3.1),
        at(0.0, 0.42, 0.05),
        pal::PLASTIC_DARK,
    );
    // Hinged upper jaw, tilted open and menacing.
    b.add(
        &cube(1.35, 0.5, 2.9),
        at(0.0, 0.95, 0.2).with_rotation(Quat::from_rotation_x((-9.0f32).to_radians())),
        pal::BOSS_TRIM,
    );
    b.add(
        &cube(1.15, 0.24, 2.6),
        at(0.0, 0.68, 0.24).with_rotation(Quat::from_rotation_x((-9.0f32).to_radians())),
        pal::STAPLE_STEEL,
    );
    // Hinge at the back.
    b.add(
        &cylinder(0.36, 1.5),
        at_rot_z(0.0, 0.7, -1.6, 90.0),
        pal::METAL,
    );
    // Staple teeth at the mouth.
    for i in 0..5 {
        let x = -0.5 + i as f32 * 0.25;
        b.add(&cube(0.1, 0.28, 0.1), at(x, 0.55, 1.42), pal::STAPLE_STEEL);
    }
    eyes(&mut b, 1.15, 1.2, 0.42, 0.17);
    b.build()
}

fn boss_hole_punch() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(2.6, 0.5, 1.7), at(0.0, 0.26, 0.0), pal::METAL_DARK);
    b.add(&cube(2.2, 0.9, 1.3), at(0.0, 0.9, 0.0), pal::BOSS_TRIM);
    // Two punch pistons.
    for side in [-1.0f32, 1.0] {
        b.add(&cylinder(0.3, 1.5), at(side * 0.7, 1.5, 0.0), pal::METAL);
        b.add(
            &cylinder(0.4, 0.24),
            at(side * 0.7, 2.3, 0.0),
            pal::PLASTIC_DARK,
        );
    }
    // Chad tray, mouth-like.
    b.add(&cube(2.3, 0.3, 0.5), at(0.0, 0.3, 0.95), pal::PLASTIC_MID);
    eyes(&mut b, 1.15, 0.68, 0.5, 0.18);
    b.build()
}

fn boss_lamp() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cylinder(1.2, 0.3), at(0.0, 0.15, 0.0), pal::METAL_DARK);
    // Segmented neck, leaning forward.
    let mut y = 0.3;
    let mut z = 0.0;
    for i in 0..4 {
        let lean = 12.0 + i as f32 * 7.0;
        b.add(
            &cylinder(0.16 - i as f32 * 0.012, 0.9),
            at(0.0, y + 0.45, z).with_rotation(Quat::from_rotation_x(lean.to_radians())),
            pal::METAL,
        );
        y += 0.82;
        z += 0.9 * lean.to_radians().sin();
    }
    // Shade.
    b.add(
        &cone(1.05, 1.5),
        at(0.0, y + 0.5, z + 0.5).with_rotation(Quat::from_rotation_x(145.0f32.to_radians())),
        pal::LAMP_SHADE,
    );
    // The bulb: an eye that stares.
    b.add(&sphere(0.44), at(0.0, y + 0.25, z + 0.85), pal::LAMP_GLOW);
    b.build()
}

// -- allies -----------------------------------------------------------------

pub fn ally_mesh(kind: AllyKind) -> Mesh {
    let mut b = MeshWeld::new();
    let trim = kind.trim_color();

    // A shared chassis keeps the squad reading as one faction across every
    // environment; the loadout on top is what distinguishes the roles.
    b.add(
        &sphere(0.28),
        at(0.0, 0.34, 0.0).with_scale(Vec3::new(1.0, 1.1, 0.9)),
        pal::PLASTIC_MID,
    );
    b.add(&sphere(0.2), at(0.0, 0.66, 0.06), pal::KEYCAP);
    // Visor.
    b.add(&cube(0.26, 0.09, 0.06), at(0.0, 0.68, 0.21), trim);
    for side in [-1.0f32, 1.0] {
        b.add(
            &cylinder(0.05, 0.26),
            at(side * 0.13, 0.12, 0.0),
            pal::PLASTIC_DARK,
        );
    }

    match kind {
        AllyKind::Scout => {
            // Light, with a tall antenna.
            b.add(&cylinder(0.018, 0.44), at(0.0, 0.95, -0.04), trim);
            b.add(&sphere(0.05), at(0.0, 1.18, -0.04), trim);
            b.add(&cube(0.1, 0.22, 0.1), at(0.22, 0.4, 0.06), pal::METAL);
        }
        AllyKind::Gunner => {
            // Shoulder-mounted barrel.
            b.add(
                &cylinder(0.07, 0.6),
                at_rot_x(0.2, 0.46, 0.22, 90.0),
                pal::METAL_DARK,
            );
            b.add(
                &cube(0.16, 0.16, 0.2),
                at(0.2, 0.46, -0.06),
                pal::PLASTIC_DARK,
            );
            b.add(&sphere(0.05), at(0.2, 0.46, 0.54), trim);
        }
        AllyKind::Bulwark => {
            // Broad body plus a slab shield.
            b.add(
                &sphere(0.34),
                at(0.0, 0.36, 0.0).with_scale(Vec3::new(1.25, 1.0, 1.0)),
                pal::PLASTIC_DARK,
            );
            b.add(&cube(0.72, 0.7, 0.1), at(0.0, 0.44, 0.34), pal::METAL);
            b.add(&cube(0.72, 0.08, 0.13), at(0.0, 0.44, 0.36), trim);
        }
        AllyKind::Medic => {
            // A floating support orb and a cross.
            b.add(&sphere(0.13), at(0.0, 1.05, 0.0), trim);
            b.add(&cube(0.24, 0.07, 0.03), at(0.0, 0.4, 0.28), pal::PAPER);
            b.add(&cube(0.07, 0.24, 0.03), at(0.0, 0.4, 0.28), pal::PAPER);
        }
    }

    b.build()
}

pub fn turret_mesh(kind: TurretKind) -> Mesh {
    let mut b = MeshWeld::new();
    let trim = kind.trim_color();

    // Shared footing so structures read as built, not dropped.
    b.add(&cylinder(0.55, 0.16), at(0.0, 0.08, 0.0), pal::PLASTIC_DARK);
    b.add(&cylinder(0.46, 0.1), at(0.0, 0.2, 0.0), pal::METAL_DARK);

    match kind {
        TurretKind::Tack => {
            b.add(&sphere(0.3), at(0.0, 0.42, 0.0), pal::PLASTIC_MID);
            b.add(
                &cylinder(0.09, 0.7),
                at_rot_x(0.0, 0.46, 0.3, 90.0),
                pal::METAL,
            );
            b.add(&cylinder(0.13, 0.1), at_rot_x(0.0, 0.46, 0.62, 90.0), trim);
        }
        TurretKind::Lobber => {
            b.add(&cube(0.5, 0.36, 0.5), at(0.0, 0.44, 0.0), pal::PLASTIC_MID);
            b.add(
                &cylinder(0.16, 0.66),
                at(0.0, 0.66, 0.16).with_rotation(Quat::from_rotation_x(50.0f32.to_radians())),
                pal::METAL_DARK,
            );
            b.add(&torus(0.04, 0.18), at_rot_x(0.0, 0.86, 0.36, 40.0), trim);
        }
        TurretKind::Shocker => {
            b.add(&cylinder(0.2, 0.6), at(0.0, 0.5, 0.0), pal::METAL);
            // Tesla coils.
            for i in 0..3 {
                let a = i as f32 / 3.0 * std::f32::consts::TAU;
                b.add(
                    &torus(0.03, 0.16),
                    at(a.cos() * 0.28, 0.62, a.sin() * 0.28),
                    pal::CLIP_STEEL,
                );
            }
            b.add(&sphere(0.24), at(0.0, 0.94, 0.0), trim);
        }
        TurretKind::Barricade => {
            // No weapon: a wall that soaks hits and shapes enemy pathing.
            b.add(&cube(1.9, 0.9, 0.34), at(0.0, 0.5, 0.0), pal::PLASTIC_MID);
            b.add(&cube(1.9, 0.12, 0.4), at(0.0, 0.94, 0.0), trim);
            for side in [-1.0f32, 1.0] {
                b.add(
                    &cube(0.2, 1.0, 0.5),
                    at(side * 0.85, 0.5, 0.0),
                    pal::METAL_DARK,
                );
            }
        }
        TurretKind::Generator => {
            // A humming box with cooling fins and a very obvious core, because
            // the player needs to spot at a glance what the enemy is chewing on.
            b.add(&cube(1.0, 0.8, 1.0), at(0.0, 0.6, 0.0), pal::PLASTIC_MID);
            for i in 0..4 {
                b.add(
                    &cube(1.15, 0.08, 1.15),
                    at(0.0, 0.36 + i as f32 * 0.18, 0.0),
                    pal::METAL_DARK,
                );
            }
            b.add(
                &Sphere::new(0.28).mesh().ico(1).unwrap(),
                at(0.0, 1.15, 0.0),
                trim,
            );
            b.add(&torus(0.05, 0.42), at(0.0, 1.15, 0.0), trim);
        }
    }

    b.build()
}

// -- projectiles ------------------------------------------------------------

/// A flying pencil, pointing `+Z`.
pub fn pencil_dart() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(
        &Mesh::from(Cylinder::new(0.09, 0.8).mesh().resolution(6)),
        at_rot_x(0.0, 0.0, 0.0, 90.0),
        pal::PENCIL_YELLOW,
    );
    b.add(&cone(0.09, 0.24), at_rot_x(0.0, 0.0, 0.5, 90.0), pal::CORK);
    b.add(
        &cone(0.04, 0.1),
        at_rot_x(0.0, 0.0, 0.62, 90.0),
        pal::GRAPHITE,
    );
    b.add(
        &cylinder(0.095, 0.1),
        at_rot_x(0.0, 0.0, -0.42, 90.0),
        pal::METAL,
    );
    b.add(
        &cylinder(0.09, 0.12),
        at_rot_x(0.0, 0.0, -0.52, 90.0),
        pal::ERASER_PINK,
    );
    b.build()
}

pub fn staple() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(
        &cube(0.36, 0.07, 0.07),
        at(0.0, 0.0, 0.1),
        pal::STAPLE_STEEL,
    );
    for side in [-1.0f32, 1.0] {
        b.add(
            &cube(0.07, 0.07, 0.22),
            at(side * 0.15, 0.0, -0.04),
            pal::STAPLE_STEEL,
        );
    }
    b.build()
}

pub fn thumbtack() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(
        &sphere(0.18),
        at(0.0, 0.0, -0.08).with_scale(Vec3::new(1.0, 1.0, 0.6)),
        pal::TACK_RED,
    );
    b.add(
        &cone(0.05, 0.34),
        at_rot_x(0.0, 0.0, 0.16, 90.0),
        pal::METAL,
    );
    b.build()
}

pub fn rubber_band() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(
        &torus(0.05, 0.22),
        at_rot_x(0.0, 0.0, 0.0, 90.0).with_scale(Vec3::new(1.0, 1.0, 1.5)),
        pal::RUBBER_BAND,
    );
    b.build()
}

pub fn paperclip() -> Mesh {
    let mut b = MeshWeld::new();
    for (i, s) in [1.0f32, 0.62].iter().enumerate() {
        b.add(
            &torus(0.035, 0.24 * s),
            at_rot_x(0.0, 0.0, -0.03 + i as f32 * 0.06, 90.0).with_scale(Vec3::new(0.55, 1.0, 1.0)),
            pal::CLIP_STEEL,
        );
    }
    b.build()
}

pub fn mine() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cylinder(0.26, 0.12), at(0.0, 0.06, 0.0), pal::TACK_RED);
    for i in 0..6 {
        let a = i as f32 / 6.0 * std::f32::consts::TAU;
        b.add(
            &cone(0.05, 0.18),
            at(a.cos() * 0.24, 0.12, a.sin() * 0.24),
            pal::METAL,
        );
    }
    b.build()
}

// -- pickups ----------------------------------------------------------------

pub fn gem() -> Mesh {
    let mut b = MeshWeld::new();
    // An icosahedron reads as a cut gem and costs 20 triangles.
    b.add(
        &Sphere::new(0.3).mesh().ico(0).unwrap(),
        at(0.0, 0.0, 0.0).with_scale(Vec3::new(1.0, 1.35, 1.0)),
        Color::WHITE,
    );
    b.build()
}

pub fn heart() -> Mesh {
    let mut b = MeshWeld::new();
    for side in [-1.0f32, 1.0] {
        b.add(&sphere(0.18), at(side * 0.13, 0.12, 0.0), Color::WHITE);
    }
    b.add(
        &cone(0.26, 0.4),
        at_rot_x(0.0, -0.12, 0.0, 180.0),
        Color::WHITE,
    );
    b.build()
}

pub fn scrap_nut() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(
        &Mesh::from(Cylinder::new(0.26, 0.14).mesh().resolution(6)),
        Transform::IDENTITY,
        Color::WHITE,
    );
    b.build()
}

pub fn supply_crate() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(0.62, 0.5, 0.62), at(0.0, 0.25, 0.0), pal::CORK);
    // Straps.
    b.add(&cube(0.66, 0.1, 0.12), at(0.0, 0.3, 0.0), pal::GEAR_GOLD);
    b.add(&cube(0.12, 0.1, 0.66), at(0.0, 0.3, 0.0), pal::GEAR_GOLD);
    b.add(
        &cube(0.66, 0.08, 0.66),
        at(0.0, 0.52, 0.0),
        pal::shade(pal::CORK, 1.2),
    );
    b.build()
}

// -- markers ----------------------------------------------------------------

pub fn zone_pillar() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cylinder(0.22, 1.6), at(0.0, 0.8, 0.0), Color::WHITE);
    b.add(&torus(0.05, 0.55), at(0.0, 0.1, 0.0), Color::WHITE);
    b.add(
        &Sphere::new(0.24).mesh().ico(0).unwrap(),
        at(0.0, 1.85, 0.0),
        Color::WHITE,
    );
    b.build()
}

pub fn arrow() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&cube(0.14, 0.06, 0.4), at(0.0, 0.0, -0.1), Color::WHITE);
    b.add(
        &cone(0.2, 0.34),
        at_rot_x(0.0, 0.0, 0.24, 90.0),
        Color::WHITE,
    );
    b.build()
}

/// A fort: a squat keep with a banner mast. Read at a glance from above, which
/// is the only angle anyone ever sees it from.
pub fn fort_keep() -> Mesh {
    let mut b = MeshWeld::new();
    // Rampart ring.
    b.add(&torus(0.5, 2.6), at(0.0, 0.5, 0.0), Color::WHITE);
    // Keep.
    b.add(&cube(2.2, 2.4, 2.2), at(0.0, 1.2, 0.0), Color::WHITE);
    b.add(&cube(2.6, 0.4, 2.6), at(0.0, 2.5, 0.0), Color::WHITE);
    // Corner posts, so the silhouette is not a plain box.
    for (x, z) in [(-1.5, -1.5), (1.5, -1.5), (-1.5, 1.5), (1.5, 1.5)] {
        b.add(&cube(0.5, 1.8, 0.5), at(x, 0.9, z), Color::WHITE);
    }
    // Mast and banner: the part that carries the faction colour.
    b.add(&cylinder(0.09, 3.4), at(0.0, 4.2, 0.0), Color::WHITE);
    b.add(&cube(1.5, 0.9, 0.08), at(0.75, 5.2, 0.0), Color::WHITE);
    b.build()
}

/// A nest: a low knot of spines. Deliberately unlike the fort - one is a place
/// you take, the other is a thing you clear.
pub fn nest_mound() -> Mesh {
    let mut b = MeshWeld::new();
    b.add(&sphere(0.85), at(0.0, 0.35, 0.0), Color::WHITE);
    let mut rng = Rng::seeded(0x_4E57);
    for i in 0..9 {
        let a = i as f32 / 9.0 * std::f32::consts::TAU;
        let lean = rng.range(30.0, 60.0);
        b.add(
            &cone(0.16, rng.range(0.8, 1.5)),
            at_rot_z(a.cos() * 0.5, 0.7, a.sin() * 0.5, lean * a.cos().signum()),
            Color::WHITE,
        );
    }
    b.add(&torus(0.09, 1.0), at(0.0, 0.06, 0.0), Color::WHITE);
    b.build()
}
