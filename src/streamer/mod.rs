use std::{collections::HashMap, sync::Arc};

use rustc_hash::{FxBuildHasher, FxHashMap};
use sysinfo::{MemoryRefreshKind, RefreshKind, System};
use winit::dpi::PhysicalSize;

use crate::{
    config::CONFIG,
    mosaic::{Mosaic, tiles::Tile},
    streamer::tile_store::TileStore,
};

mod tile_store;

// palette -> static
// mosaic texture -> every video frame / static
// atlas -> every frame
// texture views -> every frame
// page table -> every frame

pub(crate) struct Streamer {
    pub(crate) palette_view: wgpu::TextureView,
    palette_texture: wgpu::Texture,
    tiles: Arc<[Tile]>,
    stores: FxHashMap<u64, TileStore>,
    res_limit: u64,
    fast_limit: u64,
}

impl Streamer {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &mut wgpu::Queue,
        mosaic: &Mosaic,
        display_res: PhysicalSize<u32>,
        total_frames: u64,
        mean_tile_len: u64,
    ) -> Self {
        // partition the stores using the same python strategy
        let res_limit = 2u64.pow(
            (display_res.width.min(display_res.height) as f64)
                .log2()
                .floor() as u32,
        );
        //size of 2 arrays at current size + tile dictionary at 4x(default) the screen resolution * avg no. of frames per tile
        let guess_ram_usage = |dim: f64| {
            (2.0 * dim * dim * total_frames as f64 * 1.5
                + CONFIG.prefetch_multiplier.get() as f64
                    * mean_tile_len as f64
                    * display_res.width as f64
                    * display_res.height as f64
                    * 1.5)
                .ceil() as i64
        };

        let mut available_bytes = (System::new_with_specifics(
            RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()),
        )
        .free_memory() as f64
            * (CONFIG.ram_percent as f64 / 100.0)) as i64;

        let mut tile_stores = HashMap::<u64, TileStore, _>::with_hasher(FxBuildHasher);
        let mut fast_limit = 1u64;
        for exponent in 1u32..(res_limit as f64).log2() as u32 + 1 {
            let raised = 2u64.pow(exponent);
            let guessed = guess_ram_usage(raised as f64);
            if available_bytes - guessed >= 0 {
                tile_stores.insert(raised, TileStore::new(raised, total_frames, false));
                available_bytes -=
                    (raised as f64 * raised as f64 * 1.5 * total_frames as f64).ceil() as i64;
                fast_limit = raised;
            } else {
                tile_stores.insert(raised, TileStore::new(raised, total_frames, true));
            }
        }

        //create the palette texture
        let palette_raw = mosaic.generate_palette();
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

        //also write the palette texture
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
        let tiles = mosaic.tiles();
        Self {
            palette_texture,
            palette_view,
            tiles,
            stores: tile_stores,
            res_limit,
            fast_limit,
        }
    }

    pub(crate) fn stream(&mut self) {}
}
