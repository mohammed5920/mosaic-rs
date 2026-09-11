use std::collections::{HashMap, HashSet};

use parking_lot::{RwLock, RwLockReadGuard};
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};

use crate::{
    streamer::tile_stores::StoreFrame,
    types::{DenseIndex, StoreIndex},
};

pub(crate) struct SmartStore {
    pub(crate) tile_size: u64,
    //keep a separate set so differences can be made quickly and locking is more fine-grained
    tracker: RwLock<FxHashSet<DenseIndex>>,
    inner: RwLock<FxHashMap<StoreIndex, StoreFrame>>,
}

impl SmartStore {
    pub(crate) fn new(tile_size: u64) -> Self {
        Self {
            tile_size,
            tracker: HashSet::with_hasher(FxBuildHasher).into(),
            inner: HashMap::with_hasher(FxBuildHasher).into(),
        }
    }

    pub(crate) fn read_tracker(&self) -> RwLockReadGuard<'_, HashSet<DenseIndex, FxBuildHasher>> {
        self.tracker.read()
    }
}
