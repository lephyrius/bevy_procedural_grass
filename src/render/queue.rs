use bevy::{
    core_pipeline::core_3d::{Opaque3d, Opaque3dBatchSetKey, Opaque3dBinKey},
    ecs::change_detection::Tick,
    pbr::{MeshPipelineKey, RenderMeshInstances},
    prelude::*,
    render::{
        mesh::{RenderMesh, allocator::MeshAllocator},
        render_asset::RenderAssets,
        render_phase::{BinnedRenderPhaseType, DrawFunctions, ViewBinnedRenderPhases},
        render_resource::{PipelineCache, SpecializedMeshPipelines},
        sync_world::MainEntity,
        view::ExtractedView,
    },
};

use crate::grass::chunk::RenderGrassChunks;

use super::{draw::DrawGrass, pipeline::GrassPipeline};

pub(crate) fn grass_queue(
    opaque_3d_draw_functions: Res<DrawFunctions<Opaque3d>>,
    custom_pipeline: Res<GrassPipeline>,
    mut pipelines: ResMut<SpecializedMeshPipelines<GrassPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<RenderMesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    grass_meshes: Query<(Entity, &MainEntity, &RenderGrassChunks)>,
    mut opaque_render_phases: ResMut<ViewBinnedRenderPhases<Opaque3d>>,
    views: Query<(&ExtractedView, &Msaa)>,
    mesh_allocator: Res<MeshAllocator>,
    mut change_tick: Local<Tick>,
) {
    let draw_custom = opaque_3d_draw_functions.read().id::<DrawGrass>();

    for (view, msaa) in &views {
        let Some(opaque_phase) = opaque_render_phases.get_mut(&view.retained_view_entity) else {
            continue;
        };

        let view_key = MeshPipelineKey::from_msaa_samples(msaa.samples())
            | MeshPipelineKey::from_hdr(view.hdr);

        for (entity, main_entity, chunks) in &grass_meshes {
            if chunks.0.is_empty() {
                continue;
            }
            let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(*main_entity)
            else {
                continue;
            };
            let Some(mesh) = meshes.get(mesh_instance.mesh_asset_id) else {
                continue;
            };
            let (vertex_slab, index_slab) = mesh_allocator.mesh_slabs(&mesh_instance.mesh_asset_id);

            let key =
                view_key | MeshPipelineKey::from_primitive_topology(mesh.primitive_topology());
            let pipeline = pipelines
                .specialize(&pipeline_cache, &custom_pipeline, key, &mesh.layout)
                .unwrap();

            let next_change_tick = change_tick.get() + 1;
            change_tick.set(next_change_tick);

            opaque_phase.add(
                Opaque3dBatchSetKey {
                    draw_function: draw_custom,
                    pipeline,
                    material_bind_group_index: None,
                    vertex_slab: vertex_slab.unwrap_or_default(),
                    index_slab,
                    lightmap_slab: None,
                },
                Opaque3dBinKey {
                    asset_id: mesh_instance.mesh_asset_id.into(),
                },
                (entity, *main_entity),
                mesh_instance.current_uniform_index,
                BinnedRenderPhaseType::UnbatchableMesh,
                *change_tick,
            );
        }
    }
}
