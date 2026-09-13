use std::fs;

use anyhow::{Context, bail};
use camino::Utf8PathBuf;
use fast_image_resize::{
    FilterType, PixelType, ResizeAlg::Convolution, ResizeOptions, images::Image as FRImage,
};
use image::{DynamicImage, ImageReader, RgbImage};
use imohash::Hasher as ImoHasher;
use yuv::{
    YuvBiPlanarImageMut, YuvChromaSubsampling, YuvConversionMode, YuvRange, YuvStandardMatrix,
    rgb_to_yuv_nv12,
};

use crate::{
    config::CONFIG,
    mosaic::tiles::{RESIZER, calc_average_colour},
    streamer::tile_stores::StoreFrame,
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
        .with_context(|| format!("error opening {path}"))?
        .with_guessed_format()
        .with_context(|| format!("error guessing {path}"))?
        .decode()
        .with_context(|| format!("error decoding {path}"))?;
    Ok(res)
}

fn image_to_tile(rgb_image: &RgbImage, tile_size: u64) -> FRImage<'_> {
    let min_dim = rgb_image.width().min(rgb_image.height());
    let x_offset = (rgb_image.width() - min_dim) / 2;
    let y_offset = (rgb_image.height() - min_dim) / 2;

    let mut dst_image = FRImage::new(tile_size as u32, tile_size as u32, PixelType::U8x3);

    let mut options = ResizeOptions::new().crop(
        x_offset as f64,
        y_offset as f64,
        min_dim as f64,
        min_dim as f64,
    );
    options.algorithm = Convolution(FilterType::Bilinear);

    RESIZER
        .with_borrow_mut(|r| r.resize(rgb_image, &mut dst_image, &options))
        .expect("resizing options should be set correctly");

    dst_image
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
                average_colour: calc_average_colour(&decoded.into_rgb8().into_raw()),
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
        let rgb_image = open_image(&self.source_path)?.into_rgb8();
        let rgb_tile = image_to_tile(&rgb_image, tile_size);
        let mut planar_image = YuvBiPlanarImageMut::<u8>::alloc(
            tile_size as u32,
            tile_size as u32,
            YuvChromaSubsampling::Yuv420,
        );
        rgb_to_yuv_nv12(
            &mut planar_image,
            rgb_tile.buffer(),
            (tile_size as u32) * 3,
            YuvRange::Limited,
            YuvStandardMatrix::Bt601,
            YuvConversionMode::Fast,
        )?;

        Ok(StoreFrame::new(
            planar_image.y_plane.borrow().into(),
            planar_image.uv_plane.borrow().into(),
            tile_size,
        ))
    }
}
