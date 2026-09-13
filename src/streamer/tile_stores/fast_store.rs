use std::{collections::HashSet, sync::Arc};

use anyhow::Context;
use fast_image_resize::{
    FilterType, PixelType,
    ResizeAlg::Convolution,
    ResizeOptions,
    images::{Image as FRImage, ImageRef as FRImageRef},
};
use parking_lot::{RwLock, RwLockReadGuard};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::{FxBuildHasher, FxHashSet};

use crate::{
    mosaic::{
        Mosaic,
        tiles::{RESIZER, Tile},
    },
    streamer::tile_stores::{ReadTileResult, StoreFrame, TileStore},
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
    pub(crate) fn new(mosaic: &Mosaic, index_map: Arc<[StoreIndex]>, tile_size: u64) -> Self {
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

    pub(crate) fn read_tracker(&self) -> RwLockReadGuard<'_, HashSet<DenseIndex, FxBuildHasher>> {
        self.tracker.read()
    }

    pub(crate) fn write_tiles(&self, tiles: &[(DenseIndex, &[StoreFrame])]) {
        let frame_size = self.tile_size.pow(2) as usize;
        let mut y_guard = self.y_arena.write();
        let mut cbcr_guard = self.cbcr_arena.write();
        let mut tracker_guard = self.tracker.write();

        for (tile_index, frames) in tiles {
            let store_index = self.index_map[tile_index.0 as usize].0 as usize;
            for (i, frame) in frames.iter().enumerate() {
                let y_offset = (store_index + i) * frame_size;
                let cbcr_offset = y_offset / 2;
                y_guard[y_offset..y_offset + frame_size].copy_from_slice(&frame.y);
                cbcr_guard[cbcr_offset..cbcr_offset + frame_size / 2].copy_from_slice(&frame.cb_cr);
            }
            tracker_guard.insert(*tile_index);
        }
    }

    pub(crate) fn downscale_tiles(
        &self,
        tile_indices: &[DenseIndex],
        target_tile_store: &TileStore,
    ) {
        let frame_size = self.tile_size.pow(2) as usize;
        let y_guard = self.y_arena.read();
        let cbcr_guard = self.cbcr_arena.read();
        let mut options = ResizeOptions::new();
        options.algorithm = Convolution(FilterType::Bilinear);
        let source_size = self.tile_size as u32;
        let target_size = target_tile_store.tile_size() as u32;

        let all_tiles_res = tile_indices.par_iter().map(|tile_index| {
            {
                let mut y_dest = FRImage::new(target_size, target_size, PixelType::U8);
                let mut cbcr_dest = FRImage::new(target_size/2, target_size/2, PixelType::U8x2);

                let store_index = self.index_map[tile_index.0 as usize].0 as usize;
                let frame_count = self.tiles[tile_index.0 as usize].frame_count() as usize;

                let mut per_tile_res = Vec::new();
                for i in store_index..store_index+frame_count {
                    let y_offset = i * frame_size;
                    let y_slice = &y_guard[y_offset..y_offset + frame_size];
                    let y_view = FRImageRef::new(source_size, source_size, y_slice, PixelType::U8).with_context(|| format!("could not wrap y plane of tile {tile_index:?} offset {} from size {}", i-store_index, self.tile_size)).unwrap();

                    let cbcr_offset = y_offset / 2;
                    let cbcr_slice = &cbcr_guard[cbcr_offset..cbcr_offset + frame_size / 2];
                    let cb_cr_view = FRImageRef::new(source_size/2, source_size/2, cbcr_slice, PixelType::U8x2).with_context(|| format!("could not wrap uv plane of tile {tile_index:?} offset {} from size {}", i-store_index, self.tile_size)).unwrap();

                    RESIZER.with_borrow_mut(|r| {
                        r.resize(&y_view, &mut y_dest, &options).with_context(|| format!("could not downscale y plane of tile {tile_index:?} offset {} from size {}", i-store_index, self.tile_size)).unwrap();
                        r.resize(&cb_cr_view, &mut cbcr_dest, &options).with_context(|| format!("could not downscale y plane of tile {tile_index:?} offset {} from size {}", i-store_index, self.tile_size)).unwrap();
                    });

                    per_tile_res.push(StoreFrame::new(y_dest.buffer().into(), cbcr_dest.buffer().into(), target_size as u64));
                }

                (tile_index, per_tile_res)
            }
        }).collect::<Vec<_>>();

        let converted: Vec<(DenseIndex, &[StoreFrame])> = all_tiles_res
            .iter()
            .map(|(idx, frames)| (**idx, frames.as_slice()))
            .collect();

        target_tile_store.write_tiles(&converted);
    }

    pub(crate) fn with_tiles(
        &self,
        indices: impl Iterator<Item = DenseIndex>,
        frame_offset: u64,
        closure: impl FnOnce(Vec<(DenseIndex, ReadTileResult)>),
    ) {
        let mut result = Vec::new();
        let tracker_guard = self.read_tracker();
        let frame_size = self.tile_size.pow(2) as usize;
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
