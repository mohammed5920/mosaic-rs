use std::str::FromStr as _;
use std::sync::Arc;

use camino::Utf8PathBuf;
use rayon::slice::ParallelSliceMut;

use crate::{
    mosaic::{
        matchmaker::{MatchIndex, Matchmaker},
        media_source::{Source, pic_source::PicSource},
        tiles::{Tile, load_tiles},
    },
    util::benchmark,
};

pub(crate) mod matchmaker;
pub(crate) mod media_source;
pub(crate) mod tiles;

///for dynamic mosaics this is the same as the MatchIndex
#[repr(C)]
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, bytemuck::Pod, bytemuck::Zeroable,
)]
pub(crate) struct DenseIndex(pub(crate) i32);

pub(crate) enum Mosaic {
    StaticMosaic {
        source: PicSource,
        tiles: Arc<[Tile]>,
        ///array containing the unique tiles indices that are used to compose the final image (used for sizing page table, streaming heurestics, etc.)
        unique_matches: Arc<[MatchIndex]>,
        ///array of len(unique_matches) where index = MatchIndex and value = DenseIndex (reduced address space to just the tiles used in this mosaic)
        dense_map: Arc<[DenseIndex]>,
        ///array of len(source.width*source.height) of all match indices mapped through dense_map
        dense_matches: Arc<[DenseIndex]>,
    },
    DynamicMosaic,
}

impl Mosaic {
    pub(crate) fn create(source_path: &str, tiles_path: &str) -> anyhow::Result<Mosaic> {
        let tiles = benchmark("loading tiles", || {
            load_tiles(&Utf8PathBuf::from_str(tiles_path).unwrap(), 64)
        })?;
        let matchmaker = benchmark("generating match tree", || Matchmaker::from_tiles(&tiles));
        let source = benchmark("loading source", || {
            Source::open(&Utf8PathBuf::from_str(source_path).unwrap(), 1)
        })?;
        match source {
            Source::Pic(pic_source) => {
                let made_matches = benchmark("making matches", || {
                    matchmaker.matchmake(&pic_source.pixels)
                });
                let (unique_matches, dense_map) = benchmark("creating static maps", || {
                    let unique_matches: Arc<[MatchIndex]> = {
                        let mut cloned = made_matches.clone();
                        cloned.par_sort_unstable();
                        cloned.dedup();
                        cloned.into()
                    };
                    let dense_map = {
                        let mut res: Vec<DenseIndex> = Vec::with_capacity(tiles.len());
                        res.extend((0..tiles.len()).map(|_| DenseIndex(-1)));
                        for (i, tile_idx) in unique_matches.iter().enumerate() {
                            res[tile_idx.0 as usize] = DenseIndex(i as i32)
                        }
                        res
                    };
                    (unique_matches, dense_map)
                });
                Ok(Mosaic::StaticMosaic {
                    source: pic_source,
                    tiles: tiles.into(),
                    unique_matches,
                    dense_matches: made_matches
                        .iter()
                        .map(|midx| dense_map[midx.0 as usize])
                        .collect::<Vec<_>>()
                        .into(),
                    dense_map: dense_map.into(),
                })
            }
            Source::Vid => todo!(),
        }
    }

    pub(crate) fn read_frame(&mut self) -> Arc<[DenseIndex]> {
        match self {
            Mosaic::StaticMosaic { dense_matches, .. } => dense_matches.clone(),
            Mosaic::DynamicMosaic => todo!(),
        }
    }

    pub(crate) fn width(&self) -> u64 {
        match self {
            Mosaic::StaticMosaic { source, .. } => source.width,
            Mosaic::DynamicMosaic => todo!(),
        }
    }

    pub(crate) fn height(&self) -> u64 {
        match self {
            Mosaic::StaticMosaic { source, .. } => source.height,
            Mosaic::DynamicMosaic => todo!(),
        }
    }

    pub(crate) fn tiles(&self) -> Arc<[Tile]> {
        match self {
            Mosaic::StaticMosaic { tiles, .. } => tiles.clone(),
            Mosaic::DynamicMosaic => todo!(),
        }
    }

    // ///dynamic mosaics do not track unique matches (will approach total tileset anyway since videos have so many colours)
    // pub(crate) fn unique_matches(&self) -> Arc<[MatchIndex]> {
    //     match self {
    //         Mosaic::StaticMosaic { unique_matches, .. } => unique_matches.clone(),
    //         Mosaic::DynamicMosaic => Arc::from(
    //             self.tiles()
    //                 .iter()
    //                 .enumerate()
    //                 .map(|(i, _)| MatchIndex(i as i32))
    //                 .collect::<Vec<_>>(),
    //         ),
    //     }
    // }
}
