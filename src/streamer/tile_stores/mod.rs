use std::sync::Arc;

use parking_lot::RwLockReadGuard;
use rustc_hash::FxHashSet;

use crate::{
    mosaic::Mosaic,
    streamer::tile_stores::{fast_store::FastStore, smart_store::SmartStore},
    types::{DenseIndex, StoreIndex},
    util::is_power_of_two,
};

mod fast_store;
mod smart_store;

#[derive(Clone)]
pub(crate) struct StoreFrame {
    y: Arc<[u8]>,
    cb_cr: Arc<[u8]>,
}

impl StoreFrame {
    pub(crate) fn new(y: Arc<[u8]>, cb_cr: Arc<[u8]>, tile_size: u64) -> Self {
        debug_assert!(y.len() == tile_size.pow(2) as usize, "y plane is malformed");
        debug_assert!(cb_cr.len() == y.len() / 2_usize, "cb_cr plane is malformed");
        Self { y, cb_cr }
    }
}

pub(crate) enum ReadTileResult<'tile> {
    Vacant,
    Resident { y: &'tile [u8], cb_cr: &'tile [u8] },
}

pub(crate) enum TileStore {
    Fast(FastStore),
    Smart(SmartStore),
}

impl TileStore {
    pub(crate) fn new(
        mosaic: &Mosaic,
        index_map: Arc<[StoreIndex]>,
        tile_size: u64,
        is_smart: bool,
    ) -> TileStore {
        assert!(
            (2..=2048).contains(&tile_size) && is_power_of_two(tile_size),
            "tile size {tile_size} is invalid"
        );
        if !is_smart {
            println!("size {tile_size}: fast");
            TileStore::Fast(FastStore::new(mosaic, index_map, tile_size))
        } else {
            println!("size {tile_size}: smart");
            TileStore::Smart(SmartStore::new(mosaic, index_map, tile_size))
        }
    }

    pub(crate) fn read_tracker(&self) -> RwLockReadGuard<'_, FxHashSet<DenseIndex>> {
        match self {
            Self::Fast(f) => f.read_tracker(),
            Self::Smart(s) => s.read_tracker(),
        }
    }

    pub(crate) fn tile_size(&self) -> u64 {
        match self {
            TileStore::Fast(f) => f.tile_size,
            TileStore::Smart(s) => s.tile_size,
        }
    }

    pub(crate) fn contains_any_tiles(&self, mut indices: impl Iterator<Item = DenseIndex>) -> bool {
        let tracker = self.read_tracker();
        indices.any(|idx| tracker.contains(&idx))
    }

    pub(crate) fn contains_all_tiles(&self, mut indices: impl Iterator<Item = DenseIndex>) -> bool {
        let tracker = self.read_tracker();
        indices.all(|idx| tracker.contains(&idx))
    }

    pub(crate) fn write_tiles(&self, tiles: &[(DenseIndex, &[StoreFrame])]) {
        debug_assert!(
            !self.contains_any_tiles(tiles.iter().map(|t| t.0)),
            "tried to write tiles to store {} twice",
            self.tile_size()
        );
        match self {
            TileStore::Fast(f) => f.write_tiles(tiles),
            TileStore::Smart(s) => s.write_tiles(tiles),
        }
    }

    pub(crate) fn with_tiles(
        &self,
        indices: impl Iterator<Item = DenseIndex>,
        frame_offset: u64,
        closure: impl FnOnce(Vec<(DenseIndex, ReadTileResult)>),
    ) {
        match self {
            TileStore::Fast(f) => f.with_tiles(indices, frame_offset, closure),
            TileStore::Smart(s) => s.with_tiles(indices, frame_offset, closure),
        }
    }

    pub(crate) fn downscale_tiles(
        &self,
        tile_indices: &[DenseIndex],
        target_tile_store: &TileStore,
    ) {
        debug_assert!(
            self.contains_all_tiles(tile_indices.iter().copied()),
            "tried to downscale tiles that aren't in store {}",
            self.tile_size()
        );
        debug_assert!(
            !target_tile_store.contains_any_tiles(tile_indices.iter().copied()),
            "tried to write tiles to store {} twice",
            target_tile_store.tile_size()
        );
        debug_assert!(
            self.tile_size() > target_tile_store.tile_size(),
            "big tile store is actually smaller than small tile store ({}px - {}px)",
            self.tile_size(),
            target_tile_store.tile_size()
        );

        match self {
            TileStore::Fast(f) => f.downscale_tiles(tile_indices, target_tile_store),
            TileStore::Smart(s) => s.downscale_tiles(tile_indices, target_tile_store),
        }
    }
}

pub fn tile_size_to_store_index(tile_size: u64) -> usize {
    (tile_size / 2).ilog2() as usize
}
