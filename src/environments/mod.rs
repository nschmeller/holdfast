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
use crate::{AppState, GameSet, RunSetup};

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

    /// Short label for the world selector chips.
    pub fn short_name(self) -> &'static str {
        match self {
            Self::Desk => "DESK",
            Self::Forest => "WILD",
            Self::Rooftop => "CITY",
            Self::Grid => "GRID",
            Self::Arcane => "ARCANE",
        }
    }

    /// The world's signature colour, used for its chip, its detail panel border
    /// and anywhere else the UI needs to say "you are here".
    pub fn accent(self) -> Color {
        match self {
            Self::Desk => Color::srgb(1.0, 0.745, 0.29),
            Self::Forest => Color::srgb(0.494, 0.863, 0.549),
            Self::Rooftop => Color::srgb(1.0, 0.251, 0.588),
            Self::Grid => Color::srgb(0.302, 0.949, 1.0),
            Self::Arcane => Color::srgb(0.682, 0.42, 1.0),
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
        let mut scene = match self {
            Self::Desk => desk::build(rng),
            Self::Forest => forest::build(rng),
            Self::Rooftop => rooftop::build(rng),
            Self::Grid => grid::build(rng),
            Self::Arcane => arcane::build(rng),
        };

        // Pull territory markers far enough inside that their whole capture
        // radius is reachable. Authoring zones by eye against a corner is easy
        // to get slightly wrong, and the failure mode - a zone the player can
        // never fully stand in - is invisible until someone tries to hold it.
        let bounds = scene.bounds;
        for zone in &mut scene.zones {
            *zone = bounds.clamp(*zone, crate::allies::ZONE_RADIUS);
        }

        scene
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
            .add_systems(OnExit(AppState::Menu), mark_dirty.in_set(RunSetup::Reset))
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

    // Environmental hazards. Each kind gets its own emissive so the floor
    // reads at a glance: red burns, brown slows, green heals.
    for h in scene.hazards {
        let tint = match h.kind {
            HazardKind::Scald => Glow::Warning,
            HazardKind::Sticky => Glow::Scrap,
            HazardKind::Shock => Glow::Plasma,
            HazardKind::Font => Glow::ZoneHeld,
        };
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
            MeshMaterial3d(art.glow(tint)),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn build_all() -> Vec<(EnvKind, SceneData)> {
        EnvKind::ALL
            .iter()
            .map(|k| (*k, k.build(&mut Rng::seeded(0xA11CE))))
            .collect()
    }

    #[test]
    fn every_world_builds_without_panicking() {
        assert_eq!(build_all().len(), EnvKind::COUNT);
    }

    #[test]
    fn the_kind_list_matches_the_declared_count() {
        assert_eq!(EnvKind::ALL.len(), EnvKind::COUNT);
        // The name table in `enemy.rs` indexes by `env as usize`, so the
        // discriminants must line up with the array order.
        for (i, kind) in EnvKind::ALL.iter().enumerate() {
            assert_eq!(*kind as usize, i, "{kind:?} is out of order");
        }
    }

    #[test]
    fn cycling_worlds_wraps_in_both_directions() {
        for kind in EnvKind::ALL {
            assert_eq!(kind.next().prev(), kind);
            assert_eq!(kind.prev().next(), kind);
        }
        let mut seen = vec![EnvKind::Desk];
        let mut cursor = EnvKind::Desk;
        for _ in 1..EnvKind::COUNT {
            cursor = cursor.next();
            seen.push(cursor);
        }
        assert_eq!(cursor.next(), EnvKind::Desk, "the carousel must loop");
        seen.sort_by_key(|k| *k as usize);
        seen.dedup();
        assert_eq!(seen.len(), EnvKind::COUNT, "next() skipped a world");
    }

    #[test]
    fn every_world_is_fully_described() {
        for kind in EnvKind::ALL {
            assert!(!kind.title().is_empty(), "{kind:?} has no title");
            assert!(!kind.tagline().is_empty(), "{kind:?} has no tagline");
            assert!(!kind.quirk().is_empty(), "{kind:?} has no quirk");
            assert!(!kind.short_name().is_empty(), "{kind:?} has no chip label");
            assert!(
                kind.short_name().len() <= 7,
                "{kind:?} chip label will overflow its box"
            );
        }
    }

    #[test]
    fn world_titles_and_accents_are_distinct() {
        let mut titles: Vec<_> = EnvKind::ALL.iter().map(|k| k.title()).collect();
        titles.sort_unstable();
        titles.dedup();
        assert_eq!(titles.len(), EnvKind::COUNT);

        // Accents identify a world at a glance, so no two may collide.
        for a in EnvKind::ALL {
            for b in EnvKind::ALL {
                if a as usize >= b as usize {
                    continue;
                }
                let (x, y) = (a.accent().to_linear(), b.accent().to_linear());
                let delta = (x.red - y.red).abs() + (x.green - y.green).abs() + (x.blue - y.blue).abs();
                assert!(delta > 0.2, "{a:?} and {b:?} look the same");
            }
        }
    }

    #[test]
    fn every_arena_is_a_sensible_size() {
        for (kind, scene) in build_all() {
            let b = scene.bounds;
            assert!(b.half_x >= 15.0 && b.half_x <= 40.0, "{kind:?} x {}", b.half_x);
            assert!(b.half_z >= 10.0 && b.half_z <= 30.0, "{kind:?} z {}", b.half_z);
            // Wider than tall keeps the third-person overlook framing sane.
            assert!(b.half_x > b.half_z, "{kind:?} is taller than it is wide");
        }
    }

    #[test]
    fn every_arena_is_furnished() {
        for (kind, scene) in build_all() {
            assert!(scene.props.len() > 10, "{kind:?} is bare");
            assert!(!scene.lights.is_empty(), "{kind:?} has no point lights");
            assert!(scene.sun_illuminance > 0.0, "{kind:?} has no sun");
        }
    }

    #[test]
    fn every_arena_has_contestable_territory() {
        for (kind, scene) in build_all() {
            assert!(
                scene.zones.len() >= 4,
                "{kind:?} has only {} zones",
                scene.zones.len()
            );
        }
    }

    #[test]
    fn zones_sit_inside_their_arena_with_room_to_stand() {
        for (kind, scene) in build_all() {
            for zone in &scene.zones {
                let clamped = scene.bounds.clamp(*zone, crate::allies::ZONE_RADIUS);
                assert!(
                    (clamped - *zone).length() < 1e-3,
                    "{kind:?} zone {zone:?} hangs off the edge"
                );
            }
        }
    }

    #[test]
    fn zones_are_spread_out_rather_than_stacked() {
        for (kind, scene) in build_all() {
            for (i, a) in scene.zones.iter().enumerate() {
                for b in scene.zones.iter().skip(i + 1) {
                    assert!(
                        a.distance(*b) > crate::allies::ZONE_RADIUS,
                        "{kind:?} has overlapping zones at {a:?} and {b:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn hazards_are_placed_inside_the_arena() {
        for (kind, scene) in build_all() {
            for h in &scene.hazards {
                assert!(
                    scene.bounds.contains(h.pos),
                    "{kind:?} hazard at {:?} is off the board",
                    h.pos
                );
                assert!(h.radius > 0.0, "{kind:?} has a zero-radius hazard");
                assert!(h.slow > 0.0 && h.slow <= 1.0, "{kind:?} slow {}", h.slow);
            }
        }
    }

    #[test]
    fn healing_hazards_only_appear_where_they_are_meant_to() {
        for (kind, scene) in build_all() {
            let healing = scene
                .hazards
                .iter()
                .any(|h| h.kind == HazardKind::Font && h.dps < 0.0);
            assert_eq!(
                healing,
                kind == EnvKind::Arcane,
                "{kind:?} healing terrain is a Sanctum signature"
            );
        }
    }

    #[test]
    fn pulsing_hazards_have_a_legible_duty_cycle() {
        for (kind, scene) in build_all() {
            for h in &scene.hazards {
                let Some((period, on)) = h.duty else { continue };
                assert!(period > 1.0, "{kind:?} pulses too fast to read");
                assert!(
                    (0.1..0.75).contains(&on),
                    "{kind:?} duty {on} leaves no safe window"
                );
            }
        }
    }

    #[test]
    fn the_spawn_point_at_the_origin_is_never_walled_in() {
        // The player always starts at the centre; if a world buries the origin
        // in props, the run begins stuck inside a mug.
        for (kind, scene) in build_all() {
            let mut field = ObstacleField::default();
            for prop in &scene.props {
                if let Some(shape) = prop.collider {
                    field.push(prop.pos, shape, prop.blocks_shots, prop.height);
                }
            }
            let resolved = field.resolve(Vec2::ZERO, crate::player::PLAYER_RADIUS);
            assert!(
                resolved.length() < 4.0,
                "{kind:?} shoves the player {} units at spawn",
                resolved.length()
            );
        }
    }

    #[test]
    fn arenas_leave_room_to_move() {
        // Sample a lattice and require that most of it is walkable, or the
        // arena is a maze rather than a battlefield.
        for (kind, scene) in build_all() {
            let mut field = ObstacleField::default();
            for prop in &scene.props {
                if let Some(shape) = prop.collider {
                    field.push(prop.pos, shape, prop.blocks_shots, prop.height);
                }
            }
            let (mut open, mut total) = (0, 0);
            let mut x = -scene.bounds.half_x;
            while x <= scene.bounds.half_x {
                let mut z = -scene.bounds.half_z;
                while z <= scene.bounds.half_z {
                    total += 1;
                    if !field.overlaps(Vec2::new(x, z), crate::player::PLAYER_RADIUS) {
                        open += 1;
                    }
                    z += 1.0;
                }
                x += 1.0;
            }
            let share = f64::from(open) / f64::from(total);
            assert!(share > 0.6, "{kind:?} is only {share:.2} walkable");
        }
    }

    #[test]
    fn gusts_and_spotlights_are_configured_coherently() {
        for (kind, scene) in build_all() {
            let g = &scene.gust;
            assert!(!g.label.is_empty(), "{kind:?} gust has no label");
            assert!(g.duration > 0.0 && g.cooldown > 0.0, "{kind:?} gust timing");
            assert!(g.strength > 0.0, "{kind:?} gust does nothing");
            assert!(
                (g.dir.length() - 1.0).abs() < 1e-3,
                "{kind:?} gust direction is not normalised"
            );
            assert!(g.lane_half_width > 0.0);

            let s = &scene.spotlight;
            assert!(!s.label.is_empty(), "{kind:?} spotlight has no label");
            assert!(s.radius > 0.0);
            assert!(s.damage_bonus > 0.0);
            assert!(
                scene.bounds.contains(s.center),
                "{kind:?} spotlight is off the board"
            );
        }
    }

    #[test]
    fn worlds_are_visually_distinct() {
        // Two arenas that share a sky and a sun read as the same place.
        let scenes = build_all();
        for (i, (a_kind, a)) in scenes.iter().enumerate() {
            for (b_kind, b) in scenes.iter().skip(i + 1) {
                let sky_delta = {
                    let (x, y) = (a.sky.to_linear(), b.sky.to_linear());
                    (x.red - y.red).abs() + (x.green - y.green).abs() + (x.blue - y.blue).abs()
                };
                let ambient_delta = {
                    let (x, y) = (a.ambient.to_linear(), b.ambient.to_linear());
                    (x.red - y.red).abs() + (x.green - y.green).abs() + (x.blue - y.blue).abs()
                };
                assert!(
                    sky_delta > 1e-4 || ambient_delta > 0.01,
                    "{a_kind:?} and {b_kind:?} are lit identically"
                );
            }
        }
    }

    #[test]
    fn building_a_world_twice_with_one_seed_is_deterministic() {
        for kind in EnvKind::ALL {
            let a = kind.build(&mut Rng::seeded(7));
            let b = kind.build(&mut Rng::seeded(7));
            assert_eq!(a.props.len(), b.props.len(), "{kind:?} prop count drifted");
            assert_eq!(a.zones, b.zones, "{kind:?} zones drifted");
            for (pa, pb) in a.props.iter().zip(b.props.iter()) {
                assert!((pa.pos - pb.pos).length() < 1e-6);
            }
        }
    }

    #[test]
    fn different_seeds_vary_the_scatter() {
        // Only worlds with random scatter need to differ; all five have some.
        for kind in EnvKind::ALL {
            let a = kind.build(&mut Rng::seeded(1));
            let b = kind.build(&mut Rng::seeded(2));
            let same = a.props.len() == b.props.len()
                && a.props
                    .iter()
                    .zip(b.props.iter())
                    .all(|(x, y)| (x.pos - y.pos).length() < 1e-6);
            assert!(!same, "{kind:?} ignores its seed entirely");
        }
    }

    #[test]
    fn tall_props_block_shots_and_flat_ones_do_not() {
        for (kind, scene) in build_all() {
            for prop in &scene.props {
                if prop.collider.is_none() {
                    continue;
                }
                if prop.blocks_shots {
                    assert!(
                        prop.height >= 0.75,
                        "{kind:?} has a {}-high prop stopping shots",
                        prop.height
                    );
                }
            }
        }
    }

    #[test]
    fn prop_spec_builder_defaults_are_sane() {
        let p = PropSpec::new(crate::meshgen::cube(1.0, 1.0, 1.0), Vec2::ZERO);
        assert!(p.collider.is_none());
        assert!(!p.blocks_shots);

        let solid = PropSpec::new(crate::meshgen::cube(1.0, 1.0, 1.0), Vec2::ZERO)
            .solid(ColliderShape::Circle(1.0), 2.0);
        assert!(solid.blocks_shots, "tall props should stop shots");

        let low = PropSpec::new(crate::meshgen::cube(1.0, 1.0, 1.0), Vec2::ZERO)
            .solid(ColliderShape::Circle(1.0), 0.2);
        assert!(!low.blocks_shots, "flat props should not");

        let forced = solid.passthrough();
        assert!(!forced.blocks_shots);
    }
}
