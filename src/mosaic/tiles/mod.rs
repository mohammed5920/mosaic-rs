pub(crate) mod pic_tiles;
mod tile_cache;
pub(crate) mod vid_tiles;

use std::{cell::RefCell, collections::HashSet, sync::Arc};

use camino::Utf8PathBuf;
use fast_image_resize::Resizer;
use rayon::prelude::*;
use rustc_hash::FxBuildHasher;

use crate::{
    mosaic::tiles::{
        pic_tiles::PicTile,
        tile_cache::{flush_cache, load_cache, load_pic_tile, load_vid_tiles},
        vid_tiles::VidTile,
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

thread_local! {
    pub(crate) static RESIZER: RefCell<Resizer> = RefCell::new(Resizer::new());
}

#[derive(Clone)]
pub(crate) enum Tile {
    Pic(PicTile),
    Vid(VidTile),
}

impl Tile {
    pub(crate) fn average_colour(&self) -> [u8; 3] {
        match self {
            Tile::Pic(pic_tile) => pic_tile.average_colour,
            Tile::Vid(vid_tile) => vid_tile.average_colour,
        }
    }

    pub(crate) fn frame_count(&self) -> u64 {
        match self {
            Tile::Vid(vid_tile) => {
                debug_assert!(
                    (vid_tile.end_frame_index() as i64 - vid_tile.start_frame_index as i64) >= 0,
                    "{} - starts at {} but ends at {}",
                    vid_tile.source_path,
                    vid_tile.start_frame_index,
                    vid_tile.end_frame_index()
                );
                (vid_tile.end_frame_index() - vid_tile.start_frame_index) as u64
            }
            _ => 1,
        }
    }
}

pub(crate) fn load_tiles(path: &Utf8PathBuf) -> anyhow::Result<Vec<Tile>> {
    let cache = load_cache();

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
    res.par_extend(
        pics.into_par_iter()
            .filter_map(|file_path| load_pic_tile(&cache, file_path.into_string()))
            .map(Tile::Pic),
    );

    let mut vids: Vec<_> = vids
        .into_par_iter()
        .filter_map(|file_path| load_vid_tiles(&cache, Arc::from(file_path)))
        .flatten()
        .map(Tile::Vid)
        .collect();

    //prioritising filtering videos that are longer
    vids.sort_unstable_by_key(|t| t.frame_count());
    res.append(&mut vids);

    let mut colour_key_set = HashSet::with_hasher(FxBuildHasher);
    let prev_len = res.len();
    let mut res: Vec<Tile> = res
        .into_iter()
        .filter(|t| {
            let key = colour_to_key(t.average_colour());
            colour_key_set.insert(key)
        })
        .collect();
    //cheap enough and makes the renderdoc capture look a lot more coherent
    res.sort_unstable_by_key(|t| colour_to_key(t.average_colour()));

    println!(
        "filtered {}% of tiles ({}/{prev_len} tiles, {} tiles / {} frames left)",
        ((prev_len - res.len()) as f64 / prev_len as f64) * 100.0,
        prev_len - res.len(),
        res.len(),
        res.iter().map(|t| t.frame_count()).sum::<u64>()
    );

    flush_cache(cache);
    Ok(res)
}
