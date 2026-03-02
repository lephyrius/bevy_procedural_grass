use std::marker::PhantomData;

use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssets,
        render_resource::{
            BindGroup, BindGroupEntries, BindingResource, Buffer, BufferBinding,
            BufferInitDescriptor, BufferUsages, PipelineCache,
        },
        renderer::{RenderDevice, RenderQueue},
        texture::{FallbackImage, GpuImage},
    },
};

use crate::grass::{
    grass::{Grass, GrassColor},
    wind::GrassWind,
};

use super::pipeline::GrassPipeline;

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

pub(crate) fn prepare_grass_buffers(
    mut commands: Commands,
    query: Query<(
        Entity,
        &GrassColor,
        &crate::grass::grass::Blade,
        Option<&GrassBuffer>,
    )>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    for (entity, color, blade, grass_buffer) in &query {
        if let Some(grass_buffer) = grass_buffer {
            render_queue.write_buffer(
                &grass_buffer.color_buffer,
                0,
                bytemuck::cast_slice(&color.to_array()),
            );
            render_queue.write_buffer(
                &grass_buffer.blade_buffer,
                0,
                bytemuck::cast_slice(&[*blade]),
            );
            continue;
        }

        let color_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("grass color buffer"),
            contents: bytemuck::cast_slice(&color.to_array()),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        let blade_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("grass blade buffer"),
            contents: bytemuck::cast_slice(&[*blade]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        commands.entity(entity).insert(GrassBuffer {
            color_buffer,
            blade_buffer,
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

pub(crate) fn prepare_global_wind_buffers(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    wind: Res<GrassWind>,
    wind_buffer: Option<Res<WindBuffer>>,
) {
    if let Some(wind_buffer) = wind_buffer {
        render_queue.write_buffer(
            &wind_buffer.buffer,
            0,
            bytemuck::cast_slice(&[wind.wind_data]),
        );
        return;
    }

    let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("global wind buffer"),
        contents: bytemuck::cast_slice(&[wind.wind_data]),
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    });

    commands.insert_resource(WindBuffer { buffer });
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
    query: Query<(Entity, &GrassWind, Option<&WindBuffer>)>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    for (entity, grass_wind, wind_buffer) in &query {
        if let Some(wind_buffer) = wind_buffer {
            render_queue.write_buffer(
                &wind_buffer.buffer,
                0,
                bytemuck::cast_slice(&[grass_wind.wind_data]),
            );
            continue;
        }

        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("local wind buffer"),
            contents: bytemuck::cast_slice(&[grass_wind.wind_data]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        commands.entity(entity).insert(WindBuffer { buffer });
    }
}

pub(crate) fn prepare_local_wind_bind_group(
    mut commands: Commands,
    pipeline: Res<GrassPipeline>,
    pipeline_cache: Res<PipelineCache>,
    render_device: Res<RenderDevice>,
    query: Query<(Entity, &GrassWind, &WindBuffer)>,
    fallback_image: Res<FallbackImage>,
    images: Res<RenderAssets<GpuImage>>,
) {
    let layout = pipeline_cache.get_bind_group_layout(&pipeline.wind_layout);

    for (entity, grass_wind, wind_buffer) in query.iter() {
        let wind_map_texture = if let Some(texture) = images.get(&grass_wind.wind_map) {
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
            .insert(BufferBindGroup::<GrassWind>::new(bind_group));
    }
}
