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
@group(0) @binding(3)
var pager_texture: texture_2d<u32>;
@group(0) @binding(4)
var atlas_y_texture: texture_2d<f32>;
@group(0) @binding(5)
var atlas_cbcr_texture: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let uv = vec2<f32>(f32((in_vertex_index << 1u) & 2u), f32(in_vertex_index & 2u));
    out.screen_position = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let zoom = pow(2.0, f32(camera.zoom_steps) / f32(camera.steps_per_octave));
    let raw_exp = (camera.zoom_steps + i32(camera.steps_per_octave) - 1) / i32(camera.steps_per_octave);
    let tile_size = u32(clamp(pow(2.0, f32(raw_exp)), 1.0, 99999.0));
    let screen_centered = in.screen_position.xy - camera.viewport * 0.5;
    let world_pos = camera.center + screen_centered / zoom;

    let mosaic_dims = textureDimensions(mosaic_texture);
    if (world_pos.x < 0.0 || world_pos.y < 0.0 ||
        world_pos.x >= f32(mosaic_dims.x) || world_pos.y >= f32(mosaic_dims.y)) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0); //oob
    }

    //they're always the same size
    let x = u32(world_pos.x);
    let y = u32(world_pos.y);
    let dense_index = textureLoad(mosaic_texture, vec2(x, y), 0).r;

    let palette_pager_width = textureDimensions(palette_texture).x;
    let pager_entry = textureLoad(pager_texture, vec2(dense_index % palette_pager_width, dense_index / palette_pager_width), 0).rgba;
    if pager_entry.r == 0u || tile_size == 1u {
        return textureLoad(palette_texture, vec2(dense_index % palette_pager_width, dense_index / palette_pager_width), 0).rgba;
    } else {
        let decoded = pager_entry.r - 1u;
        let atlas_width = textureDimensions(atlas_y_texture).x;
        let tiles_per_row = atlas_width/tile_size;
        let offset_x = decoded % tiles_per_row;
        let offset_y = decoded / tiles_per_row;

        let local_frac = fract(world_pos);
        let local_offset = vec2<u32>(local_frac * f32(tile_size));
        let atlas_pixel_x = offset_x * tile_size + local_offset.x;
        let atlas_pixel_y = offset_y * tile_size + local_offset.y;
        let y_plane_pixel = textureLoad(atlas_y_texture, vec2(atlas_pixel_x, atlas_pixel_y), 0).r;
        let cbcr_plane_pixel = textureLoad(atlas_cbcr_texture, vec2(atlas_pixel_x/2u, atlas_pixel_y/2u), 0).rg;

        let cb = cbcr_plane_pixel.r - 0.5;
        let cr = cbcr_plane_pixel.g - 0.5;

        let r = y_plane_pixel + 1.402 * cr;
        let g = y_plane_pixel - 0.344136 * cb - 0.714136 * cr;
        let b = y_plane_pixel + 1.772 * cb;

        return vec4<f32>(r, g, b, 1.0);
    }
}