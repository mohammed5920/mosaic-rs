use std::{num::NonZero, path::Path};

use image::{
    ImageReader, ImageResult,
    imageops::{FilterType, resize},
};

use crate::RgbBuffer;

pub struct PicSource(RgbBuffer);

impl PicSource {
    pub fn from_path(
        path: impl AsRef<Path>,
        dscale_factor: NonZero<u32>,
    ) -> ImageResult<PicSource> {
        let img = ImageReader::open(path)?.decode()?.into_rgb8();
        Ok(PicSource(resize(
            &img,
            img.width() / dscale_factor,
            img.height() / dscale_factor,
            FilterType::Nearest,
        )))
    }

    pub fn as_pixels(&self) -> &[[u8; 3]] {
        self.0.as_chunks::<3>().0
    }

    ///(width, height)
    pub fn dimensions(&self) -> (u64, u64) {
        (self.0.width() as u64, self.0.height() as u64)
    }
}
