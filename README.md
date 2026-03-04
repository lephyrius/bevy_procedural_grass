# bevy_procedural_grass
[![crates.io](https://img.shields.io/crates/v/bevy_procedural_grass.svg)](https://crates.io/crates/bevy_procedural_grass)
[![Doc](https://docs.rs/bevy_procedural_grass/badge.svg)](https://docs.rs/bevy_procedural_grass)

A plugin for `bevy 0.18` that generates grass on top of any mesh.

![bevy_procedural_grass](https://github.com/jadedbay/bevy_procedural_grass/assets/86005828/6b806f78-0910-40c7-9785-2d4e42d6ebb1)

## Usage

Add `bevy_procedural_grass` to `Cargo.toml`:

```toml
[dependencies]
bevy = "0.18"
bevy_procedural_grass = "0.3"
```

`forward` is enabled by default. If you are integrating your own deferred/G-buffer path,
you can disable it:

```toml
[dependencies]
bevy = "0.18"
bevy_procedural_grass = { version = "0.3", default-features = false }
```

Spawn a terrain mesh and a `GrassBundle`:

```rust
use bevy::prelude::*;
use bevy_procedural_grass::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, ProceduralGrassPlugin::default()))
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let terrain = commands
        .spawn((
            Mesh3d(meshes.add(Plane3d::default().mesh().size(100.0, 100.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.0, 0.05, 0.0),
                ..default()
            })),
        ))
        .id();

    commands.spawn(GrassBundle {
        mesh: Mesh3d(meshes.add(GrassMesh::mesh(7))),
        lod: GrassLODMesh::new(meshes.add(GrassMesh::mesh(3))),
        grass: Grass {
            entity: Some(terrain),
            ..default()
        }
        .with_blade_size(2.0, 0.08),
        ..default()
    });
}
```

Blade size can also be set directly:

```rust
let mut grass = Grass::default();
grass.set_blade_size(1.8, 0.06);
grass.blade.length = 2.2; // equivalent direct field access
grass.blade.width = 0.07;
```

## Grass Interaction
Attach `GrassInteractor` to any entity that should push nearby grass away:

```rust
commands.spawn((
    Mesh3d(meshes.add(Capsule3d::new(0.35, 1.0))),
    MeshMaterial3d(materials.add(Color::srgb(0.8, 0.8, 0.8))),
    Transform::from_xyz(0.0, 1.0, 0.0),
    GrassInteractor {
        radius: 2.0,
        strength: 1.2,
        falloff: 2.0,
    },
));
```

`radius` controls reach, `strength` controls push amount, and `falloff` controls how quickly the effect fades toward the edge.

Deferred variants of the examples are available:
- `cargo run --example grass_deferred`
- `cargo run --example demo_deferred`
- `cargo run --example demo_deferred_pass_id`
- `cargo run --example sphere_deferred`
- `cargo run --example inspect_deferred --features bevy-inspector-egui`

Placement-map example:
- `cargo run --example grass_maps`

Interaction example (moving interactor):
- `cargo run --example grass_interaction`

## Placement Maps
You can gate grass placement with optional density, blade-height, and spawn-elevation masks sampled from mesh UV0:

```rust
commands.spawn(GrassBundle {
    grass: Grass {
        entity: Some(terrain),
        maps: GrassPlacementMaps {
            density_map: Some(asset_server.load("masks/grass_density.png")),
            height_map: Some(asset_server.load("masks/terrain_height.png")),
            height_scale: 1.0,
            elevation_map: Some(asset_server.load("masks/grass_elevation.png")),
            elevation_scale: 0.8,
            uv_scale: Vec2::ONE,
            uv_offset: Vec2::ZERO,
        },
        ..default()
    },
    ..default()
});
```

Notes:
- `density_map` is a `[0, 1]` placement mask (probability/density multiplier).
- `height_map` is a `[0, 1]` blade-height multiplier.
- `height_scale` multiplies sampled `height_map` values before clamping to `[0, 1]`.
- `elevation_map` is a `[0, 1]` spawn-offset mask.
- `elevation_scale` scales sampled `elevation_map` values in world units and offsets each spawned blade along the surface normal.
- Meshes must provide `UV_0` for map sampling.
- Images need CPU-visible data (`RenderAssetUsages` including `MAIN_WORLD`) so placement can be generated on the CPU.

Deferred mode notes:
- Add `DepthPrepass`, `NormalPrepass`, and `DeferredPrepass` to your `Camera3d`.
- The deferred grass path currently uses direct instanced draws for stability.
- Grass uses a dedicated deferred lighting pass (default pass ID `2`).
- Override pass ID via `ProceduralGrassPlugin { deferred_lighting_pass_id: ..., ..default() }` or `ProceduralGrassPlugin::default().with_deferred_lighting_pass_id(...)`.
- For maximum throughput today, prefer the default forward path with indirect draws enabled.

## Deferred Troubleshooting
- Symptom: mostly gray/flat output in deferred mode.
  Cause: deferred lighting pass ordering or pass-id mismatch.
  Fix: ensure your camera has `DeferredPrepass`, `DepthPrepass`, and `NormalPrepass`; keep grass pass ID consistent between g-buffer output and deferred lighting selection.
- Symptom: no grass in deferred example.
  Cause: deferred pipeline still compiling or shader import not ready.
  Fix: wait for pipeline cache to settle and check logs for shader parse/validation errors.
- Symptom: fallback pass-id warning appears.
  Cause: extracted camera pass-id binding was unavailable for that frame.
  Fix: verify extraction is active; set `GRASS_DEBUG=1` only when diagnosing this path.

## Performance Presets
Use a preset to apply coherent tuning for culling/LOD, wind compute rate, and far shading:

```rust
use bevy::prelude::*;
use bevy_procedural_grass::prelude::*;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            ProceduralGrassPlugin {
                performance_preset: Some(GrassPerformancePreset::balanced()),
                ..default()
            },
        ))
        .run();
}
```

Available presets:
- `GrassPerformancePreset::quality()`
- `GrassPerformancePreset::balanced()`
- `GrassPerformancePreset::performance()`

## Tuning Knobs
Recommended ranges:

- `GrassConfig::cull_distance`: `140.0 ..= 300.0`
- `GrassConfig::lod_distance`: `40.0 ..= 160.0`
- `GrassConfig::lod_transition`: `0.0 ..= 100.0`
- `GrassWind::compute_update_hz`: `20.0 ..= 60.0` (`<= 0.0` means every frame)
- `Blade::far_lod_start`: `50.0 ..= 120.0`
- `Blade::far_lod_end`: `90.0 ..= 220.0` (should be `> far_lod_start`)

Notes:
- If `lod_distance >= cull_distance`, chunks render in high LOD until culled.
- `lod_transition > 0.0` enables a dithered transition band to avoid visible LOD rings.
- Wind compute uses a seamless periodic noise field in `wind_compute.wgsl`.

## Features
- Grass positions generated from mesh triangles.
- GPU instancing with frustum/distance culling and LOD.
- Custom render pipeline compatible with Bevy 0.18 render phases.
- Compute shader wind map updates (`wind_compute.wgsl`) with optional decimated update rate.

## Notes
- The plugin inserts `NoIndirectDrawing` on `Camera3d` entities so custom instanced draws work with the current render command path.

## Resources
- [Modern Foliage Rendering - Acerola](https://www.youtube.com/watch?v=jw00MbIJcrk)
- [Procedural Grass in 'Ghost of Tsushima' - GDC](https://www.youtube.com/watch?v=Ibe1JBF5i5Y)
- [Unity-Grass - cainrademan](https://github.com/cainrademan/Unity-Grass/)
- [warbler_grass - EmiOnGit](https://github.com/EmiOnGit/warbler_grass/)
