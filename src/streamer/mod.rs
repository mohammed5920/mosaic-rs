use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread,
};

use parking_lot::Mutex;
use rustc_hash::{FxBuildHasher, FxHashSet};
use sysinfo::{MemoryRefreshKind, RefreshKind, System};
use winit::dpi::PhysicalSize;

use crate::{
    camera::AppCameraWrapper,
    config::CONFIG,
    mosaic::{Mosaic, tiles::Tile},
    streamer::{
        bg_thread::{StreamingMessage, streamer_thread},
        tile_stores::TileStore,
    },
    types::{Bb, DenseIndex},
    util::bb_util::subtract_rect,
};

mod bg_thread;
pub(crate) mod tile_stores;

// palette -> static
// mosaic texture -> every video frame / static
// atlas -> every frame
// texture views -> every frame
// page table -> every frame

pub(crate) struct Streamer {
    pub(crate) palette_view: wgpu::TextureView,
    stores: Arc<[TileStore]>,
    res_limit: u64,
    fast_limit: u64,

    ram_usage_bytes: u64,
    ram_limit_bytes: u64,

    ///where index is a DenseIndex, value is refcount visible on screen
    visibility_map: Vec<u64>,
    visibility_set: Arc<Mutex<FxHashSet<DenseIndex>>>,
    last_frame_visible_bb: Option<Bb>,
    is_dirty: bool,

    stream_thread_sender: Sender<StreamingMessage>,
    stream_thread_reciever: Receiver<StreamingMessage>,
    stream_kill_flag: Arc<AtomicBool>,
}

impl Streamer {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &mut wgpu::Queue,
        mosaic: &Mosaic,
        monitor_res: PhysicalSize<u32>,
        total_frames: u64,
        mean_tile_len: u64,
        res_limit: u64,
    ) -> Self {
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

        //size of 2 arrays at current size + tile dictionary at 4x(default) the screen resolution * avg no. of frames per tile
        let guess_ram_usage = |dim: f64| {
            (2.0 * dim * dim * total_frames as f64 * 1.5
                + CONFIG.prefetch_multiplier.get() as f64
                    * mean_tile_len as f64
                    * monitor_res.width as f64
                    * monitor_res.height as f64
                    * 1.5)
                .ceil() as i64
        };

        let mut available_bytes = (System::new_with_specifics(
            RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()),
        )
        .free_memory() as f64
            * (CONFIG.ram_percent as f64 / 100.0)) as i64;
        let ram_limit_bytes = available_bytes as u64;

        let mut tile_stores = Vec::new();
        let mut fast_limit = 1u64;
        for exponent in 1u32..(res_limit as f64).log2() as u32 + 1 {
            let raised = 2u64.pow(exponent);
            let guessed = guess_ram_usage(raised as f64);
            if available_bytes - guessed >= 0 {
                tile_stores.push(TileStore::new(raised, total_frames, false));
                available_bytes -=
                    (raised as f64 * raised as f64 * 1.5 * total_frames as f64).ceil() as i64;
                fast_limit = raised;
            } else {
                tile_stores.push(TileStore::new(raised, total_frames, true));
            }
        }

        let kill_flag: Arc<_> = AtomicBool::new(false).into();
        let vis_set: Arc<Mutex<_>> = Mutex::new(HashSet::with_hasher(FxBuildHasher)).into();
        let tile_stores: Arc<[TileStore]> = tile_stores.into();

        //for communicating to the thread
        let (to_tx, to_rx) = mpsc::channel();
        //for hearing back from the thread
        let (from_tx, from_rx) = mpsc::channel();

        let flag_clone = kill_flag.clone();
        let set_clone = vis_set.clone();
        let stores_clone = tile_stores.clone();
        let tiles_clone = mosaic.tiles();
        thread::spawn(move || {
            streamer_thread(
                to_rx,
                from_tx,
                flag_clone,
                set_clone,
                stores_clone,
                tiles_clone,
                fast_limit,
                res_limit,
            )
        });

        Self {
            res_limit,
            fast_limit,
            palette_view,
            ram_limit_bytes,
            is_dirty: false,
            stores: tile_stores,
            visibility_set: vis_set,
            last_frame_visible_bb: None,
            stream_kill_flag: kill_flag,
            stream_thread_sender: to_tx,
            stream_thread_reciever: from_rx,
            ram_usage_bytes: ram_limit_bytes - available_bytes as u64,
            visibility_map: mosaic.tiles().iter().map(|_| 0).collect(),
        }
    }

    ///call while zooming *in* if transitioning to a new LOD level
    pub(crate) fn on_lod_change(&mut self) {
        self.stream_kill_flag.store(true, Ordering::Relaxed);
        self.is_dirty = true;
    }

    ///call when shutting down, or the bg thread panics when the main thread exits
    pub(crate) fn shutdown(&self) {
        let _ = self.stream_thread_sender.send(StreamingMessage::Shutdown);
    }

    pub(crate) fn update_visibility(&mut self, camera: &AppCameraWrapper, mosaic: &Mosaic) {
        if !camera.is_zoomed_in() {
            return;
        }
        let new_bb = camera.calc_visible_mosaic_bounding_box(mosaic);
        let mut new_acc = Vec::new();
        let mut old_acc = Vec::new();
        match (self.last_frame_visible_bb, new_bb) {
            (None, None) => return,
            (Some(old_bb), None) => {
                old_acc.extend_from_slice(&mosaic.slice_bb_from_dense((old_bb.0, old_bb.1)));
                self.last_frame_visible_bb = None;
            }
            (None, Some(new_bb)) => {
                new_acc.extend_from_slice(&mosaic.slice_bb_from_dense((new_bb.0, new_bb.1)));
                self.last_frame_visible_bb = Some(new_bb);
            }
            (Some(old_bb), Some(new_bb)) => {
                for r in subtract_rect(old_bb, new_bb) {
                    old_acc.extend_from_slice(&mosaic.slice_bb_from_dense((r.0, r.1)));
                }
                for r in subtract_rect(new_bb, old_bb) {
                    new_acc.extend_from_slice(&mosaic.slice_bb_from_dense((r.0, r.1)));
                }
                self.last_frame_visible_bb = Some(new_bb);
            }
        };
        let mut set_guard = self.visibility_set.lock();
        for k in new_acc.iter() {
            let prev = self
                .visibility_map
                .get_mut(k.0 as usize)
                .expect("index is not in visibility map (initialiser broken?)");
            //eat the cost of a hash only on fresh tile
            if *prev == 0 {
                //will flag when panning, resizing and zooming out
                self.is_dirty = true;
                set_guard.insert(*k);
            }
            *prev += 1;
        }

        for k in old_acc.iter() {
            let prev = self
                .visibility_map
                .get_mut(k.0 as usize)
                .expect("index is not in visibility map (initialiser broken?)");
            //eat the cost of a hash only on stale tile
            if *prev == 1 {
                set_guard.remove(k);
            }
            *prev -= 1;
        }
    }

    ///call every frame to populate the tiles
    pub(crate) fn check_refresh(&mut self, tile_size: u64) {
        if !self.is_dirty || tile_size <= 1 {
            return;
        }
        self.is_dirty = false;

        match self.stream_thread_reciever.try_recv() {
            Ok(StreamingMessage::Init) => {
                //skip the check for the very first cycle
            }
            Ok(StreamingMessage::CycleEnd { after_ram_bytes }) => {
                self.ram_usage_bytes = after_ram_bytes
            }
            Err(TryRecvError::Empty) => {
                self.is_dirty = true;
                return;
            }
            Ok(_) => {
                panic!("streamer recieved invalid flag from bg thread")
            }
            Err(TryRecvError::Disconnected) => {
                panic!("streamer thread should keep bg channel open")
            }
        }

        self.stream_kill_flag.store(false, Ordering::Relaxed);
        self.stream_thread_sender
            .send(StreamingMessage::CycleStart {
                tile_size,
                before_ram_bytes: self.ram_usage_bytes,
                ram_limit_bytes: self.ram_limit_bytes,
            })
            .expect("streamer thread should keep bg channel open");
    }
}
