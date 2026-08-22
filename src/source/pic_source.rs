use std::{num::NonZeroU32, path::Path};

use image::{
    ImageBuffer, ImageReader, ImageResult, Rgb,
    imageops::{FilterType, resize},
};

pub struct PicSource(ImageBuffer<Rgb<u8>, Vec<u8>>);

impl PicSource {
    pub fn open(path: impl AsRef<Path>, dscale_factor: NonZeroU32) -> ImageResult<PicSource> {
        let img = ImageReader::open(path)?.decode()?.into_rgb8();
        Ok(PicSource(resize(
            &img,
            img.width() / dscale_factor,
            img.height() / dscale_factor,
            FilterType::Nearest,
        )))
    }
}
