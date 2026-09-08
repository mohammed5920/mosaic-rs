pub(crate) mod pic_tiles;
pub(crate) mod syn_tiles;
pub(crate) mod vid_tiles;

use std::{collections::HashSet, num::NonZero};

use camino::Utf8PathBuf;
use rayon::prelude::*;
use rustc_hash::FxBuildHasher;

use crate::{
    config::CONFIG,
    mosaic::tiles::{
        pic_tiles::PicTile,
        syn_tiles::SynTile,
        vid_tiles::{VidTile, vid_tiles_from_path},
    },
    util::{
        colour_to_key,
        file_util::{MediaType, check_supported_extension, walk_dir},
    },
};

pub(crate) fn calc_average_colour(pixels: &[u8]) -> [u8; 3] {
    debug_assert!(
        pixels.len().is_multiple_of(3),
        "pixel array is not divisible by 3 (not valid RGB)"
    );
    let mut sums = [0u64; 3];
    let chunked = pixels.as_chunks::<3>().0;
    for pixel in chunked {
        sums[0] += pixel[0] as u64;
        sums[1] += pixel[1] as u64;
        sums[2] += pixel[2] as u64;
    }
    [
        (sums[0] / chunked.len() as u64) as u8,
        (sums[1] / chunked.len() as u64) as u8,
        (sums[2] / chunked.len() as u64) as u8,
    ]
}

pub(crate) enum Tile {
    Pic(PicTile),
    Vid(VidTile),
    Syn(SynTile),
}

impl Tile {
    pub(crate) fn average_colour(&self) -> [u8; 3] {
        match self {
            Tile::Pic(pic_tile) => pic_tile.average_colour,
            Tile::Vid(vid_tile) => vid_tile.average_colour,
            Tile::Syn(syn_tile) => syn_tile.average_colour,
        }
    }
}

pub(crate) fn load_tiles(path: &Utf8PathBuf) -> anyhow::Result<Vec<Tile>> {
    let files = walk_dir(path)?;
    let (mut res, mut pics, mut vids) = (Vec::new(), Vec::new(), Vec::new());
    for file_path in files.into_iter() {
        match check_supported_extension(&file_path) {
            MediaType::Pic => pics.push(file_path),
            MediaType::Vid => vids.push(file_path),
            MediaType::Etc => {}
        }
    }

    //process images first and then videos so that filtering for duplicates prioritises images first
    res.par_extend(pics.into_par_iter().filter_map(|file_path| {
        PicTile::open(file_path.clone())
            .inspect_err(|e| eprintln!("{file_path} - {e:?}"))
            .ok()
            .map(Tile::Pic)
    }));
    res.par_extend(
        vids.into_par_iter()
            .filter_map(|file_path| {
                vid_tiles_from_path(
                    file_path.clone(),
                    CONFIG.cache_path.clone(),
                    CONFIG.tile_base_res.get(),
                    CONFIG.difference_threshold,
                )
                .inspect(|v| println!("{file_path} - {} tiles", v.len()))
                .inspect_err(|e| eprintln!("{file_path} - {e:?}"))
                .ok()
            })
            .flatten()
            .map(Tile::Vid),
    );

    let mut colour_key_set = HashSet::with_hasher(FxBuildHasher);
    let prev_len = res.len();
    let res: Vec<Tile> = res
        .into_iter()
        .filter(|t| {
            let key = colour_to_key(t.average_colour());
            colour_key_set.insert(key)
        })
        .collect();

    println!(
        "filtered {}% of tiles ({}/{prev_len} tiles, {} left)",
        ((prev_len - res.len()) as f64 / prev_len as f64) * 100.0,
        prev_len - res.len(),
        res.len()
    );

    Ok(res)
}

pub(crate) fn load_synthetic_tiles(limit: NonZero<u64>) -> Vec<Tile> {
    debug_assert!(limit.get() <= 16_777_216, "Max no. of synthetic tiles is 16_777_216");
    (0..limit.get() as i32)
        .into_par_iter()
        .map(|i| {
            let i = i * (16_777_216 / limit.get() as i32);
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
