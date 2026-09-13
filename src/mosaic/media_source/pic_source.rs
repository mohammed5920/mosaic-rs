use std::path::Path;

use image::{ImageReader, ImageResult};

pub(crate) struct PicSource {
    pub(crate) pixels: Vec<u8>,
    pub(crate) width: u64,
    pub(crate) height: u64,
}

impl PicSource {
    pub(crate) fn new(path: impl AsRef<Path>) -> ImageResult<PicSource> {
        let img = ImageReader::open(path)?
            .with_guessed_format()?
            .decode()?
            .into_rgb8();
        Ok(PicSource {
            width: img.width().into(),
            height: img.height().into(),
            pixels: img.into_vec(),
        })
    }
}
