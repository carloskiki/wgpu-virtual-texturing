struct VertexInput {
    @location(0) position: vec3<f32>,
    // @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct ViewProjection {
    mat: mat4x4<f32>,
}

//                                                    (0 for now)
// log2(feedback_attachement_width / window_width) + dynamic_lod_bias
@group(0) @binding(0)
var<uniform> feedback_lod_bias: f32;
// @group(1) @binding(0)
// var<uniform> view_projection: ViewProjection;

struct PrepassInterpolators {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_prepass(in: VertexInput) -> PrepassInterpolators {
    var out: PrepassInterpolators;
    out.position = // view_projection.mat *
                    vec4<f32>(in.position, 1.0);
    out.uv = in.uv;
    return out;
}

// From the uv, calculate the page index and mip level.
//
// Output Format: Rgba8Uint -> (page_x (8), page_y (8), mip_level (8), _padding);
//
// Reminder: page format = 128x128 (120 data, 4 padding on all sides).
// 
// TODO: Anisotropic filtering
@fragment
fn fs_prepass(in: PrepassInterpolators) -> @location(0) vec4<u32> {
    /// Hardcoded for now, but is this value known at compile time?
    let virtual_texture_page_width = 256;
    let page_texel_width = 128;
    let border_size = 4;
    let texel_width_per_page = page_texel_width - 2 * border_size;
    let virtual_texture_texel_width = texel_width_per_page * virtual_texture_page_width;

    let tex_coords = in.uv * f32(virtual_texture_texel_width);

    let dx = dpdx(tex_coords);
    let dy = dpdy(tex_coords);
    let px = dot(dx, dx);
    let py = dot(dy, dy);

    // let max_lod = 0.5 * log2(max(px, py)); for anisotropic filtering
    let min_lod = 0.5 * log2(min(px, py)); // log2(sqrt(...)) == 0.5 * log2(...)

    let desired_lod = max(min_lod + feedback_lod_bias, 0.0);
    let page_coords = vec2<u32>(in.uv * f32(virtual_texture_page_width));

    let out_texel = vec4<u32>(page_coords.x, page_coords.y, u32(desired_lod), 255u);
    return out_texel;
}

struct RenderInterpolators {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

@vertex
fn vs_render() -> RenderInterpolators {
    var result: RenderInterpolators;
    result.position = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    result.tex_coords = vec2<f32>(0.0, 0.0);
    return result;
}

@fragment
fn fs_render(in: RenderInterpolators) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}

struct DebugInterpolators {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

// meant to be called with 3 vertex indices: 0, 1, 2
// draws one large triangle over the clip space like this:
// (the asterisks represent the clip space bounds)
//-1,1           1,1
// ---------------------------------
// |              *              .
// |              *           .
// |              *        .
// |              *      .
// |              *    . 
// |              * .
// |***************
// |            . 1,-1 
// |          .
// |       .
// |     .
// |   .
// |.
@vertex
fn vs_debug_prepass(@builtin(vertex_index) vertex_index: u32) -> DebugInterpolators {
    var result: DebugInterpolators;
    let x = i32(vertex_index) / 2;
    let y = i32(vertex_index) & 1;
    let tc = vec2<f32>(
        f32(x) * 2.0,
        f32(y) * 2.0
    );
    result.position = vec4<f32>(
        tc.x * 2.0 - 1.0,
        1.0 - tc.y * 2.0,
        0.0,
        1.0
    );
    result.tex_coords = tc;
    return result;
}

@group(0) @binding(0)
var debug_tex: texture_2d<u32>;

// When I load the texture, it is 0.0, 0.0, 0.0, 0.0 for some reason.
@fragment
fn fs_debug_prepass(in: DebugInterpolators) -> @location(0) vec4<f32> {
    let texture_dims = vec2<f32>(textureDimensions(debug_tex));
    let texture_index = vec2<u32>(in.tex_coords * texture_dims);
    let texel = textureLoad(debug_tex, texture_index, 0);


    let color = vec4<f32>(
        // f32(texel.x) / 255,
        // f32(texel.y) / 255,
        in.tex_coords.x,
        in.tex_coords.y,
        0.0,
        1.0
        // f32(texel.w),
    );
    return color;
}
