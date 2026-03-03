use bevy::{
    app::SubApp,
    core_pipeline::{
        core_3d::graph::{Core3d, Node3d},
        deferred::copy_lighting_id::DeferredLightingIdDepthTexture,
        prepass::DeferredPrepass,
    },
    ecs::query::QueryItem,
    pbr::{
        MeshViewBindGroup, ViewEnvironmentMapUniformOffset, ViewFogUniformOffset,
        ViewLightProbesUniformOffset, ViewLightsUniformOffset,
        ViewScreenSpaceReflectionsUniformOffset, deferred::DeferredLightingPipeline,
        graph::NodePbr,
    },
    prelude::*,
    render::{
        extract_component::{ComponentUniforms, ExtractComponent},
        extract_resource::ExtractResource,
        render_graph::{
            NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
        },
        render_resource::{
            BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, Buffer,
            BufferInitDescriptor, BufferUsages, LoadOp, Operations, PipelineCache,
            RenderPassDepthStencilAttachment, RenderPassDescriptor, ShaderStages, ShaderType,
            StoreOp, binding_types::uniform_buffer,
        },
        renderer::{RenderContext, RenderDevice},
        view::{ViewTarget, ViewUniformOffset},
    },
};
use bytemuck::{Pod, Zeroable};
use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};

static REPORTED_FALLBACK_PASS_ID_BINDING: AtomicBool = AtomicBool::new(false);

pub const GRASS_DEFERRED_LIGHTING_PASS_ID: u8 = 2;

#[derive(Resource, Clone, Copy)]
pub struct GrassDeferredLightingSettings {
    pub pass_id: u8,
}

impl Default for GrassDeferredLightingSettings {
    fn default() -> Self {
        Self {
            pass_id: GRASS_DEFERRED_LIGHTING_PASS_ID,
        }
    }
}

impl ExtractResource for GrassDeferredLightingSettings {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        *source
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GrassDeferredLightingPassIdRaw {
    depth_id: u32,
    _padding: [u32; 3],
}

#[derive(Resource)]
pub struct GrassDeferredLightingPassIdFallbackBuffer {
    buffer: Buffer,
}

#[derive(Component, Clone, Copy, ExtractComponent, ShaderType)]
pub struct GrassDeferredLightingPassId {
    depth_id: u32,
}

impl GrassDeferredLightingPassId {
    pub fn new(value: u8) -> Self {
        Self {
            depth_id: value as u32,
        }
    }
}

impl Default for GrassDeferredLightingPassId {
    fn default() -> Self {
        Self::new(GRASS_DEFERRED_LIGHTING_PASS_ID)
    }
}

pub fn init_grass_deferred_lighting_fallback_buffer(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    settings: Option<Res<GrassDeferredLightingSettings>>,
) {
    let pass_id = settings.map_or(GRASS_DEFERRED_LIGHTING_PASS_ID, |s| s.pass_id);
    let raw = GrassDeferredLightingPassIdRaw {
        depth_id: pass_id as u32,
        _padding: [0; 3],
    };
    let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("grass_deferred_lighting_pass_id_fallback"),
        contents: bytemuck::bytes_of(&raw),
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    });
    commands.insert_resource(GrassDeferredLightingPassIdFallbackBuffer { buffer });
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct GrassDeferredLightingPassNodeLabel;

#[derive(Default)]
pub struct GrassDeferredLightingPassNode;

impl ViewNode for GrassDeferredLightingPassNode {
    type ViewQuery = (
        &'static ViewUniformOffset,
        &'static ViewLightsUniformOffset,
        &'static ViewFogUniformOffset,
        &'static ViewLightProbesUniformOffset,
        &'static ViewScreenSpaceReflectionsUniformOffset,
        &'static ViewEnvironmentMapUniformOffset,
        &'static MeshViewBindGroup,
        &'static ViewTarget,
        &'static DeferredLightingIdDepthTexture,
        &'static DeferredLightingPipeline,
    );

    fn run(
        &self,
        _graph_context: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (
            view_uniform_offset,
            view_lights_offset,
            view_fog_offset,
            view_light_probes_offset,
            view_ssr_offset,
            view_environment_map_offset,
            mesh_view_bind_group,
            target,
            deferred_lighting_id_depth_texture,
            deferred_lighting_pipeline,
        ): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(pipeline) =
            pipeline_cache.get_render_pipeline(deferred_lighting_pipeline.pipeline_id)
        else {
            return Ok(());
        };

        let deferred_lighting_pass_id =
            world.resource::<ComponentUniforms<GrassDeferredLightingPassId>>();
        let fallback = world.resource::<GrassDeferredLightingPassIdFallbackBuffer>();

        let bind_group_2 = if let Some(deferred_lighting_pass_id_binding) =
            deferred_lighting_pass_id.uniforms().binding()
        {
            render_context.render_device().create_bind_group(
                "grass_deferred_lighting_layout_group_2",
                &pipeline_cache.get_bind_group_layout(&grass_deferred_lighting_layout()),
                &BindGroupEntries::single(deferred_lighting_pass_id_binding),
            )
        } else {
            if !REPORTED_FALLBACK_PASS_ID_BINDING.swap(true, Ordering::Relaxed) {
                if grass_debug_enabled() {
                    eprintln!(
                        "grass deferred lighting: using fallback pass-id uniform (camera extraction missing)"
                    );
                }
            }
            render_context.render_device().create_bind_group(
                "grass_deferred_lighting_layout_group_2_fallback",
                &pipeline_cache.get_bind_group_layout(&grass_deferred_lighting_layout()),
                &BindGroupEntries::single(fallback.buffer.as_entire_buffer_binding()),
            )
        };

        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("grass_deferred_lighting"),
            color_attachments: &[Some(target.get_color_attachment())],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &deferred_lighting_id_depth_texture.texture.default_view,
                depth_ops: Some(Operations {
                    load: LoadOp::Load,
                    // Preserve depth-id values for the subsequent default deferred pass.
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(
            0,
            &mesh_view_bind_group.main,
            &[
                view_uniform_offset.offset,
                view_lights_offset.offset,
                view_fog_offset.offset,
                **view_light_probes_offset,
                **view_ssr_offset,
                **view_environment_map_offset,
            ],
        );
        render_pass.set_bind_group(1, &mesh_view_bind_group.binding_array, &[]);
        render_pass.set_bind_group(2, &bind_group_2, &[]);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}

fn grass_deferred_lighting_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "grass_deferred_lighting_layout",
        &BindGroupLayoutEntries::single(
            ShaderStages::VERTEX_FRAGMENT,
            uniform_buffer::<GrassDeferredLightingPassId>(false),
        ),
    )
}

pub fn insert_grass_deferred_lighting_pass_id_component(
    mut commands: Commands,
    settings: Option<Res<GrassDeferredLightingSettings>>,
    views: Query<(Entity, Option<&GrassDeferredLightingPassId>), With<DeferredPrepass>>,
) {
    let pass_id = settings.map_or(GRASS_DEFERRED_LIGHTING_PASS_ID, |s| s.pass_id);
    for (entity, current) in views.iter() {
        if current.is_some_and(|current| current.depth_id == pass_id as u32) {
            continue;
        }
        commands
            .entity(entity)
            .insert(GrassDeferredLightingPassId::new(pass_id));
    }
}

pub fn add_grass_deferred_lighting_pass_node(render_app: &mut SubApp) {
    render_app.add_render_graph_node::<ViewNodeRunner<GrassDeferredLightingPassNode>>(
        Core3d,
        GrassDeferredLightingPassNodeLabel,
    );
    let mut render_graph = render_app
        .world_mut()
        .resource_mut::<bevy::render::render_graph::RenderGraph>();
    let sub_graph = render_graph.sub_graph_mut(Core3d);
    let _ = sub_graph.try_add_node_edge(Node3d::StartMainPass, GrassDeferredLightingPassNodeLabel);
    let _ = sub_graph.try_add_node_edge(
        GrassDeferredLightingPassNodeLabel,
        NodePbr::DeferredLightingPass,
    );
    if sub_graph
        .get_node_state(NodePbr::ScreenSpaceReflections)
        .is_ok()
    {
        let _ = sub_graph.try_add_node_edge(
            GrassDeferredLightingPassNodeLabel,
            NodePbr::ScreenSpaceReflections,
        );
    }
}

fn grass_debug_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("GRASS_DEBUG").is_ok_and(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            !normalized.is_empty()
                && normalized != "0"
                && normalized != "false"
                && normalized != "off"
        })
    })
}
