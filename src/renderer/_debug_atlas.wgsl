
struct VertexOutput {
    @builtin(position) screen_position: vec4<f32>,
    @location(0) atlas_uv: vec2<f32>,
};

struct CameraUniform {
    center: vec2<f32>,   ///mosaic space coordinate the camera looks at
    zoom_steps: i32,
    steps_per_octave: u32, ///usually just 30
    viewport: vec2<f32>, ///screen space dimensions
};

@group(0) @binding(0) 
var<uniform> camera: CameraUniform;
@group(0) @binding(1)
var atlas_y_texture: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 1.0),
    );
    let local = corners[in_vertex_index];

    let atlas_dims = vec2<f32>(textureDimensions(atlas_y_texture));
    let atlas_aspect = atlas_dims.x / atlas_dims.y;

    let scale_fraction = 0.33; 
    let box_h_px = camera.viewport.y * scale_fraction;
    let box_w_px = box_h_px * atlas_aspect;

    let box_w_ndc = (box_w_px / camera.viewport.x) * 2.0;
    let box_h_ndc = (box_h_px / camera.viewport.y) * 2.0;

    let ndc_x = (1.0 - box_w_ndc) + local.x * box_w_ndc;
    let ndc_y = (1.0 - box_h_ndc) + local.y * box_h_ndc;

    out.screen_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.atlas_uv = vec2<f32>(local.x, 1.0 - local.y);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dims = textureDimensions(atlas_y_texture);
    let x = u32(in.atlas_uv.x * f32(dims.x));
    let y = u32(in.atlas_uv.y * f32(dims.y));
    return vec4(textureLoad(atlas_y_texture, vec2(x, y), 0).rrr, 0.75);
}