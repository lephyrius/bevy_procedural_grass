use bevy::prelude::*;
use bevy_procedural_grass::prelude::*;

#[derive(Component)]
struct GrassInteractorMover {
    radius: f32,
    speed: f32,
    height: f32,
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, ProceduralGrassPlugin::default()))
        .add_systems(Startup, setup)
        .add_systems(Update, move_interactor)
        .run();
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let terrain = commands
        .spawn((
            Mesh3d(meshes.add(Plane3d::default().mesh().size(100.0, 100.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.0, 0.05, 0.0),
                reflectance: 0.0,
                ..default()
            })),
            Transform::from_scale(Vec3::new(1.0, 3.0, 1.0)),
        ))
        .id();

    commands.spawn(GrassBundle {
        mesh: Mesh3d(meshes.add(GrassMesh::mesh(7))),
        lod: GrassLODMesh::new(meshes.add(GrassMesh::mesh(3))),
        grass: Grass {
            entity: Some(terrain),
            ..default()
        }
        .with_blade_size(2.0, 0.08),
        ..default()
    });

    let mover = GrassInteractorMover {
        radius: 14.0,
        speed: 0.9,
        height: 1.2,
    };
    commands.spawn((
        Mesh3d(meshes.add(Capsule3d::new(0.35, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.95, 0.95, 0.95),
            ..default()
        })),
        Transform::from_xyz(mover.radius, mover.height, 0.0),
        GrassInteractor {
            radius: 2.5,
            strength: 1.4,
            falloff: 2.0,
        },
        mover,
    ));

    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_xyzw(
            -0.420_735_5,
            -0.420_735_5,
            0.229_848_86,
            0.770_151_14,
        )),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-18.0, 12.0, 18.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
    ));
}

fn move_interactor(time: Res<Time>, mut query: Query<(&mut Transform, &GrassInteractorMover)>) {
    let t = time.elapsed_secs();
    for (mut transform, mover) in &mut query {
        let angle = t * mover.speed;
        let position = Vec3::new(
            angle.cos() * mover.radius,
            mover.height,
            angle.sin() * mover.radius,
        );
        let tangent = Vec3::new(-angle.sin(), 0.0, angle.cos());
        transform.translation = position;
        transform.look_to(tangent, Vec3::Y);
    }
}
