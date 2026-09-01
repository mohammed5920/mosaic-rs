use camino::Utf8PathBuf;
use image::{ImageReader, ImageResult};

use crate::tiles::calc_average_colour;

pub struct PicTile {
    pub average_colour: [u8; 3],
    pub source_path: Utf8PathBuf,
}

impl PicTile {
    pub fn open(path: Utf8PathBuf) -> ImageResult<PicTile> {
        let decoded = ImageReader::open(&path)?.decode()?;
        Ok(PicTile {
            average_colour: calc_average_colour(&decoded.to_rgb8().into_raw()),
            source_path: path,
        })
    }
}
