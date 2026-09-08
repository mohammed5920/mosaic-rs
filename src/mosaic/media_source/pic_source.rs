use std::{path::Path, sync::Arc};

use image::{ImageReader, ImageResult};

pub(crate) struct PicSource {
    pub(crate) pixels: Arc<[u8]>,
    pub(crate) width: u64,
    pub(crate) height: u64,
}

impl PicSource {
    pub(crate) fn open(path: impl AsRef<Path>) -> ImageResult<PicSource> {
        let img = ImageReader::open(path)?.decode()?.into_rgb8();
        Ok(PicSource {
            width: img.width().into(),
            height: img.height().into(),
            pixels: img.into_vec().into(),
        })
    }
}
