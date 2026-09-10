use std::{rc::Rc, sync::Arc};

use crate::{
    config::CONFIG,
    mosaic::{matchmaker::MatchMaker, media_source::pic_source::PicSource, tiles::Tile},
    types::{DenseIndex, MatchIndex},
    util::{benchmark, vec_unique},
};

pub(crate) struct StaticMosaic {
    pub(crate) source: PicSource,
    pub(crate) tiles: Arc<[Tile]>,
    ///array containing the unique tiles indices that are used to compose the final image (used for sizing page table, streaming heurestics, etc.)
    pub(crate) unique_matches: Arc<[MatchIndex]>,
    ///array of len(tiles) where index = MatchIndex and value = DenseIndex (reduced address space to just the tiles used in this mosaic)
    pub(crate) dense_map: Arc<[DenseIndex]>,
    ///array of len(source.width*source.height) of all match indices mapped through dense_map
    pub(crate) dense_matches: Arc<[DenseIndex]>,
}

impl StaticMosaic {
    pub(crate) fn new(pic_source: PicSource, tiles: Vec<Tile>, matchmaker: MatchMaker) -> Self {
        let made_matches = benchmark("making matches", || {
            matchmaker.matchmake(&pic_source.pixels)
        });
        let (unique_matches, dense_map) = benchmark("creating static maps", || {
            let unique_matches: Arc<[MatchIndex]> = {
                let mut cloned = made_matches.clone();
                vec_unique(&mut cloned);
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
        Self {
            source: pic_source,
            tiles: tiles.into(),
            unique_matches,
            dense_matches: made_matches
                .iter()
                .map(|midx| dense_map[midx.0 as usize])
                .collect::<Vec<_>>()
                .into(),
            dense_map: dense_map.into(),
        }
    }

    pub(crate) fn total_tile_frames(&self) -> u64 {
        if CONFIG.force_static_tiles {
            self.unique_matches.len() as u64
        } else {
            self.unique_matches
                .iter()
                .map(|mi| self.tiles[mi.0 as usize].frame_count())
                .sum()
        }
    }

    pub(crate) fn generate_palette(&self) -> Rc<[[u8; 4]]> {
        let padded_len = (self.unique_matches.len() as f64).sqrt().ceil().powi(2) as usize;
        let mut res = Vec::with_capacity(padded_len);
        res.extend((0..padded_len).map(|_| [0u8; 4]));
        for match_index in self.unique_matches.iter() {
            let [r, g, b] = self.tiles[match_index.0 as usize].average_colour();
            res[self.dense_map[match_index.0 as usize].0 as usize] = [r, g, b, 255];
        }
        res.into()
    }
}
