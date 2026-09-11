use std::fs;

use anyhow::{Context, bail};
use camino::Utf8PathBuf;
use image::{DynamicImage, ImageReader};
use imohash::Hasher as ImoHasher;
use yuv::{
    YuvChromaSubsampling, YuvConversionMode, YuvPlanarImageMut, YuvRange, YuvStandardMatrix,
    rgb_to_yuv420,
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
                average_colour: calc_average_colour(&decoded.to_rgb8().into_raw()),
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
        let decoded = open_image(&self.source_path)?;
        // decoded.resize(nwidth, nheight, filter);
        // let mut planar_image = YuvPlanarImageMut::<u8>::alloc(
        //     tile_size as u32,
        //     tile_size as u32,
        //     YuvChromaSubsampling::Yuv420,
        // );
        // rgb_to_yuv420(
        //     &mut planar_image,
        //     &src_bytes,
        //     rgba_stride as u32,
        //     YuvRange::Limited,
        //     YuvStandardMatrix::Bt601,
        //     YuvConversionMode::Fast
        // );
        bail!("")
    }
}
