#import bevy_pbr::mesh_functions::mesh_position_local_to_clip
#import bevy_pbr::pbr_deferred_functions::deferred_gbuffer_from_pbr_input
#import bevy_pbr::pbr_types
#import bevy_render::globals::Globals

const PI: f32 = 3.14159265358979323846;

@group(0) @binding(1)
var<uniform> globals: Globals;

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,

    @location(3) i_pos: vec3<f32>,
    @location(4) i_normal_packed: vec4<f32>,
    @location(5) i_chunk_uvw_packed: vec4<f32>,
};

struct Color {
    ao: vec4<f32>,
    color_1: vec4<f32>,
    color_2: vec4<f32>,
};
@group(3) @binding(0)
var<uniform> color: Color;

struct Blade {
    length: f32,
    width: f32,
    tilt: f32,
    tilt_variance: f32,
    p1_flexibility: f32,
    p2_flexibility: f32,
    curve: f32,
    specular: f32,
    far_lod_start: f32,
    far_lod_end: f32,
    _padding: vec2<f32>,
}
@group(3) @binding(1)
var<uniform> blade: Blade;

struct Wind {
    speed: f32,
    amplitude: f32,
    frequency: f32,
    direction: f32,
    oscillation: f32,
    scale: f32,
    _padding: vec2<f32>,
};
@group(4) @binding(0)
var<uniform> wind: Wind;

@group(4) @binding(1)
var t_wind_map: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) world_position: vec3<f32>,
    @location(3) world_normal: vec3<f32>,
    @location(4) bezier_tangent: vec3<f32>,
};

struct GrassDeferredFragmentOutput {
#ifdef NORMAL_PREPASS
    @location(0) normal: vec4<f32>,
#endif

#ifdef MOTION_VECTOR_PREPASS
    @location(1) motion_vector: vec2<f32>,
#endif

#ifdef DEFERRED_PREPASS
    @location(2) deferred: vec4<u32>,
    @location(3) deferred_lighting_pass_id: u32,
#endif
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let i_normal = normalize(vertex.i_normal_packed.xyz);
    let uv = vertex.uv;

    var hash_id = random1D(vertex.i_pos.x * 100.0 + vertex.i_pos.y * 100.0 + vertex.i_pos.z * 0.05 + 2.0);
    hash_id = random1D(hash_id * 100000.0);

    var position = vertex.position;

    let rad = wind.direction * PI / 180.0;
    let wind_direction = vec2<f32>(cos(rad), sin(rad));

    let random_point = vec2<f32>(fract(vertex.i_pos.x * 0.1 * hash_id), fract(vertex.i_pos.y * 0.1 * hash_id));
    let r = sample_wind_map(random_point, wind.speed).r;

    let wind_pos = fract(vec2<f32>(vertex.i_pos.x, vertex.i_pos.z) / wind.scale);
    let t = sample_wind_map(wind_pos, wind.speed).r;

    let blade_length = mix(blade.length, blade.length + blade.length / 2.0, fract(hash_id));

    let theta = 2.0 * PI * random1D(hash_id);
    let radius = blade_length * mix(blade.tilt - blade.tilt_variance, blade.tilt, fract(hash_id * 123.0));
    var xz = radius * vec2<f32>(cos(theta), sin(theta));
    let base_p3 = vec3<f32>(xz.x, sqrt(blade_length * blade_length - dot(xz, xz)), xz.y);
    let base_normal = normalize(vec2<f32>(-base_p3.z, base_p3.x));

    xz += -wind_direction * (0.5 * (sin(t * wind.frequency))) * wind.amplitude;
    xz += base_normal * sin(r * 0.2) * wind.oscillation;

    let y = max(-pow((length(xz) * 0.5), 2.0) + blade_length, 0.01);
    let p3 = vec3<f32>(xz.x, y, xz.y);

    let p0 = vec3<f32>(0.0);
    var p1 = 0.33 * p3;
    var p2 = 0.66 * p3;

    let blade_dir_normal = normalize(vec2<f32>(-p3.z, p3.x));
    let blade_normal = normalize(cross(normalize(p3), vec3<f32>(blade_dir_normal.x, 0.0, blade_dir_normal.y)));

    p1 += blade_normal * (y - blade_length) * mix(blade.p1_flexibility, blade.p1_flexibility + 0.2, fract(hash_id * 99.0));
    p2 += blade_normal * (y - blade_length) * mix(blade.p2_flexibility, blade.p2_flexibility + 0.2, fract(hash_id * 2480.0));

    let bezier = cubic_bezier(uv.y, p0, p1, p2, p3);
    let tangent = bezier_tangent(uv.y, p0, p1, p2, p3);
    position.y = bezier.y;
    let width = blade.width * (1.0 - pow(uv.y, 2.0));
    let xz_pos = bezier.xz + (base_normal * vertex.position.x * width);
    position.x = xz_pos.x;
    position.z = xz_pos.y;

    let rotation_matrix = rotate_align(vec3<f32>(0.0, 1.0, 0.0), i_normal);
    position = rotation_matrix * position;

    var normal = normalize(cross(tangent, vec3<f32>(blade_dir_normal.x, 0.0, blade_dir_normal.y)));
    normal = rotation_matrix * normal;
    out.normal = normal;

    position += vertex.i_pos.xyz;

    out.clip_position = mesh_position_local_to_clip(
        identity_matrix,
        vec4<f32>(position, 1.0)
    );

    out.uv = uv;
    out.world_position = position;
    out.world_normal = i_normal;
    out.bezier_tangent = tangent;

    return out;
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> GrassDeferredFragmentOutput {
    var normal = in.normal;
    let uv_x_transformed = in.uv.x * 2.0 - 1.0;
    var normal_curve = blade.curve * -1.0;
    if (!is_front) {
        normal = -normal;
        normal_curve = blade.curve;
    }
    normal = normalize(rotate_vector(normal, in.bezier_tangent, normal_curve * uv_x_transformed));

    let base_color = (mix(color.color_1, color.color_2, in.uv.y)).rgb;
    let ao = (mix(color.ao, vec4<f32>(1.0, 1.0, 1.0, 1.0), in.uv.y)).rgb;
    let roughness = clamp(1.0 - blade.specular * 8.0, 0.15, 0.95);
    var out: GrassDeferredFragmentOutput;

#ifdef NORMAL_PREPASS
    out.normal = vec4(normal * 0.5 + vec3<f32>(0.5), 1.0);
#endif

#ifdef MOTION_VECTOR_PREPASS
    out.motion_vector = vec2<f32>(0.0);
#endif

#ifdef DEFERRED_PREPASS
    // Baby step 2: same gbuffer path, now lit by deferred PBR.
    var pbr = pbr_types::pbr_input_new();
    pbr.world_normal = normalize(in.world_normal);
    pbr.N = normal;
    pbr.diffuse_occlusion = ao;
    pbr.material.base_color = vec4<f32>(base_color, 1.0);
    pbr.material.perceptual_roughness = roughness;
    pbr.material.metallic = 0.0;
    pbr.material.reflectance = vec3<f32>(0.5);
    // Step 1: re-enable transmission-related material properties.
    pbr.material.diffuse_transmission = clamp((1.0 - in.uv.y) * 0.35, 0.0, 0.35);
    pbr.material.specular_transmission = 0.05;
    pbr.material.thickness = mix(0.05, 0.2, 1.0 - in.uv.y);
    pbr.material.attenuation_color = vec4<f32>(base_color, 1.0);
    pbr.material.attenuation_distance = 0.5;

    out.deferred = deferred_gbuffer_from_pbr_input(pbr);
    out.deferred_lighting_pass_id = 1u;
#endif

    return out;
}

fn rotate_vector(v: vec3<f32>, n: vec3<f32>, degrees: f32) -> vec3<f32> {
    let theta = degrees * PI / 180.0;
    let cos_theta = cos(theta);
    let sin_theta = sin(theta);

    return v * cos_theta + cross(n, v) * sin_theta + n * dot(n, v) * (1.0 - cos_theta);
}

fn random1D(n: f32) -> f32 {
    return fract(sin(n) * 43758.5453123);
}

fn cubic_bezier(t: f32, p0: vec3<f32>, p1: vec3<f32>, p2: vec3<f32>, p3: vec3<f32>) -> vec3<f32> {
    let u = 1.0 - t;
    let tt = t * t;
    let uu = u * u;
    let uuu = uu * u;
    let ttt = tt * t;

    var p = uuu * p0;
    p = p + 3.0 * uu * t * p1;
    p = p + 3.0 * u * tt * p2;
    p = p + ttt * p3;

    return p;
}

fn bezier_tangent(t: f32, p0: vec3<f32>, p1: vec3<f32>, p2: vec3<f32>, p3: vec3<f32>) -> vec3<f32> {
    let u = 1.0 - t;
    let u2 = u * u;
    let t2 = t * t;

    let tangent = -3.0 * u2 * p0
        + 3.0 * u2 * p1
        - 6.0 * u * t * p1
        + 6.0 * u * t * p2
        - 3.0 * t2 * p2
        + 3.0 * t2 * p3;

    return tangent;
}

fn rotate_align(v1: vec3<f32>, v2: vec3<f32>) -> mat3x3<f32> {
    let axis = cross(v1, v2);
    let cos_a = dot(v1, v2);
    let k = 1.0 / (1.0 + cos_a);

    return mat3x3f(
        (axis.x * axis.x * k) + cos_a, (axis.x * axis.y * k) + axis.z, (axis.x * axis.z * k) - axis.y,
        (axis.y * axis.x * k) - axis.z, (axis.y * axis.y * k) + cos_a,  (axis.y * axis.z * k) + axis.x,
        (axis.z * axis.x * k) + axis.y, (axis.z * axis.y * k) - axis.x, (axis.z * axis.z * k) + cos_a
    );
}

fn sample_wind_map(uv: vec2<f32>, speed: f32) -> vec4<f32> {
    let texture_size = textureDimensions(t_wind_map);
    let texture_size_i = vec2<i32>(texture_size);
    let texture_size_f = vec2<f32>(texture_size);

    let rad = wind.direction * PI / 180.0;
    let direction = vec2<f32>(cos(rad), sin(rad));

    let scrolled_uv = uv + direction * globals.time * speed;
    let wrapped_uv = fract(scrolled_uv) * texture_size_f;

    let base = vec2<i32>(floor(wrapped_uv));
    let frac_uv = fract(wrapped_uv);

    let x0 = ((base.x % texture_size_i.x) + texture_size_i.x) % texture_size_i.x;
    let y0 = ((base.y % texture_size_i.y) + texture_size_i.y) % texture_size_i.y;
    let x1 = (x0 + 1) % texture_size_i.x;
    let y1 = (y0 + 1) % texture_size_i.y;

    let c00 = textureLoad(t_wind_map, vec2<i32>(x0, y0), 0);
    let c10 = textureLoad(t_wind_map, vec2<i32>(x1, y0), 0);
    let c01 = textureLoad(t_wind_map, vec2<i32>(x0, y1), 0);
    let c11 = textureLoad(t_wind_map, vec2<i32>(x1, y1), 0);

    let cx0 = mix(c00, c10, frac_uv.x);
    let cx1 = mix(c01, c11, frac_uv.x);
    return mix(cx0, cx1, frac_uv.y);
}

const identity_matrix: mat4x4<f32> = mat4x4<f32>(
    vec4<f32>(1.0, 0.0, 0.0, 0.0),
    vec4<f32>(0.0, 1.0, 0.0, 0.0),
    vec4<f32>(0.0, 0.0, 1.0, 0.0),
    vec4<f32>(0.0, 0.0, 0.0, 1.0)
);
