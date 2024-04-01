use bevy::{prelude::*, sprite::MaterialMesh2dBundle};

const BALL_RADIUS: f32 = 10.0;
const BALL_STARTING_SPEED: f32 = 200.0;
const BALL_GRAVITY: f32 = -300.;

const CAGE_COLOR: Color = Color::rgb(1.0, 1.0, 1.0);
const CAGE_RADIUS: f32 = 100.0;
// Since the collision math does not actually use this value, it's completely visual.
const CAGE_WALL_THICKNESS: f32 = 2.0;

const BACKGROUND_COLOR: Color = Color::rgb(0.1, 0.1, 0.1);

fn main() {
    App::new()
        .add_event::<CageCollisionEvent>()
        .add_event::<OtherCollisionEvent>()
        .add_systems(Startup, setup)
        .add_systems(
            FixedUpdate,
            (apply_gravity, apply_velocity, collide_cage, collide_others).chain(),
        )
        .add_systems(
            Update,
            (
                play_collision_sound,
                bevy::window::close_on_esc,
                spawn_ball_on_space,
                maybe_spawn_ball,
            ),
        )
        .insert_resource(ClearColor(BACKGROUND_COLOR))
        .add_plugins(DefaultPlugins)
        .run();
}

#[derive(Component)]
struct Ball;

#[derive(Component, Deref, DerefMut)]
struct Velocity(Vec2);

#[derive(Component)]
struct Gravity(f32);

#[derive(Component)]
struct Collision;

#[derive(Event)]
struct CageCollisionEvent {
    #[allow(dead_code)]
    entity: Entity,
}

#[derive(Event)]
struct OtherCollisionEvent {
    #[allow(dead_code)]
    self_entity: Entity,
    #[allow(dead_code)]
    other_entity: Entity,
}

#[derive(Resource)]
struct CollisionSound(Handle<AudioSource>);

fn spawn_ball(
    commands: &mut Commands,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    meshes: &mut ResMut<Assets<Mesh>>,
) {
    let colour = Color::rgb(
        rand::random::<f32>(),
        rand::random::<f32>(),
        rand::random::<f32>(),
    );
    let starting_direction = Vec2::new(
        rand::random::<f32>() * 2.0 - 1.0,
        rand::random::<f32>() * 2.0 - 1.0,
    );

    commands.spawn((
        MaterialMesh2dBundle {
            mesh: meshes.add(Circle::default()).into(),
            material: materials.add(colour),
            transform: Transform {
                translation: Vec3::new(0.0, 0.0, 1.0),
                scale: Vec3::new(BALL_RADIUS, BALL_RADIUS, 1.0),
                ..Default::default()
            },
            ..Default::default()
        },
        Ball,
        Velocity(starting_direction.normalize() * BALL_STARTING_SPEED),
        Gravity(BALL_GRAVITY),
        Collision,
    ));
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
) {
    commands.spawn(Camera2dBundle::default());

    let ball_collision_sound = asset_server.load("sounds/wall_collision.ogg");
    commands.insert_resource(CollisionSound(ball_collision_sound));

    // Cage outside
    commands.spawn((MaterialMesh2dBundle {
        mesh: meshes
            .add(Circle {
                radius: CAGE_RADIUS + CAGE_WALL_THICKNESS,
            })
            .into(),
        material: materials.add(CAGE_COLOR),
        ..Default::default()
    },));

    // Cage inside
    commands.spawn((MaterialMesh2dBundle {
        mesh: meshes
            .add(Circle {
                radius: CAGE_RADIUS,
            })
            .into(),
        transform: Transform {
            translation: Vec3::new(0.0, 0.0, 0.1),
            ..Default::default()
        },
        material: materials.add(BACKGROUND_COLOR),
        ..Default::default()
    },));
}

fn apply_velocity(mut query: Query<(&mut Transform, &Velocity)>, time: Res<Time>) {
    for (mut transform, velocity) in &mut query {
        transform.translation.x += velocity.x * time.delta_seconds();
        transform.translation.y += velocity.y * time.delta_seconds();
    }
}

fn apply_gravity(mut query: Query<(&mut Velocity, &Gravity)>, time: Res<Time>) {
    for (mut velocity, gravity) in &mut query {
        velocity.y += gravity.0 * time.delta_seconds();
    }
}

fn collide_cage(
    mut ball_query: Query<(Entity, &mut Transform, &mut Velocity, &Collision)>,
    mut collision_events: EventWriter<CageCollisionEvent>,
) {
    for (entity, mut ball_transform, mut ball_velocity, _) in &mut ball_query {
        let mut ball_position = ball_transform.translation.truncate();
        let ball_radius = BALL_RADIUS;

        let cage_position = Vec2::ZERO;
        let cage_radius = CAGE_RADIUS;

        let distance = ball_position.distance(cage_position);
        if distance + (ball_radius / 2.0) > cage_radius {
            let normal = (cage_position - ball_position).normalize();
            ball_velocity.0 = {
                let velocity = ball_velocity.0;
                velocity - 2.0 * velocity.dot(normal) * normal
            };

            let overlap = ball_radius / 2.0 + distance - cage_radius;
            ball_position += overlap * normal;
            ball_transform.translation = ball_position.extend(ball_transform.translation.z);

            collision_events.send(CageCollisionEvent { entity });
        }
    }
}

fn collide_others(
    mut ball_query: Query<(Entity, &mut Transform, &mut Velocity, &Collision), With<Ball>>,
    mut collision_events: EventWriter<OtherCollisionEvent>,
) {
    let ball_positions: Vec<(Entity, Vec2)> = ball_query
        .iter()
        .map(|(entity, transform, _, _)| (entity, transform.translation.truncate()))
        .collect();
    for (entity, mut ball_transform, mut ball_velocity, _) in &mut ball_query {
        let ball_position = ball_transform.translation.truncate();
        let ball_radius = BALL_RADIUS;

        for (other_entity, other_position) in ball_positions.iter() {
            if ball_position == *other_position {
                continue;
            }
            let other_radius = BALL_RADIUS;

            let distance = ball_position.distance(*other_position);
            if distance < (ball_radius / 2.) + (other_radius / 2.) {
                let normal = (*other_position - ball_position).normalize();
                ball_velocity.0 = {
                    let velocity = ball_velocity.0;
                    velocity - 2.0 * velocity.dot(normal) * normal
                };

                let overlap = (ball_radius / 2.) + (other_radius / 2.) - distance;
                ball_transform.translation -= overlap * normal.extend(0.0);

                collision_events.send(OtherCollisionEvent {
                    self_entity: entity,
                    other_entity: *other_entity,
                });
            }
        }
    }
}

fn maybe_spawn_ball(
    mut commands: Commands,
    mut collision_events: EventReader<CageCollisionEvent>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if !collision_events.is_empty() {
        collision_events.clear();
        if (rand::random::<f32>() * 100.0) < 10.0 {
            spawn_ball(&mut commands, &mut materials, &mut meshes);
        }
    }
}

// fn remove_colliding_balls(
//     mut commands: Commands,
//     mut collision_events: EventReader<OtherCollisionEvent>,
// ) {
//     if !collision_events.is_empty() {
//         collision_events.read().for_each(|event| {
//             commands.entity(event.self_entity).despawn();
//         });
//     }
// }

fn play_sound(commands: &mut Commands, sound: &Res<CollisionSound>) {
    commands.spawn(AudioBundle {
        source: sound.0.clone(),
        // auto-despawn the entity when playback finishes
        settings: PlaybackSettings::DESPAWN,
    });
}

fn play_collision_sound(
    mut commands: Commands,
    mut wall_collision_events: EventReader<CageCollisionEvent>,
    mut ball_collision_events: EventReader<OtherCollisionEvent>,
    sound: Res<CollisionSound>,
) {
    // Play a sound once per frame if a collision occurred.
    if !wall_collision_events.is_empty() {
        // This prevents events staying active on the next frame.
        wall_collision_events.clear();
        play_sound(&mut commands, &sound);
    }

    if !ball_collision_events.is_empty() {
        ball_collision_events.clear();
        play_sound(&mut commands, &sound);
    }
}

fn spawn_ball_on_space(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    query: Query<Entity, With<Ball>>,
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if keyboard_input.just_pressed(KeyCode::Space) {
        for entity in query.iter() {
            // Despawn all balls
            commands.entity(entity).despawn();
        }
        spawn_ball(&mut commands, &mut materials, &mut meshes);
    }
}
