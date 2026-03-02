// Terminal cell renderer shader.
//
// Renders cell backgrounds (as colored quads) and text glyphs using:
// - Bitmap mode: grayscale alpha from texture atlas
// - MSDF mode: multi-channel signed distance field for resolution-independent text

struct Uniforms {
    projection: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(1) @binding(0)
var atlas_texture: texture_2d<f32>;

@group(1) @binding(1)
var atlas_sampler: sampler;

// Per-instance vertex input.
struct VertexInput {
    // Quad corner (0-3)
    @builtin(vertex_index) vertex_index: u32,

    // Instance data
    @location(0) cell_pos: vec2<f32>,    // cell position (pixels)
    @location(1) cell_size: vec2<f32>,   // cell size (pixels)
    @location(2) fg_color: vec4<f32>,    // foreground color
    @location(3) bg_color: vec4<f32>,    // background color
    @location(4) uv_offset: vec2<f32>,   // atlas UV offset (normalized)
    @location(5) uv_size: vec2<f32>,     // atlas UV size (normalized)
    @location(6) glyph_offset: vec2<f32>, // glyph offset within cell
    @location(7) glyph_size: vec2<f32>,  // glyph size in pixels
    @location(8) glyph_mode: f32,        // 0.0=bg-only, 1.0=bitmap, 2.0=MSDF
    @location(9) msdf_px_range: f32,     // MSDF screen pixel range (0 for bitmap)
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) fg_color: vec4<f32>,
    @location(1) bg_color: vec4<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) glyph_mode: f32,
    @location(4) msdf_px_range: f32,
};

@vertex
fn vs_background(input: VertexInput) -> VertexOutput {
    // Generate quad vertices (2 triangles = 6 vertices, but we use 4 + index)
    let corners = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 0.0), // top-left
        vec2<f32>(1.0, 0.0), // top-right
        vec2<f32>(0.0, 1.0), // bottom-left
        vec2<f32>(1.0, 1.0), // bottom-right
    );

    let corner = corners[input.vertex_index];
    let pos = input.cell_pos + corner * input.cell_size;

    var output: VertexOutput;
    output.position = uniforms.projection * vec4<f32>(pos, 0.0, 1.0);
    output.fg_color = input.fg_color;
    output.bg_color = input.bg_color;
    output.uv = vec2<f32>(0.0, 0.0);
    output.glyph_mode = 0.0;
    output.msdf_px_range = 0.0;
    return output;
}

@fragment
fn fs_background(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.bg_color;
}

@vertex
fn vs_glyph(input: VertexInput) -> VertexOutput {
    let corners = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
    );

    let corner = corners[input.vertex_index];
    let pos = input.cell_pos + input.glyph_offset + corner * input.glyph_size;
    let uv = input.uv_offset + corner * input.uv_size;

    var output: VertexOutput;
    output.position = uniforms.projection * vec4<f32>(pos, 0.0, 1.0);
    output.fg_color = input.fg_color;
    output.bg_color = input.bg_color;
    output.uv = uv;
    output.glyph_mode = input.glyph_mode;
    output.msdf_px_range = input.msdf_px_range;
    return output;
}

/// Median of three values — extracts the signed distance from MSDF channels.
fn median3(r: f32, g: f32, b: f32) -> f32 {
    return max(min(r, g), min(max(r, g), b));
}

@fragment
fn fs_glyph(input: VertexOutput) -> @location(0) vec4<f32> {
    let atlas_sample = textureSample(atlas_texture, atlas_sampler, input.uv);

    var alpha: f32;
    if input.glyph_mode > 1.5 {
        // MSDF mode: compute signed distance from RGB channels
        let sd = median3(atlas_sample.r, atlas_sample.g, atlas_sample.b);
        let screen_px_distance = input.msdf_px_range * (sd - 0.5);
        alpha = clamp(screen_px_distance + 0.5, 0.0, 1.0);
    } else {
        // Bitmap mode: use alpha channel directly
        alpha = atlas_sample.a;
    }

    return vec4<f32>(input.fg_color.rgb, input.fg_color.a * alpha);
}
