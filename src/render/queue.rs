use bevy::{
    core_pipeline::{
        core_3d::{Opaque3d, Opaque3dBatchSetKey, Opaque3dBinKey},
        deferred::Opaque3dDeferred,
        prepass::{
            DeferredPrepass, DepthPrepass, MotionVectorPrepass, NormalPrepass,
            OpaqueNoLightmap3dBatchSetKey, OpaqueNoLightmap3dBinKey,
        },
    },
    ecs::change_detection::Tick,
    pbr::{MeshPipelineKey, RenderMeshInstances, ViewKeyCache},
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

use super::{
    draw::{DrawGrass, DrawGrassDeferred},
    pipeline::{GrassDeferredPipeline, GrassPipeline},
};

pub(crate) fn grass_queue(
    opaque_3d_draw_functions: Res<DrawFunctions<Opaque3d>>,
    custom_pipeline: Res<GrassPipeline>,
    mut pipelines: ResMut<SpecializedMeshPipelines<GrassPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<RenderMesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    grass_meshes: Query<(Entity, &MainEntity, &RenderGrassChunks)>,
    mut opaque_render_phases: ResMut<ViewBinnedRenderPhases<Opaque3d>>,
    view_key_cache: Option<Res<ViewKeyCache>>,
    views: Query<(&ExtractedView, &Msaa, Has<DeferredPrepass>)>,
    mesh_allocator: Res<MeshAllocator>,
    mut change_tick: Local<Tick>,
    mut reported_specialize_error: Local<bool>,
) {
    let draw_custom = opaque_3d_draw_functions.read().id::<DrawGrass>();

    for (view, msaa, deferred_prepass) in &views {
        if deferred_prepass {
            continue;
        }

        let Some(opaque_phase) = opaque_render_phases.get_mut(&view.retained_view_entity) else {
            continue;
        };

        let fallback_view_key = MeshPipelineKey::from_msaa_samples(msaa.samples())
            | MeshPipelineKey::from_hdr(view.hdr);
        let view_key = view_key_cache
            .as_ref()
            .and_then(|cache| cache.get(&view.retained_view_entity).copied())
            .unwrap_or(fallback_view_key);

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

            let key = view_key | MeshPipelineKey::from_bits_retain(mesh.key_bits.bits());
            let pipeline =
                match pipelines.specialize(&pipeline_cache, &custom_pipeline, key, &mesh.layout) {
                    Ok(pipeline) => pipeline,
                    Err(err) => {
                        if !*reported_specialize_error {
                            *reported_specialize_error = true;
                            eprintln!("grass forward pipeline specialization failed: {err}");
                        }
                        continue;
                    }
                };

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

pub(crate) fn grass_queue_deferred(
    opaque_3d_draw_functions: Res<DrawFunctions<Opaque3dDeferred>>,
    custom_pipeline: Option<Res<GrassDeferredPipeline>>,
    mut pipelines: ResMut<SpecializedMeshPipelines<GrassDeferredPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<RenderMesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    grass_meshes: Query<(Entity, &MainEntity, &RenderGrassChunks)>,
    mut opaque_render_phases: ResMut<ViewBinnedRenderPhases<Opaque3dDeferred>>,
    views: Query<(
        &ExtractedView,
        &Msaa,
        Has<DepthPrepass>,
        Has<DeferredPrepass>,
        Has<NormalPrepass>,
        Has<MotionVectorPrepass>,
    )>,
    mesh_allocator: Res<MeshAllocator>,
    mut change_tick: Local<Tick>,
    mut reported_specialize_error: Local<bool>,
) {
    let Some(custom_pipeline) = custom_pipeline else {
        return;
    };

    let draw_custom = opaque_3d_draw_functions.read().id::<DrawGrassDeferred>();

    for (view, msaa, depth_prepass, deferred_prepass, normal_prepass, motion_vector_prepass) in
        &views
    {
        if !deferred_prepass {
            continue;
        }

        let Some(opaque_phase) = opaque_render_phases.get_mut(&view.retained_view_entity) else {
            continue;
        };

        // Match prepass/deferred keying instead of full forward keying so bind groups
        // and shader defs stay compatible with the deferred pass layouts.
        let mut view_key =
            MeshPipelineKey::from_msaa_samples(msaa.samples()) | MeshPipelineKey::DEFERRED_PREPASS;
        if depth_prepass {
            view_key |= MeshPipelineKey::DEPTH_PREPASS;
        }
        if normal_prepass {
            view_key |= MeshPipelineKey::NORMAL_PREPASS;
        }
        if motion_vector_prepass {
            view_key |= MeshPipelineKey::MOTION_VECTOR_PREPASS;
        }

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

            let key = view_key | MeshPipelineKey::from_bits_retain(mesh.key_bits.bits());
            let pipeline =
                match pipelines.specialize(&pipeline_cache, &custom_pipeline, key, &mesh.layout) {
                    Ok(pipeline) => pipeline,
                    Err(err) => {
                        if !*reported_specialize_error {
                            *reported_specialize_error = true;
                            eprintln!("grass deferred pipeline specialization failed: {err}");
                        }
                        continue;
                    }
                };

            let next_change_tick = change_tick.get() + 1;
            change_tick.set(next_change_tick);

            opaque_phase.add(
                OpaqueNoLightmap3dBatchSetKey {
                    draw_function: draw_custom,
                    pipeline,
                    material_bind_group_index: None,
                    vertex_slab: vertex_slab.unwrap_or_default(),
                    index_slab,
                },
                OpaqueNoLightmap3dBinKey {
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
