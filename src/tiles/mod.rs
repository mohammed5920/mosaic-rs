pub mod pic_tiles;
pub mod vid_tiles;

use camino::Utf8PathBuf;
use rayon::prelude::*;

use crate::{
    RgbBuffer,
    file_io::{MediaType, check_supported_extension, walk_dir},
    tiles::pic_tiles::PicTile,
};

//NOTE: this is single threaded because we parallelise the creation of many tiles instead
pub fn imagebuffer_average(imgbuff: &RgbBuffer) -> [u8; 3] {
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

///(x, y, width, height)
pub fn get_crop_offsets(width: u32, height: u32) -> (u32, u32, u32, u32) {
    let min_dim = width.min(height);
    let crop_x_offset = (width - min_dim) / 2;
    let crop_y_offset = (height - min_dim) / 2;
    (crop_x_offset, crop_y_offset, min_dim, min_dim)
}

pub enum Tile {
    Pic(PicTile),
    Vid,
}

impl Tile {
    pub fn average_colour(&self) -> [u8; 3] {
        match self {
            Tile::Pic(pic_tile) => pic_tile.avg_colour,
            Tile::Vid => todo!(),
        }
    }
}

pub fn load_tiles(path: &Utf8PathBuf, tile_base_res: u64) -> anyhow::Result<Vec<Tile>> {
    let res = walk_dir(path)?
        .into_par_iter()
        .filter_map(|file_path| match check_supported_extension(&file_path) {
            MediaType::Pic => PicTile::from_path(file_path.clone())
                .inspect_err(|e| eprintln!("{file_path} - {e:?}"))
                .ok()
                .map(Tile::Pic),
            MediaType::Vid => todo!(),
            MediaType::Etc => None,
        })
        .collect::<Vec<_>>();

    Ok(res)
}
