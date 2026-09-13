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
            width: width,
            height: height,
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
        write_texture(queue, &texture, buff, bpp);
    }
    (
        texture.create_view(&wgpu::TextureViewDescriptor::default()),
        texture,
    )
}

pub(crate) fn write_texture(queue: &wgpu::Queue, texture: &wgpu::Texture, data: &[u8], bpp: u32) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(bpp * texture.width()),
            rows_per_image: None,
        },
        wgpu::Extent3d {
            width: texture.width(),
            height: texture.height(),
            depth_or_array_layers: 1,
        },
    );
}
