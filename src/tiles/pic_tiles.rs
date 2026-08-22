use camino::Utf8PathBuf;
use image::{ImageReader, ImageResult};

use crate::tiles::imagebuffer_average;

pub struct PicTile {
    pub source: Utf8PathBuf,
    pub avg_colour: [u8; 3],
}

impl PicTile {
    pub fn open(path: Utf8PathBuf) -> ImageResult<PicTile> {
        Ok(PicTile {
            avg_colour: imagebuffer_average(&ImageReader::open(&path)?.decode()?.into_rgb8()),
            source: path,
        })
    }
}
