use std::{rc::Rc, sync::Arc};

use camino::Utf8PathBuf;

use crate::{
    config::CONFIG,
    mosaic::{
        matchmaker::MatchMaker,
        media_source::Source,
        mosaic_static::StaticMosaic,
        tiles::{Tile, load_tiles, syn_tiles::load_synthetic_tiles},
    },
    types::{DenseIndex, MatchIndex},
    util::benchmark,
};

pub(crate) mod matchmaker;
pub(crate) mod media_source;
pub(crate) mod mosaic_static;
pub(crate) mod tiles;

pub(crate) enum Mosaic {
    StaticMosaic(StaticMosaic),
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
        let matchmaker = benchmark("generating match tree", || MatchMaker::new(&tiles));
        let source = benchmark("loading source", || Source::open(source_path))?;
        match source {
            Source::Pic(pic_source) => Ok(Mosaic::StaticMosaic(StaticMosaic::new(
                pic_source, tiles, matchmaker,
            ))),
            Source::Vid => todo!(),
        }
    }

    ///read the next frame of a dynamic mosaic
    ///
    ///(no-op for static ones)
    pub(crate) fn advance_frame(&mut self) {
        if let Mosaic::DynamicMosaic = self {
            todo!()
        };
    }

    ///seek to a given frame of a dynamic mosaic
    ///
    ///(not guaranteed to be frame accurate, no-op for static ones)
    pub(crate) fn seek_to_frame(&mut self, frame_idx: u64) {
        if let Mosaic::DynamicMosaic = self {
            todo!()
        };
    }

    ///get array of len(source.width*source.height) of indices into mosaic.tiles
    pub(crate) fn sparse_matches(&self) -> Arc<[MatchIndex]> {
        match self {
            Mosaic::StaticMosaic(m) => m.sparse_matches.clone(),
            Mosaic::DynamicMosaic => todo!(),
        }
    }

    ///get array of len(source.width*source.height) of all match indices mapped through mosaic.dense_map
    ///
    ///(falls back to sparse_matches for dynamic mosaics)
    pub(crate) fn dense_matches(&self) -> Arc<[DenseIndex]> {
        match self {
            Mosaic::StaticMosaic(m) => m.dense_matches.clone(),
            Mosaic::DynamicMosaic => todo!(),
        }
    }

    ///get array of only the unique indices used in the mosaic
    ///
    ///(falls back to mosaic.tiles for dynamic mosaics)
    pub(crate) fn unique_matches(&self) -> Arc<[MatchIndex]> {
        match self {
            Mosaic::StaticMosaic(m) => m.unique_matches.clone(),
            Mosaic::DynamicMosaic => todo!(),
        }
    }

    pub(crate) fn width(&self) -> u64 {
        match self {
            Mosaic::StaticMosaic(m) => m.source.width,
            Mosaic::DynamicMosaic => todo!(),
        }
    }

    pub(crate) fn height(&self) -> u64 {
        match self {
            Mosaic::StaticMosaic(m) => m.source.height,
            Mosaic::DynamicMosaic => todo!(),
        }
    }

    ///get all tiles loaded in from the tile folder
    pub(crate) fn tiles(&self) -> Arc<[Tile]> {
        match self {
            Mosaic::StaticMosaic(m) => m.tiles.clone(),
            Mosaic::DynamicMosaic => todo!(),
        }
    }

    pub(crate) fn total_tile_frames(&self) -> u64 {
        match self {
            Mosaic::StaticMosaic(m) => m.total_tile_frames(),
            Mosaic::DynamicMosaic => todo!(),
        }
    }

    ///palette of the first frame of only the tiles used in the mosaic
    ///
    ///(for dynamic mosaics, falls back to every tile instead)
    pub(crate) fn generate_palette(&self) -> Rc<[[u8; 4]]> {
        match self {
            Mosaic::StaticMosaic(m) => m.generate_palette(),
            Mosaic::DynamicMosaic => todo!(),
        }
    }
}
