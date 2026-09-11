pub(crate) mod pic_tiles;
pub(crate) mod vid_tiles;

use std::collections::HashSet;

use camino::Utf8PathBuf;
use rayon::prelude::*;
use rustc_hash::FxBuildHasher;

use crate::{
    mosaic::tiles::{
        pic_tiles::PicTile,
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

    ///panics if not pictile
    pub(crate) fn as_pic(&self) -> &PicTile {
        match self {
            Tile::Pic(p) => p,
            Tile::Vid(_) => panic!("called .as_pic() on a vidtile"),
        }
    }

    ///panics if not vidtile
    pub(crate) fn as_vid(&self) -> &VidTile {
        match self {
            Tile::Pic(_) => panic!("called .as_vid() on a vidtile"),
            Tile::Vid(v) => v,
        }
    }

    pub(crate) fn frame_count(&self) -> u64 {
        match self {
            Tile::Vid(vid_tile) => {
                debug_assert!(
                    (vid_tile.end_frame_index as i64 - vid_tile.start_frame_index as i64) >= 0,
                    "{} - starts at {} but ends at {}",
                    vid_tile.source_path,
                    vid_tile.start_frame_index,
                    vid_tile.end_frame_index
                );
                (vid_tile.end_frame_index - vid_tile.start_frame_index) as u64
            }
            _ => 1,
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
        PicTile::new(file_path.clone())
            .inspect_err(|e| eprintln!("{file_path} - {e:?}"))
            .ok()
            .map(Tile::Pic)
    }));
    let mut vids: Vec<_> = vids
        .into_par_iter()
        .filter_map(|file_path| {
            vid_tiles_from_path(file_path.clone())
                .inspect(|v| println!("{file_path} - {} tiles", v.len()))
                .inspect_err(|e| eprintln!("{file_path} - {e:?}"))
                .ok()
        })
        .flatten()
        .map(Tile::Vid)
        .collect();

    //prioritising filtering videos that are longer
    vids.par_sort_unstable_by_key(|t| t.frame_count());
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
    res.par_sort_unstable_by_key(|t| colour_to_key(t.average_colour()));

    println!(
        "filtered {}% of tiles ({}/{prev_len} tiles, {} tiles / {} frames left)",
        ((prev_len - res.len()) as f64 / prev_len as f64) * 100.0,
        prev_len - res.len(),
        res.len(),
        res.iter().map(|t| t.frame_count()).sum::<u64>()
    );

    Ok(res)
}
