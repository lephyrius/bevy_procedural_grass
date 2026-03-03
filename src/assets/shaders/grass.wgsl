#import bevy_pbr::mesh_functions::mesh_position_local_to_clip
#import bevy_pbr::mesh_bindings::mesh
#import bevy_pbr::mesh_view_bindings::globals
#import bevy_pbr::mesh_view_bindings::lights
#import bevy_pbr::mesh_view_bindings::view
#import bevy_pbr::shadows

const PI: f32 = 3.14159265358979323846;

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,

    @location(3) i_pos: vec3<f32>,
    @location(4) i_normal: vec3<f32>,
    @location(5) i_chunk_uvw: vec3<f32>,
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
    @location(1) uv: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) world_position: vec3<f32>,
    @location(4) world_normal: vec3<f32>,
    @location(5) bezier_tangent: vec3<f32>,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    let uv = vertex.uv;

    var hash_id = random1D(vertex.i_pos.x * 100. + vertex.i_pos.y * 100. + vertex.i_pos.z * 0.05 + 2.);
    hash_id = random1D(hash_id * 100000.);

    var position = vertex.position;

    let rad = wind.direction * PI / 180.0;
    let wind_direction = vec2<f32>(cos(rad), sin(rad));
    var facing = normalize(vec2<f32>(mix(-1., 1., hash_id), mix(-1., 1., random1D(hash_id * vertex.i_pos.x))));

    let random_point = vec2<f32>(fract(vertex.i_pos.x * 0.1 * hash_id), fract(vertex.i_pos.y * 0.1 * hash_id));
    let r = sample_wind_map(random_point, wind.speed).r;

    var wind_pos = fract(vec2<f32>(vertex.i_pos.x, vertex.i_pos.z) / wind.scale);
    let t = sample_wind_map(wind_pos, wind.speed).r;

    let blade_length = mix(blade.length, blade.length + blade.length / 2., fract(hash_id));

    let theta = 2.0 * PI * random1D(hash_id);
    let radius = blade_length * mix(blade.tilt - blade.tilt_variance, blade.tilt, fract(hash_id * 123.));
    var xz = radius * vec2<f32>(cos(theta), sin(theta));
    let base_p3 = vec3<f32>(xz.x, sqrt(blade_length * blade_length - dot(xz, xz)), xz.y);
    let base_normal = normalize(vec2<f32>(-base_p3.z, base_p3.x));

    //let xz_displacement = sample_displacement_image(vertex.i_chunk_uvw.xz);

    //let angle = xz_displacement.r * 2.0 * PI;
    //let displace_direction = vec2<f32>(-cos(angle), -sin(angle));
    //var displace_strength = xz_displacement.a * (1.0 - clamp(abs(xz_displacement.b - vertex.i_chunk_uvw.y) / (length / 30.0), 0.0, 1.0));

    //xz += displace_direction * (length + blade.tilt) * displace_strength;

    xz += -wind_direction * (0.5 * (sin(t * wind.frequency))) * wind.amplitude;
    xz += base_normal * sin(r * 0.2) * wind.oscillation;

    var y = max(-pow((length(xz) * 0.5), 2.) + blade_length, 0.01);
    var p3 = vec3<f32>(xz.x, y, xz.y);

    let p0 = vec3<f32>(0.0);
    var p1 = (0.33) * p3;
    var p2 = (0.66) * p3;

    var blade_dir_normal = normalize(vec2<f32>(-p3.z, p3.x));
    var blade_normal = normalize(cross(normalize(p3), vec3<f32>(blade_dir_normal.x, 0., blade_dir_normal.y)));

    let distance = distance(base_p3, p3);

    p1 += blade_normal * (y - blade_length) * mix(blade.p1_flexibility, blade.p1_flexibility + 0.2, fract(hash_id * 99.));
    p2 += blade_normal * (y - blade_length) * mix(blade.p2_flexibility, blade.p2_flexibility + 0.2, fract(hash_id * 2480.));

    let bezier = cubic_bezier(uv.y, p0, p1, p2, p3);
    let tangent = bezier_tangent(uv.y, p0, p1, p2, p3);
    position.y = bezier.y;
    let width = blade.width * (1.0 - pow(uv.y, 2.));
    let xz_pos = bezier.xz + (base_normal * vertex.position.x * width);
    position.x = xz_pos.x;
    position.z = xz_pos.y;

    let rotation_matrix = rotate_align(vec3<f32>(0.0, 1.0, 0.0), vertex.i_normal);
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
    out.world_normal = vertex.i_normal;
    out.bezier_tangent = tangent;

    return out;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> @location(0) vec4<f32> {
    var normal = in.normal;
    let uv_x_transformed = in.uv.x * 2.0 - 1.0;
    var normal_curve = blade.curve * -1.0;
    if (!is_front) {
        normal = -normal;
        normal_curve = blade.curve;
    }
    normal = normalize(rotate_vector(normal, in.bezier_tangent, normal_curve * uv_x_transformed));
    let N = normal;
    let V = normalize(view.world_position - in.world_position);

    let base_color = (mix(color.color_1, color.color_2, in.uv.y)).rgb;
    let ao = (mix(color.ao, vec4<f32>(1.0, 1.0, 1.0, 1.0), in.uv.y)).rgb;
    let distance = length(view.world_position - in.world_position);
    let far_lod_start = 80.0;
    let far_lod_end = 140.0;
    let far_lod = clamp((distance - far_lod_start) / (far_lod_end - far_lod_start), 0.0, 1.0);
    let spec_strength = mix(0.5, 0.0, clamp((distance - 20.0) / 20.0, 0.0, 1.0)) * blade.specular;
    let roughness = clamp(1.0 - blade.specular * 8.0, 0.15, 0.95);
    let spec_power = mix(64.0, 8.0, roughness);

    var direct = vec3<f32>(0.0);
    let view_z = dot(vec4<f32>(
        view.view_from_world[0].z,
        view.view_from_world[1].z,
        view.view_from_world[2].z,
        view.view_from_world[3].z
    ), vec4<f32>(in.world_position, 1.0));

    let n_directional_lights = lights.n_directional_lights;
    if (far_lod > 0.0 && n_directional_lights > 0u) {
        // Cheaper far path: single directional light, no shadows, no specular.
        let L = normalize(lights.directional_lights[0].direction_to_light);
        let NdotL = max(dot(N, L), 0.0);
        direct = base_color * NdotL * lights.directional_lights[0].color.rgb;
    } else {
        for (var i: u32 = 0u; i < n_directional_lights; i = i + 1u) {
            let L = normalize(lights.directional_lights[i].direction_to_light);
            let NdotL = max(dot(N, L), 0.0);
            if (NdotL <= 0.0) {
                continue;
            }

            let shadow = clamp(shadows::fetch_directional_shadow(i, vec4<f32>(in.world_position, 1.0), in.world_normal, view_z), 0.1, 1.0);
            let H = normalize(L + V);
            let spec = pow(max(dot(N, H), 0.0), spec_power) * spec_strength;
            let diffuse = base_color * NdotL;
            let light_rgb = lights.directional_lights[i].color.rgb;
            direct += (diffuse + vec3<f32>(spec)) * light_rgb * shadow;
        }
    }

    let ambient = base_color * max(lights.ambient_color.rgb, vec3<f32>(0.02));
    // Blend toward cheaper far lighting smoothly to hide transition.
    let n_directional = max(lights.n_directional_lights, 1u);
    let direct_avg = direct / f32(n_directional);
    let final_direct = mix(direct, direct_avg, far_lod * 0.35);
    let final_color = (ambient + final_direct) * ao * view.exposure;

    return vec4<f32>(final_color.rgb, 1.0);
}

fn rotate_vector(v: vec3<f32>, n: vec3<f32>, degrees: f32) -> vec3<f32> {
    let theta = degrees * PI / 180.;
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

    let result = mat3x3f(
            (axis.x * axis.x * k) + cos_a, (axis.x * axis.y * k) + axis.z, (axis.x * axis.z * k) - axis.y,
            (axis.y * axis.x * k) - axis.z, (axis.y * axis.y * k) + cos_a,  (axis.y * axis.z * k) + axis.x,
            (axis.z * axis.x * k) + axis.y, (axis.z * axis.y * k) - axis.x, (axis.z * axis.z * k) + cos_a
        );

    return result;
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
