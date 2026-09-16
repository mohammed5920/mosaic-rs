use std::{collections::HashSet, sync::Arc};

use fast_image_resize::ResizeOptions;
use parking_lot::RwLock;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::{FxBuildHasher, FxHashSet};

use crate::{
    mosaic::{Mosaic, tiles::Tile},
    streamer::tile_stores::{ReadTileResult, StoreFrame, TileStore, scale_frame},
    types::{DenseIndex, StoreIndex},
};

///fast = uses arenas to avoid pointer chasing when writing millions of tiles to the atlas
pub(crate) struct FastStore {
    pub(crate) tile_size: u64,
    tiles: Arc<[Tile]>,
    index_map: Arc<[StoreIndex]>,
    tracker: RwLock<FxHashSet<DenseIndex>>,
    y_arena: RwLock<Vec<u8>>,
    cbcr_arena: RwLock<Vec<u8>>,
}

impl FastStore {
    pub(crate) fn new(mosaic: &Mosaic, tile_size: u64, index_map: Arc<[StoreIndex]>) -> Self {
        let tile_pixels = tile_size.pow(2);
        let tiles = mosaic.tiles();
        let total_frames = mosaic.total_tile_frames();
        Self {
            cbcr_arena: RwLock::new(vec![0u8; (tile_pixels / 2 * total_frames) as usize]),
            y_arena: RwLock::new(vec![0u8; (tile_pixels * total_frames) as usize]),
            tracker: HashSet::with_hasher(FxBuildHasher).into(),
            index_map,
            tile_size,
            tiles,
        }
    }

    pub(crate) fn contains_tile(&self, index: DenseIndex) -> bool {
        self.tracker.read().contains(&index)
    }

    ///tiles that are in the passed indices, but not in the store
    pub(crate) fn difference(&self, indices: impl Iterator<Item = DenseIndex>) -> Vec<DenseIndex> {
        let guard = self.tracker.read();
        indices.filter(|i| !guard.contains(i)).collect()
    }

    pub(crate) fn write_tiles(&self, tiles: Vec<(DenseIndex, Vec<StoreFrame>)>) {
        let frame_size = self.tile_size.pow(2) as usize;
        let mut tracker_guard = self.tracker.write();
        let mut y_guard = self.y_arena.write();
        let mut cbcr_guard = self.cbcr_arena.write();
        for (tile_index, frames) in tiles {
            let store_index = self.index_map[tile_index.0 as usize].0 as usize;
            for (i, frame) in frames.iter().enumerate() {
                let y_offset = (store_index + i) * frame_size;
                let cbcr_offset = y_offset / 2;
                y_guard[y_offset..y_offset + frame_size].copy_from_slice(&frame.y);
                cbcr_guard[cbcr_offset..cbcr_offset + frame_size / 2].copy_from_slice(&frame.cb_cr);
            }
            tracker_guard.insert(tile_index);
        }
    }

    pub(crate) fn downscale_tiles(
        &self,
        tile_indices: &[DenseIndex],
        target_tile_store: &TileStore,
        options: &ResizeOptions,
    ) {
        let frame_size = self.tile_size.pow(2) as usize;
        let y_guard = self.y_arena.read();
        let cbcr_guard = self.cbcr_arena.read();

        let all_tiles_res = tile_indices
            .par_iter()
            .copied()
            .map(|tile_index| {
                let store_index = self.index_map[tile_index.0 as usize].0 as usize;
                let frame_count = self.tiles[tile_index.0 as usize].frame_count() as usize;
                let mut per_tile_res = Vec::new();
                for i in store_index..store_index + frame_count {
                    let y_offset = i * frame_size;
                    let y_slice = &y_guard[y_offset..y_offset + frame_size];
                    let cbcr_offset = y_offset / 2;
                    let cbcr_slice = &cbcr_guard[cbcr_offset..cbcr_offset + frame_size / 2];
                    per_tile_res.push(scale_frame(
                        options,
                        y_slice,
                        cbcr_slice,
                        self.tile_size as u32,
                        target_tile_store.tile_size() as u32,
                    ));
                }
                (tile_index, per_tile_res)
            })
            .collect::<Vec<_>>();

        target_tile_store.write_tiles(all_tiles_res);
    }

    pub(crate) fn with_tiles(
        &self,
        indices: impl ExactSizeIterator<Item = DenseIndex>,
        frame_offset: u64,
        closure: impl FnOnce(Vec<(DenseIndex, ReadTileResult)>),
    ) {
        let mut result = Vec::with_capacity(indices.len());
        let frame_size = self.tile_size.pow(2) as usize;
        let tracker_guard = self.tracker.read();
        let y_guard = self.y_arena.read();
        let cbcr_guard = self.cbcr_arena.read();

        for tile_index in indices {
            if !tracker_guard.contains(&tile_index) {
                result.push((tile_index, ReadTileResult::Vacant));
                continue;
            }
            let mod_frame =
                frame_offset as usize % self.tiles[tile_index.0 as usize].frame_count() as usize;
            let store_index = self.index_map[tile_index.0 as usize].0 as usize + mod_frame;
            let y_offset = store_index * frame_size;
            let y_slice = &y_guard[y_offset..y_offset + frame_size];
            let cbcr_offset = y_offset / 2;
            let cbcr_slice = &cbcr_guard[cbcr_offset..cbcr_offset + frame_size / 2];
            result.push((
                tile_index,
                ReadTileResult::Resident {
                    y: y_slice,
                    cb_cr: cbcr_slice,
                },
            ));
        }

        closure(result)
    }
}
