use rand::RngExt;
use std::collections::HashMap;

use bevy::{
    camera::visibility::NoFrustumCulling, ecs::query::QueryItem, mesh::VertexAttributeValues,
    prelude::*, render::extract_component::ExtractComponent,
};
#[cfg(feature = "bevy-inspector-egui")]
use bevy_inspector_egui::{InspectorOptions, prelude::ReflectInspectorOptions};

use bytemuck::{Pod, Zeroable};
use rand::rngs::SmallRng;

use crate::render::instance::{GrassChunkData, GrassData};

use super::chunk::GrassChunks;

#[derive(Bundle, Default)]
pub struct GrassBundle {
    pub mesh: Mesh3d,
    pub lod: GrassLODMesh,
    pub grass: Grass,
    pub grass_chunks: GrassChunks,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub visibility: Visibility,
    pub inherited_visibility: InheritedVisibility,
    pub view_visibility: ViewVisibility,
    pub frustum_culling: NoFrustumCulling,
}

pub fn generate_grass(
    mut query: Query<(&Grass, &mut GrassChunks)>,
    mesh_entity_query: Query<(&Transform, &Mesh3d)>,
    meshes: Res<Assets<Mesh>>,
) {
    for (grass, mut chunks) in query.iter_mut() {
        if !chunks.chunks.is_empty() {
            continue;
        }
        let Some(entity) = grass.entity else {
            continue;
        };
        let Ok((transform, mesh3d)) = mesh_entity_query.get(entity) else {
            continue;
        };
        let Some(mesh) = meshes.get(&mesh3d.0) else {
            continue;
        };

        chunks.chunks = grass.generate_grass(transform, mesh, chunks.chunk_size);
    }
}

#[derive(Component)]
#[cfg_attr(feature = "bevy-inspector-egui", derive(Reflect, InspectorOptions))]
#[cfg_attr(feature = "bevy-inspector-egui", reflect(InspectorOptions))]
pub struct Grass {
    pub entity: Option<Entity>,
    pub density: u32,
    pub color: GrassColor,
    pub blade: Blade,
}

impl Default for Grass {
    fn default() -> Self {
        Self {
            density: 25,
            entity: None,
            color: GrassColor::default(),
            blade: Blade::default(),
        }
    }
}

impl Grass {
    fn generate_grass(
        &self,
        transform: &Transform,
        mesh: &Mesh,
        chunk_size: f32,
    ) -> HashMap<(i32, i32, i32), GrassChunkData> {
        let mut chunks: HashMap<(i32, i32, i32), GrassChunkData> = HashMap::default();
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            return chunks;
        };

        let normals = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
            Some(VertexAttributeValues::Float32x3(normals)) => Some(normals),
            _ => None,
        };

        let inv_chunk_size = 1.0 / chunk_size;
        let density = self.density as f32;
        let mut rng: SmallRng = rand::make_rng();

        let mut scatter_triangle = |i0: usize, i1: usize, i2: usize| {
            let v0 = Vec3::from(positions[i0]) * transform.scale;
            let v1 = Vec3::from(positions[i1]) * transform.scale;
            let v2 = Vec3::from(positions[i2]) * transform.scale;

            let face_normal = (v1 - v0).cross(v2 - v0);
            let mut normal = face_normal.normalize_or_zero();
            if let Some(normals) = normals {
                let reference_normal =
                    (Vec3::from(normals[i0]) + Vec3::from(normals[i1]) + Vec3::from(normals[i2]))
                        .normalize_or_zero();
                if reference_normal != Vec3::ZERO && normal.dot(reference_normal) < 0.0 {
                    normal = -normal;
                }
            }

            let area = face_normal.length() * 0.5;
            let scaled_density = (density * area).ceil() as u32;
            if scaled_density == 0 {
                return;
            }

            for _ in 0..scaled_density {
                let r1 = rng.random_range(0.0..1.0_f32).sqrt();
                let r2 = rng.random_range(0.0..1.0_f32);
                let barycentric = Vec3::new(1.0 - r1, r1 * (1.0 - r2), r1 * r2);

                let position = (v0 * barycentric.x + v1 * barycentric.y + v2 * barycentric.z)
                    + transform.translation;

                let chunk_coords = (
                    (position.x * inv_chunk_size).floor() as i32,
                    (position.y * inv_chunk_size).floor() as i32,
                    (position.z * inv_chunk_size).floor() as i32,
                );

                let chunk_base = Vec3::new(
                    chunk_coords.0 as f32,
                    chunk_coords.1 as f32,
                    chunk_coords.2 as f32,
                ) * chunk_size;
                let chunk_pos = position - chunk_base;
                let chunk_uvw = chunk_pos * inv_chunk_size;

                let instance = GrassData::new(position, normal, chunk_uvw);

                chunks.entry(chunk_coords).or_default().0.push(instance);
            }
        };

        if let Some(indices) = mesh.indices() {
            let mut triangle = [0usize; 3];
            let mut triangle_len = 0usize;
            for index in indices.iter() {
                triangle[triangle_len] = index;
                triangle_len += 1;
                if triangle_len != 3 {
                    continue;
                }
                scatter_triangle(triangle[0], triangle[1], triangle[2]);
                triangle_len = 0;
            }
        } else {
            let triangle_count = positions.len() / 3;
            for triangle_idx in 0..triangle_count {
                let base = triangle_idx * 3;
                scatter_triangle(base, base + 1, base + 2);
            }
        }

        chunks
    }
}

impl ExtractComponent for Grass {
    type QueryData = &'static Grass;
    type QueryFilter = ();
    type Out = (GrassColor, Blade);

    #[inline]
    fn extract_component(item: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some((item.color, item.blade))
    }
}

#[derive(Component, Clone, Copy)]
#[cfg_attr(feature = "bevy-inspector-egui", derive(Reflect, InspectorOptions))]
#[cfg_attr(feature = "bevy-inspector-egui", reflect(InspectorOptions))]
pub struct GrassColor {
    pub ao: Color,
    pub color_1: Color,
    pub color_2: Color,
}

impl GrassColor {
    #[inline]
    pub fn to_array(&self) -> [[f32; 4]; 3] {
        [
            LinearRgba::from(self.ao).to_f32_array(),
            LinearRgba::from(self.color_1).to_f32_array(),
            LinearRgba::from(self.color_2).to_f32_array(),
        ]
    }
}

impl Default for GrassColor {
    fn default() -> Self {
        Self {
            ao: Color::srgb(0.01, 0.02, 0.05),
            color_1: Color::srgb(0.1, 0.23, 0.09),
            color_2: Color::srgb(0.12, 0.39, 0.15),
        }
    }
}

#[derive(Component, Clone, Copy, PartialEq, Pod, Zeroable)]
#[cfg_attr(feature = "bevy-inspector-egui", derive(Reflect, InspectorOptions))]
#[cfg_attr(feature = "bevy-inspector-egui", reflect(InspectorOptions))]
#[repr(C)]
pub struct Blade {
    pub length: f32,
    pub width: f32,
    pub tilt: f32,
    pub tilt_variance: f32,
    pub p1_flexibility: f32,
    pub p2_flexibility: f32,
    pub curve: f32,
    pub specular: f32,
    pub far_lod_start: f32,
    pub far_lod_end: f32,
    pub _padding: [f32; 2],
}

impl Default for Blade {
    fn default() -> Self {
        Self {
            length: 1.5,
            width: 0.05,
            tilt: 0.5,
            tilt_variance: 0.2,
            p1_flexibility: 0.5,
            p2_flexibility: 0.5,
            curve: 15.0,
            specular: 0.02,
            far_lod_start: 80.0,
            far_lod_end: 140.0,
            _padding: [0.0, 0.0],
        }
    }
}

#[derive(Component, Default, Clone)]
pub struct GrassLODMesh {
    pub mesh_handle: Option<Handle<Mesh>>,
}

impl GrassLODMesh {
    #[inline]
    pub fn new(mesh_handle: Handle<Mesh>) -> Self {
        Self {
            mesh_handle: Some(mesh_handle),
        }
    }
}

impl ExtractComponent for GrassLODMesh {
    type QueryData = &'static GrassLODMesh;
    type QueryFilter = ();
    type Out = Self;

    #[inline]
    fn extract_component(item: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(item.clone())
    }
}
