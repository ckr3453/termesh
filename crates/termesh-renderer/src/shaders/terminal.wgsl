// Terminal cell renderer shader.
//
// Renders both cell backgrounds (as colored quads) and
// text glyphs (from a texture atlas) in a single pass.

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
    @location(8) has_glyph: f32,         // 1.0 if has glyph, 0.0 for bg-only
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) fg_color: vec4<f32>,
    @location(1) bg_color: vec4<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) is_glyph: f32,
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
    output.is_glyph = 0.0;
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
    output.is_glyph = 1.0;
    return output;
}

@fragment
fn fs_glyph(input: VertexOutput) -> @location(0) vec4<f32> {
    let atlas_sample = textureSample(atlas_texture, atlas_sampler, input.uv);
    let alpha = atlas_sample.a;
    return vec4<f32>(input.fg_color.rgb, input.fg_color.a * alpha);
}
