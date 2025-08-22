
struct ViewUniform {
    view_matrix: mat4x4<f32>,
    view_proj_matrix: mat4x4<f32>,
};

struct DrawUniform {
    world_matrix: mat4x4<f32>,
    base_color: vec4<f32>,
    world_inverse_matrix: mat4x4<f32>,
};

struct VertexIn {
    @location(0) vertex: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> view_uniform: ViewUniform;
@group(1) @binding(0) var<uniform> draw_uniform: DrawUniform;

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

fn blend_color(a: vec4<f32>, b: vec4<f32>) -> vec4<f32> {
    let alpha: f32 = mix(a.w, 1.0, b.w);
    let rgb: vec3<f32> = mix(b.w * b.xyz, a.xyz, a.w);
    if alpha > 0.001 {
        return vec4(rgb / alpha, alpha);
    } else {
        return vec4(rgb, alpha);
    }
}

@vertex
fn vs_main(vertex: VertexIn) -> VertexOut {
    var out: VertexOut;

    out.position = vec4<f32>(v_positions[v_idx], 0.0, 1.0);
    out.position.x = out.position.x * cos(uniforms.angle);
    out.color = v_colors[v_idx];

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return in.color;
}
