use std::sync::Arc;

use camino::Utf8PathBuf;
use image::ImageReader;
use rustc_hash::FxHashMap;

use crate::{
    mosaic::{matchmaker::MatchMaker, tiles::Tile},
    types::DenseIndex,
    util::{benchmark, vec_unique},
};

pub(crate) struct StaticMosaic {
    pub(crate) width: u32,
    pub(crate) height: u32,
    ///filtered down to only the tiles used in the mosaic, accessed via DenseIndex
    pub(crate) tiles: Arc<[Tile]>,
    ///array of len(source.width*source.height) of all dense (filtered down) match indices
    pub(crate) dense_matches: Vec<DenseIndex>,
}

impl StaticMosaic {
    pub(crate) fn new(
        source_path: Utf8PathBuf,
        tiles: Vec<Tile>,
        matchmaker: MatchMaker,
    ) -> anyhow::Result<Self> {
        let source = ImageReader::open(source_path)?
            .with_guessed_format()?
            .decode()?
            .into_rgb8();

        let made_matches = benchmark("making matches", || matchmaker.matchmake(source.as_raw()));

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

        Ok(Self {
            tiles: new_tiles.into(),
            dense_matches,
            width: source.width(),
            height: source.height(),
        })
    }
}
