use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use fast_image_resize::ResizeOptions;
use parking_lot::{RwLock, RwLockReadGuard};
use rayon::prelude::*;
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};

use crate::{
    mosaic::{Mosaic, tiles::Tile},
    streamer::tile_stores::{ReadTileResult, StoreFrame, TileStore, scale_frame},
    types::DenseIndex,
};

///smart = tracks usage via LRU, can free dynamically to fit RAM budgets, allocates each tile separately
pub(crate) struct SmartStore {
    pub(crate) tile_size: u64,
    tiles: Arc<[Tile]>,
    //keep a separate set so differences can be made quickly and locking is more fine-grained
    tracker: RwLock<FxHashSet<DenseIndex>>,
    inner: RwLock<FxHashMap<DenseIndex, Vec<StoreFrame>>>,
}

impl SmartStore {
    pub(crate) fn new(mosaic: &Mosaic, tile_size: u64) -> Self {
        let tiles = mosaic.tiles();
        Self {
            inner: HashMap::<_, _, _>::with_hasher(FxBuildHasher).into(),
            tracker: HashSet::with_hasher(FxBuildHasher).into(),
            tile_size,
            tiles,
        }
    }

    pub(crate) fn read_tracker(&self) -> RwLockReadGuard<'_, HashSet<DenseIndex, FxBuildHasher>> {
        self.tracker.read()
    }

    pub(crate) fn write_tiles(&self, tiles: Vec<(DenseIndex, Vec<StoreFrame>)>) {
        let mut inner_guard = self.inner.write();
        let mut tracker_guard = self.tracker.write();
        for (tile_index, frames) in tiles {
            inner_guard.insert(tile_index, frames);
            tracker_guard.insert(tile_index);
        }
    }

    pub(crate) fn downscale_tiles(
        &self,
        tile_indices: &[DenseIndex],
        target_tile_store: &TileStore,
        options: &ResizeOptions,
    ) {
        let inner_guard = self.inner.read();

        let all_tiles_res = tile_indices
            .par_iter()
            .copied()
            .map(|tile_index| {
                let mut per_tile_res = Vec::new();
                let tile_frame_count = self.tiles[tile_index.0 as usize].frame_count();
                for i in 0..tile_frame_count {
                    let frame = &inner_guard[&tile_index][i as usize];
                    per_tile_res.push(scale_frame(
                        options,
                        &frame.y,
                        &frame.cb_cr,
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
        let inner_guard = self.inner.read();

        for tile_index in indices {
            match inner_guard.get(&tile_index) {
                None => result.push((tile_index, ReadTileResult::Vacant)),
                Some(frames) => {
                    let mod_frame =
                        (frame_offset % self.tiles[tile_index.0 as usize].frame_count()) as usize;
                    let store_frame = &frames[mod_frame];
                    result.push((
                        tile_index,
                        ReadTileResult::Resident {
                            y: &store_frame.y,
                            cb_cr: &store_frame.cb_cr,
                        },
                    ));
                }
            }
        }

        closure(result)
    }
}
