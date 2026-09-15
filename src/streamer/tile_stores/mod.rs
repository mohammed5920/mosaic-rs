use std::sync::Arc;

use fast_image_resize::{
    FilterType, PixelType,
    ResizeAlg::Convolution,
    ResizeOptions,
    images::{Image as FRImage, ImageRef as FRImageRef},
};
use parking_lot::RwLockReadGuard;
use rustc_hash::FxHashSet;

use crate::{
    mosaic::{Mosaic, tiles::RESIZER},
    streamer::tile_stores::{fast_store::FastStore, smart_store::SmartStore},
    types::{DenseIndex, StoreIndex},
    util::is_power_of_two,
};

mod fast_store;
mod smart_store;

pub(crate) struct StoreFrame {
    y: Vec<u8>,
    cb_cr: Vec<u8>,
}

pub(crate) enum ReadTileResult<'a> {
    Vacant,
    Resident { y: &'a [u8], cb_cr: &'a [u8] },
}

impl StoreFrame {
    pub(crate) fn new(y: Vec<u8>, cb_cr: Vec<u8>, tile_size: u64) -> Self {
        debug_assert!(y.len() == tile_size.pow(2) as usize, "y plane is malformed");
        debug_assert!(cb_cr.len() == y.len() / 2_usize, "cb_cr plane is malformed");
        Self { y, cb_cr }
    }
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
            TileStore::Fast(FastStore::new(mosaic, tile_size, index_map))
        } else {
            println!("size {tile_size}: smart");
            TileStore::Smart(SmartStore::new(mosaic, tile_size))
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

    pub(crate) fn write_tiles(&self, tiles: Vec<(DenseIndex, Vec<StoreFrame>)>) {
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
        indices: impl ExactSizeIterator<Item = DenseIndex>,
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
        let mut options = ResizeOptions::new();
        options.algorithm = Convolution(FilterType::Bilinear);
        match self {
            TileStore::Fast(f) => f.downscale_tiles(tile_indices, target_tile_store, &options),
            TileStore::Smart(s) => s.downscale_tiles(tile_indices, target_tile_store, &options),
        }
    }
}

pub(crate) fn tile_size_to_store_index(tile_size: u64) -> usize {
    (tile_size / 2).ilog2() as usize
}

pub(crate) fn scale_frame(
    options: &ResizeOptions,
    y_slice: &[u8],
    cbcr_slice: &[u8],
    source_dim: u32,
    target_dim: u32,
) -> StoreFrame {
    let y_view =
        FRImageRef::new(source_dim, source_dim, y_slice, PixelType::U8).unwrap_or_else(|e| {
            panic!(
                "couldn't wrap y plane of len {} as size {source_dim}: {e}",
                y_slice.len()
            )
        });
    let cb_cr_view = FRImageRef::new(source_dim / 2, source_dim / 2, cbcr_slice, PixelType::U8x2)
        .unwrap_or_else(|e| {
            panic!(
                "couldn't wrap cbcr plane of len {} as size {}: {e}",
                cbcr_slice.len(),
                source_dim / 2
            )
        });
    let mut y_dest = FRImage::new(target_dim, target_dim, PixelType::U8);
    let mut cbcr_dest = FRImage::new(target_dim / 2, target_dim / 2, PixelType::U8x2);
    RESIZER.with_borrow_mut(|r| {
        r.resize(&y_view, &mut y_dest, options).unwrap_or_else(|e| {
            panic!("couldn't scale y plane from size {source_dim} to size {target_dim}: {e}")
        });
        r.resize(&cb_cr_view, &mut cbcr_dest, options)
            .unwrap_or_else(|e| {
                panic!(
                    "couldn't scale cbcr plane from size {} to size {}: {e}",
                    source_dim / 2,
                    target_dim / 2
                )
            });
    });
    StoreFrame::new(y_dest.into_vec(), cbcr_dest.into_vec(), target_dim as u64)
}
