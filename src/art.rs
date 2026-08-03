//! The runtime asset registry.
//!
//! Nothing is loaded from disk. At startup we generate every mesh the game will
//! ever spawn and a handful of shared materials, then hand out handles. Because
//! the meshes carry per-vertex colour, one `solid` material covers almost the
//! whole scene, which keeps Bevy's batching effective even with several hundred
//! entities on screen.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use crate::enemy::EnemyKind;
use crate::meshgen::{MeshWeld, cube, sphere, sphere_hi};
use crate::models;
use crate::palette;

#[derive(Debug)]
pub struct ArtPlugin;

impl Plugin for ArtPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, build_art);
    }
}

/// Keys for the emissive material cache. Emissive strength cannot be driven by
/// vertex colour, so glowing things need one material per colour.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Glow {
    Xp,
    Heal,
    Scrap,
    Gear,
    PlayerShot,
    EnemyShot,
    Beam,
    Screen,
    Lamp,
    Neon,
    Plasma,
    Elite,
    Boss,
    Ally,
    Zone,
    ZoneHeld,
    Warning,
}

impl Glow {
    fn spec(self) -> (Color, f32) {
        match self {
            Self::Xp => (palette::XP_GREEN, 4.0),
            Self::Heal => (palette::HEAL_RED, 4.0),
            Self::Scrap => (palette::METAL, 1.6),
            Self::Gear => (palette::GEAR_GOLD, 4.5),
            Self::PlayerShot => (palette::ACCENT, 6.0),
            Self::EnemyShot => (palette::DANGER, 6.0),
            Self::Beam => (Color::srgb(1.0, 0.95, 0.4), 9.0),
            Self::Screen => (palette::SCREEN_GLOW, 2.4),
            Self::Lamp => (palette::LAMP_GLOW, 3.2),
            Self::Neon => (Color::srgb(1.0, 0.28, 0.62), 5.0),
            Self::Plasma => (Color::srgb(0.35, 0.95, 1.0), 5.5),
            Self::Elite => (palette::ELITE_TRIM, 3.5),
            Self::Boss => (palette::BOSS_TRIM, 3.5),
            Self::Ally => (Color::srgb(0.45, 0.85, 1.0), 3.0),
            Self::Zone => (Color::srgb(0.9, 0.75, 0.35), 2.0),
            Self::ZoneHeld => (Color::srgb(0.4, 1.0, 0.6), 2.6),
            Self::Warning => (Color::srgb(1.0, 0.35, 0.2), 3.0),
        }
    }
}

/// Handles for everything spawnable. Meshes indexed by an enum's discriminant
/// are stored in `Vec`s so lookup is a bounds-checked index, not a hash.
#[derive(Debug, Resource)]
pub struct GameArt {
    // materials
    pub solid: Handle<StandardMaterial>,
    pub matte: Handle<StandardMaterial>,
    pub metal: Handle<StandardMaterial>,
    pub glass: Handle<StandardMaterial>,
    pub unlit: Handle<StandardMaterial>,
    /// Fog overlay: colour lives here, only opacity varies per vertex.
    pub fog: Handle<StandardMaterial>,
    pub ground: Handle<StandardMaterial>,
    glows: HashMap<Glow, Handle<StandardMaterial>>,

    // actors
    pub player: Handle<Mesh>,
    pub enemies: Vec<Handle<Mesh>>,
    pub allies: Vec<Handle<Mesh>>,
    pub turrets: Vec<Handle<Mesh>>,

    // projectiles
    pub dart: Handle<Mesh>,
    pub staple: Handle<Mesh>,
    pub tack: Handle<Mesh>,
    pub band: Handle<Mesh>,
    pub pellet: Handle<Mesh>,
    pub beam_seg: Handle<Mesh>,
    pub clip_orbit: Handle<Mesh>,
    pub mine: Handle<Mesh>,

    // pickups
    pub xp_orb: Handle<Mesh>,
    pub xp_gem: Handle<Mesh>,
    pub heart: Handle<Mesh>,
    pub scrap: Handle<Mesh>,
    pub crate_mesh: Handle<Mesh>,

    // effects and markers
    pub particle: Handle<Mesh>,
    pub shard: Handle<Mesh>,
    pub ring: Handle<Mesh>,
    pub disc: Handle<Mesh>,
    pub shadow: Handle<Mesh>,
    pub zone_pillar: Handle<Mesh>,
    pub fort: Handle<Mesh>,
    pub nest: Handle<Mesh>,
    banners: HashMap<u8, Handle<StandardMaterial>>,
    pub arrow: Handle<Mesh>,
    pub unit_cube: Handle<Mesh>,
    pub unit_sphere: Handle<Mesh>,
}

impl GameArt {
    /// The material a holding of `faction` renders with.
    pub fn banner(&self, faction: crate::factions::Faction) -> Handle<StandardMaterial> {
        self.banners
            .get(&(faction.index() as u8))
            .cloned()
            .unwrap_or_else(|| self.solid.clone())
    }

    pub fn glow(&self, g: Glow) -> Handle<StandardMaterial> {
        // Every variant is inserted during setup, so a miss is a programming
        // error rather than a runtime condition worth handling gracefully.
        self.glows
            .get(&g)
            .expect("glow material registered at startup")
            .clone()
    }

    pub fn enemy_mesh(&self, kind: EnemyKind) -> Handle<Mesh> {
        self.enemies[kind as usize].clone()
    }
}

fn build_art(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // -- materials ---------------------------------------------------------
    // base_color stays white so the per-vertex colour survives the multiply.
    let solid = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.72,
        metallic: 0.0,
        reflectance: 0.16,
        ..default()
    });
    let matte = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.98,
        metallic: 0.0,
        reflectance: 0.04,
        ..default()
    });
    let metal = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.28,
        metallic: 0.88,
        reflectance: 0.6,
        ..default()
    });
    let glass = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.35),
        perceptual_roughness: 0.08,
        metallic: 0.0,
        reflectance: 0.5,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let unlit = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    // Fog carries its colour in the material and only its opacity in the
    // mesh. A near-black vertex colour turned out not to render at all, and
    // whatever the reason, the colour has no business being per-vertex when
    // every cell shares it - only how much of it shows differs.
    let fog = materials.add(StandardMaterial {
        base_color: Color::srgb(0.012, 0.014, 0.03),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let ground = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.9,
        metallic: 0.0,
        reflectance: 0.1,
        ..default()
    });

    let mut glows = HashMap::default();
    for g in [
        Glow::Xp,
        Glow::Heal,
        Glow::Scrap,
        Glow::Gear,
        Glow::PlayerShot,
        Glow::EnemyShot,
        Glow::Beam,
        Glow::Screen,
        Glow::Lamp,
        Glow::Neon,
        Glow::Plasma,
        Glow::Elite,
        Glow::Boss,
        Glow::Ally,
        Glow::Zone,
        Glow::ZoneHeld,
        Glow::Warning,
    ] {
        let (color, strength) = g.spec();
        let lin = color.to_linear();
        glows.insert(
            g,
            materials.add(StandardMaterial {
                base_color: color,
                emissive: LinearRgba::rgb(
                    lin.red * strength,
                    lin.green * strength,
                    lin.blue * strength,
                ),
                perceptual_roughness: 0.4,
                ..default()
            }),
        );
    }

    // -- actor meshes ------------------------------------------------------
    let player = meshes.add(models::player_duck());

    let enemies = EnemyKind::ALL
        .iter()
        .map(|k| meshes.add(models::enemy_mesh(*k)))
        .collect();

    let allies = (0..crate::allies::AllyKind::ALL.len())
        .map(|i| meshes.add(models::ally_mesh(crate::allies::AllyKind::ALL[i])))
        .collect();

    let turrets = (0..crate::allies::TurretKind::ALL.len())
        .map(|i| meshes.add(models::turret_mesh(crate::allies::TurretKind::ALL[i])))
        .collect();

    // -- projectiles -------------------------------------------------------
    let dart = meshes.add(models::pencil_dart());
    let staple = meshes.add(models::staple());
    let tack = meshes.add(models::thumbtack());
    let band = meshes.add(models::rubber_band());
    let pellet = meshes.add({
        let mut b = MeshWeld::new();
        b.add(&sphere(0.16), Transform::IDENTITY, Color::WHITE);
        b.build()
    });
    let beam_seg = meshes.add({
        let mut b = MeshWeld::new();
        b.add(&cube(0.34, 0.34, 1.0), Transform::IDENTITY, Color::WHITE);
        b.build()
    });
    let clip_orbit = meshes.add(models::paperclip());
    let mine = meshes.add(models::mine());

    // -- pickups -----------------------------------------------------------
    let xp_orb = meshes.add({
        let mut b = MeshWeld::new();
        b.add(&sphere(0.26), Transform::IDENTITY, Color::WHITE);
        b.build()
    });
    let xp_gem = meshes.add(models::gem());
    let heart = meshes.add(models::heart());
    let scrap = meshes.add(models::scrap_nut());
    let crate_mesh = meshes.add(models::supply_crate());

    // -- effects -----------------------------------------------------------
    let particle = meshes.add({
        let mut b = MeshWeld::new();
        b.add(&cube(0.2, 0.2, 0.2), Transform::IDENTITY, Color::WHITE);
        b.build()
    });
    let shard = meshes.add({
        let mut b = MeshWeld::new();
        b.add(&cube(0.1, 0.1, 0.44), Transform::IDENTITY, Color::WHITE);
        b.build()
    });
    let ring = meshes.add(Mesh::from(
        Torus::new(0.94, 1.0)
            .mesh()
            .major_resolution(40)
            .minor_resolution(6),
    ));
    let disc = meshes.add(Mesh::from(Cylinder::new(1.0, 0.04).mesh().resolution(40)));
    let shadow = meshes.add(Mesh::from(Cylinder::new(1.0, 0.01).mesh().resolution(14)));
    let zone_pillar = meshes.add(models::zone_pillar());
    let fort = meshes.add(models::fort_keep());
    let nest = meshes.add(models::nest_mound());

    // One emissive material per faction, built once. Every holding on the map
    // wears its owner's colour, which is the whole navigational read.
    let mut banners = HashMap::new();
    for faction in crate::factions::Faction::ALL {
        let colour = faction.color();
        banners.insert(
            faction.index() as u8,
            materials.add(StandardMaterial {
                base_color: colour,
                emissive: colour.to_linear() * 2.4,
                perceptual_roughness: 0.6,
                ..default()
            }),
        );
    }
    let arrow = meshes.add(models::arrow());
    let unit_cube = meshes.add(cube(1.0, 1.0, 1.0));
    let unit_sphere = meshes.add(sphere_hi(0.5));

    commands.insert_resource(GameArt {
        solid,
        matte,
        metal,
        glass,
        unlit,
        fog,
        ground,
        glows,
        player,
        enemies,
        allies,
        turrets,
        dart,
        staple,
        tack,
        band,
        pellet,
        beam_seg,
        clip_orbit,
        mine,
        xp_orb,
        xp_gem,
        heart,
        scrap,
        crate_mesh,
        particle,
        shard,
        ring,
        disc,
        shadow,
        zone_pillar,
        fort,
        nest,
        banners,
        arrow,
        unit_cube,
        unit_sphere,
    });
}
