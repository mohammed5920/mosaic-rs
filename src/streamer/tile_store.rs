use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};

use crate::types::{DenseIndex, StoreIndex};

pub(crate) enum TileStore {
    Fast(FastStore),
    Smart(SmartStore),
}

pub(crate) struct StoredFrame {
    y: Rc<[u8]>,
    cb_cr: Rc<[u8]>,
}

pub(crate) struct FastStore {
    tracker: FxHashSet<DenseIndex>,
    inner: Vec<StoredFrame>,
}

pub(crate) struct SmartStore {
    inner: FxHashMap<StoreIndex, StoredFrame>,
}

impl TileStore {
    pub(crate) fn new(tile_size: u64, length: u64, is_smart: bool) -> TileStore {
        assert!(
            (2..=2048).contains(&tile_size) && (tile_size as f64).log2().fract() == 0.0,
            "tile size {tile_size} is invalid"
        );
        if !is_smart {
            println!("size {tile_size}: fast");
            TileStore::Fast(FastStore {
                tracker: HashSet::with_hasher(FxBuildHasher),
                inner: Vec::with_capacity(length as usize),
            })
        } else {
            println!("size {tile_size}: smart");
            TileStore::Smart(SmartStore {
                inner: HashMap::with_hasher(FxBuildHasher),
            })
        }
    }
}
