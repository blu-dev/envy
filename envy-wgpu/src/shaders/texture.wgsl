
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
    @location(1) texcoord: vec2<f32>,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) texcoord: vec2<f32>,
};

@group(0) @binding(0) var<uniform> view_uniform: ViewUniform;
@group(1) @binding(0) var<uniform> draw_uniform: DrawUniform;
@group(2) @binding(0) var color_sampler: sampler;
@group(2) @binding(1) var texture: texture_2d<f32>;
@group(3) @binding(0) var mask_sampler: sampler;
@group(3) @binding(1) var mask_texture: texture_2d<f32>;

@vertex
fn vertex(vertex: VertexIn) -> VertexOut {
    var out: VertexOut;

    out.position = view_uniform.view_proj_matrix * view_uniform.view_matrix * draw_uniform.world_matrix * vec4(vertex.vertex, 1.0);
    out.color = draw_uniform.base_color;
    out.texcoord = vertex.texcoord;

    return out;
}

@fragment
fn fragment(in: VertexOut) -> @location(0) vec4<f32> {
    var mask_color = textureSample(mask_texture, mask_sampler, in.texcoord);
    mask_color = vec4(mask_color.xyz * mask_color.a, mask_color.a);
    return textureSample(texture, color_sampler, in.texcoord) * mask_color * in.color;
}
