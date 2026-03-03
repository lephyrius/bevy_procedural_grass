use bevy::prelude::*;

#[cfg(feature = "forward")]
pub mod compute;
#[cfg(feature = "forward")]
pub mod deferred_lighting;
#[cfg(feature = "forward")]
pub mod draw;
#[cfg(feature = "forward")]
pub mod pipeline;
#[cfg(feature = "forward")]
pub mod prepare;
#[cfg(feature = "forward")]
pub mod queue;

pub mod instance;

#[derive(Resource, Clone, Copy)]
#[cfg_attr(not(feature = "forward"), allow(dead_code))]
pub struct GrassIndirectSettings {
    pub enabled: bool,
}

impl Default for GrassIndirectSettings {
    #[inline]
    fn default() -> Self {
        Self { enabled: true }
    }
}
