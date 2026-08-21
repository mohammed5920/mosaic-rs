use std::{num::NonZeroU32, path::Path};

use image::{
    ImageBuffer, ImageReader, Rgb,
    imageops::{FilterType, resize},
};

pub fn load_source(path: impl AsRef<Path>, dscale_factor: NonZeroU32) -> anyhow::Result<ImageBuffer<Rgb<u8>, Vec<u8>>> {
    let img = ImageReader::open(path)?.decode()?.into_rgb8();
    Ok(resize(
        &img,
        img.width() / dscale_factor,
        img.height() / dscale_factor,
        FilterType::Nearest,
    ))
}
