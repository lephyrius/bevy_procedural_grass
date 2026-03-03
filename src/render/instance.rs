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
    pub normal_packed: [i16; 4],
    pub chunk_uvw_packed: [u16; 4],
}

impl GrassData {
    #[inline]
    pub fn new(position: Vec3, normal: Vec3, chunk_uvw: Vec3) -> Self {
        let normal = normal.normalize_or_zero();
        Self {
            position,
            normal_packed: [
                pack_snorm16(normal.x),
                pack_snorm16(normal.y),
                pack_snorm16(normal.z),
                0,
            ],
            chunk_uvw_packed: [
                pack_unorm16(chunk_uvw.x),
                pack_unorm16(chunk_uvw.y),
                pack_unorm16(chunk_uvw.z),
                0,
            ],
        }
    }
}

#[inline]
fn pack_snorm16(v: f32) -> i16 {
    (v.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

#[inline]
fn pack_unorm16(v: f32) -> u16 {
    (v.clamp(0.0, 1.0) * 65535.0).round() as u16
}

#[derive(Default, Component, Deref, Clone, Asset, TypePath)]
pub struct GrassChunkData(pub Vec<GrassData>);

pub struct GrassChunkBuffer {
    pub buffer: Buffer,
    pub length: usize,
    pub cpu_data: Vec<GrassData>,
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
            cpu_data: source_asset.0,
        })
    }
}
