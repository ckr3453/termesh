// Terminal cell background renderer shader.
//
// Renders cell backgrounds, cursor blocks, and pane dividers as colored quads.
// Text rendering is handled by glyphon.

struct Uniforms {
    projection: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// Per-instance vertex input.
struct VertexInput {
    // Quad corner (0-3)
    @builtin(vertex_index) vertex_index: u32,

    // Instance data
    @location(0) cell_pos: vec2<f32>,    // cell position (pixels)
    @location(1) cell_size: vec2<f32>,   // cell size (pixels)
    @location(2) bg_color: vec4<f32>,    // background color
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) bg_color: vec4<f32>,
};

@vertex
fn vs_background(input: VertexInput) -> VertexOutput {
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
    output.bg_color = input.bg_color;
    return output;
}

@fragment
fn fs_background(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.bg_color;
}
