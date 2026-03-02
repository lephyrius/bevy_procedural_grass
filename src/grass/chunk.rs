use std::collections::HashMap;

use bevy::{ecs::query::QueryItem, prelude::*, render::extract_component::ExtractComponent};

use crate::render::instance::GrassChunkData;

use super::config::GrassConfig;

#[derive(Clone, Copy)]
pub enum GrassLOD {
    High,
    Low,
}

#[derive(Clone, Copy, Default)]
pub enum CullDimension {
    #[default]
    D2,
    D3,
}

pub type GrassRenderInfo = (GrassLOD, Handle<GrassChunkData>);

#[derive(Component, Clone)]
pub struct GrassChunks {
    pub chunk_size: f32,
    pub cull_dimension: CullDimension,
    pub chunks: HashMap<(i32, i32, i32), GrassChunkData>,
    pub loaded: HashMap<(i32, i32, i32), Handle<GrassChunkData>>,
    pub render: Vec<GrassRenderInfo>,
}

impl Default for GrassChunks {
    fn default() -> Self {
        Self {
            chunk_size: 30.0,
            cull_dimension: CullDimension::D2,
            chunks: HashMap::default(),
            loaded: HashMap::default(),
            render: Vec::new(),
        }
    }
}

impl ExtractComponent for GrassChunks {
    type QueryData = &'static GrassChunks;
    type QueryFilter = ();
    type Out = RenderGrassChunks;

    fn extract_component(item: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(RenderGrassChunks(item.render.clone()))
    }
}

#[derive(Component, Default, Clone)]
pub struct RenderGrassChunks(pub Vec<GrassRenderInfo>);

pub(crate) fn grass_culling(
    mut query: Query<&mut GrassChunks>,
    camera_query: Query<&GlobalTransform, With<Camera3d>>,
    mut grass_asset: ResMut<Assets<GrassChunkData>>,
    grass_config: Res<GrassConfig>,
) {
    for mut chunks in query.iter_mut() {
        chunks.render.clear();

        for camera_transform in &camera_query {
            let chunk_coords: Vec<(i32, i32, i32)> = chunks.chunks.keys().copied().collect();
            let mut chunks_inside = Vec::new();
            let mut chunks_outside = Vec::new();

            for chunk_coord in chunk_coords {
                let (x, y, z) = chunk_coord;
                let world_pos = Vec3::new(x as f32, y as f32, z as f32) * chunks.chunk_size;
                let chunk_center = world_pos + Vec3::splat(chunks.chunk_size * 0.5);
                let cam_pos = camera_transform.translation();

                let d3_distance = (chunk_center - cam_pos).length();
                let lod_type = if d3_distance <= grass_config.lod_distance {
                    GrassLOD::High
                } else {
                    GrassLOD::Low
                };

                let cull_distance = match chunks.cull_dimension {
                    CullDimension::D2 => (chunk_center.xz() - cam_pos.xz()).length(),
                    CullDimension::D3 => d3_distance,
                };

                // In Bevy 0.18, the old per-chunk frustum test became overly aggressive for this
                // chunk AABB layout. Keep robust distance culling until chunk bounds are
                // reworked to match the generated blade extents.
                if cull_distance <= grass_config.cull_distance {
                    chunks_inside.push((chunk_coord, lod_type));
                } else {
                    chunks_outside.push(chunk_coord);
                }
            }

            for chunk_coord in chunks_outside {
                chunks.loaded.remove(&chunk_coord);
            }

            for (chunk_coord, _) in &chunks_inside {
                if !chunks.loaded.contains_key(chunk_coord) {
                    let instance = &chunks.chunks.get(chunk_coord).unwrap().0;
                    let handle = grass_asset.add(GrassChunkData(instance.clone()));
                    chunks.loaded.insert(*chunk_coord, handle);
                }
            }

            let mut render_chunks = Vec::with_capacity(chunks_inside.len());
            for (chunk_coord, lod) in chunks_inside {
                if let Some(handle) = chunks.loaded.get(&chunk_coord) {
                    render_chunks.push((lod, handle.clone()));
                }
            }

            chunks.render.extend(render_chunks);
        }
    }
}
