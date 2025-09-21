struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) texcoord: vec2<f32>,
};

@group(0) @binding(0) var color_sampler: sampler;
@group(0) @binding(1) var texture: texture_2d<f32>;

@vertex
fn vertex(@builtin(vertex_index) vertex: u32) -> VertexOutput {
    var output: VertexOutput;
    if vertex == 0u {
        output.position = vec4(-1.0, 1.0, 0.0, 1.0);
        output.texcoord = vec2(0.0);
    } else if vertex == 1u || vertex == 4u {
        output.position = vec4(1.0, 1.0, 0.0, 1.0);
        output.texcoord = vec2(1.0, 0.0);
    } else if vertex == 2u || vertex == 3u {
        output.position = vec4(-1.0, -1.0, 0.0, 1.0);
        output.texcoord = vec2(0.0, 1.0);
    } else if vertex == 5u {
        output.position = vec4(1.0, -1.0, 0.0, 1.0);
        output.texcoord = vec2(1.0);
    }

    return output;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(texture, color_sampler, in.texcoord);
}
