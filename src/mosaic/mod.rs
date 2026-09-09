use std::{rc::Rc, sync::Arc};

use camino::Utf8PathBuf;
use rayon::slice::ParallelSliceMut;

use crate::{
    config::CONFIG,
    mosaic::{
        matchmaker::Matchmaker,
        media_source::{Source, pic_source::PicSource},
        tiles::{Tile, load_tiles, syn_tiles::load_synthetic_tiles},
    },
    types::{DenseIndex, MatchIndex},
    util::benchmark,
};

pub(crate) mod matchmaker;
pub(crate) mod media_source;
pub(crate) mod tiles;

pub(crate) enum Mosaic {
    StaticMosaic {
        source: PicSource,
        tiles: Arc<[Tile]>,
        ///array containing the unique tiles indices that are used to compose the final image (used for sizing page table, streaming heurestics, etc.)
        unique_matches: Arc<[MatchIndex]>,
        ///array of len(tiles) where index = MatchIndex and value = DenseIndex (reduced address space to just the tiles used in this mosaic)
        dense_map: Arc<[DenseIndex]>,
        ///array of len(source.width*source.height) of all match indices mapped through dense_map
        dense_matches: Arc<[DenseIndex]>,
    },
    DynamicMosaic,
}

impl Mosaic {
    pub(crate) fn create(
        source_path: &Utf8PathBuf,
        tiles_path: &Utf8PathBuf,
    ) -> anyhow::Result<Mosaic> {
        let tiles = if CONFIG.synthetic_tile_count.is_some() {
            benchmark("generating synthetic tiles", || {
                load_synthetic_tiles(CONFIG.synthetic_tile_count.unwrap())
            })
        } else {
            benchmark("loading tiles", || load_tiles(tiles_path))?
        };
        let matchmaker = benchmark("generating match tree", || Matchmaker::from_tiles(&tiles));
        let source = benchmark("loading source", || Source::open(source_path))?;
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

    pub(crate) fn total_frames(&self) -> u64 {
        match self {
            Mosaic::StaticMosaic {
                unique_matches,
                tiles,
                ..
            } => unique_matches
                .iter()
                .map(|mi| tiles[mi.0 as usize].frame_count())
                .sum(),
            Mosaic::DynamicMosaic => todo!(),
        }
    }

    pub(crate) fn generate_palette(&self) -> Rc<[[u8; 4]]> {
        match self {
            Mosaic::StaticMosaic {
                unique_matches,
                dense_map,
                tiles,
                ..
            } => {
                let padded_len = (unique_matches.len() as f64).sqrt().ceil().powi(2) as usize;
                let mut res = Vec::with_capacity(padded_len);
                res.extend((0..padded_len).map(|_| [0u8; 4]));
                for match_index in unique_matches.iter() {
                    let [r, g, b] = tiles[match_index.0 as usize].average_colour();
                    res[dense_map[match_index.0 as usize].0 as usize] = [r, g, b, 255];
                }
                res.into()
            }
            Mosaic::DynamicMosaic => todo!(),
        }
    }
}
