use bevy::{mesh::VertexAttributeValues, prelude::*, window::PrimaryWindow};
use bevy_procedural_grass::grass::chunk::GrassChunks;
use bevy_procedural_grass::prelude::*;
use noise::NoiseFn;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            ProceduralGrassPlugin {
                config: GrassConfig::default(),
                wind: GrassWind {
                    wind_data: Wind {
                        speed: 0.1,
                        amplitude: 4.0,
                        ..default()
                    },
                    ..default()
                },
            },
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, update_debug_overlay)
        .run();
}

fn update_debug_overlay(
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    chunks_query: Query<&GrassChunks>,
    time: Res<Time>,
    mut tick_accum: Local<f32>,
) {
    if std::env::var_os("GRASS_DEBUG_OVERLAY").is_none() {
        return;
    }

    *tick_accum += time.delta_secs();
    if *tick_accum < 0.2 {
        return;
    }
    *tick_accum = 0.0;

    let mut grass_entities = 0usize;
    let mut chunks_total = 0usize;
    let mut chunks_loaded = 0usize;
    let mut chunks_render = 0usize;

    for chunks in &chunks_query {
        grass_entities += 1;
        chunks_total += chunks.chunks.len();
        chunks_loaded += chunks.loaded.len();
        chunks_render += chunks.render.len();
    }

    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    window.title = format!(
        "demo | g:{} c:{} l:{} r:{}",
        grass_entities, chunks_total, chunks_loaded, chunks_render
    );
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let mut terrain_mesh = Plane3d::default()
        .mesh()
        .size(100.0, 100.0)
        .subdivisions(100)
        .build();
    if let Some(VertexAttributeValues::Float32x3(positions)) =
        terrain_mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        for position in positions.iter_mut() {
            let y = noise::Perlin::new(1)
                .get([(position[0] * 0.05) as f64, (position[2] * 0.05) as f64])
                as f32;
            position[1] += y;
        }
    }

    let terrain = commands
        .spawn((
            Mesh3d(meshes.add(terrain_mesh)),
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
        Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::new(2.5, 3.5, 0.0), Vec3::Y),
    ));
}
