use std::collections::HashSet;

use parking_lot::{RwLock, RwLockReadGuard};
use rustc_hash::{FxBuildHasher, FxHashSet};

use crate::types::DenseIndex;

pub(crate) struct FastStore {
    pub(crate) tile_size: u64,
    tracker: RwLock<FxHashSet<DenseIndex>>,
    y_arena: RwLock<Vec<u8>>,
    cbcr_arena: RwLock<Vec<u8>>,
}

impl FastStore {
    pub(crate) fn new(tile_size: u64, total_frames: u64) -> Self {
        let tile_pixels = tile_size * tile_size;
        Self {
            tile_size,
            tracker: HashSet::with_hasher(FxBuildHasher).into(),
            y_arena: RwLock::new(vec![0u8; (tile_pixels * total_frames) as usize]),
            cbcr_arena: RwLock::new(vec![0u8; (tile_pixels / 2 * total_frames) as usize]),
        }
    }

    pub(crate) fn read_tracker(&self) -> RwLockReadGuard<'_, HashSet<DenseIndex, FxBuildHasher>> {
        self.tracker.read()
    }
}
