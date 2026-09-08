struct VertexOutput {
    @builtin(position) screen_position: vec4<f32>,
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
var mosaic_texture: texture_2d<u32>;
@group(0) @binding(2)
var palette_texture: texture_2d<f32>;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let zoom = pow(2.0, f32(camera.zoom_steps) / f32(camera.steps_per_octave));

    let screen_centered = in.screen_position.xy - camera.viewport * 0.5;
    let world_pos = camera.center + screen_centered / zoom;

    let mosaic_dims = textureDimensions(mosaic_texture);
    if (world_pos.x < 0.0 || world_pos.y < 0.0 ||
        world_pos.x >= f32(mosaic_dims.x) || world_pos.y >= f32(mosaic_dims.y)) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0); //oob
    }

    let x = u32(world_pos.x);
    let y = u32(world_pos.y);
    let dense_index = textureLoad(mosaic_texture, vec2(x, y), 0).r;

    let palette_width = textureDimensions(palette_texture).x;
    return textureLoad(palette_texture, vec2(dense_index % palette_width, dense_index / palette_width), 0).rgba;
}