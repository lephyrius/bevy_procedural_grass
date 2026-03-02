use std::borrow::Cow;

use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssets,
        render_graph::{self, RenderLabel},
        render_resource::{
            BindGroup, BindGroupEntries, BufferBinding, ComputePassDescriptor,
            ComputePipelineDescriptor, PipelineCache, StorageTextureAccess, TextureFormat,
            binding_types::{texture_storage_2d, uniform_buffer},
        },
        renderer::{RenderContext, RenderDevice},
        texture::GpuImage,
    },
};

use crate::{
    GRASS_WIND_COMPUTE_SHADER_HANDLE,
    grass::wind::{GrassWind, Wind},
};

use super::prepare::WindBuffer;

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
}

pub fn prepare_wind_compute_bind_group(
    mut commands: Commands,
    pipeline: Res<GrassWindComputePipeline>,
    pipeline_cache: Res<PipelineCache>,
    render_device: Res<RenderDevice>,
    wind: Res<GrassWind>,
    wind_buffer: Option<Res<WindBuffer>>,
    images: Res<RenderAssets<GpuImage>>,
) {
    let Some(wind_buffer) = wind_buffer else {
        return;
    };
    let Some(wind_texture) = images.get(&wind.wind_map) else {
        return;
    };

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
        texture_size: wind_texture.size_2d(),
    });
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
