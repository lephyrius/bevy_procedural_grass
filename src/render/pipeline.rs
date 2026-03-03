use bevy::{
    core_pipeline::prepass::prepass_target_descriptors,
    mesh::{MeshVertexBufferLayoutRef, VertexBufferLayout},
    pbr::{MeshPipeline, MeshPipelineKey, PrepassPipeline},
    prelude::*,
    render::render_resource::{
        BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BufferBindingType,
        RenderPipelineDescriptor, ShaderStages, SpecializedMeshPipeline,
        SpecializedMeshPipelineError, TextureSampleType, TextureViewDimension, VertexAttribute,
        VertexFormat, VertexStepMode,
    },
};

use super::instance::GrassData;
use crate::{GRASS_DEFERRED_SHADER_HANDLE, GRASS_SHADER_HANDLE};

fn build_grass_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "grass_layout",
        &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::VERTEX_FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    )
}

fn build_wind_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "wind_layout",
        &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX | ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: false },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    )
}

fn grass_instance_layout() -> VertexBufferLayout {
    VertexBufferLayout {
        array_stride: size_of::<GrassData>() as u64,
        step_mode: VertexStepMode::Instance,
        attributes: vec![
            VertexAttribute {
                format: VertexFormat::Float32x3,
                offset: 0,
                shader_location: 3,
            },
            VertexAttribute {
                format: VertexFormat::Snorm16x4,
                offset: size_of::<Vec3>() as u64,
                shader_location: 4,
            },
        ],
    }
}

#[derive(Resource)]
pub struct GrassPipeline {
    pub shader: Handle<Shader>,
    pub mesh_pipeline: MeshPipeline,
    pub grass_layout: BindGroupLayoutDescriptor,
    pub wind_layout: BindGroupLayoutDescriptor,
}

impl FromWorld for GrassPipeline {
    fn from_world(world: &mut World) -> Self {
        let mesh_pipeline = world.resource::<MeshPipeline>().clone();

        Self {
            shader: GRASS_SHADER_HANDLE,
            mesh_pipeline,
            grass_layout: build_grass_layout(),
            wind_layout: build_wind_layout(),
        }
    }
}

impl SpecializedMeshPipeline for GrassPipeline {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut descriptor = self.mesh_pipeline.specialize(key, layout)?;

        descriptor
            .vertex
            .shader_defs
            .push("PREPASS_PIPELINE".into());
        descriptor
            .vertex
            .shader_defs
            .push("PREPASS_FRAGMENT".into());
        descriptor.vertex.shader = self.shader.clone();
        descriptor.vertex.buffers.push(grass_instance_layout());
        descriptor.fragment.as_mut().unwrap().shader = self.shader.clone();
        descriptor.primitive.cull_mode = None;
        descriptor.layout.push(self.grass_layout.clone());
        descriptor.layout.push(self.wind_layout.clone());

        Ok(descriptor)
    }
}

#[derive(Resource)]
pub struct GrassDeferredPipeline {
    pub shader: Handle<Shader>,
    pub mesh_pipeline: MeshPipeline,
    pub prepass_pipeline: PrepassPipeline,
    pub grass_layout: BindGroupLayoutDescriptor,
    pub wind_layout: BindGroupLayoutDescriptor,
}

impl FromWorld for GrassDeferredPipeline {
    fn from_world(world: &mut World) -> Self {
        let mesh_pipeline = world.resource::<MeshPipeline>().clone();
        let prepass_pipeline = world.resource::<PrepassPipeline>().clone();
        Self {
            shader: GRASS_DEFERRED_SHADER_HANDLE,
            mesh_pipeline,
            prepass_pipeline,
            grass_layout: build_grass_layout(),
            wind_layout: build_wind_layout(),
        }
    }
}

impl SpecializedMeshPipeline for GrassDeferredPipeline {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut descriptor = self.mesh_pipeline.specialize(key, layout)?;

        descriptor.vertex.shader = self.shader.clone();
        descriptor.vertex.buffers.push(grass_instance_layout());

        let normal_prepass = key.contains(MeshPipelineKey::NORMAL_PREPASS);
        let motion_vector_prepass = key.contains(MeshPipelineKey::MOTION_VECTOR_PREPASS);
        let deferred_prepass = key.contains(MeshPipelineKey::DEFERRED_PREPASS);

        descriptor.layout[0] = if motion_vector_prepass {
            self.prepass_pipeline.view_layout_motion_vectors.clone()
        } else {
            self.prepass_pipeline.view_layout_no_motion_vectors.clone()
        };
        descriptor.layout[1] = self.prepass_pipeline.empty_layout.clone();

        let fragment = descriptor.fragment.as_mut().unwrap();
        fragment.shader_defs.push("PREPASS_FRAGMENT".into());
        fragment.shader = self.shader.clone();
        fragment.targets =
            prepass_target_descriptors(normal_prepass, motion_vector_prepass, deferred_prepass);

        descriptor.primitive.cull_mode = None;
        descriptor.layout.push(self.grass_layout.clone());
        descriptor.layout.push(self.wind_layout.clone());

        Ok(descriptor)
    }
}

pub(crate) fn ensure_grass_deferred_pipeline(
    mut commands: Commands,
    mesh_pipeline: Res<MeshPipeline>,
    prepass_pipeline: Option<Res<PrepassPipeline>>,
    existing_pipeline: Option<Res<GrassDeferredPipeline>>,
) {
    if existing_pipeline.is_some() {
        return;
    }

    let Some(prepass_pipeline) = prepass_pipeline else {
        return;
    };

    commands.insert_resource(GrassDeferredPipeline {
        shader: GRASS_DEFERRED_SHADER_HANDLE,
        mesh_pipeline: mesh_pipeline.as_ref().clone(),
        prepass_pipeline: prepass_pipeline.as_ref().clone(),
        grass_layout: build_grass_layout(),
        wind_layout: build_wind_layout(),
    });
}
