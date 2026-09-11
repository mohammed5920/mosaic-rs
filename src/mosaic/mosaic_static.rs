use std::{rc::Rc, sync::Arc};

use rustc_hash::FxHashMap;

use crate::{
    config::CONFIG,
    mosaic::{matchmaker::MatchMaker, media_source::pic_source::PicSource, tiles::Tile},
    types::DenseIndex,
    util::{benchmark, vec_unique},
};

pub(crate) struct StaticMosaic {
    pub(crate) source: PicSource,
    ///filtered down to only the tiles used in the mosaic, accessed via DenseIndex
    pub(crate) tiles: Arc<[Tile]>,
    ///array of len(source.width*source.height) of all dense (filtered down) match indices
    pub(crate) dense_matches: Arc<[DenseIndex]>,
}

impl StaticMosaic {
    pub(crate) fn new(pic_source: PicSource, tiles: Vec<Tile>, matchmaker: MatchMaker) -> Self {
        let made_matches = benchmark("making matches", || {
            matchmaker.matchmake(&pic_source.pixels)
        });

        let (new_tiles, dense_matches) = benchmark("filtering static mosaic", || {
            let unique_matches = {
                let mut cloned = made_matches.clone();
                vec_unique(&mut cloned);
                cloned
            };
            let sparse_dense_map = unique_matches
                .iter()
                .enumerate()
                .map(|(dense, sparse)| (*sparse, DenseIndex(dense as i32)))
                .collect::<FxHashMap<_, _>>();
            let dense_matches = made_matches
                .into_iter()
                .map(|sparse| sparse_dense_map[&sparse])
                .collect();
            let new_tiles = unique_matches
                .iter()
                .map(|sparse| tiles[sparse.0 as usize].clone())
                .collect::<Vec<_>>();
            (new_tiles, dense_matches)
        });

        Self {
            source: pic_source,
            tiles: new_tiles.into(),
            dense_matches,
        }
    }

    pub(crate) fn total_tile_frames(&self) -> u64 {
        if CONFIG.force_static_tiles {
            self.tiles.len() as u64
        } else {
            self.tiles.iter().map(|tile| tile.frame_count()).sum()
        }
    }
}
