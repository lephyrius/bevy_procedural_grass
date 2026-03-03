use bevy::prelude::*;

pub mod compute;
pub mod draw;
pub mod pipeline;
pub mod prepare;
pub mod queue;

pub mod instance;

#[derive(Resource, Clone, Copy)]
pub struct GrassIndirectSettings {
    pub enabled: bool,
}

impl Default for GrassIndirectSettings {
    #[inline]
    fn default() -> Self {
        Self { enabled: true }
    }
}
