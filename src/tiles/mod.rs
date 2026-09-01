pub mod pic_tiles;
pub mod vid_tiles;

use camino::Utf8PathBuf;
use rayon::prelude::*;

use crate::{
    file_util::{MediaType, check_supported_extension, walk_dir},
    tiles::{
        pic_tiles::PicTile,
        vid_tiles::{VidTile, vid_tiles_from_path},
    },
};

pub fn calc_average_colour(pixels: &[u8]) -> [u8; 3] {
    debug_assert!(pixels.len()%3 == 0, "pixel array is not divisible by 3 (not valid RGB)");
    let mut sums = [0u64; 3];
    for pixel in pixels.as_chunks::<3>().0.iter() {
        sums[0] += pixel[0] as u64;
        sums[1] += pixel[1] as u64;
        sums[2] += pixel[2] as u64;
    }
    [
        (sums[0] / pixels.len() as u64) as u8,
        (sums[1] / pixels.len() as u64) as u8,
        (sums[2] / pixels.len() as u64) as u8,
    ]
}

pub enum Tile {
    Pic(PicTile),
    Vid(VidTile),
}

impl Tile {
    pub fn average_colour(&self) -> [u8; 3] {
        match self {
            Tile::Pic(pic_tile) => pic_tile.average_colour,
            Tile::Vid(vid_tile) => vid_tile.average_colour,
        }
    }
}

pub fn load_tiles(path: &Utf8PathBuf, tile_base_res: u64) -> anyhow::Result<Vec<Tile>> {
    let res = walk_dir(path)?
        .into_par_iter()
        .filter_map(|file_path| match check_supported_extension(&file_path) {
            MediaType::Pic => PicTile::open(file_path.clone())
                .inspect_err(|e| eprintln!("{file_path} - {e:?}"))
                .ok()
                .map(|t| vec![Tile::Pic(t)]),

            MediaType::Vid => vid_tiles_from_path(file_path.clone(), tile_base_res)
                .inspect_err(|e| eprintln!("{file_path} - {e:?}"))
                .ok()
                .map(|ts| ts.into_iter().map(Tile::Vid).collect()),

            MediaType::Etc => None,
        })
        .flatten()
        .collect::<Vec<_>>();

    Ok(res)
}
