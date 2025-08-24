
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
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> view_uniform: ViewUniform;
@group(1) @binding(0) var<uniform> draw_uniform: DrawUniform;

@vertex
fn vertex(vertex: VertexIn) -> VertexOut {
    var out: VertexOut;

    out.position = view_uniform.view_proj_matrix * view_uniform.view_matrix * draw_uniform.world_matrix * vec4(vertex.vertex, 1.0);
    out.color = draw_uniform.base_color;

    return out;
}

@fragment
fn fragment(in: VertexOut) -> @location(0) vec4<f32> {
    return in.color;
}
