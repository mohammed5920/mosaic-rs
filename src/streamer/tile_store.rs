use std::rc::Rc;

use rustc_hash::FxHashMap;

use crate::types::StoreIndex;

pub(crate) enum TileStore {
    Fast(FastStore),
    Smart(SmartStore),
}

struct StoredFrame {
    y: Rc<[u8]>,
    cb_cr: Rc<[u8]>,
}

struct FastStore {
    inner: Vec<StoredFrame>,
}

struct SmartStore {
    inner: FxHashMap<StoreIndex, StoredFrame>,
}

impl TileStore {
    pub(crate) fn new(tile_size: u64, length: u64, is_smart: bool) -> TileStore {
        assert!(
            tile_size >= 2 && tile_size <= 2048 && (tile_size as f64).log2().fract() == 0.0,
            "tile size {tile_size} is invalid"
        );
        todo!()
    }
}
