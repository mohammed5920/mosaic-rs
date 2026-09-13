use std::sync::Arc;

use camino::Utf8PathBuf;

use crate::{
    mosaic::{
        matchmaker::MatchMaker,
        media_source::Source,
        mosaic_dynamic::DynamicMosaic,
        mosaic_static::StaticMosaic,
        tiles::{Tile, load_tiles},
    },
    types::{Bb, DenseIndex},
    util::benchmark,
};

pub(crate) mod matchmaker;
pub(crate) mod media_source;
pub(crate) mod mosaic_dynamic;
pub(crate) mod mosaic_static;
pub(crate) mod tiles;

pub(crate) enum Mosaic {
    Static(StaticMosaic),
    Dynamic(DynamicMosaic),
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
            Source::Pic(p) => Ok(Mosaic::Static(StaticMosaic::new(p, tiles, matchmaker))),
            Source::Vid(v) => Ok(Mosaic::Dynamic(DynamicMosaic::new(v, tiles, matchmaker))),
        }
    }

    ///read the next frame of a dynamic mosaic
    ///
    ///(no-op for static ones)
    pub(crate) fn advance_frame(&mut self) {
        if let Mosaic::Dynamic(d) = self {
            todo!()
        };
    }

    ///seek to a given frame of a dynamic mosaic
    ///
    ///(not guaranteed to be frame accurate, no-op for static ones)
    pub(crate) fn seek_to_frame(&mut self, frame_idx: u64) {
        if let Mosaic::Dynamic(d) = self {
            todo!()
        };
    }

    ///get array of len(source.width*source.height) of all match indices mapped through mosaic.dense_map
    ///
    ///(falls back to sparse matches for dynamic mosaics)
    pub(crate) fn dense_matches(&self) -> &Vec<DenseIndex> {
        match self {
            Mosaic::Static(s) => &s.dense_matches,
            Mosaic::Dynamic(d) => todo!(),
        }
    }

    pub(crate) fn slice_bb_from_dense(&self, (start, end): Bb) -> Vec<DenseIndex> {
        debug_assert!(
            start.0 >= 0 && start.0 <= end.0 && start.1 >= 0 && start.1 <= end.1,
            "invalid slicing coordinates (start: {start:?} - end: {end:?})"
        );

        let arr = self.dense_matches();
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
        res
    }

    pub(crate) fn width(&self) -> u64 {
        match self {
            Mosaic::Static(s) => s.source.width,
            Mosaic::Dynamic(d) => todo!(),
        }
    }

    pub(crate) fn height(&self) -> u64 {
        match self {
            Mosaic::Static(s) => s.source.height,
            Mosaic::Dynamic(d) => todo!(),
        }
    }

    ///get all tiles used in the mosaic (or all of them for dynamic mosaics)
    pub(crate) fn tiles(&self) -> Arc<[Tile]> {
        match self {
            Mosaic::Static(s) => s.tiles.clone(),
            Mosaic::Dynamic(d) => todo!(),
        }
    }

    pub(crate) fn total_tile_frames(&self) -> u64 {
        match self {
            Mosaic::Static(s) => s.total_tile_frames(),
            Mosaic::Dynamic(d) => todo!(),
        }
    }

    ///palette of the first frame of only the tiles used in the mosaic
    ///
    ///(for dynamic mosaics, falls back to every tile instead)
    pub(crate) fn generate_palette(&self) -> Vec<[u8; 4]> {
        let tiles = self.tiles();
        let padded_len = (tiles.len() as f64).sqrt().ceil().powi(2) as usize;
        let mut res = Vec::with_capacity(padded_len);
        res.extend((0..padded_len).map(|_| [0u8; 4]));
        for (dense_index, tile) in tiles.iter().enumerate() {
            let [r, g, b] = tile.average_colour();
            res[dense_index] = [r, g, b, 255];
        }
        res
    }
}
