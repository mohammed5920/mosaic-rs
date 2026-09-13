use std::{collections::HashSet, sync::Arc};

use parking_lot::{RwLock, RwLockReadGuard};
use rustc_hash::{FxBuildHasher, FxHashSet};

use crate::{
    mosaic::{Mosaic, tiles::Tile},
    streamer::tile_stores::{ReadTileResult, StoreFrame, TileStore},
    types::{DenseIndex, StoreIndex},
};

///smart = tracks usage via LRU, can free dynamically to fit RAM budgets, allocates each tile separately
pub(crate) struct SmartStore {
    pub(crate) tile_size: u64,
    index_map: Arc<[StoreIndex]>,
    tiles: Arc<[Tile]>,
    //keep a separate set so differences can be made quickly and locking is more fine-grained
    tracker: RwLock<FxHashSet<DenseIndex>>,
    inner: RwLock<Vec<Option<Arc<[StoreFrame]>>>>,
}

impl SmartStore {
    pub(crate) fn new(mosaic: &Mosaic, index_map: Arc<[StoreIndex]>, tile_size: u64) -> Self {
        let tiles = mosaic.tiles();
        Self {
            inner: (0..tiles.len()).map(|_| None).collect::<Vec<_>>().into(),
            tracker: HashSet::with_hasher(FxBuildHasher).into(),
            tile_size,
            index_map,
            tiles,
        }
    }

    pub(crate) fn read_tracker(&self) -> RwLockReadGuard<'_, HashSet<DenseIndex, FxBuildHasher>> {
        self.tracker.read()
    }

    pub(crate) fn write_tiles(&self, tiles: &[(DenseIndex, &[StoreFrame])]) {
        let mut inner_guard = self.inner.write();
        for (tile_index, frames) in tiles {
            let store_index = self.index_map[tile_index.0 as usize].0 as usize;
            inner_guard[store_index] = Some((*frames).into());
        }
    }

    pub(crate) fn with_tiles(
        &self,
        indices: impl Iterator<Item = DenseIndex>,
        frame_offset: u64,
        closure: impl FnOnce(Vec<(DenseIndex, ReadTileResult)>),
    ) {
        todo!()
    }

    pub(crate) fn downscale_tiles(
        &self,
        tile_indices: &[DenseIndex],
        target_tile_store: &TileStore,
    ) {
        todo!()
    }
}
