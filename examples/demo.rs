use bevy::{mesh::VertexAttributeValues, prelude::*, window::PrimaryWindow};
use bevy_procedural_grass::prelude::*;
use bevy_procedural_grass::{debug, grass::chunk::GrassChunks};
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

fn update_debug_overlay(
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    chunk_query: Query<&GrassChunks>,
) {
    let mut source_chunks = 0usize;
    let mut loaded_chunks = 0usize;
    let mut render_chunks = 0usize;
    for chunks in &chunk_query {
        source_chunks += chunks.chunks.len();
        loaded_chunks += chunks.loaded.len();
        render_chunks += chunks.render.len();
    }

    let render_debug = debug::render_debug_snapshot();
    window.title = format!(
        "demo | main s:{} l:{} r:{} | render qe:{} qh:{} pq:{} pc:{} pr:{} pp:{} pe:{} sg:{}/{} sw:{} draw:{} dc:{} di:{} miss:{}",
        source_chunks,
        loaded_chunks,
        render_chunks,
        render_debug.queued_entities,
        render_debug.queued_chunk_handles,
        render_debug.pipeline_queued,
        render_debug.pipeline_creating,
        render_debug.pipeline_ready,
        render_debug.pipeline_pending,
        render_debug.pipeline_error,
        render_debug.set_grass_calls,
        render_debug.set_grass_missing,
        render_debug.set_wind_calls,
        render_debug.draw_calls,
        render_debug.drawn_chunks,
        render_debug.drawn_instances,
        render_debug.missing_chunk_buffers,
    );
}
