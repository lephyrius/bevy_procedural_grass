use bevy::prelude::*;

use super::{
    config::GrassConfig,
    grass::{Blade, Grass},
    wind::GrassWind,
};

#[derive(Resource, Clone, Copy, Debug)]
pub enum GrassPerformancePreset {
    Quality,
    Balanced,
    Performance,
}

impl GrassPerformancePreset {
    #[inline]
    pub fn quality() -> Self {
        Self::Quality
    }

    #[inline]
    pub fn balanced() -> Self {
        Self::Balanced
    }

    #[inline]
    pub fn performance() -> Self {
        Self::Performance
    }

    #[inline]
    fn apply_to_config(self, config: &mut GrassConfig) {
        match self {
            Self::Quality => {
                config.cull_distance = 260.0;
                config.lod_distance = 130.0;
                config.lod_transition = 80.0;
            }
            Self::Balanced => {
                config.cull_distance = 200.0;
                config.lod_distance = 90.0;
                config.lod_transition = 50.0;
            }
            Self::Performance => {
                config.cull_distance = 160.0;
                config.lod_distance = 55.0;
                config.lod_transition = 25.0;
            }
        }
    }

    #[inline]
    fn apply_to_wind(self, wind: &mut GrassWind) {
        wind.compute_update_hz = match self {
            Self::Quality => 60.0,
            Self::Balanced => 30.0,
            Self::Performance => 20.0,
        };
    }

    #[inline]
    fn apply_to_blade(self, blade: &mut Blade) {
        match self {
            Self::Quality => {
                blade.far_lod_start = 110.0;
                blade.far_lod_end = 190.0;
            }
            Self::Balanced => {
                blade.far_lod_start = 80.0;
                blade.far_lod_end = 140.0;
            }
            Self::Performance => {
                blade.far_lod_start = 55.0;
                blade.far_lod_end = 95.0;
            }
        }
    }
}

#[derive(Component)]
pub struct PerformancePresetApplied;

pub fn apply_preset_to_resources(
    preset: Option<Res<GrassPerformancePreset>>,
    mut config: ResMut<GrassConfig>,
    mut wind: ResMut<GrassWind>,
) {
    let Some(preset) = preset else {
        return;
    };
    preset.apply_to_config(&mut config);
    preset.apply_to_wind(&mut wind);
}

pub fn apply_preset_to_grass(
    mut commands: Commands,
    preset: Option<Res<GrassPerformancePreset>>,
    mut query: Query<(Entity, &mut Blade), (With<Grass>, Without<PerformancePresetApplied>)>,
) {
    let Some(preset) = preset else {
        return;
    };

    for (entity, mut blade) in &mut query {
        preset.apply_to_blade(&mut blade);
        commands.entity(entity).insert(PerformancePresetApplied);
    }
}
