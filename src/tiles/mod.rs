pub mod pic_tiles;
pub mod vid_tiles;

use camino::Utf8PathBuf;
use image::{ImageBuffer, Rgb};
use rayon::prelude::*;

use crate::{MediaType, check_supported_extension, tiles::pic_tiles::PicTile, walk_dir};

//NOTE: this is single threaded because we parallelise the creation of many tiles instead
pub fn imagebuffer_average(imgbuff: &ImageBuffer<Rgb<u8>, Vec<u8>>) -> [u8; 3] {
    let mut sums = [0u64; 3];
    for chunk in imgbuff.as_chunks::<3>().0 {
        sums[0] += chunk[0] as u64;
        sums[1] += chunk[1] as u64;
        sums[2] += chunk[2] as u64;
    }
    let n = (imgbuff.len() / 3) as u64;
    [
        (sums[0] / n) as u8,
        (sums[1] / n) as u8,
        (sums[2] / n) as u8,
    ]
}

pub enum Tile {
    Pic(PicTile),
    Vid,
}

pub fn load_tiles(path: &Utf8PathBuf, tile_base_res: u64) -> anyhow::Result<Vec<Tile>> {
    let res = walk_dir(path)?
        .into_par_iter()
        .filter_map(|file_path| match check_supported_extension(&file_path) {
            MediaType::Pic => PicTile::open(file_path.clone())
                .inspect_err(|e| eprintln!("{file_path} - {e:?}"))
                .ok()
                .map(Tile::Pic),
            MediaType::Vid => Some(Tile::Vid),
            MediaType::Etc => None,
        })
        .collect::<Vec<_>>();

    Ok(res)
}
