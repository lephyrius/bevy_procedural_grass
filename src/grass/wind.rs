use bevy::{
    asset::RenderAssetUsages,
    ecs::query::QueryItem,
    prelude::*,
    render::{
        extract_component::ExtractComponent,
        extract_resource::ExtractResource,
        render_resource::{Extent3d, ShaderType, TextureDimension, TextureFormat, TextureUsages},
    },
};
#[cfg(feature = "bevy-inspector-egui")]
use bevy_inspector_egui::{InspectorOptions, prelude::ReflectInspectorOptions};
use bytemuck::{Pod, Zeroable};

#[derive(Clone, Copy, Pod, Zeroable, ShaderType)]
#[cfg_attr(feature = "bevy-inspector-egui", derive(Reflect, InspectorOptions))]
#[cfg_attr(feature = "bevy-inspector-egui", reflect(InspectorOptions))]
#[repr(C)]
pub struct Wind {
    pub speed: f32,
    pub amplitude: f32,
    pub frequency: f32,
    pub direction: f32,
    pub oscillation: f32,
    pub scale: f32,
    pub _padding: [f32; 2],
}

impl Default for Wind {
    fn default() -> Self {
        Self {
            speed: 0.15,
            amplitude: 1.0,
            frequency: 1.0,
            direction: 0.0,
            oscillation: 1.5,
            scale: 100.0,
            _padding: [0.0, 0.0],
        }
    }
}

#[derive(Component, Resource, Default, Clone)]
#[cfg_attr(feature = "bevy-inspector-egui", derive(Reflect, InspectorOptions))]
#[cfg_attr(feature = "bevy-inspector-egui", reflect(Resource, InspectorOptions))]
pub struct GrassWind {
    pub wind_data: Wind,
    pub wind_map: Handle<Image>,
}

impl ExtractComponent for GrassWind {
    type QueryData = &'static GrassWind;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(item.clone())
    }
}

impl ExtractResource for GrassWind {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

pub fn create_wind_map(mut wind: ResMut<GrassWind>, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_fill(
        Extent3d {
            width: 1024,
            height: 1024,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0; 16],
        TextureFormat::Rgba32Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage |= TextureUsages::STORAGE_BINDING;

    wind.wind_map = images.add(image);
}

pub fn update_wind_time(mut wind: ResMut<GrassWind>, time: Res<Time>) {
    wind.wind_data._padding[0] = time.elapsed_secs();
}
