#import bevy_pbr::mesh_functions::mesh_position_local_to_clip
#import bevy_pbr::pbr_deferred_functions::deferred_gbuffer_from_pbr_input
#import bevy_pbr::pbr_prepass_functions::calculate_motion_vector
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

struct DeferredPassId {
    data: vec4<u32>,
};
@group(3) @binding(2)
var<uniform> deferred_pass_id: DeferredPassId;

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

struct GrassInteractor {
    position_radius: vec4<f32>,
    params: vec4<f32>,
}

struct GrassInteraction {
    count_and_padding: vec4<u32>,
    interactors: array<GrassInteractor, 32>,
}
@group(5) @binding(0)
var<uniform> interaction: GrassInteraction;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) bezier_tangent: vec3<f32>,
    @location(5) world_normal: vec3<f32>,
    @location(6) material_variation: f32,
#ifdef MOTION_VECTOR_PREPASS
    @location(3) world_position: vec3<f32>,
    @location(4) previous_world_position: vec3<f32>,
#endif
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
    let prev_time = globals.time - globals.delta_time;
#ifdef MOTION_VECTOR_PREPASS
    let prev_r = sample_wind_map_at_time(random_point, wind.speed, prev_time).r;
#endif

    let wind_pos = fract(vec2<f32>(vertex.i_pos.x, vertex.i_pos.z) / wind.scale);
    let t = sample_wind_map(wind_pos, wind.speed).r;
#ifdef MOTION_VECTOR_PREPASS
    let prev_t = sample_wind_map_at_time(wind_pos, wind.speed, prev_time).r;
#endif

    let height_factor = clamp(vertex.i_normal_packed.w, 0.0, 1.0);
    let base_blade_length = mix(blade.length, blade.length + blade.length / 2.0, fract(hash_id));
    let blade_length = max(base_blade_length * height_factor, 0.01);
    let rotation_matrix = rotate_align(vec3<f32>(0.0, 1.0, 0.0), i_normal);

    let theta = 2.0 * PI * random1D(hash_id);
    let radius = blade_length * mix(blade.tilt - blade.tilt_variance, blade.tilt, fract(hash_id * 123.0));
    let xz_base = radius * vec2<f32>(cos(theta), sin(theta));
    var xz = xz_base;
    let base_p3 = vec3<f32>(xz_base.x, sqrt(blade_length * blade_length - dot(xz_base, xz_base)), xz_base.y);
    let base_normal = normalize(vec2<f32>(-base_p3.z, base_p3.x));

    xz += -wind_direction * (0.5 * (sin(t * wind.frequency))) * wind.amplitude;
    xz += base_normal * sin(r * 0.2) * wind.oscillation;
    let interaction_push_world = compute_interaction_push(vertex.i_pos.xyz, i_normal);
    let interaction_push_local = transpose(rotation_matrix) * interaction_push_world;
    xz += interaction_push_local.xz * (height_factor * height_factor);

    let xz_len_half = length(xz) * 0.5;
    let y = max(-(xz_len_half * xz_len_half) + blade_length, 0.01);
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
    let uv_y2 = uv.y * uv.y;
    let width = blade.width * (1.0 - uv_y2);
    let xz_pos = bezier.xz + (base_normal * vertex.position.x * width);
    position.x = xz_pos.x;
    position.z = xz_pos.y;

    position = rotation_matrix * position;

#ifdef MOTION_VECTOR_PREPASS
    var prev_position = vertex.position;
    var prev_xz = xz_base;
    prev_xz += -wind_direction * (0.5 * (sin(prev_t * wind.frequency))) * wind.amplitude;
    prev_xz += base_normal * sin(prev_r * 0.2) * wind.oscillation;
    prev_xz += interaction_push_local.xz * (height_factor * height_factor);

    let prev_xz_len_half = length(prev_xz) * 0.5;
    let prev_y = max(-(prev_xz_len_half * prev_xz_len_half) + blade_length, 0.01);
    let prev_p3 = vec3<f32>(prev_xz.x, prev_y, prev_xz.y);

    var prev_p1 = 0.33 * prev_p3;
    var prev_p2 = 0.66 * prev_p3;
    prev_p1 += blade_normal * (prev_y - blade_length) * mix(blade.p1_flexibility, blade.p1_flexibility + 0.2, fract(hash_id * 99.0));
    prev_p2 += blade_normal * (prev_y - blade_length) * mix(blade.p2_flexibility, blade.p2_flexibility + 0.2, fract(hash_id * 2480.0));

    let prev_bezier = cubic_bezier(uv.y, p0, prev_p1, prev_p2, prev_p3);
    prev_position.y = prev_bezier.y;
    let prev_xz_pos = prev_bezier.xz + (base_normal * vertex.position.x * width);
    prev_position.x = prev_xz_pos.x;
    prev_position.z = prev_xz_pos.y;
    prev_position = rotation_matrix * prev_position;
#endif

    var normal = normalize(cross(tangent, vec3<f32>(blade_dir_normal.x, 0.0, blade_dir_normal.y)));
    normal = rotation_matrix * normal;
    out.normal = normal;

    position += vertex.i_pos.xyz;
#ifdef MOTION_VECTOR_PREPASS
    prev_position += vertex.i_pos.xyz;
#endif

    out.clip_position = mesh_position_local_to_clip(
        identity_matrix,
        vec4<f32>(position, 1.0)
    );

    out.uv = uv;
    out.bezier_tangent = tangent;
    out.world_normal = i_normal;
    out.material_variation =
        random1D(vertex.i_pos.x * 17.0 + vertex.i_pos.y * 59.0 + vertex.i_pos.z * 131.0 + 7.0);
#ifdef MOTION_VECTOR_PREPASS
    out.world_position = position;
    out.previous_world_position = prev_position;
#endif

    return out;
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> GrassDeferredFragmentOutput {
    var shading_normal = in.normal;
    let shadow_normal = normalize(in.world_normal);
    let uv_x_transformed = in.uv.x * 2.0 - 1.0;
    var normal_curve = blade.curve * -1.0;
    if (!is_front) {
        shading_normal = -shading_normal;
        normal_curve = blade.curve;
    }
    shading_normal =
        normalize(rotate_vector(shading_normal, in.bezier_tangent, normal_curve * uv_x_transformed));

    var out: GrassDeferredFragmentOutput;

#ifdef NORMAL_PREPASS
    out.normal = vec4(shadow_normal * 0.5 + vec3<f32>(0.5), 1.0);
#endif

#ifdef MOTION_VECTOR_PREPASS
    out.motion_vector = calculate_motion_vector(
        vec4<f32>(in.world_position, 1.0),
        vec4<f32>(in.previous_world_position, 1.0),
    );
#endif

#ifdef DEFERRED_PREPASS
    let base_color = (mix(color.color_1, color.color_2, in.uv.y)).rgb;
    let color_variation_hash = random1D(in.material_variation * 29.0 + 3.0);
    let roughness_variation_hash = random1D(in.material_variation * 53.0 + 11.0);
    let transmission_variation_hash = random1D(in.material_variation * 97.0 + 17.0);
    let color_multiplier = mix(0.9, 1.1, color_variation_hash);
    let varied_base_color = clamp(base_color * color_multiplier, vec3<f32>(0.0), vec3<f32>(1.0));
    let ao = (mix(color.ao, vec4<f32>(1.0, 1.0, 1.0, 1.0), in.uv.y)).rgb;
    let base_roughness = clamp(1.0 - blade.specular * 8.0, 0.15, 0.95);
    let roughness = clamp(base_roughness + mix(-0.07, 0.08, roughness_variation_hash), 0.12, 0.98);
    let transmission_strength = mix(0.85, 1.2, transmission_variation_hash);

    // Deferred G-buffer material payload consumed by Bevy deferred lighting.
    var pbr = pbr_types::pbr_input_new();
    pbr.world_normal = shadow_normal;
    pbr.N = shading_normal;
    pbr.diffuse_occlusion = ao;
    pbr.material.base_color = vec4<f32>(varied_base_color, 1.0);
    pbr.material.perceptual_roughness = roughness;
    pbr.material.metallic = 0.0;
    pbr.material.reflectance = vec3<f32>(0.5);
    pbr.material.diffuse_transmission = clamp((1.0 - in.uv.y) * 0.35 * transmission_strength, 0.0, 0.45);
    pbr.material.specular_transmission = clamp(0.05 * mix(0.7, 1.25, transmission_variation_hash), 0.01, 0.1);
    pbr.material.thickness = mix(0.05, 0.2, 1.0 - in.uv.y);
    pbr.material.attenuation_color = vec4<f32>(varied_base_color, 1.0);
    pbr.material.attenuation_distance = mix(0.4, 0.75, transmission_variation_hash);

    out.deferred = deferred_gbuffer_from_pbr_input(pbr);
    out.deferred_lighting_pass_id = deferred_pass_id.data.x;
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

fn compute_interaction_push(blade_pos: vec3<f32>, surface_normal: vec3<f32>) -> vec3<f32> {
    var push = vec3<f32>(0.0);
    let count = min(interaction.count_and_padding.x, 32u);

    for (var i: u32 = 0u; i < count; i = i + 1u) {
        let interactor = interaction.interactors[i];
        let radius = max(interactor.position_radius.w, 0.001);
        let strength = max(interactor.params.x, 0.0);
        let falloff = max(interactor.params.y, 0.1);
        if (strength <= 0.0) {
            continue;
        }

        let delta = blade_pos - interactor.position_radius.xyz;
        let distance = length(delta);
        if (distance >= radius) {
            continue;
        }

        let planar = delta - surface_normal * dot(delta, surface_normal);
        let planar_len = length(planar);
        if (planar_len <= 0.0001) {
            continue;
        }

        let influence = pow(clamp(1.0 - distance / radius, 0.0, 1.0), falloff) * strength;
        push += (planar / planar_len) * influence;
    }

    let push_len = length(push);
    if (push_len > 2.0) {
        push = (push / push_len) * 2.0;
    }

    return push;
}

fn sample_wind_map_at_time(uv: vec2<f32>, speed: f32, time: f32) -> vec4<f32> {
    let texture_size = textureDimensions(t_wind_map);
    let texture_size_i = vec2<i32>(texture_size);
    let texture_size_f = vec2<f32>(texture_size);

    let rad = wind.direction * PI / 180.0;
    let direction = vec2<f32>(cos(rad), sin(rad));

    let scrolled_uv = uv + direction * time * speed;
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

fn sample_wind_map(uv: vec2<f32>, speed: f32) -> vec4<f32> {
    return sample_wind_map_at_time(uv, speed, globals.time);
}

const identity_matrix: mat4x4<f32> = mat4x4<f32>(
    vec4<f32>(1.0, 0.0, 0.0, 0.0),
    vec4<f32>(0.0, 1.0, 0.0, 0.0),
    vec4<f32>(0.0, 0.0, 1.0, 0.0),
    vec4<f32>(0.0, 0.0, 0.0, 1.0)
);
