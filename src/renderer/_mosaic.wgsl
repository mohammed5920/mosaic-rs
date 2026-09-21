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
var atlas_sampler: sampler;
@group(0) @binding(5)
var atlas_y_texture: texture_2d<f32>;
@group(0) @binding(6)
var atlas_cbcr_texture: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let uv = vec2<f32>(f32((in_vertex_index << 1u) & 2u), f32(in_vertex_index & 2u));
    out.screen_position = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

fn shade(world_pos: vec2<f32>, tile_size: f32, zoom: f32) -> vec3<f32> {
    let mosaic_dims = textureDimensions(mosaic_texture);
    if (world_pos.x < 0.0 || world_pos.y < 0.0 ||
        world_pos.x >= f32(mosaic_dims.x) || world_pos.y >= f32(mosaic_dims.y)) {
        return vec3<f32>(0.); //oob
    }

    let dense_index = textureLoad(mosaic_texture, vec2<u32>(world_pos), 0).r;
    let pager_width = textureDimensions(palette_texture).x;
    let pager_entry = textureLoad(pager_texture, vec2(dense_index % pager_width, dense_index / pager_width), 0).rgba;
    if pager_entry.r == 0u || tile_size == 1. {
        return textureLoad(palette_texture, vec2(dense_index % pager_width, dense_index / pager_width), 0).rgb;
    } 

    let decoded = pager_entry.r - 1u;
    let atlas_dims = textureDimensions(atlas_y_texture);
    let tiles_per_row = atlas_dims.x/u32(tile_size);
    let offset_x = decoded % tiles_per_row;
    let offset_y = decoded / tiles_per_row;

    let tile_start = vec2<f32>(f32(offset_x), f32(offset_y)) * tile_size;
    let tile_local_offset = fract(world_pos) * tile_size;

    //clamp the sampler so it doesnt blend between unrelated tiles
    let y_px = tile_start + clamp(tile_local_offset, vec2<f32>(0.5), vec2<f32>(tile_size - 0.5));
    //double the clamp range for the chroma range since it's half res
    let c_px = tile_start + clamp(tile_local_offset, vec2<f32>(1.0), vec2<f32>(tile_size - 1.0));

    let y_plane_pixel = textureSampleLevel(atlas_y_texture, atlas_sampler, y_px / vec2<f32>(atlas_dims), 0.0).r;
    let cbcr_plane_pixel  = textureSampleLevel(atlas_cbcr_texture, atlas_sampler, c_px / vec2<f32>(atlas_dims), 0.0).rg;

    let cb = cbcr_plane_pixel.r - 0.5;
    let cr = cbcr_plane_pixel.g - 0.5;

    let r = y_plane_pixel + 1.402 * cr;
    let g = y_plane_pixel - 0.344136 * cb - 0.714136 * cr;
    let b = y_plane_pixel + 1.772 * cb;
    var rgb = vec3(r,g,b);

    // for tile size = 2 (zoom start), slowly introduce the tiles to avoid heavy shimmering
    if tile_size == 2. {
        let avg_color = textureLoad(palette_texture, vec2(dense_index % pager_width, dense_index / pager_width), 0).rgb;
        rgb = mix(avg_color, rgb, log2(zoom));
    } 

    return rgb;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let zoom = pow(2.0, f32(camera.zoom_steps) / f32(camera.steps_per_octave));
    let raw_exponent = (camera.zoom_steps + i32(camera.steps_per_octave) - 1) / i32(camera.steps_per_octave);
    
    let tile_size = clamp(pow(2.0, f32(raw_exponent)), 1.0, 99999.0);
    let screen_centered = in.screen_position.xy - camera.viewport * 0.5;
    let world_pos = camera.center + screen_centered / zoom;

    //change to 4 for good ssaa at the cost of roughly 10x slower frametimes 
    let n = 1;
    let inverse_n = 1.0 / f32(n);
    var acc = vec3<f32>(0.0);
    for (var iy = 0; iy < n; iy++) {
        for (var ix = 0; ix < n; ix++) {
            let o = (vec2<f32>(f32(ix), f32(iy)) + 0.5) * inverse_n - 0.5;
            acc += shade(world_pos + o / zoom, tile_size, zoom);
        }
    }

    return vec4<f32>(acc * (inverse_n * inverse_n), 1.0);
}