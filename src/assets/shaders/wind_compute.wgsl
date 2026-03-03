struct Wind {
    speed: f32,
    amplitude: f32,
    frequency: f32,
    direction: f32,
    oscillation: f32,
    scale: f32,
    _padding: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> wind: Wind;

@group(0) @binding(1)
var wind_map: texture_storage_2d<rgba32float, write>;

fn hash(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

fn wrap_scalar(v: f32, period: f32) -> f32 {
    return v - floor(v / period) * period;
}

fn value_noise_tiled(p: vec2<f32>, period: f32) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let ix0 = wrap_scalar(i.x, period);
    let iy0 = wrap_scalar(i.y, period);
    let ix1 = wrap_scalar(i.x + 1.0, period);
    let iy1 = wrap_scalar(i.y + 1.0, period);

    let a = hash(vec2<f32>(ix0, iy0));
    let b = hash(vec2<f32>(ix1, iy0));
    let c = hash(vec2<f32>(ix0, iy1));
    let d = hash(vec2<f32>(ix1, iy1));

    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

@compute @workgroup_size(8, 8, 1)
fn update_wind(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = textureDimensions(wind_map);
    if (id.x >= dims.x || id.y >= dims.y) {
        return;
    }

    let uv = vec2<f32>(id.xy) / vec2<f32>(dims);
    let angle = radians(wind.direction);
    let direction = vec2<f32>(cos(angle), sin(angle));
    let time = wind._padding.x * max(wind.speed, 0.0001);

    // Match tiling period to the UV span of this generated map so wrapped sampling has no seam.
    let base_period = max(wind.scale, 0.01) * 0.05;
    let p = uv * base_period + direction * time;
    let f = max(wind.frequency, 0.01);

    let n1 = value_noise_tiled(p * f, base_period * f);
    let n2 = value_noise_tiled(
        p * f * 1.7 + vec2<f32>(17.0, 9.0),
        base_period * f * 1.7,
    );
    let n3 = value_noise_tiled(p * 0.5 + vec2<f32>(time, -time), base_period * 0.5);

    textureStore(wind_map, vec2<i32>(id.xy), vec4<f32>(n1, n2, n3, 1.0));
}
