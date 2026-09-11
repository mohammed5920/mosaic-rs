use std::sync::Arc;

use parking_lot::RwLockReadGuard;
use rustc_hash::FxHashSet;

use crate::{
    streamer::tile_stores::{fast_store::FastStore, smart_store::SmartStore},
    types::DenseIndex,
    util::is_power_of_two,
};

mod fast_store;
mod smart_store;

pub(crate) struct StoreFrame {
    y: Arc<[u8]>,
    cb_cr: Arc<[u8]>,
}

impl StoreFrame {
    pub(crate) fn new(y: Arc<[u8]>, cb_cr: Arc<[u8]>, tile_size: u64) -> Self {
        debug_assert!(y.len() == tile_size.pow(2) as usize, "y plane is malformed");
        debug_assert!(
            cb_cr.len() == y.len() / 2 as usize,
            "cb_cr plane is malformed"
        );
        Self { y, cb_cr }
    }
}

pub(crate) enum TileStore {
    Fast(FastStore),
    Smart(SmartStore),
}

impl TileStore {
    pub(crate) fn new(tile_size: u64, length: u64, is_smart: bool) -> TileStore {
        assert!(
            (2..=2048).contains(&tile_size) && is_power_of_two(tile_size),
            "tile size {tile_size} is invalid"
        );
        if !is_smart {
            println!("size {tile_size}: fast");
            TileStore::Fast(FastStore::new(tile_size, length))
        } else {
            println!("size {tile_size}: smart");
            TileStore::Smart(SmartStore::new(tile_size))
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

    pub(crate) fn contains(&self, index: DenseIndex) {
        let guard = match self {
            Self::Fast(f) => f.read_tracker(),
            Self::Smart(s) => s.read_tracker(),
        };
    }
}
