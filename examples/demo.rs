use bevy::prelude::*;
use bevy_procedural_grass::prelude::*;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            ProceduralGrassPlugin {
                config: GrassConfig::default(),
                wind: GrassWind {
                    wind_data: Wind {
                        speed: 0.18,
                        amplitude: 1.4,
                        frequency: 1.6,
                        direction: 35.0,
                        oscillation: 0.75,
                        scale: 55.0,
                        ..default()
                    },
                    ..default()
                },
            },
        ))
        .add_systems(Startup, setup)
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
        },
        ..default()
    });

    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.75, 4.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 1.0, 1.0),
            ..default()
        })),
        Transform::from_xyz(0.0, 2.0, 0.0),
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
        Transform::from_xyz(-2.5, 6.0, 12.0).looking_at(Vec3::new(2.5, 3.5, 0.0), Vec3::Y),
    ));
}
