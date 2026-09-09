use std::sync::Arc;

use crate::{
    mosaic::{Mosaic, tiles::Tile},
    types::DenseIndex,
};

mod tile_store;

// palette -> static
// mosaic texture -> every video frame / static
// atlas -> every frame
// texture views -> every frame
// page table -> every frame

pub(crate) struct Streamer {
    pub(crate) palette_view: wgpu::TextureView,
    pub(crate) mosaic_view: wgpu::TextureView,
    mosaic_texture: wgpu::Texture,
    palette_texture: wgpu::Texture,
    tiles: Arc<[Tile]>,
}

impl Streamer {
    pub(crate) fn initialise(
        device: &wgpu::Device,
        queue: &mut wgpu::Queue,
        mosaic: &Mosaic,
    ) -> Self {
        let tiles = mosaic.tiles();

        //create the mosaic texture
        let mosaic_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("saic_Mosaic Texture"),
            size: wgpu::Extent3d {
                width: mosaic.width() as u32,
                height: mosaic.height() as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let mosaic_view = mosaic_texture.create_view(&wgpu::TextureViewDescriptor::default());

        //create & write the palette texture
        let palette_raw = match mosaic {
            Mosaic::StaticMosaic {
                unique_matches,
                dense_map,
                tiles,
                ..
            } => {
                let padded_len = (unique_matches.len() as f64).sqrt().ceil().powi(2) as usize;
                let mut res = Vec::with_capacity(padded_len);
                res.extend((0..padded_len).map(|_| [0u8; 4]));
                for match_index in unique_matches.iter() {
                    let [r, g, b] = tiles[match_index.0 as usize].average_colour();
                    res[dense_map[match_index.0 as usize].0 as usize] = [r, g, b, 255];
                }
                res
            }
            Mosaic::DynamicMosaic => todo!(),
        };
        let palette_dim = (palette_raw.len() as f64).sqrt() as u32;
        let palette_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("saic_Palette Texture"),
            size: wgpu::Extent3d {
                width: palette_dim,
                height: palette_dim,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let palette_view = palette_texture.create_view(&wgpu::TextureViewDescriptor::default());

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &palette_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&palette_raw),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * palette_dim),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: palette_dim,
                height: palette_dim,
                depth_or_array_layers: 1,
            },
        );

        Self {
            mosaic_texture,
            mosaic_view,
            palette_texture,
            palette_view,
            tiles,
        }
    }

    pub(crate) fn update(&mut self, queue: &mut wgpu::Queue, frame_matches: &[DenseIndex]) {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.mosaic_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(frame_matches),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * self.mosaic_texture.width()),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: self.mosaic_texture.width(),
                height: self.mosaic_texture.height(),
                depth_or_array_layers: 1,
            },
        );
    }
}
