use std::{rc::Rc, sync::Arc};

use camino::Utf8PathBuf;

use crate::{
    mosaic::{
        matchmaker::MatchMaker,
        media_source::Source,
        mosaic_static::StaticMosaic,
        tiles::{Tile, load_tiles},
    },
    types::{Bb, DenseIndex},
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
        let tiles = benchmark("loading tiles", || load_tiles(tiles_path))?;
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

    ///get array of len(source.width*source.height) of all match indices mapped through mosaic.dense_map
    ///
    ///(falls back to sparse matches for dynamic mosaics)
    pub(crate) fn dense_matches(&self) -> Arc<[DenseIndex]> {
        match self {
            Mosaic::StaticMosaic(m) => m.dense_matches.clone(),
            Mosaic::DynamicMosaic => todo!(),
        }
    }

    pub(crate) fn slice_bb_from_dense(&self, (start, end): Bb) -> Rc<[DenseIndex]> {
        debug_assert!(
            start.0 >= 0 && start.0 <= end.0 && start.1 >= 0 && start.1 <= end.1,
            "invalid slicing coordinates (start: {start:?} - end: {end:?})"
        );

        let arr = match self {
            Mosaic::StaticMosaic(m) => &m.dense_matches,
            Mosaic::DynamicMosaic => todo!(),
        };

        let (start_x, start_y) = start;
        let (end_x, end_y) = end;
        let col_offset = start_x;
        let col_len = end_x - start_x;

        let stride = self.width() as i64;
        let mut res = Vec::new();
        for row in start_y..end_y {
            let row_offset = row * stride;
            res.extend_from_slice(
                &arr[(row_offset + col_offset) as usize
                    ..(row_offset + col_offset + col_len) as usize],
            );
        }
        res.into()
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

    ///get all tiles used in the mosaic (or all of them for dynamic mosaics)
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
        let tiles = self.tiles();
        let padded_len = (tiles.len() as f64).sqrt().ceil().powi(2) as usize;
        let mut res = Vec::with_capacity(padded_len);
        res.extend((0..padded_len).map(|_| [0u8; 4]));
        for (dense_index, tile) in tiles.iter().enumerate() {
            let [r, g, b] = tile.average_colour();
            res[dense_index] = [r, g, b, 255];
        }
        res.into()
    }
}
