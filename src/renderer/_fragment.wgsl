struct VertexOutput {
    @builtin(position) screen_position: vec4<f32>,
};

@group(0) @binding(0)
var mosaic_texture: texture_2d<u32>;
@group(0) @binding(1)
var palette_texture: texture_2d<f32>;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let x = u32(in.screen_position.x);
    let y = u32(in.screen_position.y);
    let dense_index = textureLoad(mosaic_texture, vec2(x, y), 0).r;

    let palette_width = textureDimensions(palette_texture).x;
    return textureLoad(palette_texture, vec2(dense_index % palette_width, dense_index / palette_width), 0).rgba;
}