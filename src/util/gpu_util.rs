use std::fs;

pub(crate) fn create_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    debug_name: &str,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    //data, bpp
    init: Option<(&[u8], u32)>,
) -> (wgpu::TextureView, wgpu::Texture) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(debug_name),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    if let Some((buff, bpp)) = init {
        write_texture(queue, &texture, buff, bpp, None);
    }
    (
        texture.create_view(&wgpu::TextureViewDescriptor::default()),
        texture,
    )
}

pub(crate) fn write_texture(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    data: &[u8],
    bpp: u32,
    height_limit: Option<usize>,
) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        height_limit
            .map(|hl| &data[..texture.width() as usize * bpp as usize * hl])
            .unwrap_or(data),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(bpp * texture.width()),
            rows_per_image: None,
        },
        wgpu::Extent3d {
            width: texture.width(),
            height: height_limit.unwrap_or(texture.height() as usize) as u32,
            depth_or_array_layers: 1,
        },
    );
}

pub(crate) fn copy_tile_into_atlas(
    dst: &mut [u8],
    src: &[u8],
    index: usize,
    tiles_per_row: usize,
    tile_size_px: usize,
    bpp: usize,
    atlas_width: usize,
) {
    let atlas_stride_bytes = atlas_width * bpp;
    let tile_size_bytes = tile_size_px * bpp;
    let dst_row_start = (index / tiles_per_row) * tile_size_px * atlas_stride_bytes;
    let dst_col_start = (index % tiles_per_row) * tile_size_bytes;

    for row in 0..tile_size_px {
        let src_off = row * tile_size_bytes;
        let dst_off = dst_row_start + dst_col_start + row * atlas_stride_bytes;
        dst[dst_off..dst_off + tile_size_bytes]
            .copy_from_slice(&src[src_off..src_off + tile_size_bytes]);
    }
}

pub(crate) fn create_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    shader_path: &str,
    shader_debug_name: &str,
    pipeline_debug_name: &str,
) -> wgpu::RenderPipeline {
    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(shader_debug_name),
        source: wgpu::ShaderSource::Wgsl(
            fs::read_to_string(shader_path)
                .unwrap_or_else(|e| format!("cannot read shader at {shader_path} because of {e}"))
                .into(),
        ),
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(pipeline_debug_name),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader_module,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader_module,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}
