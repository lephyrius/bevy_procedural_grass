use bevy::prelude::*;
use bevy_procedural_grass::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, ProceduralGrassPlugin::default()))
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let terrain_mesh = Sphere::new(1.0).mesh().ico(6).unwrap();

    let terrain = commands
        .spawn((
            Mesh3d(meshes.add(terrain_mesh)),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.0, 0.05, 0.0),
                reflectance: 0.0,
                ..default()
            })),
            Transform::from_scale(Vec3::splat(20.0)),
        ))
        .id();

    commands.spawn(GrassBundle {
        mesh: Mesh3d(meshes.add(GrassMesh::mesh(7))),
        grass: Grass {
            density: 25,
            entity: Some(terrain),
            ..default()
        },
        ..default()
    });

    commands.spawn((
        DirectionalLight::default(),
        Transform::from_rotation(Quat::from_xyzw(
            -0.420_735_5,
            -0.420_735_5,
            0.229_848_86,
            0.770_151_14,
        )),
    ));

    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.75, 4.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 1.0, 1.0),
            ..default()
        })),
        Transform::from_xyz(0.0, 2.0, 0.0),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-8.0, 8.0, 16.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
