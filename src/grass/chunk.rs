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
    mut chunk_coords_cache: Local<Vec<(i32, i32, i32)>>,
) {
    let camera_position = camera_query.iter().next().map(GlobalTransform::translation);

    for mut chunks in query.iter_mut() {
        chunks.render.clear();

        chunk_coords_cache.clear();
        chunk_coords_cache.extend(chunks.chunks.keys().copied());
        if chunk_coords_cache.is_empty() {
            continue;
        }

        let chunk_size = chunks.chunk_size;
        let chunk_center_offset = Vec3::splat(chunk_size * 0.5);
        let chunk_count = chunk_coords_cache.len();

        let Some(cam_pos) = camera_position else {
            chunks.render.reserve(chunk_count);
            for chunk_coord in chunk_coords_cache.iter().copied() {
                let handle = if let Some(handle) = chunks.loaded.get(&chunk_coord) {
                    handle.clone()
                } else {
                    let instance = &chunks.chunks.get(&chunk_coord).unwrap().0;
                    let handle = grass_asset.add(GrassChunkData(instance.clone()));
                    chunks.loaded.insert(chunk_coord, handle.clone());
                    handle
                };
                chunks.render.push((GrassLOD::High, handle));
            }
            continue;
        };

        let lod_distance_sq = grass_config.lod_distance * grass_config.lod_distance;
        let cull_distance_sq = grass_config.cull_distance * grass_config.cull_distance;
        let cull_dimension = chunks.cull_dimension;

        chunks.render.reserve(chunk_count);
        for chunk_coord in chunk_coords_cache.iter().copied() {
            let world_pos = Vec3::new(
                chunk_coord.0 as f32,
                chunk_coord.1 as f32,
                chunk_coord.2 as f32,
            ) * chunk_size;
            let chunk_center = world_pos + chunk_center_offset;

            let d3_vec = chunk_center - cam_pos;
            let d3_distance_sq = d3_vec.length_squared();
            let cull_distance_current_sq = match cull_dimension {
                CullDimension::D2 => (chunk_center.xz() - cam_pos.xz()).length_squared(),
                CullDimension::D3 => d3_distance_sq,
            };
            if cull_distance_current_sq > cull_distance_sq {
                chunks.loaded.remove(&chunk_coord);
                continue;
            }

            let lod_type = if d3_distance_sq <= lod_distance_sq {
                GrassLOD::High
            } else {
                GrassLOD::Low
            };

            let handle = if let Some(handle) = chunks.loaded.get(&chunk_coord) {
                handle.clone()
            } else {
                let instance = &chunks.chunks.get(&chunk_coord).unwrap().0;
                let handle = grass_asset.add(GrassChunkData(instance.clone()));
                chunks.loaded.insert(chunk_coord, handle.clone());
                handle
            };
            chunks.render.push((lod_type, handle));
        }

        if chunks.render.is_empty() && !chunks.chunks.is_empty() {
            for chunk_coord in chunk_coords_cache.iter().copied() {
                let handle = if let Some(handle) = chunks.loaded.get(&chunk_coord) {
                    handle.clone()
                } else {
                    let instance = &chunks.chunks.get(&chunk_coord).unwrap().0;
                    let handle = grass_asset.add(GrassChunkData(instance.clone()));
                    chunks.loaded.insert(chunk_coord, handle.clone());
                    handle
                };
                chunks.render.push((GrassLOD::High, handle));
            }
        }
    }
}
