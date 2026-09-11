use std::fs;

use anyhow::{Context, bail};
use camino::Utf8PathBuf;
use image::{DynamicImage, ImageReader, imageops::FilterType};
use imohash::Hasher as ImoHasher;
use yuv::{
    YuvBiPlanarImageMut, YuvChromaSubsampling, YuvConversionMode, YuvRange,
    YuvStandardMatrix, rgb_to_yuv_nv12,
};

use crate::{
    config::CONFIG, mosaic::tiles::calc_average_colour, streamer::tile_stores::StoreFrame,
};

#[derive(bincode::Decode, bincode::Encode, Clone)]
pub(crate) struct PicTile {
    pub(crate) average_colour: [u8; 3],
    pub(crate) source_path: String,
}

#[derive(bincode::Decode, bincode::Encode)]
enum CachedPicTile {
    Valid(PicTile),
    Unreadable { reason: String },
}

fn open_image(path: &str) -> anyhow::Result<DynamicImage> {
    let res = ImageReader::open(path)
        .with_context(|| format!("Error opening {path}"))?
        .with_guessed_format()
        .with_context(|| format!("Error guessing {path}"))?
        .decode()
        .with_context(|| format!("Error decoding {path}"))?;
    Ok(res)
}

fn image_to_tile(image: &DynamicImage, tile_size: u64) -> DynamicImage {
    let min_dim = image.width().min(image.height());
    let x_offset = (image.width() - min_dim) / 2;
    let y_offset = (image.height() - min_dim) / 2;
    let cropped = image.crop_imm(x_offset, y_offset, min_dim as u32, min_dim as u32);
    cropped.resize(tile_size as u32, tile_size as u32, FilterType::Nearest)
}

impl PicTile {
    pub(crate) fn new(path: Utf8PathBuf) -> anyhow::Result<PicTile> {
        let hasher = ImoHasher::new();
        let hash = hasher.sum_file(path.as_str())?;
        let mut cache_entry_path = CONFIG.cache_path.clone();
        cache_entry_path.push(hash.to_string());

        if let Some(cached_tile) = 'cache_check: {
            let Ok(cached_bytes) = fs::read(&cache_entry_path) else {
                break 'cache_check None;
            };
            let Ok((decoded, _)) = bincode::decode_from_slice::<CachedPicTile, _>(
                &cached_bytes,
                bincode::config::standard(),
            ) else {
                break 'cache_check None;
            };
            match decoded {
                CachedPicTile::Valid(PicTile { average_colour, .. }) => Some(PicTile {
                    average_colour,
                    source_path: path.to_string(),
                }),
                CachedPicTile::Unreadable { reason } => bail!(reason),
            }
        } {
            return Ok(cached_tile);
        }

        let res = match open_image(path.as_str()) {
            Ok(decoded) => CachedPicTile::Valid(PicTile {
                average_colour: calc_average_colour(
                    &image_to_tile(&decoded, CONFIG.tile_base_res.get())
                        .into_rgb8()
                        .into_raw(),
                ),
                source_path: path.to_string(),
            }),
            Err(e) => CachedPicTile::Unreadable {
                reason: e.to_string(),
            },
        };

        let encoded = bincode::encode_to_vec(&res, bincode::config::standard())
            .expect("cache should be encoded to vec");
        fs::write(&cache_entry_path, encoded).expect("cache should be written");

        match res {
            CachedPicTile::Valid(pic_tile) => Ok(pic_tile),
            CachedPicTile::Unreadable { reason } => bail!(reason),
        }
    }

    pub(crate) fn stream_in(&self, tile_size: u64) -> anyhow::Result<StoreFrame> {
        let rgb_tile = image_to_tile(&open_image(&self.source_path)?, tile_size);
        let mut planar_image: YuvBiPlanarImageMut<'_, u8> = YuvBiPlanarImageMut::<u8>::alloc(
            tile_size as u32,
            tile_size as u32,
            YuvChromaSubsampling::Yuv420,
        );
        rgb_to_yuv_nv12(
            &mut planar_image,
            &rgb_tile.into_rgb8().into_raw(),
            (tile_size as u32) * 3,
            YuvRange::Limited,
            YuvStandardMatrix::Bt601,
            YuvConversionMode::Fast,
        )?;
        let YuvBiPlanarImageMut {
            y_plane, uv_plane, ..
        } = planar_image;
        Ok(StoreFrame::new(
            y_plane.borrow().into(),
            uv_plane.borrow().into(),
            tile_size,
        ))
    }
}
