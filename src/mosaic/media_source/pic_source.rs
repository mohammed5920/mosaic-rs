use std::{path::Path, sync::Arc};

use image::{
    ImageReader, ImageResult,
    imageops::{FilterType, resize},
};

pub(crate) struct PicSource {
    pub(crate) pixels: Arc<[u8]>,
    pub(crate) width: u64,
    pub(crate) height: u64,
}

impl PicSource {
    pub(crate) fn open(path: impl AsRef<Path>, dscale_factor: u32) -> ImageResult<PicSource> {
        let img = ImageReader::open(path)?.decode()?.into_rgb8();
        Ok(PicSource {
            width: img.width().into(),
            height: img.height().into(),
            pixels: resize(
                &img,
                img.width() / dscale_factor,
                img.height() / dscale_factor,
                FilterType::Nearest,
            )
            .into_vec()
            .into(),
        })
    }
}
