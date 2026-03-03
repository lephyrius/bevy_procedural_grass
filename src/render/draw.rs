use bevy::{
    ecs::system::{SystemParamItem, lifetimeless::*},
    pbr::{
        RenderMeshInstances, SetMeshBindGroup, SetMeshViewBindGroup,
        SetMeshViewBindingArrayBindGroup, SetPrepassViewBindGroup, SetPrepassViewEmptyBindGroup,
    },
    render::{
        mesh::{RenderMesh, RenderMeshBufferInfo, allocator::MeshAllocator},
        render_asset::RenderAssets,
        render_phase::{
            PhaseItem, RenderCommand, RenderCommandResult, SetItemPipeline, TrackedRenderPass,
        },
    },
};

use crate::grass::{
    chunk::{GrassLOD, RenderGrassChunks},
    grass::{Grass, GrassLODMesh},
    wind::GrassWind,
};

use super::{
    GrassIndirectSettings,
    instance::GrassChunkBuffer,
    prepare::{BufferBindGroup, GrassIndirectBuffers},
};

pub type DrawGrass = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshViewBindingArrayBindGroup<1>,
    SetMeshBindGroup<2>,
    SetGrassBindGroup<3>,
    SetWindBindGroup<4>,
    DrawGrassInstanced,
);

pub type DrawGrassDeferred = (
    SetItemPipeline,
    SetPrepassViewBindGroup<0>,
    SetPrepassViewEmptyBindGroup<1>,
    SetMeshBindGroup<2>,
    SetGrassBindGroup<3>,
    SetWindBindGroup<4>,
    DrawGrassInstancedDeferred,
);

pub struct SetGrassBindGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetGrassBindGroup<I> {
    type Param = ();
    type ViewQuery = ();
    type ItemQuery = Read<BufferBindGroup<Grass>>;

    fn render<'w>(
        _item: &P,
        _view: (),
        bind_group: Option<&'w BufferBindGroup<Grass>>,
        _param: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(bind_group) = bind_group else {
            return RenderCommandResult::Skip;
        };
        pass.set_bind_group(I, &bind_group.bind_group, &[]);
        RenderCommandResult::Success
    }
}

pub struct SetWindBindGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetWindBindGroup<I> {
    type Param = SRes<BufferBindGroup<GrassWind>>;
    type ViewQuery = ();
    type ItemQuery = Option<Read<BufferBindGroup<GrassWind>>>;

    fn render<'w>(
        _item: &P,
        _view: (),
        local_wind: Option<Option<&'w BufferBindGroup<GrassWind>>>,
        global_wind: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let bind_group = if let Some(local_wind) = local_wind.flatten() {
            local_wind
        } else {
            global_wind.into_inner()
        };
        pass.set_bind_group(I, &bind_group.bind_group, &[]);
        RenderCommandResult::Success
    }
}

pub struct DrawGrassInstanced;
impl<P: PhaseItem> RenderCommand<P> for DrawGrassInstanced {
    type Param = (
        SRes<RenderAssets<RenderMesh>>,
        SRes<RenderMeshInstances>,
        SRes<MeshAllocator>,
        SRes<RenderAssets<GrassChunkBuffer>>,
        SRes<GrassIndirectSettings>,
    );
    type ViewQuery = ();
    type ItemQuery = (
        Option<Read<GrassLODMesh>>,
        Read<RenderGrassChunks>,
        Option<Read<GrassIndirectBuffers>>,
    );

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        item_data: Option<(
            Option<&'w GrassLODMesh>,
            &'w RenderGrassChunks,
            Option<&'w GrassIndirectBuffers>,
        )>,
        (meshes, render_mesh_instances, mesh_allocator, grass_data, indirect_settings): SystemParamItem<
            'w,
            '_,
            Self::Param,
        >,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some((lod, chunks, indirect_buffers)) = item_data else {
            return RenderCommandResult::Skip;
        };

        let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(item.main_entity())
        else {
            return RenderCommandResult::Skip;
        };

        let meshes = meshes.into_inner();
        let grass_data = grass_data.into_inner();
        let mesh_allocator = mesh_allocator.into_inner();
        let indirect_enabled = indirect_settings.into_inner().enabled;

        let high_mesh_id = mesh_instance.mesh_asset_id;
        let has_high = chunks
            .0
            .iter()
            .any(|chunk| matches!(chunk.0, GrassLOD::High));
        let has_low = chunks
            .0
            .iter()
            .any(|chunk| matches!(chunk.0, GrassLOD::Low));

        let high_resources = if has_high {
            let Some(gpu_mesh_high) = meshes.get(high_mesh_id) else {
                return RenderCommandResult::Skip;
            };
            let Some(high_vertex_slice) = mesh_allocator.mesh_vertex_slice(&high_mesh_id) else {
                return RenderCommandResult::Skip;
            };
            let high_index_slice = mesh_allocator.mesh_index_slice(&high_mesh_id);
            Some((gpu_mesh_high, high_vertex_slice, high_index_slice))
        } else {
            None
        };

        let low_mesh_id = lod.and_then(|lod| lod.mesh_handle.as_ref().map(|handle| handle.id()));
        let low_resources = if has_low {
            low_mesh_id.and_then(|id| {
                let gpu_mesh_low = meshes.get(id)?;
                let low_vertex_slice = mesh_allocator.mesh_vertex_slice(&id)?;
                let low_index_slice = mesh_allocator.mesh_index_slice(&id);
                Some((gpu_mesh_low, low_vertex_slice, low_index_slice))
            })
        } else {
            None
        };

        if indirect_enabled {
            if let Some(indirect) = indirect_buffers {
                let mut used_indirect = false;

                if let Some((gpu_mesh, vertex_slice, index_slice)) = high_resources.as_ref() {
                    if let (Some(instance_buffer), Some(indirect_buffer)) = (
                        indirect.high_instance_buffer.as_ref(),
                        indirect.high_indirect_buffer.as_ref(),
                    ) {
                        if indirect.high_draw_count > 0 {
                            pass.set_vertex_buffer(0, vertex_slice.buffer.slice(..));
                            pass.set_vertex_buffer(1, instance_buffer.slice(..));
                            match &gpu_mesh.buffer_info {
                                RenderMeshBufferInfo::Indexed { index_format, .. }
                                    if indirect.high_indexed =>
                                {
                                    let Some(index_slice) = index_slice else {
                                        return RenderCommandResult::Skip;
                                    };
                                    pass.set_index_buffer(
                                        index_slice.buffer.slice(..),
                                        *index_format,
                                    );
                                    pass.multi_draw_indexed_indirect(
                                        indirect_buffer,
                                        0,
                                        indirect.high_draw_count,
                                    );
                                    used_indirect = true;
                                }
                                RenderMeshBufferInfo::NonIndexed if !indirect.high_indexed => {
                                    pass.multi_draw_indirect(
                                        indirect_buffer,
                                        0,
                                        indirect.high_draw_count,
                                    );
                                    used_indirect = true;
                                }
                                _ => {}
                            }
                        }
                    }
                }

                if let Some((gpu_mesh, vertex_slice, index_slice)) = low_resources.as_ref() {
                    if let (Some(instance_buffer), Some(indirect_buffer)) = (
                        indirect.low_instance_buffer.as_ref(),
                        indirect.low_indirect_buffer.as_ref(),
                    ) {
                        if indirect.low_draw_count > 0 {
                            pass.set_vertex_buffer(0, vertex_slice.buffer.slice(..));
                            pass.set_vertex_buffer(1, instance_buffer.slice(..));
                            match &gpu_mesh.buffer_info {
                                RenderMeshBufferInfo::Indexed { index_format, .. }
                                    if indirect.low_indexed =>
                                {
                                    let Some(index_slice) = index_slice else {
                                        return RenderCommandResult::Skip;
                                    };
                                    pass.set_index_buffer(
                                        index_slice.buffer.slice(..),
                                        *index_format,
                                    );
                                    pass.multi_draw_indexed_indirect(
                                        indirect_buffer,
                                        0,
                                        indirect.low_draw_count,
                                    );
                                    used_indirect = true;
                                }
                                RenderMeshBufferInfo::NonIndexed if !indirect.low_indexed => {
                                    pass.multi_draw_indirect(
                                        indirect_buffer,
                                        0,
                                        indirect.low_draw_count,
                                    );
                                    used_indirect = true;
                                }
                                _ => {}
                            }
                        }
                    }
                }

                if used_indirect {
                    return RenderCommandResult::Success;
                }
            }
        }

        let mut bound_lod: Option<GrassLOD> = None;

        for chunk in &chunks.0 {
            let Some(gpu_grass) = grass_data.get(chunk.1.id()) else {
                continue;
            };

            let (lod_kind, gpu_mesh, vertex_slice, index_slice) = match chunk.0 {
                GrassLOD::High => {
                    let Some((gpu_mesh_high, high_vertex_slice, high_index_slice)) =
                        high_resources.as_ref()
                    else {
                        continue;
                    };
                    (
                        GrassLOD::High,
                        *gpu_mesh_high,
                        high_vertex_slice,
                        high_index_slice.as_ref(),
                    )
                }
                GrassLOD::Low => {
                    let Some((gpu_mesh_low, low_vertex_slice, low_index_slice)) =
                        low_resources.as_ref()
                    else {
                        continue;
                    };
                    (
                        GrassLOD::Low,
                        *gpu_mesh_low,
                        low_vertex_slice,
                        low_index_slice.as_ref(),
                    )
                }
            };

            if bound_lod != Some(lod_kind) {
                pass.set_vertex_buffer(0, vertex_slice.buffer.slice(..));
                if let RenderMeshBufferInfo::Indexed { index_format, .. } = &gpu_mesh.buffer_info {
                    let Some(index_slice) = index_slice else {
                        continue;
                    };
                    pass.set_index_buffer(index_slice.buffer.slice(..), *index_format);
                }
                bound_lod = Some(lod_kind);
            }

            pass.set_vertex_buffer(1, gpu_grass.buffer.slice(..));

            match &gpu_mesh.buffer_info {
                RenderMeshBufferInfo::Indexed { count, .. } => {
                    let Some(index_slice) = index_slice else {
                        continue;
                    };
                    pass.draw_indexed(
                        index_slice.range.start..(index_slice.range.start + count),
                        vertex_slice.range.start as i32,
                        0..gpu_grass.length as u32,
                    );
                }
                RenderMeshBufferInfo::NonIndexed => {
                    pass.draw(vertex_slice.range.clone(), 0..gpu_grass.length as u32);
                }
            }
        }

        RenderCommandResult::Success
    }
}

pub struct DrawGrassInstancedDeferred;
impl<P: PhaseItem> RenderCommand<P> for DrawGrassInstancedDeferred {
    type Param = (
        SRes<RenderAssets<RenderMesh>>,
        SRes<RenderMeshInstances>,
        SRes<MeshAllocator>,
        SRes<RenderAssets<GrassChunkBuffer>>,
        SRes<GrassIndirectSettings>,
    );
    type ViewQuery = ();
    type ItemQuery = (
        Option<Read<GrassLODMesh>>,
        Read<RenderGrassChunks>,
        Option<Read<GrassIndirectBuffers>>,
    );

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        item_data: Option<(
            Option<&'w GrassLODMesh>,
            &'w RenderGrassChunks,
            Option<&'w GrassIndirectBuffers>,
        )>,
        (meshes, render_mesh_instances, mesh_allocator, grass_data, indirect_settings): SystemParamItem<
            'w,
            '_,
            Self::Param,
        >,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some((lod, chunks, indirect_buffers)) = item_data else {
            return RenderCommandResult::Skip;
        };

        let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(item.main_entity())
        else {
            return RenderCommandResult::Skip;
        };

        let meshes = meshes.into_inner();
        let grass_data = grass_data.into_inner();
        let mesh_allocator = mesh_allocator.into_inner();
        let indirect_enabled = indirect_settings.into_inner().enabled;

        let high_mesh_id = mesh_instance.mesh_asset_id;
        let has_high = chunks
            .0
            .iter()
            .any(|chunk| matches!(chunk.0, GrassLOD::High));
        let has_low = chunks
            .0
            .iter()
            .any(|chunk| matches!(chunk.0, GrassLOD::Low));

        let high_resources = if has_high {
            let Some(gpu_mesh_high) = meshes.get(high_mesh_id) else {
                return RenderCommandResult::Skip;
            };
            let Some(high_vertex_slice) = mesh_allocator.mesh_vertex_slice(&high_mesh_id) else {
                return RenderCommandResult::Skip;
            };
            let high_index_slice = mesh_allocator.mesh_index_slice(&high_mesh_id);
            Some((gpu_mesh_high, high_vertex_slice, high_index_slice))
        } else {
            None
        };

        let low_mesh_id = lod.and_then(|lod| lod.mesh_handle.as_ref().map(|handle| handle.id()));
        let low_resources = if has_low {
            low_mesh_id.and_then(|id| {
                let gpu_mesh_low = meshes.get(id)?;
                let low_vertex_slice = mesh_allocator.mesh_vertex_slice(&id)?;
                let low_index_slice = mesh_allocator.mesh_index_slice(&id);
                Some((gpu_mesh_low, low_vertex_slice, low_index_slice))
            })
        } else {
            None
        };

        if indirect_enabled {
            if let Some(indirect) = indirect_buffers {
                let mut used_indirect = false;

                if let Some((gpu_mesh, vertex_slice, index_slice)) = high_resources.as_ref() {
                    if let (Some(instance_buffer), Some(indirect_buffer)) = (
                        indirect.high_instance_buffer.as_ref(),
                        indirect.high_indirect_buffer.as_ref(),
                    ) {
                        if indirect.high_draw_count > 0 {
                            pass.set_vertex_buffer(0, vertex_slice.buffer.slice(..));
                            pass.set_vertex_buffer(1, instance_buffer.slice(..));
                            match &gpu_mesh.buffer_info {
                                RenderMeshBufferInfo::Indexed { index_format, .. }
                                    if indirect.high_indexed =>
                                {
                                    let Some(index_slice) = index_slice else {
                                        return RenderCommandResult::Skip;
                                    };
                                    pass.set_index_buffer(
                                        index_slice.buffer.slice(..),
                                        *index_format,
                                    );
                                    pass.multi_draw_indexed_indirect(
                                        indirect_buffer,
                                        0,
                                        indirect.high_draw_count,
                                    );
                                    used_indirect = true;
                                }
                                RenderMeshBufferInfo::NonIndexed if !indirect.high_indexed => {
                                    pass.multi_draw_indirect(
                                        indirect_buffer,
                                        0,
                                        indirect.high_draw_count,
                                    );
                                    used_indirect = true;
                                }
                                _ => {}
                            }
                        }
                    }
                }

                if let Some((gpu_mesh, vertex_slice, index_slice)) = low_resources.as_ref() {
                    if let (Some(instance_buffer), Some(indirect_buffer)) = (
                        indirect.low_instance_buffer.as_ref(),
                        indirect.low_indirect_buffer.as_ref(),
                    ) {
                        if indirect.low_draw_count > 0 {
                            pass.set_vertex_buffer(0, vertex_slice.buffer.slice(..));
                            pass.set_vertex_buffer(1, instance_buffer.slice(..));
                            match &gpu_mesh.buffer_info {
                                RenderMeshBufferInfo::Indexed { index_format, .. }
                                    if indirect.low_indexed =>
                                {
                                    let Some(index_slice) = index_slice else {
                                        return RenderCommandResult::Skip;
                                    };
                                    pass.set_index_buffer(
                                        index_slice.buffer.slice(..),
                                        *index_format,
                                    );
                                    pass.multi_draw_indexed_indirect(
                                        indirect_buffer,
                                        0,
                                        indirect.low_draw_count,
                                    );
                                    used_indirect = true;
                                }
                                RenderMeshBufferInfo::NonIndexed if !indirect.low_indexed => {
                                    pass.multi_draw_indirect(
                                        indirect_buffer,
                                        0,
                                        indirect.low_draw_count,
                                    );
                                    used_indirect = true;
                                }
                                _ => {}
                            }
                        }
                    }
                }

                if used_indirect {
                    return RenderCommandResult::Success;
                }
            }
        }

        let mut bound_lod: Option<GrassLOD> = None;

        for chunk in &chunks.0 {
            let Some(gpu_grass) = grass_data.get(chunk.1.id()) else {
                continue;
            };

            let (lod_kind, gpu_mesh, vertex_slice, index_slice) = match chunk.0 {
                GrassLOD::High => {
                    let Some((gpu_mesh_high, high_vertex_slice, high_index_slice)) =
                        high_resources.as_ref()
                    else {
                        continue;
                    };
                    (
                        GrassLOD::High,
                        *gpu_mesh_high,
                        high_vertex_slice,
                        high_index_slice.as_ref(),
                    )
                }
                GrassLOD::Low => {
                    let Some((gpu_mesh_low, low_vertex_slice, low_index_slice)) =
                        low_resources.as_ref()
                    else {
                        continue;
                    };
                    (
                        GrassLOD::Low,
                        *gpu_mesh_low,
                        low_vertex_slice,
                        low_index_slice.as_ref(),
                    )
                }
            };

            if bound_lod != Some(lod_kind) {
                pass.set_vertex_buffer(0, vertex_slice.buffer.slice(..));
                if let RenderMeshBufferInfo::Indexed { index_format, .. } = &gpu_mesh.buffer_info {
                    let Some(index_slice) = index_slice else {
                        continue;
                    };
                    pass.set_index_buffer(index_slice.buffer.slice(..), *index_format);
                }
                bound_lod = Some(lod_kind);
            }

            pass.set_vertex_buffer(1, gpu_grass.buffer.slice(..));

            match &gpu_mesh.buffer_info {
                RenderMeshBufferInfo::Indexed { count, .. } => {
                    let Some(index_slice) = index_slice else {
                        continue;
                    };
                    pass.draw_indexed(
                        index_slice.range.start..(index_slice.range.start + count),
                        vertex_slice.range.start as i32,
                        0..gpu_grass.length as u32,
                    );
                }
                RenderMeshBufferInfo::NonIndexed => {
                    pass.draw(vertex_slice.range.clone(), 0..gpu_grass.length as u32);
                }
            }
        }

        RenderCommandResult::Success
    }
}
