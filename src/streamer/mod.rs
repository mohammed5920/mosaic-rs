use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
};

use parking_lot::RwLock;
use rustc_hash::{FxBuildHasher, FxHashSet};
use sysinfo::{MemoryRefreshKind, RefreshKind, System};
use winit::dpi::PhysicalSize;

use crate::{
    camera::AppCameraWrapper,
    config::CONFIG,
    mosaic::Mosaic,
    streamer::{
        bg_thread::{StreamingMessage, streamer_thread},
        tile_stores::{TileStore, tile_size_to_store_index},
    },
    types::{Bb, DenseIndex, StoreIndex},
    util::{
        bb_util::subtract_rect,
        gpu_util::{copy_tile_into_atlas, create_texture, write_texture},
    },
};

mod bg_thread;
pub(crate) mod tile_stores;

pub(crate) struct Streamer {
    ///pager encoding: 0 = not resident, > 0 = modulo'd index into atlas, + 1
    pager_buff: Vec<u32>,
    atlas_y_buff: Vec<u8>,
    atlas_cbcr_buff: Vec<u8>,
    pager_texture: wgpu::Texture,
    atlas_y_texture: wgpu::Texture,
    atlas_cbcr_texture: wgpu::Texture,
    pub(crate) pager_view: wgpu::TextureView,
    pub(crate) atlas_y_view: wgpu::TextureView,
    pub(crate) atlas_cbcr_view: wgpu::TextureView,

    ///where index is a DenseIndex, value is refcount visible on screen
    visibility_map: Vec<u64>,
    visibility_set: Arc<RwLock<FxHashSet<DenseIndex>>>,
    last_frame_visible_bb: Option<Bb>,
    is_atlas_dirty: bool,
    is_streaming_dirty: bool,
    is_streaming_busy: bool,

    stores: Arc<[TileStore]>,
    ram_usage_bytes: u64,
    ram_limit_bytes: u64,
    stream_thread_sender: Sender<StreamingMessage>,
    stream_thread_receiver: Receiver<StreamingMessage>,
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
        let tiles = mosaic.tiles();

        //create pager texture
        let pager_dim = (tiles.len() as f64).sqrt().ceil() as u32;
        let (pager_view, pager_texture) = create_texture(
            device,
            queue,
            "saic_Pager Texture",
            pager_dim,
            pager_dim,
            wgpu::TextureFormat::R32Uint,
            None,
        );

        //create atlas texture
        //atlas is as big as (ceil(monitor res / tile size) + 1) * res_limit
        //adjust monitor res for tile supersampling
        let atlas_width = ((monitor_res.width as f64 * 2.0 / res_limit as f64).ceil() + 1.0)
            as usize
            * res_limit as usize;
        let atlas_height = ((monitor_res.height as f64 * 2.0 / res_limit as f64).ceil() + 1.0)
            as usize
            * res_limit as usize;
        let (atlas_y_view, atlas_y_texture) = create_texture(
            device,
            queue,
            "saic_Atlas Y Texture",
            atlas_width as u32,
            atlas_height as u32,
            wgpu::TextureFormat::R8Unorm,
            None,
        );
        let (atlas_cbcr_view, atlas_cbcr_texture) = create_texture(
            device,
            queue,
            "saic_Atlas Cb_Cr Texture",
            atlas_width as u32 / 2,
            atlas_height as u32 / 2,
            wgpu::TextureFormat::Rg8Unorm,
            None,
        );

        //create cpu side copies of each atlas
        let pager_buff = vec![0u32; (pager_texture.width() * pager_texture.height()) as usize];
        let atlas_y_buff = vec![0u8; (atlas_y_texture.width() * atlas_y_texture.height()) as usize];
        let atlas_cbcr_buff =
            vec![0u8; (atlas_cbcr_texture.width() * atlas_cbcr_texture.height()) as usize * 2];

        //initialise stores
        //size of 2 arrays at current size + tile dictionary at 4x(default) the screen resolution * avg no. of frames per tile
        //adjust monitor res for tile supersampling
        let guess_ram_usage = |dim: f64| {
            (2.0 * dim * dim * total_frames as f64 * 1.5
                + CONFIG.prefetch_multiplier.get() as f64
                    * mean_tile_len as f64
                    * monitor_res.width as f64
                    * monitor_res.height as f64
                    * 3.0)
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
        let mut dense_map = Vec::with_capacity(tiles.len());
        let mut offset = 0;
        for tile in tiles.iter() {
            dense_map.push(StoreIndex(offset));
            offset += tile.frame_count() as i32;
        }
        let dense_map: Arc<[StoreIndex]> = dense_map.into();
        for exponent in 1u32..(res_limit as f64).log2() as u32 + 1 {
            let raised = 2u64.pow(exponent);
            let guessed = guess_ram_usage(raised as f64);
            if available_bytes - guessed >= 0 {
                tile_stores.push(TileStore::new(mosaic, dense_map.clone(), raised, false));
                available_bytes -=
                    (raised as f64 * raised as f64 * 1.5 * total_frames as f64).ceil() as i64;
                fast_limit = raised;
            } else {
                tile_stores.push(TileStore::new(mosaic, dense_map.clone(), raised, true));
            }
        }

        //initialise bg thread
        let kill_flag: Arc<_> = AtomicBool::new(false).into();
        let vis_set: Arc<RwLock<_>> = RwLock::new(HashSet::with_hasher(FxBuildHasher)).into();
        let tile_stores: Arc<[TileStore]> = tile_stores.into();

        //for communicating to the thread
        let (to_tx, to_rx) = mpsc::channel();
        //for hearing back from the thread
        let (from_tx, from_rx) = mpsc::channel();

        let flag_clone = kill_flag.clone();
        let set_clone = vis_set.clone();
        let stores_clone = tile_stores.clone();
        let tiles_clone = mosaic.tiles();
        rayon::spawn(move || {
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
            pager_view,
            pager_buff,
            atlas_y_view,
            atlas_y_buff,
            pager_texture,
            atlas_y_texture,
            ram_limit_bytes,
            atlas_cbcr_view,
            atlas_cbcr_buff,
            atlas_cbcr_texture,
            stores: tile_stores,
            is_atlas_dirty: false,
            visibility_set: vis_set,
            is_streaming_busy: true,
            is_streaming_dirty: false,
            last_frame_visible_bb: None,
            stream_kill_flag: kill_flag,
            stream_thread_sender: to_tx,
            stream_thread_receiver: from_rx,
            ram_usage_bytes: ram_limit_bytes - available_bytes as u64,
            visibility_map: mosaic.tiles().iter().map(|_| 0).collect(),
        }
    }

    ///call while zooming if transitioning to a new LOD level
    pub(crate) fn on_lod_change(&mut self) {
        self.stream_kill_flag.store(true, Ordering::Relaxed);
        self.is_atlas_dirty = true;
        self.is_streaming_dirty = true;
    }

    pub(crate) fn on_tile_offset_change(&mut self) {
        self.is_atlas_dirty = true;
    }

    pub(crate) fn on_mosaic_frame_change(&mut self, camera: &AppCameraWrapper, mosaic: &Mosaic) {
        self.is_streaming_dirty = true;
        self.is_atlas_dirty = true;
        // self.stream_kill_flag.store(true, Ordering::Relaxed);
        if self.last_frame_visible_bb.is_some() {
            self.visibility_set.write().drain();
            self.visibility_map.iter_mut().for_each(|v| *v = 0);
        }
        self.last_frame_visible_bb = None;
        self.update_visibility(camera, mosaic);
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
        let mut set_guard = self.visibility_set.write();
        for k in new_acc.iter() {
            let prev = self
                .visibility_map
                .get_mut(k.0 as usize)
                .expect("index is not in visibility map (initialiser broken?)");
            //eat the cost of a hash only on fresh tile
            if *prev == 0 {
                //will flag when panning or resizing
                self.is_atlas_dirty = true;
                self.is_streaming_dirty = true;
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
        if self.is_streaming_busy {
            match self.stream_thread_receiver.try_recv() {
                //skip the check for the very first cycle
                Ok(StreamingMessage::Init) => self.is_streaming_busy = false,
                //thread is busy
                Err(TryRecvError::Empty) => return,
                //thread finished
                Ok(StreamingMessage::CycleEnd {
                    after_ram_bytes,
                    did_onscreen_tiles_change,
                }) => {
                    self.is_streaming_busy = false;
                    self.ram_usage_bytes = after_ram_bytes;
                    self.is_atlas_dirty = self.is_atlas_dirty || did_onscreen_tiles_change;
                }
                Ok(invalid_message) => {
                    panic!("streamer recieved {invalid_message:?} from bg thread")
                }
                Err(TryRecvError::Disconnected) => {
                    panic!("streamer thread should keep bg channel open")
                }
            }
        }

        if !self.is_streaming_busy && self.is_streaming_dirty && tile_size >= 2 {
            self.is_streaming_dirty = false;
            self.is_streaming_busy = true;
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

    pub(crate) fn write_atlas(&mut self, queue: &wgpu::Queue, tile_size: u64, frame_offset: u64) {
        let tile_size = tile_size as usize;
        if !self.is_atlas_dirty || tile_size <= 1 {
            return;
        }
        self.is_atlas_dirty = false;
        let visible_guard = self.visibility_set.read();
        let mut resident_count = 0;

        let y_width = self.atlas_y_texture.width() as usize;
        let y_tiles_per_row = y_width / tile_size;

        let cbcr_bpp = 2;
        let cbcr_tile_size_px = tile_size / 2;
        let cbcr_width = self.atlas_cbcr_texture.width() as usize;
        let cbcr_tiles_per_row = self.atlas_cbcr_texture.width() as usize / cbcr_tile_size_px;

        self.stores[tile_size_to_store_index(tile_size as u64)].with_tiles(
            visible_guard.iter().copied(),
            frame_offset,
            |results| {
                for (tile_index, result) in results {
                    match result {
                        tile_stores::ReadTileResult::Vacant => {
                            self.pager_buff[tile_index.0 as usize] = 0u32;
                        }
                        tile_stores::ReadTileResult::Resident { y, cb_cr } => {
                            self.pager_buff[tile_index.0 as usize] = (resident_count as u32) + 1;

                            copy_tile_into_atlas(
                                &mut self.atlas_y_buff,
                                y,
                                resident_count,
                                y_tiles_per_row,
                                tile_size,
                                1,
                                y_width,
                            );
                            copy_tile_into_atlas(
                                &mut self.atlas_cbcr_buff,
                                cb_cr,
                                resident_count,
                                cbcr_tiles_per_row,
                                cbcr_tile_size_px,
                                cbcr_bpp,
                                cbcr_width,
                            );

                            resident_count += 1;
                        }
                    }
                }
            },
        );

        write_texture(
            queue,
            &self.pager_texture,
            bytemuck::cast_slice(&self.pager_buff),
            4,
            None,
        );
        if resident_count > 0 {
            let tiles_per_row = self.atlas_y_texture.width() as usize / tile_size;
            let height_limit = resident_count.div_ceil(tiles_per_row) * tile_size;
            write_texture(
                queue,
                &self.atlas_y_texture,
                bytemuck::cast_slice(&self.atlas_y_buff),
                1,
                Some(height_limit),
            );
            write_texture(
                queue,
                &self.atlas_cbcr_texture,
                bytemuck::cast_slice(&self.atlas_cbcr_buff),
                2,
                Some(height_limit / 2),
            );
        }
    }
}
