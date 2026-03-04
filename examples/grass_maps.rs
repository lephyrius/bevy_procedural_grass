use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
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
    mut images: ResMut<Assets<Image>>,
) {
    let density_map = images.add(build_density_map(256));
    let height_map = images.add(build_height_map(256));
    let elevation_map = images.add(build_elevation_map(256));

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
            maps: GrassPlacementMaps {
                density_map: Some(density_map),
                height_map: Some(height_map),
                height_scale: 1.0,
                elevation_map: Some(elevation_map),
                elevation_scale: 0.8,
                uv_scale: Vec2::splat(2.0),
                uv_offset: Vec2::new(0.1, 0.0),
            },
            ..default()
        },
        ..default()
    });

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 5.0, 1.0))),
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
        Transform::from_xyz(-8.0, 10.0, 16.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn build_density_map(size: u32) -> Image {
    build_mask_map(size, |uv| {
        let centered = uv * 2.0 - Vec2::ONE;
        let radial = (1.0 - centered.length()).clamp(0.0, 1.0);
        radial.powf(1.6)
    })
}

fn build_height_map(size: u32) -> Image {
    build_mask_map(size, |uv| {
        let wave = 0.5 + 0.5 * (uv.x * std::f32::consts::TAU * 2.0).sin();
        let crest = 0.2 + wave * 0.6;
        let signed = (uv.y - crest) * 4.0;
        (0.5 + signed * 0.5).clamp(0.0, 1.0)
    })
}

fn build_elevation_map(size: u32) -> Image {
    build_mask_map(size, |uv| {
        let wave_x = 0.5 + 0.5 * (uv.x * std::f32::consts::TAU * 1.5).sin();
        let wave_y = 0.5 + 0.5 * (uv.y * std::f32::consts::TAU * 1.1).cos();
        (wave_x * wave_y).clamp(0.0, 1.0)
    })
}

fn build_mask_map(size: u32, mut f: impl FnMut(Vec2) -> f32) -> Image {
    let mut image = Image::new_fill(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0],
        TextureFormat::R8Unorm,
        RenderAssetUsages::default(),
    );

    if let Some(data) = image.data.as_mut() {
        let inv_size = if size > 1 {
            1.0 / (size - 1) as f32
        } else {
            1.0
        };
        for y in 0..size {
            for x in 0..size {
                let uv = Vec2::new(x as f32 * inv_size, y as f32 * inv_size);
                let value = (f(uv).clamp(0.0, 1.0) * 255.0).round() as u8;
                data[(y * size + x) as usize] = value;
            }
        }
    }

    image
}
