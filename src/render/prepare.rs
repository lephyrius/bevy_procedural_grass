use std::{
    hash::{Hash, Hasher},
    marker::PhantomData,
};

use bevy::{
    pbr::RenderMeshInstances,
    prelude::*,
    render::{
        mesh::{RenderMesh, RenderMeshBufferInfo, allocator::MeshAllocator},
        render_asset::RenderAssets,
        render_resource::{
            BindGroup, BindGroupEntries, BindingResource, Buffer, BufferBinding,
            BufferInitDescriptor, BufferUsages, PipelineCache, ShaderType, WgpuFeatures,
        },
        renderer::{RenderDevice, RenderQueue},
        sync_world::MainEntity,
        texture::{FallbackImage, GpuImage},
    },
};
use bytemuck::{Pod, Zeroable};

use crate::grass::{
    chunk::{GrassLOD, RenderGrassChunks},
    grass::GrassLODMesh,
    grass::{Blade, Grass, GrassColor},
    wind::{GrassWind, Wind},
};

use super::{GrassIndirectSettings, instance::GrassChunkBuffer, pipeline::GrassPipeline};

#[derive(Component, Resource, Clone)]
pub struct BufferBindGroup<T> {
    pub bind_group: BindGroup,
    _marker: PhantomData<T>,
}

impl<T> BufferBindGroup<T> {
    pub fn new(bind_group: BindGroup) -> Self {
        Self {
            bind_group,
            _marker: PhantomData,
        }
    }
}

#[derive(Component, Clone)]
pub struct GrassBuffer {
    pub color_buffer: Buffer,
    pub blade_buffer: Buffer,
}

#[derive(Component, Clone)]
pub struct PreparedGrassUniforms {
    pub color: [[f32; 4]; 3],
    pub blade: Blade,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, ShaderType)]
pub struct DrawIndexedIndirectCommand {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub base_vertex: i32,
    pub first_instance: u32,
}

#[derive(Component, Clone)]
pub struct GrassIndirectBuffers {
    pub high_instance_buffer: Option<Buffer>,
    pub low_instance_buffer: Option<Buffer>,
    pub high_chunk_meta_buffer: Option<Buffer>,
    pub low_chunk_meta_buffer: Option<Buffer>,
    pub high_mesh_params_buffer: Option<Buffer>,
    pub low_mesh_params_buffer: Option<Buffer>,
    pub high_indirect_buffer: Option<Buffer>,
    pub low_indirect_buffer: Option<Buffer>,
    pub high_draw_count: u32,
    pub low_draw_count: u32,
    pub high_indexed: bool,
    pub low_indexed: bool,
    pub signature: u64,
}

#[derive(Clone, Copy, Hash)]
struct LodMeshInfo {
    index_count: u32,
    first_index: u32,
    base_vertex: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, ShaderType)]
pub struct ChunkIndirectMeta {
    pub instance_count: u32,
    pub first_instance: u32,
    pub _pad0: u32,
    pub _pad1: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, ShaderType)]
pub struct IndexedIndirectMeshParams {
    pub index_count: u32,
    pub first_index: u32,
    pub base_vertex: i32,
    pub _pad0: u32,
}

fn create_buffer_with_data<T: Pod>(
    render_device: &RenderDevice,
    label: &'static str,
    usage: BufferUsages,
    data: &[T],
) -> Option<Buffer> {
    if data.is_empty() {
        None
    } else {
        Some(
            render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(data),
                usage,
            }),
        )
    }
}

fn create_lod_indirect_input(
    chunks: &RenderGrassChunks,
    lod_kind: GrassLOD,
    grass_data: &RenderAssets<GrassChunkBuffer>,
) -> (Vec<super::instance::GrassData>, Vec<ChunkIndirectMeta>) {
    let mut instances = Vec::new();
    let mut chunk_meta = Vec::new();
    let mut first_instance = 0u32;

    for (chunk_lod, handle) in &chunks.0 {
        if *chunk_lod != lod_kind {
            continue;
        }
        let Some(chunk) = grass_data.get(handle.id()) else {
            continue;
        };
        let instance_count = chunk.length as u32;
        if instance_count == 0 {
            continue;
        }

        chunk_meta.push(ChunkIndirectMeta {
            instance_count,
            first_instance,
            _pad0: 0,
            _pad1: 0,
        });

        instances.extend_from_slice(&chunk.cpu_data);
        first_instance += instance_count;
    }

    (instances, chunk_meta)
}

fn build_indexed_indirect_cpu(
    chunk_meta: &[ChunkIndirectMeta],
    mesh: IndexedIndirectMeshParams,
) -> Vec<DrawIndexedIndirectCommand> {
    let mut commands = Vec::with_capacity(chunk_meta.len());
    for meta in chunk_meta {
        commands.push(DrawIndexedIndirectCommand {
            index_count: mesh.index_count,
            instance_count: meta.instance_count,
            first_index: mesh.first_index,
            base_vertex: mesh.base_vertex,
            first_instance: meta.first_instance,
        });
    }
    commands
}

pub(crate) fn prepare_indirect_buffers(
    mut commands: Commands,
    indirect_settings: Res<GrassIndirectSettings>,
    render_device: Res<RenderDevice>,
    render_mesh_instances: Res<RenderMeshInstances>,
    mesh_allocator: Res<MeshAllocator>,
    meshes: Res<RenderAssets<RenderMesh>>,
    grass_data: Res<RenderAssets<GrassChunkBuffer>>,
    query: Query<(
        Entity,
        &MainEntity,
        &RenderGrassChunks,
        Option<&GrassLODMesh>,
        Option<&GrassIndirectBuffers>,
    )>,
) {
    if !indirect_settings.enabled {
        for (entity, _, _, _, state) in &query {
            if state.is_some() {
                commands.entity(entity).remove::<GrassIndirectBuffers>();
            }
        }
        return;
    }

    let features = render_device.features();
    let required = WgpuFeatures::INDIRECT_FIRST_INSTANCE;
    if !features.contains(required) {
        for (entity, _, _, _, state) in &query {
            if state.is_some() {
                commands.entity(entity).remove::<GrassIndirectBuffers>();
            }
        }
        return;
    }

    for (entity, main_entity, chunks, lod_mesh, state) in &query {
        if chunks.0.is_empty() {
            if state.is_some() {
                commands.entity(entity).remove::<GrassIndirectBuffers>();
            }
            continue;
        }

        let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(*main_entity) else {
            continue;
        };
        let Some(high_mesh) = meshes.get(mesh_instance.mesh_asset_id) else {
            continue;
        };
        let Some(high_vertex_slice) =
            mesh_allocator.mesh_vertex_slice(&mesh_instance.mesh_asset_id)
        else {
            continue;
        };
        let high_index_slice = mesh_allocator.mesh_index_slice(&mesh_instance.mesh_asset_id);

        let high_info = match &high_mesh.buffer_info {
            RenderMeshBufferInfo::Indexed { count, .. } => {
                let Some(index_slice) = high_index_slice else {
                    continue;
                };
                LodMeshInfo {
                    index_count: *count,
                    first_index: index_slice.range.start,
                    base_vertex: high_vertex_slice.range.start as i32,
                }
            }
            RenderMeshBufferInfo::NonIndexed => continue,
        };

        let low_info = lod_mesh
            .and_then(|lod| lod.mesh_handle.as_ref().map(|handle| handle.id()))
            .and_then(|mesh_id| {
                let low_mesh = meshes.get(mesh_id)?;
                let low_vertex_slice = mesh_allocator.mesh_vertex_slice(&mesh_id)?;
                let low_index_slice = mesh_allocator.mesh_index_slice(&mesh_id);

                Some(match &low_mesh.buffer_info {
                    RenderMeshBufferInfo::Indexed { count, .. } => {
                        let index_slice = low_index_slice?;
                        LodMeshInfo {
                            index_count: *count,
                            first_index: index_slice.range.start,
                            base_vertex: low_vertex_slice.range.start as i32,
                        }
                    }
                    RenderMeshBufferInfo::NonIndexed => return None,
                })
            });

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        chunks.0.hash(&mut hasher);
        mesh_instance.mesh_asset_id.hash(&mut hasher);
        high_info.hash(&mut hasher);
        low_info.hash(&mut hasher);
        let signature = hasher.finish();

        if state.is_some_and(|state| state.signature == signature) {
            continue;
        }

        let (high_instances, high_chunk_meta) =
            create_lod_indirect_input(chunks, GrassLOD::High, grass_data.as_ref());
        let (low_instances, low_chunk_meta) = if low_info.is_some() {
            create_lod_indirect_input(chunks, GrassLOD::Low, grass_data.as_ref())
        } else {
            (Vec::new(), Vec::new())
        };

        let high_instance_buffer = create_buffer_with_data(
            &render_device,
            "grass indirect high instance buffer",
            BufferUsages::VERTEX | BufferUsages::COPY_DST,
            &high_instances,
        );
        let low_instance_buffer = create_buffer_with_data(
            &render_device,
            "grass indirect low instance buffer",
            BufferUsages::VERTEX | BufferUsages::COPY_DST,
            &low_instances,
        );
        let high_chunk_meta_buffer = create_buffer_with_data(
            &render_device,
            "grass indirect high chunk meta buffer",
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
            &high_chunk_meta,
        );
        let low_chunk_meta_buffer = create_buffer_with_data(
            &render_device,
            "grass indirect low chunk meta buffer",
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
            &low_chunk_meta,
        );

        let high_mesh_params = IndexedIndirectMeshParams {
            index_count: high_info.index_count,
            first_index: high_info.first_index,
            base_vertex: high_info.base_vertex,
            _pad0: 0,
        };
        let high_mesh_params_buffer = create_buffer_with_data(
            &render_device,
            "grass indirect high mesh params buffer",
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            &[high_mesh_params],
        );
        let low_mesh_params = low_info.map(|info| IndexedIndirectMeshParams {
            index_count: info.index_count,
            first_index: info.first_index,
            base_vertex: info.base_vertex,
            _pad0: 0,
        });
        let low_mesh_params_buffer = low_mesh_params.map(|params| {
            render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("grass indirect low mesh params buffer"),
                contents: bytemuck::cast_slice(&[params]),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            })
        });

        let high_indirect_commands = build_indexed_indirect_cpu(&high_chunk_meta, high_mesh_params);
        let low_indirect_commands = if let Some(params) = low_mesh_params {
            build_indexed_indirect_cpu(&low_chunk_meta, params)
        } else {
            Vec::new()
        };
        let high_indirect_buffer = create_buffer_with_data(
            &render_device,
            "grass indirect high draw buffer",
            BufferUsages::STORAGE | BufferUsages::INDIRECT | BufferUsages::COPY_DST,
            &high_indirect_commands,
        );
        let low_indirect_buffer = create_buffer_with_data(
            &render_device,
            "grass indirect low draw buffer",
            BufferUsages::STORAGE | BufferUsages::INDIRECT | BufferUsages::COPY_DST,
            &low_indirect_commands,
        );

        commands.entity(entity).insert(GrassIndirectBuffers {
            high_instance_buffer,
            low_instance_buffer,
            high_chunk_meta_buffer,
            low_chunk_meta_buffer,
            high_mesh_params_buffer,
            low_mesh_params_buffer,
            high_indirect_buffer,
            low_indirect_buffer,
            high_draw_count: high_chunk_meta.len() as u32,
            low_draw_count: low_chunk_meta.len() as u32,
            high_indexed: true,
            low_indexed: !low_chunk_meta.is_empty(),
            signature,
        });
    }
}

pub(crate) fn prepare_grass_buffers(
    mut commands: Commands,
    query: Query<(
        Entity,
        &GrassColor,
        &Blade,
        Option<&GrassBuffer>,
        Option<&PreparedGrassUniforms>,
    )>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    for (entity, color, blade, grass_buffer, prepared_uniforms) in &query {
        let color_array = color.to_array();
        if prepared_uniforms
            .is_some_and(|prepared| prepared.color == color_array && prepared.blade == *blade)
        {
            continue;
        }

        if let Some(grass_buffer) = grass_buffer {
            render_queue.write_buffer(
                &grass_buffer.color_buffer,
                0,
                bytemuck::cast_slice(&color_array),
            );
            render_queue.write_buffer(
                &grass_buffer.blade_buffer,
                0,
                bytemuck::cast_slice(&[*blade]),
            );
            commands.entity(entity).insert(PreparedGrassUniforms {
                color: color_array,
                blade: *blade,
            });
            continue;
        }

        let color_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("grass color buffer"),
            contents: bytemuck::cast_slice(&color_array),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        let blade_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("grass blade buffer"),
            contents: bytemuck::cast_slice(&[*blade]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        commands
            .entity(entity)
            .insert(GrassBuffer {
                color_buffer,
                blade_buffer,
            })
            .insert(PreparedGrassUniforms {
                color: color_array,
                blade: *blade,
            });
    }
}

pub(crate) fn prepare_grass_bind_group(
    mut commands: Commands,
    pipeline: Res<GrassPipeline>,
    pipeline_cache: Res<PipelineCache>,
    render_device: Res<RenderDevice>,
    query: Query<(Entity, &GrassBuffer, Option<&BufferBindGroup<Grass>>)>,
) {
    let layout = pipeline_cache.get_bind_group_layout(&pipeline.grass_layout);

    for (entity, grass, existing_bind_group) in query.iter() {
        if existing_bind_group.is_some() {
            continue;
        }

        let bind_group = render_device.create_bind_group(
            Some("grass bind group"),
            &layout,
            &BindGroupEntries::sequential((
                BufferBinding {
                    buffer: &grass.color_buffer,
                    offset: 0,
                    size: None,
                },
                BufferBinding {
                    buffer: &grass.blade_buffer,
                    offset: 0,
                    size: None,
                },
            )),
        );

        commands
            .entity(entity)
            .insert(BufferBindGroup::<Grass>::new(bind_group));
    }
}

#[derive(Component, Resource, Clone)]
pub struct WindBuffer {
    pub buffer: Buffer,
}

#[derive(Resource, Clone, Copy)]
pub struct PreparedGlobalWindData(pub Wind);

#[derive(Component, Clone)]
pub struct LocalWindBindGroupState {
    pub wind_map: Handle<Image>,
    pub using_fallback: bool,
}

#[derive(Component, Clone, Copy)]
pub struct PreparedLocalWindData(pub Wind);

pub(crate) fn prepare_global_wind_buffers(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    wind: Res<GrassWind>,
    wind_buffer: Option<Res<WindBuffer>>,
    prepared_global_wind: Option<Res<PreparedGlobalWindData>>,
) {
    if prepared_global_wind.is_some_and(|prepared| prepared.0 == wind.wind_data) {
        return;
    }

    if let Some(wind_buffer) = wind_buffer {
        render_queue.write_buffer(
            &wind_buffer.buffer,
            0,
            bytemuck::cast_slice(&[wind.wind_data]),
        );
        commands.insert_resource(PreparedGlobalWindData(wind.wind_data));
        return;
    }

    let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("global wind buffer"),
        contents: bytemuck::cast_slice(&[wind.wind_data]),
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    });

    commands.insert_resource(WindBuffer { buffer });
    commands.insert_resource(PreparedGlobalWindData(wind.wind_data));
}

pub(crate) fn prepare_global_wind_bind_group(
    mut commands: Commands,
    pipeline: Res<GrassPipeline>,
    pipeline_cache: Res<PipelineCache>,
    render_device: Res<RenderDevice>,
    wind: Res<GrassWind>,
    wind_buffer: Res<WindBuffer>,
    fallback_image: Res<FallbackImage>,
    images: Res<RenderAssets<GpuImage>>,
    existing_bind_group: Option<Res<BufferBindGroup<GrassWind>>>,
    mut last_wind_map: Local<Option<Handle<Image>>>,
    mut last_using_fallback: Local<bool>,
) {
    let layout = pipeline_cache.get_bind_group_layout(&pipeline.wind_layout);

    let maybe_wind_map_texture = images.get(&wind.wind_map);
    let using_fallback = maybe_wind_map_texture.is_none();
    if existing_bind_group.is_some()
        && last_wind_map
            .as_ref()
            .is_some_and(|last_wind_map| *last_wind_map == wind.wind_map)
        && *last_using_fallback == using_fallback
    {
        return;
    }

    let wind_map_texture = if let Some(texture) = maybe_wind_map_texture {
        &texture.texture_view
    } else {
        &fallback_image.d2.texture_view
    };

    let bind_group = render_device.create_bind_group(
        Some("global wind bind group"),
        &layout,
        &BindGroupEntries::sequential((
            BufferBinding {
                buffer: &wind_buffer.buffer,
                offset: 0,
                size: None,
            },
            BindingResource::TextureView(wind_map_texture),
        )),
    );

    commands.insert_resource(BufferBindGroup::<GrassWind>::new(bind_group));
    *last_wind_map = Some(wind.wind_map.clone());
    *last_using_fallback = using_fallback;
}

pub(crate) fn prepare_local_wind_buffers(
    mut commands: Commands,
    query: Query<(
        Entity,
        &GrassWind,
        Option<&WindBuffer>,
        Option<&PreparedLocalWindData>,
    )>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    for (entity, grass_wind, wind_buffer, prepared_wind) in &query {
        if prepared_wind.is_some_and(|prepared_wind| prepared_wind.0 == grass_wind.wind_data) {
            continue;
        }

        if let Some(wind_buffer) = wind_buffer {
            render_queue.write_buffer(
                &wind_buffer.buffer,
                0,
                bytemuck::cast_slice(&[grass_wind.wind_data]),
            );
            commands
                .entity(entity)
                .insert(PreparedLocalWindData(grass_wind.wind_data));
            continue;
        }

        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("local wind buffer"),
            contents: bytemuck::cast_slice(&[grass_wind.wind_data]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        commands
            .entity(entity)
            .insert(WindBuffer { buffer })
            .insert(PreparedLocalWindData(grass_wind.wind_data));
    }
}

pub(crate) fn prepare_local_wind_bind_group(
    mut commands: Commands,
    pipeline: Res<GrassPipeline>,
    pipeline_cache: Res<PipelineCache>,
    render_device: Res<RenderDevice>,
    query: Query<(
        Entity,
        &GrassWind,
        &WindBuffer,
        Option<&BufferBindGroup<GrassWind>>,
        Option<&LocalWindBindGroupState>,
    )>,
    fallback_image: Res<FallbackImage>,
    images: Res<RenderAssets<GpuImage>>,
) {
    let layout = pipeline_cache.get_bind_group_layout(&pipeline.wind_layout);

    for (entity, grass_wind, wind_buffer, existing_bind_group, existing_state) in query.iter() {
        let maybe_wind_texture = images.get(&grass_wind.wind_map);
        let using_fallback = maybe_wind_texture.is_none();
        if existing_bind_group.is_some()
            && existing_state.is_some_and(|state| {
                state.wind_map == grass_wind.wind_map && state.using_fallback == using_fallback
            })
        {
            continue;
        }

        let wind_map_texture = if let Some(texture) = maybe_wind_texture {
            &texture.texture_view
        } else {
            &fallback_image.d2.texture_view
        };

        let bind_group = render_device.create_bind_group(
            Some("local wind bind group"),
            &layout,
            &BindGroupEntries::sequential((
                BufferBinding {
                    buffer: &wind_buffer.buffer,
                    offset: 0,
                    size: None,
                },
                BindingResource::TextureView(wind_map_texture),
            )),
        );

        commands
            .entity(entity)
            .insert(BufferBindGroup::<GrassWind>::new(bind_group))
            .insert(LocalWindBindGroupState {
                wind_map: grass_wind.wind_map.clone(),
                using_fallback,
            });
    }
}
