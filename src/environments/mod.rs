//! Environments: five arenas that share one rulebook.
//!
//! An environment is pure data. Each module below is a function from a seed to
//! a `SceneData` - a floor mesh, a pile of props with colliders, some lights
//! and some hazards. Nothing in here touches the ECS, which means an arena can
//! be unit-reasoned about, swapped at runtime, and extended without going near
//! the simulation.

mod arcane;
mod desk;
mod forest;
mod grid;
mod rooftop;

use bevy::prelude::*;

use crate::arena::{ArenaBounds, ColliderShape, Gust, Hazard, HazardKind, ObstacleField, Spotlight};
use crate::art::{GameArt, Glow};
use crate::common::{Body, RunEntity, to_world};
use crate::rng::Rng;
use crate::{AppState, GameSet};

#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub enum EnvKind {
    #[default]
    Desk,
    Forest,
    Rooftop,
    Grid,
    Arcane,
}

impl EnvKind {
    pub const COUNT: usize = 5;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Desk,
        Self::Forest,
        Self::Rooftop,
        Self::Grid,
        Self::Arcane,
    ];

    pub fn next(self) -> Self {
        Self::ALL[(self as usize + 1) % Self::COUNT]
    }

    pub fn prev(self) -> Self {
        Self::ALL[(self as usize + Self::COUNT - 1) % Self::COUNT]
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Desk => "THE DESK",
            Self::Forest => "THE UNDERGROWTH",
            Self::Rooftop => "BLOCK 9 ROOFTOP",
            Self::Grid => "GRID ZERO",
            Self::Arcane => "THE ARCANE SANCTUM",
        }
    }

    pub fn tagline(self) -> &'static str {
        match self {
            Self::Desk => "2AM. The stationery has opinions.",
            Self::Forest => "You are four inches tall and the moss is hostile.",
            Self::Rooftop => "Neon, rust, and something in the vents.",
            Self::Grid => "A test platform in hard vacuum. Something is testing back.",
            Self::Arcane => "A broken sanctum where the wards still hold. Barely.",
        }
    }

    /// One line on what plays differently here, shown on the select screen.
    pub fn quirk(self) -> &'static str {
        match self {
            Self::Desk => "Tight and cluttered. The USB fan sweeps a lane every few seconds.",
            Self::Forest => "Wide and open, but mud slows everything that crosses it.",
            Self::Rooftop => "Long sightlines. Steam vents erupt on a timer.",
            Self::Grid => "Almost no cover. Plasma conduits burn anything standing on them.",
            Self::Arcane => "Ley lines heal whoever holds them. So the enemy wants them too.",
        }
    }

    pub fn build(self, rng: &mut Rng) -> SceneData {
        match self {
            Self::Desk => desk::build(rng),
            Self::Forest => forest::build(rng),
            Self::Rooftop => rooftop::build(rng),
            Self::Grid => grid::build(rng),
            Self::Arcane => arcane::build(rng),
        }
    }
}

/// Which shared material a prop renders with.
#[derive(Clone, Copy)]
pub enum Surface {
    Solid,
    Matte,
    Metal,
    Glass,
    Glow(Glow),
}

pub struct PropSpec {
    pub mesh: Mesh,
    pub pos: Vec2,
    pub y: f32,
    pub rot_y: f32,
    pub surface: Surface,
    /// `None` for decorative props that nothing collides with.
    pub collider: Option<ColliderShape>,
    pub blocks_shots: bool,
    pub height: f32,
}

impl PropSpec {
    pub fn new(mesh: Mesh, pos: Vec2) -> Self {
        Self {
            mesh,
            pos,
            y: 0.0,
            rot_y: 0.0,
            surface: Surface::Solid,
            collider: None,
            blocks_shots: false,
            height: 0.0,
        }
    }

    pub fn solid(mut self, shape: ColliderShape, height: f32) -> Self {
        self.collider = Some(shape);
        self.height = height;
        // Anything tall enough to hide behind should also stop shots; the
        // threshold matches roughly waist height on the player model.
        self.blocks_shots = height >= 0.75;
        self
    }

    pub fn passthrough(mut self) -> Self {
        self.blocks_shots = false;
        self
    }

    pub fn surface(mut self, surface: Surface) -> Self {
        self.surface = surface;
        self
    }

    pub fn rot(mut self, degrees: f32) -> Self {
        self.rot_y = degrees.to_radians();
        self
    }

    pub fn raised(mut self, y: f32) -> Self {
        self.y = y;
        self
    }
}

pub struct LightSpec {
    pub pos: Vec3,
    pub color: Color,
    pub intensity: f32,
    pub range: f32,
    pub shadows: bool,
}

pub struct HazardSpec {
    pub pos: Vec2,
    pub radius: f32,
    pub kind: HazardKind,
    pub dps: f32,
    pub slow: f32,
    /// Permanent features pulse on a cycle instead of expiring.
    pub duty: Option<(f32, f32)>,
}

/// Everything an environment contributes.
pub struct SceneData {
    pub bounds: ArenaBounds,
    pub ground: Mesh,
    pub props: Vec<PropSpec>,
    pub lights: Vec<LightSpec>,
    pub hazards: Vec<HazardSpec>,
    pub sky: Color,
    pub ambient: Color,
    pub ambient_brightness: f32,
    pub sun_color: Color,
    pub sun_illuminance: f32,
    /// Direction the sun points, from the light towards the scene.
    pub sun_dir: Vec3,
    pub gust: Gust,
    pub spotlight: Spotlight,
    /// Where territory markers go. Environment-authored so they sit in
    /// interesting places rather than being scattered blindly.
    pub zones: Vec<Vec2>,
}

impl SceneData {
    pub fn new(half_x: f32, half_z: f32, ground: Mesh) -> Self {
        Self {
            bounds: ArenaBounds { half_x, half_z },
            ground,
            props: Vec::new(),
            lights: Vec::new(),
            hazards: Vec::new(),
            sky: Color::srgb(0.016, 0.018, 0.028),
            ambient: Color::srgb(0.42, 0.5, 0.78),
            ambient_brightness: 260.0,
            sun_color: Color::srgb(1.0, 0.94, 0.86),
            sun_illuminance: 3200.0,
            sun_dir: Vec3::new(-0.5, -1.0, -0.35),
            gust: Gust::default(),
            spotlight: Spotlight::default(),
            zones: Vec::new(),
        }
    }

    pub fn prop(&mut self, p: PropSpec) -> &mut Self {
        self.props.push(p);
        self
    }

    pub fn light(&mut self, pos: Vec3, color: Color, intensity: f32, range: f32) -> &mut Self {
        self.lights.push(LightSpec {
            pos,
            color,
            intensity,
            range,
            shadows: false,
        });
        self
    }
}

/// Marker for everything spawned by the current environment, so switching
/// arenas is a single despawn query.
#[derive(Component)]
pub struct EnvEntity;

/// Set when a rebuild is required (new run, or the player changed arena).
#[derive(Resource, Default)]
pub struct EnvDirty(pub bool);

/// A permanent hazard that cycles on and off, like a steam vent.
#[derive(Component)]
pub struct PulsingHazard {
    pub period: f32,
    pub on_fraction: f32,
    pub phase: f32,
    pub base_dps: f32,
}

pub struct ArenaPlugin;

impl Plugin for ArenaPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EnvKind>()
            .init_resource::<ArenaBounds>()
            .init_resource::<ObstacleField>()
            .init_resource::<EnvDirty>()
            .init_resource::<Gust>()
            .init_resource::<Spotlight>()
            .add_systems(OnExit(AppState::Menu), mark_dirty)
            .add_systems(
                Update,
                rebuild_environment.run_if(|d: Res<EnvDirty>| d.0),
            )
            .add_systems(Update, (tick_gust, tick_pulsing_hazards).in_set(GameSet::Think));
    }
}

fn mark_dirty(mut dirty: ResMut<EnvDirty>) {
    dirty.0 = true;
}

#[allow(clippy::too_many_arguments)]
fn rebuild_environment(
    mut commands: Commands,
    mut dirty: ResMut<EnvDirty>,
    env: Res<EnvKind>,
    art: Res<GameArt>,
    mut rng: ResMut<Rng>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut bounds: ResMut<ArenaBounds>,
    mut obstacles: ResMut<ObstacleField>,
    mut gust: ResMut<Gust>,
    mut spotlight: ResMut<Spotlight>,
    mut clear: ResMut<ClearColor>,
    mut ambient: ResMut<GlobalAmbientLight>,
    existing: Query<Entity, With<EnvEntity>>,
    mut zone_spawns: MessageWriter<crate::allies::SpawnZone>,
) {
    dirty.0 = false;

    for e in &existing {
        commands.entity(e).despawn();
    }
    obstacles.clear();

    let scene = env.build(&mut rng);

    *bounds = scene.bounds;
    *gust = scene.gust;
    *spotlight = scene.spotlight;
    clear.0 = scene.sky;
    ambient.color = scene.ambient;
    ambient.brightness = scene.ambient_brightness;

    // Floor.
    commands.spawn((
        Mesh3d(meshes.add(scene.ground)),
        MeshMaterial3d(art.ground.clone()),
        Transform::IDENTITY,
        EnvEntity,
    ));

    // Sun.
    commands.spawn((
        DirectionalLight {
            color: scene.sun_color,
            illuminance: scene.sun_illuminance,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_translation(-scene.sun_dir.normalize() * 40.0)
            .looking_to(scene.sun_dir, Vec3::Y),
        EnvEntity,
    ));

    // Props.
    for prop in scene.props {
        let material = match prop.surface {
            Surface::Solid => art.solid.clone(),
            Surface::Matte => art.matte.clone(),
            Surface::Metal => art.metal.clone(),
            Surface::Glass => art.glass.clone(),
            Surface::Glow(g) => art.glow(g),
        };
        if let Some(shape) = prop.collider {
            obstacles.push(prop.pos, shape, prop.blocks_shots, prop.height);
        }
        commands.spawn((
            Mesh3d(meshes.add(prop.mesh)),
            MeshMaterial3d(material),
            Transform::from_translation(to_world(prop.pos, prop.y))
                .with_rotation(Quat::from_rotation_y(prop.rot_y)),
            EnvEntity,
        ));
    }

    // Point lights.
    for l in scene.lights {
        commands.spawn((
            PointLight {
                color: l.color,
                intensity: l.intensity,
                range: l.range,
                shadow_maps_enabled: l.shadows,
                ..default()
            },
            Transform::from_translation(l.pos),
            EnvEntity,
        ));
    }

    // Environmental hazards.
    for h in scene.hazards {
        let mut e = commands.spawn((
            Hazard {
                kind: h.kind,
                radius: h.radius,
                dps: h.dps,
                slow: h.slow,
                life: None,
                hurts_player: true,
                hurts_enemies: true,
            },
            Body::new(h.pos, h.radius),
            Mesh3d(art.disc.clone()),
            MeshMaterial3d(art.unlit.clone()),
            Transform::from_translation(to_world(h.pos, 0.03))
                .with_scale(Vec3::new(h.radius, 1.0, h.radius)),
            EnvEntity,
        ));
        if let Some((period, on_fraction)) = h.duty {
            e.insert(PulsingHazard {
                period,
                on_fraction,
                phase: 0.0,
                base_dps: h.dps,
            });
        }
    }

    // Territory markers.
    for pos in scene.zones {
        zone_spawns.write(crate::allies::SpawnZone { pos });
    }
}

fn tick_gust(time: Res<Time>, mut gust: ResMut<Gust>) {
    if !gust.enabled {
        gust.blowing = false;
        return;
    }
    let dt = time.delta_secs();
    gust.remaining -= dt;
    if gust.remaining <= 0.0 {
        gust.blowing = !gust.blowing;
        gust.remaining = if gust.blowing {
            gust.duration
        } else {
            gust.cooldown
        };
    }
}

fn tick_pulsing_hazards(
    time: Res<Time>,
    mut q: Query<(&mut PulsingHazard, &mut Hazard, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (mut pulse, mut hazard, mut transform) in &mut q {
        pulse.phase = (pulse.phase + dt / pulse.period).fract();
        let on = pulse.phase < pulse.on_fraction;
        hazard.dps = if on { pulse.base_dps } else { 0.0 };
        // Shrink to a faint ring when dormant so the telegraph is legible.
        let scale = if on {
            hazard.radius
        } else {
            hazard.radius * 0.35
        };
        transform.scale = Vec3::new(scale, 1.0, scale);
    }
}
