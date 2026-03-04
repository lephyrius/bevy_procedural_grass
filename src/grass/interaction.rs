use bevy::{
    prelude::*,
    render::{extract_resource::ExtractResource, render_resource::ShaderType},
};
#[cfg(feature = "bevy-inspector-egui")]
use bevy_inspector_egui::{InspectorOptions, prelude::ReflectInspectorOptions};
use bytemuck::{Pod, Zeroable};

pub const MAX_GRASS_INTERACTORS: usize = 32;

#[derive(Component, Clone, Copy)]
#[cfg_attr(feature = "bevy-inspector-egui", derive(Reflect, InspectorOptions))]
#[cfg_attr(feature = "bevy-inspector-egui", reflect(InspectorOptions))]
pub struct GrassInteractor {
    /// Radius in world units where this interactor affects grass.
    pub radius: f32,
    /// Push strength multiplier applied inside `radius`.
    pub strength: f32,
    /// Falloff exponent (`1.0` is linear, higher values are sharper near center).
    pub falloff: f32,
}

impl GrassInteractor {
    #[inline]
    pub fn new(radius: f32) -> Self {
        Self {
            radius,
            ..default()
        }
    }
}

impl Default for GrassInteractor {
    #[inline]
    fn default() -> Self {
        Self {
            radius: 1.5,
            strength: 1.0,
            falloff: 2.0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Pod, Zeroable, ShaderType)]
pub struct GrassInteractorUniform {
    pub position_radius: [f32; 4],
    /// x = strength, y = falloff exponent
    pub params: [f32; 4],
}

impl GrassInteractorUniform {
    pub const ZERO: Self = Self {
        position_radius: [0.0; 4],
        params: [0.0; 4],
    };
}

#[repr(C)]
#[derive(Resource, Clone, Copy, PartialEq, Pod, Zeroable, ShaderType)]
pub struct GrassInteractionUniform {
    pub count_and_padding: [u32; 4],
    pub interactors: [GrassInteractorUniform; MAX_GRASS_INTERACTORS],
}

impl Default for GrassInteractionUniform {
    #[inline]
    fn default() -> Self {
        Self {
            count_and_padding: [0; 4],
            interactors: [GrassInteractorUniform::ZERO; MAX_GRASS_INTERACTORS],
        }
    }
}

impl ExtractResource for GrassInteractionUniform {
    type Source = Self;

    #[inline]
    fn extract_resource(source: &Self::Source) -> Self {
        *source
    }
}

/// Collects world-space grass interactor data into a fixed-size GPU uniform payload.
pub fn update_grass_interaction_uniform(
    mut interaction: ResMut<GrassInteractionUniform>,
    query: Query<(&GlobalTransform, &GrassInteractor)>,
) {
    let mut count = 0usize;
    for (transform, interactor) in query.iter() {
        if count >= MAX_GRASS_INTERACTORS {
            break;
        }

        let strength = interactor.strength.max(0.0);
        if strength <= f32::EPSILON {
            continue;
        }

        let position = transform.translation();
        interaction.interactors[count] = GrassInteractorUniform {
            position_radius: [
                position.x,
                position.y,
                position.z,
                interactor.radius.max(0.01),
            ],
            params: [strength, interactor.falloff.max(0.1), 0.0, 0.0],
        };
        count += 1;
    }

    for slot in &mut interaction.interactors[count..] {
        *slot = GrassInteractorUniform::ZERO;
    }
    interaction.count_and_padding[0] = count as u32;
}
