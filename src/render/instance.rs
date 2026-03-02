use bevy::{
    asset::AssetId,
    ecs::system::{SystemParamItem, lifetimeless::SRes},
    prelude::*,
    render::{
        render_asset::{PrepareAssetError, RenderAsset},
        render_resource::{Buffer, BufferInitDescriptor, BufferUsages},
        renderer::RenderDevice,
    },
};
use bytemuck::{Pod, Zeroable};

#[derive(Clone, Copy, Pod, Zeroable, Reflect, Debug)]
#[repr(C)]
pub struct GrassData {
    pub position: Vec3,
    pub normal: Vec3,
    pub chunk_uvw: Vec3,
}

#[derive(Component, Deref, Clone, Asset, TypePath)]
pub struct GrassChunkData(pub Vec<GrassData>);

impl Default for GrassChunkData {
    fn default() -> Self {
        Self(Vec::new())
    }
}

pub struct GrassChunkBuffer {
    pub buffer: Buffer,
    pub length: usize,
}

impl RenderAsset for GrassChunkBuffer {
    type SourceAsset = GrassChunkData;
    type Param = SRes<RenderDevice>;

    fn prepare_asset(
        source_asset: Self::SourceAsset,
        _asset_id: AssetId<Self::SourceAsset>,
        render_device: &mut SystemParamItem<Self::Param>,
        _previous_asset: Option<&Self>,
    ) -> Result<Self, PrepareAssetError<Self::SourceAsset>> {
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("grass chunk instance buffer"),
            contents: bytemuck::cast_slice(source_asset.as_slice()),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST | BufferUsages::STORAGE,
        });

        Ok(Self {
            buffer,
            length: source_asset.len(),
        })
    }
}
