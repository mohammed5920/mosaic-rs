use std::collections::{HashMap, hash_map::Entry};

use image::RgbImage;
use rustc_hash::FxBuildHasher;

use crate::{source::pic_source::PicSource, tiles::Tile};

pub fn save_to_file(
    source: &PicSource,
    tiles: &[Tile],
    tile_size: u64,
    made_matches: &[i32],
    filename: &str,
) -> anyhow::Result<()> {
    let (src_width, src_height) = source.dimensions();
    let dest_width = src_width * tile_size;

    let mut tile_cache = HashMap::with_hasher(FxBuildHasher);
    let mut res_buffer = RgbImage::new(
        (src_width * tile_size) as u32,
        (src_height * tile_size) as u32,
    );

    for (i, tile_idx) in made_matches.iter().copied().enumerate() {
        let col = i as u64 % src_width;
        let row = i as u64 / src_width;

        let tile_img = {
            if let Entry::Vacant(e) = tile_cache.entry(tile_idx) {
                e.insert(match &tiles[tile_idx as usize] {
                    Tile::Pic(pic_tile) => pic_tile.load_as_res(tile_size)?,
                    Tile::Vid(vid_tile) => vid_tile.first_frame_as_res(tile_size),
                });
            }
            tile_cache.get(&tile_idx).unwrap()
        };

        for y in 0..tile_size {
            let src_start = y * tile_size * 3;
            let src_end = src_start + (tile_size * 3);
            let src_slice = &tile_img[src_start as usize..src_end as usize];
            let dest_y = (row * tile_size) + y;
            let dest_x = col * tile_size;
            let dest_idx = (dest_y * dest_width + dest_x) * 3;
            let dest_slice = &mut res_buffer.as_flat_samples_mut().samples
                [dest_idx as usize..(dest_idx + (tile_size * 3)) as usize];
            dest_slice.copy_from_slice(src_slice);
        }
    }

    res_buffer.save(filename)?;
    Ok(())
}
