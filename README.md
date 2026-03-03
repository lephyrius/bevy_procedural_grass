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
        },
        ..default()
    });
}
```

Deferred variants of the examples are available:
- `cargo run --example grass_deferred`
- `cargo run --example demo_deferred`
- `cargo run --example sphere_deferred`
- `cargo run --example inspect_deferred --features bevy-inspector-egui`

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
