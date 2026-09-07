use camino::Utf8PathBuf;
use image::{ImageReader, ImageResult};

use crate::mosaic::tiles::calc_average_colour;

pub(crate) struct PicTile {
    pub(crate) average_colour: [u8; 3],
    source_path: Utf8PathBuf,
}

impl PicTile {
    pub(crate) fn open(path: Utf8PathBuf) -> ImageResult<PicTile> {
        let decoded = ImageReader::open(&path)?.decode()?;
        Ok(PicTile {
            average_colour: calc_average_colour(&decoded.to_rgb8().into_raw()),
            source_path: path,
        })
    }
}
