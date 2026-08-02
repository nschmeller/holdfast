//! API smoke test - temporary. Exercises every Bevy 0.19 surface the game needs
//! so the compiler can tell us the real signatures before we write the game.

use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
enum GameState {
    #[default]
    Menu,
    Playing,
}

#[derive(Resource)]
struct Score(u32);

#[derive(Component)]
struct Spinner;

#[derive(Message)]
struct DamageMessage {
    amount: f32,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_state::<GameState>()
        .insert_resource(Score(0))
        .add_message::<DamageMessage>()
        .add_systems(Startup, setup)
        .add_systems(Update, (spin, read_input, read_damage))
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 6.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 8000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 10.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.6, 0.7, 1.0),
        brightness: 200.0,
        ..default()
    });

    let mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.4, 0.2),
        perceptual_roughness: 0.6,
        ..default()
    });

    commands.spawn((Mesh3d(mesh), MeshMaterial3d(mat), Transform::default(), Spinner));

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
        Text::new("DESK FREE-FOR-ALL"),
        TextFont {
            font_size: 28.0,
            ..default()
        },
        TextColor(Color::WHITE),
    ));
}

fn spin(time: Res<Time>, mut q: Query<&mut Transform, With<Spinner>>) {
    for mut t in &mut q {
        t.rotate_y(time.delta_secs());
    }
}

fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut writer: MessageWriter<DamageMessage>,
    mut score: ResMut<Score>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if keys.just_pressed(KeyCode::Space) {
        writer.write(DamageMessage { amount: 5.0 });
        score.0 += 1;
    }
    if keys.just_pressed(KeyCode::Enter) && *state.get() == GameState::Menu {
        next.set(GameState::Playing);
    }
}

fn read_damage(mut reader: MessageReader<DamageMessage>) {
    for msg in reader.read() {
        info!("damage {}", msg.amount);
    }
}
