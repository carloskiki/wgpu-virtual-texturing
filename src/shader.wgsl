struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct RenderInterpolators {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

@vertex
fn vs_render(in: VertexInput) -> RenderInterpolators {
    var result: RenderInterpolators;
    result.position = vec4<f32>(in.position, 1.0);
    result.tex_coords = in.uv;
    return result;
}

@fragment
fn fs_render(in: RenderInterpolators) -> @location(0) vec4<f32> {
    return vec4<f32>(in.tex_coords, 0.0, 1.0);
}
