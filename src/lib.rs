#[cfg(feature = "forward")]
use bevy::asset::{load_internal_asset, uuid_handle};
#[cfg(feature = "forward")]
use bevy::render::extract_component::UniformComponentPlugin;
#[cfg(feature = "forward")]
use bevy::{
    core_pipeline::{core_3d::Opaque3d, deferred::Opaque3dDeferred},
    render::Render,
    render::RenderApp,
    render::RenderStartup,
    render::RenderSystems,
    render::graph::CameraDriverLabel,
    render::render_graph::RenderGraph,
    render::render_phase::AddRenderCommand,
    render::render_resource::SpecializedMeshPipelines,
    render::view::NoIndirectDrawing,
};
use bevy::{
    prelude::*,
    render::{
        extract_component::ExtractComponentPlugin, extract_resource::ExtractResourcePlugin,
        render_asset::RenderAssetPlugin,
    },
};

use grass::{
    chunk::GrassChunks,
    config::GrassConfig,
    grass::{Grass, GrassLODMesh},
    wind::GrassWind,
};
use render::instance::{GrassChunkBuffer, GrassChunkData};
#[cfg(feature = "forward")]
use render::{
    GrassIndirectSettings,
    compute::{GrassWindComputeLabel, GrassWindComputeNode},
    deferred_lighting::GrassDeferredLightingPassId,
    draw::{DrawGrass, DrawGrassDeferred},
    pipeline::{GrassDeferredPipeline, GrassPipeline},
};

pub mod grass;
mod render;
mod util;

pub use grass::performance::GrassPerformancePreset;
#[cfg(feature = "forward")]
pub use render::deferred_lighting::{
    GRASS_DEFERRED_LIGHTING_PASS_ID, GrassDeferredLightingSettings,
};

pub mod prelude {
    #[cfg(feature = "forward")]
    pub use crate::GRASS_DEFERRED_LIGHTING_PASS_ID;
    #[cfg(feature = "forward")]
    pub use crate::GrassDeferredLightingSettings;
    pub use crate::grass::{
        config::GrassConfig,
        grass::{Grass, GrassBundle, GrassLODMesh, GrassPlacementMaps},
        mesh::GrassMesh,
        wind::{GrassWind, Wind},
    };
    pub use crate::{GrassPerformancePreset, ProceduralGrassPlugin};
}

#[cfg(feature = "forward")]
pub(crate) const GRASS_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("6e91d4b1-491c-4a26-b15f-5fd4f57764df");
#[cfg(feature = "forward")]
pub(crate) const GRASS_WIND_COMPUTE_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("9ced8fce-d838-457a-a5d9-3ce7d5b6d557");
#[cfg(feature = "forward")]
pub(crate) const GRASS_INDIRECT_COMPUTE_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("6a5c509b-6da8-4ea2-8320-8f2f5d7bf4f6");
#[cfg(feature = "forward")]
pub(crate) const GRASS_DEFERRED_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("0f4f5d16-0db2-4f58-9f5c-c381e1ee8a5c");

#[derive(Clone)]
pub struct ProceduralGrassPlugin {
    pub config: GrassConfig,
    pub wind: GrassWind,
    pub performance_preset: Option<GrassPerformancePreset>,
    pub indirect_draws: bool,
    pub deferred_lighting_pass_id: u8,
}

impl Default for ProceduralGrassPlugin {
    #[inline]
    fn default() -> Self {
        Self {
            config: GrassConfig::default(),
            wind: GrassWind::default(),
            performance_preset: None,
            indirect_draws: true,
            deferred_lighting_pass_id: 2,
        }
    }
}

impl ProceduralGrassPlugin {
    #[inline]
    /// Overrides the deferred lighting pass id used by grass in deferred mode.
    ///
    /// Values are clamped to at least `1` because `0` is reserved by Bevy's deferred-id depth clear.
    pub fn with_deferred_lighting_pass_id(mut self, pass_id: u8) -> Self {
        self.deferred_lighting_pass_id = pass_id.max(1);
        self
    }
}

impl Plugin for ProceduralGrassPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "forward")]
        {
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
            load_internal_asset!(
                app,
                GRASS_INDIRECT_COMPUTE_SHADER_HANDLE,
                "assets/shaders/indirect_compute.wgsl",
                Shader::from_wgsl
            );
            load_internal_asset!(
                app,
                GRASS_DEFERRED_SHADER_HANDLE,
                "assets/shaders/grass_deferred.wgsl",
                Shader::from_wgsl
            );
        }

        #[cfg(feature = "bevy-inspector-egui")]
        {
            app.register_type::<Grass>()
                .register_type::<GrassWind>()
                .register_type::<GrassConfig>();
        }

        #[cfg(feature = "forward")]
        let indirect_settings = GrassIndirectSettings {
            enabled: self.indirect_draws,
        };
        #[cfg(feature = "forward")]
        let deferred_lighting_settings = render::deferred_lighting::GrassDeferredLightingSettings {
            // 0 would collide with the depth clear value in the deferred-id texture.
            pass_id: self.deferred_lighting_pass_id.max(1),
        };

        app.insert_resource(self.wind.clone())
            .insert_resource(self.config)
            .add_systems(
                Startup,
                (
                    grass::performance::apply_preset_to_resources,
                    grass::wind::create_wind_map,
                )
                    .chain(),
            )
            .add_systems(
                PostStartup,
                (
                    grass::performance::apply_preset_to_grass,
                    grass::grass::generate_grass,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    grass::performance::apply_preset_to_grass,
                    grass::grass::generate_grass,
                    grass::chunk::grass_culling,
                    grass::wind::update_wind_time_decimated,
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
        #[cfg(feature = "forward")]
        app.insert_resource(deferred_lighting_settings)
            .add_plugins(ExtractResourcePlugin::<
                render::deferred_lighting::GrassDeferredLightingSettings,
            >::default());
        #[cfg(feature = "forward")]
        app.add_plugins((
            ExtractComponentPlugin::<GrassDeferredLightingPassId>::default(),
            UniformComponentPlugin::<GrassDeferredLightingPassId>::default(),
        ))
        .add_systems(
            PostUpdate,
            render::deferred_lighting::insert_grass_deferred_lighting_pass_id_component,
        );
        #[cfg(feature = "forward")]
        {
            app.insert_resource(indirect_settings);
            app.add_systems(Update, ensure_no_indirect_drawing);
        }

        if let Some(preset) = self.performance_preset {
            app.insert_resource(preset);
        }

        #[cfg(feature = "forward")]
        {
            let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
                return;
            };

            render_app
                .add_render_command::<Opaque3d, DrawGrass>()
                .add_render_command::<Opaque3dDeferred, DrawGrassDeferred>()
                .init_resource::<SpecializedMeshPipelines<GrassPipeline>>()
                .init_resource::<SpecializedMeshPipelines<GrassDeferredPipeline>>()
                .add_systems(
                    RenderStartup,
                    (
                        render::deferred_lighting::init_grass_deferred_lighting_fallback_buffer,
                        render::compute::init_wind_compute_pipeline,
                        render::compute::init_indirect_compute_pipeline,
                    ),
                )
                .add_systems(
                    Render,
                    (
                        render::pipeline::ensure_grass_deferred_pipeline
                            .before(RenderSystems::QueueMeshes),
                        render::queue::grass_queue.in_set(RenderSystems::QueueMeshes),
                        render::queue::grass_queue_deferred.in_set(RenderSystems::QueueMeshes),
                        render::prepare::prepare_grass_buffers
                            .in_set(RenderSystems::PrepareResources),
                        render::prepare::prepare_indirect_buffers
                            .in_set(RenderSystems::PrepareResources),
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
                        render::compute::prepare_indirect_compute_bind_group
                            .in_set(RenderSystems::PrepareBindGroups),
                    ),
                )
                .insert_resource(indirect_settings);

            let mut render_graph = render_app.world_mut().resource_mut::<RenderGraph>();
            render_graph.add_node(GrassWindComputeLabel, GrassWindComputeNode);
            render_graph.add_node(
                render::compute::GrassIndirectComputeLabel,
                render::compute::GrassIndirectComputeNode,
            );
            render_graph.add_node_edge(GrassWindComputeLabel, CameraDriverLabel);
            render_graph.add_node_edge(
                render::compute::GrassIndirectComputeLabel,
                CameraDriverLabel,
            );
            drop(render_graph);
            render::deferred_lighting::add_grass_deferred_lighting_pass_node(render_app);
        }
    }

    fn finish(&self, app: &mut App) {
        #[cfg(feature = "forward")]
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<GrassPipeline>();
        }

        #[cfg(not(feature = "forward"))]
        let _ = app;
    }
}

#[inline]
#[cfg(feature = "forward")]
fn ensure_no_indirect_drawing(
    mut commands: Commands,
    indirect_settings: Res<GrassIndirectSettings>,
    cameras_without_flag: Query<Entity, (With<Camera3d>, Without<NoIndirectDrawing>)>,
    cameras_with_flag: Query<Entity, (With<Camera3d>, With<NoIndirectDrawing>)>,
) {
    if indirect_settings.enabled {
        for entity in &cameras_with_flag {
            commands.entity(entity).remove::<NoIndirectDrawing>();
        }
    } else {
        for entity in &cameras_without_flag {
            commands.entity(entity).insert(NoIndirectDrawing);
        }
    }
}
