use std::borrow::Cow;

use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssets,
        render_graph::{self, RenderLabel},
        render_resource::{
            BindGroup, BindGroupEntries, BufferBinding, ComputePassDescriptor,
            ComputePipelineDescriptor, PipelineCache, StorageTextureAccess, TextureFormat,
            binding_types::{storage_buffer, texture_storage_2d, uniform_buffer},
        },
        renderer::{RenderContext, RenderDevice},
        texture::GpuImage,
    },
};

use crate::{
    GRASS_INDIRECT_COMPUTE_SHADER_HANDLE, GRASS_WIND_COMPUTE_SHADER_HANDLE,
    grass::wind::{GrassWind, Wind},
};

use super::prepare::{GrassIndirectBuffers, WindBuffer};

#[derive(Resource)]
pub struct GrassWindComputePipeline {
    pub bind_group_layout: bevy::render::render_resource::BindGroupLayoutDescriptor,
    pub pipeline: bevy::render::render_resource::CachedComputePipelineId,
}

#[derive(Resource)]
pub struct GrassWindComputeBindGroup {
    pub bind_group: BindGroup,
    pub texture_size: UVec2,
}

#[derive(Resource, Default)]
pub struct GrassWindComputeDispatch(pub bool);

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct GrassWindComputeLabel;

pub fn init_wind_compute_pipeline(mut commands: Commands, pipeline_cache: Res<PipelineCache>) {
    let bind_group_layout = bevy::render::render_resource::BindGroupLayoutDescriptor::new(
        "grass_wind_compute_layout",
        &bevy::render::render_resource::BindGroupLayoutEntries::sequential(
            bevy::render::render_resource::ShaderStages::COMPUTE,
            (
                uniform_buffer::<Wind>(false),
                texture_storage_2d(TextureFormat::Rgba32Float, StorageTextureAccess::WriteOnly),
            ),
        ),
    );

    let pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("grass_wind_compute_pipeline".into()),
        layout: vec![bind_group_layout.clone()],
        shader: GRASS_WIND_COMPUTE_SHADER_HANDLE,
        entry_point: Some(Cow::Borrowed("update_wind")),
        ..default()
    });

    commands.insert_resource(GrassWindComputePipeline {
        bind_group_layout,
        pipeline,
    });
    commands.insert_resource(GrassWindComputeDispatch(false));
}

pub fn prepare_wind_compute_bind_group(
    mut commands: Commands,
    pipeline: Res<GrassWindComputePipeline>,
    pipeline_cache: Res<PipelineCache>,
    render_device: Res<RenderDevice>,
    wind: Res<GrassWind>,
    wind_buffer: Option<Res<WindBuffer>>,
    images: Res<RenderAssets<GpuImage>>,
    existing_bind_group: Option<Res<GrassWindComputeBindGroup>>,
    mut last_wind_map: Local<Option<Handle<Image>>>,
    mut last_texture_size: Local<Option<UVec2>>,
    mut last_wind_time: Local<f32>,
) {
    let mut should_dispatch = false;
    let wind_time = wind.wind_data._padding[0];
    if *last_wind_time != wind_time {
        *last_wind_time = wind_time;
        should_dispatch = true;
    }

    let Some(wind_buffer) = wind_buffer else {
        commands.insert_resource(GrassWindComputeDispatch(false));
        return;
    };
    let Some(wind_texture) = images.get(&wind.wind_map) else {
        commands.insert_resource(GrassWindComputeDispatch(false));
        return;
    };
    let texture_size = wind_texture.size_2d();

    let needs_rebuild = existing_bind_group.is_none()
        || !last_wind_map
            .as_ref()
            .is_some_and(|last_wind_map| *last_wind_map == wind.wind_map)
        || *last_texture_size != Some(texture_size);

    if needs_rebuild {
        should_dispatch = true;

        let layout = pipeline_cache.get_bind_group_layout(&pipeline.bind_group_layout);
        let bind_group = render_device.create_bind_group(
            Some("grass_wind_compute_bind_group"),
            &layout,
            &BindGroupEntries::sequential((
                BufferBinding {
                    buffer: &wind_buffer.buffer,
                    offset: 0,
                    size: None,
                },
                &wind_texture.texture_view,
            )),
        );

        commands.insert_resource(GrassWindComputeBindGroup {
            bind_group,
            texture_size,
        });
        *last_wind_map = Some(wind.wind_map.clone());
        *last_texture_size = Some(texture_size);
    }

    commands.insert_resource(GrassWindComputeDispatch(should_dispatch));
}

#[derive(Default)]
pub struct GrassWindComputeNode;

impl render_graph::Node for GrassWindComputeNode {
    fn run(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        let Some(dispatch) = world.get_resource::<GrassWindComputeDispatch>() else {
            return Ok(());
        };
        if !dispatch.0 {
            return Ok(());
        }

        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<GrassWindComputePipeline>();

        let Some(bind_group) = world.get_resource::<GrassWindComputeBindGroup>() else {
            return Ok(());
        };

        let Some(compute_pipeline) = pipeline_cache.get_compute_pipeline(pipeline.pipeline) else {
            return Ok(());
        };

        let mut pass =
            render_context
                .command_encoder()
                .begin_compute_pass(&ComputePassDescriptor {
                    label: Some("grass_wind_compute_pass"),
                    ..default()
                });
        pass.set_pipeline(compute_pipeline);
        pass.set_bind_group(0, &bind_group.bind_group, &[]);

        let workgroup_size = 8;
        let dispatch_x = bind_group.texture_size.x.div_ceil(workgroup_size);
        let dispatch_y = bind_group.texture_size.y.div_ceil(workgroup_size);
        pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);

        Ok(())
    }
}

#[derive(Resource)]
pub struct GrassIndirectComputePipeline {
    pub bind_group_layout: bevy::render::render_resource::BindGroupLayoutDescriptor,
    pub pipeline: bevy::render::render_resource::CachedComputePipelineId,
}

pub struct GrassIndirectComputeDispatchItem {
    pub high: Option<(BindGroup, u32)>,
    pub low: Option<(BindGroup, u32)>,
}

#[derive(Resource, Default)]
pub struct GrassIndirectComputeDispatches(pub Vec<GrassIndirectComputeDispatchItem>);

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct GrassIndirectComputeLabel;

pub fn init_indirect_compute_pipeline(mut commands: Commands, pipeline_cache: Res<PipelineCache>) {
    let bind_group_layout = bevy::render::render_resource::BindGroupLayoutDescriptor::new(
        "grass_indirect_compute_layout",
        &bevy::render::render_resource::BindGroupLayoutEntries::sequential(
            bevy::render::render_resource::ShaderStages::COMPUTE,
            (
                storage_buffer::<super::prepare::ChunkIndirectMeta>(true),
                uniform_buffer::<super::prepare::IndexedIndirectMeshParams>(false),
                storage_buffer::<super::prepare::DrawIndexedIndirectCommand>(false),
            ),
        ),
    );

    let pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("grass_indirect_compute_pipeline".into()),
        layout: vec![bind_group_layout.clone()],
        shader: GRASS_INDIRECT_COMPUTE_SHADER_HANDLE,
        entry_point: Some(Cow::Borrowed("build_indexed_indirect")),
        ..default()
    });

    commands.insert_resource(GrassIndirectComputePipeline {
        bind_group_layout,
        pipeline,
    });
    commands.insert_resource(GrassIndirectComputeDispatches::default());
}

pub fn prepare_indirect_compute_bind_group(
    mut dispatches: ResMut<GrassIndirectComputeDispatches>,
    pipeline: Res<GrassIndirectComputePipeline>,
    pipeline_cache: Res<PipelineCache>,
    render_device: Res<RenderDevice>,
    query: Query<&GrassIndirectBuffers>,
) {
    let layout = pipeline_cache.get_bind_group_layout(&pipeline.bind_group_layout);
    dispatches.0.clear();

    for indirect in &query {
        let high = match (
            indirect.high_chunk_meta_buffer.as_ref(),
            indirect.high_mesh_params_buffer.as_ref(),
            indirect.high_indirect_buffer.as_ref(),
        ) {
            (Some(chunk_meta), Some(mesh_params), Some(indirect_buffer))
                if indirect.high_draw_count > 0 =>
            {
                Some(render_device.create_bind_group(
                    Some("grass_indirect_compute_bind_group_high"),
                    &layout,
                    &BindGroupEntries::sequential((
                        BufferBinding {
                            buffer: chunk_meta,
                            offset: 0,
                            size: None,
                        },
                        BufferBinding {
                            buffer: mesh_params,
                            offset: 0,
                            size: None,
                        },
                        BufferBinding {
                            buffer: indirect_buffer,
                            offset: 0,
                            size: None,
                        },
                    )),
                ))
            }
            _ => None,
        };

        let low = match (
            indirect.low_chunk_meta_buffer.as_ref(),
            indirect.low_mesh_params_buffer.as_ref(),
            indirect.low_indirect_buffer.as_ref(),
        ) {
            (Some(chunk_meta), Some(mesh_params), Some(indirect_buffer))
                if indirect.low_draw_count > 0 =>
            {
                Some(render_device.create_bind_group(
                    Some("grass_indirect_compute_bind_group_low"),
                    &layout,
                    &BindGroupEntries::sequential((
                        BufferBinding {
                            buffer: chunk_meta,
                            offset: 0,
                            size: None,
                        },
                        BufferBinding {
                            buffer: mesh_params,
                            offset: 0,
                            size: None,
                        },
                        BufferBinding {
                            buffer: indirect_buffer,
                            offset: 0,
                            size: None,
                        },
                    )),
                ))
            }
            _ => None,
        };

        dispatches.0.push(GrassIndirectComputeDispatchItem {
            high: high.map(|bind_group| (bind_group, indirect.high_draw_count)),
            low: low.map(|bind_group| (bind_group, indirect.low_draw_count)),
        });
    }
}

#[derive(Default)]
pub struct GrassIndirectComputeNode;

impl render_graph::Node for GrassIndirectComputeNode {
    fn run(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<GrassIndirectComputePipeline>();
        let dispatches = world.resource::<GrassIndirectComputeDispatches>();
        if dispatches.0.is_empty() {
            return Ok(());
        }
        let Some(compute_pipeline) = pipeline_cache.get_compute_pipeline(pipeline.pipeline) else {
            return Ok(());
        };

        let mut pass =
            render_context
                .command_encoder()
                .begin_compute_pass(&ComputePassDescriptor {
                    label: Some("grass_indirect_compute_pass"),
                    ..default()
                });
        pass.set_pipeline(compute_pipeline);

        for dispatch in &dispatches.0 {
            if let Some((high_bind_group, high_draw_count)) = &dispatch.high {
                pass.set_bind_group(0, high_bind_group, &[]);
                let dispatch_x = high_draw_count.div_ceil(64);
                if dispatch_x > 0 {
                    pass.dispatch_workgroups(dispatch_x, 1, 1);
                }
            }
            if let Some((low_bind_group, low_draw_count)) = &dispatch.low {
                pass.set_bind_group(0, low_bind_group, &[]);
                let dispatch_x = low_draw_count.div_ceil(64);
                if dispatch_x > 0 {
                    pass.dispatch_workgroups(dispatch_x, 1, 1);
                }
            }
        }

        Ok(())
    }
}
