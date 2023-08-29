struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
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
// Output Format: Rgba8Uint -> (R: page_x_big (8), G: page_x_little (6) page_y_big (2),
//                              B: page_y_mid (8), A: page_y_little (4) page_ mip_level (4))
//
// Reminder: page format = 128x128 (120 data, 4 padding on all sides).
@fragment
fn fs_prepass(in: PrepassInterpolators) -> @location(0) vec4<u32> {
    /// Hardcoded for now, but is these values could be variables with naga_oil.
    let max_anisotropic_samples = 4.;
    let max_anisotropic_log2 = 2.; // log2(4) = 2;
    let virtual_texture_page_width = 16384;
    let page_texel_width = 128;
    let border_size = 4;
    let texel_width_per_page = page_texel_width - 2 * border_size;
    let virtual_texture_texel_width = texel_width_per_page * virtual_texture_page_width;

    let tex_coords = in.uv * f32(virtual_texture_texel_width);

    let dx = dpdx(tex_coords);
    let dy = dpdy(tex_coords);
    let px = dot(dx, dx);
    let py = dot(dy, dy);

    let max_lod = 0.5 * log2(max(px, py)); // log2(sqrt(...)) == 0.5 * log2(...)
    let min_lod = 0.5 * log2(min(px, py)); 

    let aniso_lod = max_lod - max(max_lod - min_lod, f32(max_anisotropic_log2));
    let desired_lod = max(aniso_lod + feedback_lod_bias, 0.0);
    let page_coords = vec2<u32>(in.uv * f32(virtual_texture_page_width));

    return feedback_to_rgba(page_coords, u32(round(desired_lod)));
}

// ==============
// Debug Prepass
// ==============

// Render the contents of the feedback buffer to the screen.

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

@fragment
fn fs_debug_prepass(in: DebugInterpolators) -> @location(0) vec4<f32> {
    let texture_dims = vec2<f32>(textureDimensions(debug_tex));
    let texture_index = vec2<u32>(in.tex_coords * texture_dims);
    let texel = textureLoad(debug_tex, texture_index, 0);

    let color = vec4<f32>(
        f32(texel.x) / 255.,
        f32(texel.y) / 255.,
        f32(texel.z) / 255.,
        1.0,
    );
    return color;
}

// Output Format: Rgba8Uint -> (R: page_x_big (8), G: page_x_little (6) page_y_big (2),
//                              B: page_y_mid (8), A: page_y_little (4) page_ mip_level (4))
fn feedback_to_rgba(page_coords: vec2<u32>, mip: u32) -> vec4<u32> {
    let page_x_upper = page_coords.x >> 6u; // upper 8 bits of 14 bits int
    let page_x_lower = page_coords.x & 0x3Fu; // lower 6 bits of 14 bits int
    let page_y_upper = page_coords.y >> 12u; // upper 2 bits of 14 bits int
    let page_y_mid = (page_coords.y >> 4u) & 0xFFu; // middle 8 bits of 14 bits int
    let page_y_lower = page_coords.y & 0xFu; // lower 4 bits of 14 bits int
    let r = page_x_upper;
    let g = (page_x_lower << 2u) | page_y_upper;
    let b = page_y_mid;
    let a = (page_y_lower << 4u) | mip;

    return vec4<u32>(r, g, b, a);
}
