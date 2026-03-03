use bevy::{
    asset::{load_internal_asset, uuid_handle},
    core_pipeline::core_3d::Opaque3d,
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems, extract_component::ExtractComponentPlugin,
        extract_resource::ExtractResourcePlugin, graph::CameraDriverLabel,
        render_asset::RenderAssetPlugin, render_graph::RenderGraph, render_phase::AddRenderCommand,
        render_resource::SpecializedMeshPipelines, view::NoIndirectDrawing,
    },
};

use grass::{
    chunk::GrassChunks,
    config::GrassConfig,
    grass::{Grass, GrassLODMesh},
    wind::GrassWind,
};
use render::{
    compute::{GrassWindComputeLabel, GrassWindComputeNode},
    draw::DrawGrass,
    instance::{GrassChunkBuffer, GrassChunkData},
    pipeline::GrassPipeline,
};

pub mod grass;
mod render;
mod util;

pub mod prelude {
    pub use crate::ProceduralGrassPlugin;
    pub use crate::grass::{
        config::GrassConfig,
        grass::{Grass, GrassBundle, GrassLODMesh},
        mesh::GrassMesh,
        wind::{GrassWind, Wind},
    };
}

pub(crate) const GRASS_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("6e91d4b1-491c-4a26-b15f-5fd4f57764df");
pub(crate) const GRASS_WIND_COMPUTE_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("9ced8fce-d838-457a-a5d9-3ce7d5b6d557");

#[derive(Default, Clone)]
pub struct ProceduralGrassPlugin {
    pub config: GrassConfig,
    pub wind: GrassWind,
}

impl Plugin for ProceduralGrassPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            GRASS_SHADER_HANDLE,
            "assets/shaders/grass.wgsl",
            Shader::from_wgsl
        );
        load_internal_asset!(
            app,
            GRASS_WIND_COMPUTE_SHADER_HANDLE,
            "assets/shaders/wind_compute.wgsl",
            Shader::from_wgsl
        );

        #[cfg(feature = "bevy-inspector-egui")]
        {
            app.register_type::<Grass>()
                .register_type::<GrassWind>()
                .register_type::<GrassConfig>();
        }

        app.insert_resource(self.wind.clone())
            .insert_resource(self.config)
            .add_systems(Startup, grass::wind::create_wind_map)
            .add_systems(PostStartup, grass::grass::generate_grass)
            .add_systems(
                Update,
                (
                    grass::grass::generate_grass,
                    grass::chunk::grass_culling,
                    grass::wind::update_wind_time_decimated,
                    ensure_no_indirect_drawing,
                )
                    .chain(),
            )
            .init_asset::<GrassChunkData>()
            .add_plugins(RenderAssetPlugin::<GrassChunkBuffer>::default())
            .add_plugins((
                ExtractComponentPlugin::<Grass>::default(),
                ExtractComponentPlugin::<GrassChunks>::default(),
                ExtractComponentPlugin::<GrassLODMesh>::default(),
                ExtractComponentPlugin::<GrassWind>::default(),
                ExtractResourcePlugin::<GrassWind>::default(),
            ));

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .add_render_command::<Opaque3d, DrawGrass>()
            .init_resource::<SpecializedMeshPipelines<GrassPipeline>>()
            .add_systems(RenderStartup, render::compute::init_wind_compute_pipeline)
            .add_systems(
                Render,
                (
                    render::queue::grass_queue.in_set(RenderSystems::QueueMeshes),
                    render::prepare::prepare_grass_buffers.in_set(RenderSystems::PrepareResources),
                    render::prepare::prepare_global_wind_buffers
                        .in_set(RenderSystems::PrepareResources),
                    render::prepare::prepare_local_wind_buffers
                        .in_set(RenderSystems::PrepareResources),
                    render::prepare::prepare_grass_bind_group
                        .in_set(RenderSystems::PrepareBindGroups),
                    render::prepare::prepare_global_wind_bind_group
                        .in_set(RenderSystems::PrepareBindGroups),
                    render::prepare::prepare_local_wind_bind_group
                        .in_set(RenderSystems::PrepareBindGroups),
                    render::compute::prepare_wind_compute_bind_group
                        .in_set(RenderSystems::PrepareBindGroups),
                ),
            );

        let mut render_graph = render_app.world_mut().resource_mut::<RenderGraph>();
        render_graph.add_node(GrassWindComputeLabel, GrassWindComputeNode);
        render_graph.add_node_edge(GrassWindComputeLabel, CameraDriverLabel);
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<GrassPipeline>();
        }
    }
}

fn ensure_no_indirect_drawing(
    mut commands: Commands,
    cameras: Query<Entity, (With<Camera3d>, Without<NoIndirectDrawing>)>,
) {
    for entity in &cameras {
        commands.entity(entity).insert(NoIndirectDrawing);
    }
}
