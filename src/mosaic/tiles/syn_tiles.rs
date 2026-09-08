use rayon::prelude::*;

use std::num::NonZero;

use crate::mosaic::tiles::Tile;

pub(crate) struct SynTile {
    pub(crate) average_colour: [u8; 3],
}

impl SynTile {
    //streaming stubs go here later
}

pub(crate) fn load_synthetic_tiles(limit: NonZero<u64>) -> Vec<Tile> {
    assert!(
        limit.get() <= 16_777_216,
        "max no. of synthetic tiles is 16_777_216 (RGB colour space)"
    );
    (0..limit.get() as i32)
        .into_par_iter()
        .map(|i| {
            let i = (i as f64 * (16_777_216.0 / limit.get() as f64)).ceil() as i32;
            let (r, g, b) = (
                (i / 256 / 256 % 256) as u8,
                (i / 256 % 256) as u8,
                (i % 256) as u8,
            );
            Tile::Syn(SynTile {
                average_colour: [r, g, b],
            })
        })
        .collect()
}
