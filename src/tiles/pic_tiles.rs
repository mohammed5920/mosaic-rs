use camino::Utf8PathBuf;
use image::{DynamicImage, ImageReader, ImageResult, imageops::FilterType};

use crate::tiles::{get_crop_offsets, imagebuffer_average};

pub struct PicTile {
    source_path: Utf8PathBuf,
    //NOTE: remove this when renderer is started, to save RAM
    _cached_source: DynamicImage,
    pub avg_colour: [u8; 3],
}

impl PicTile {
    pub fn from_path(path: Utf8PathBuf) -> ImageResult<PicTile> {
        let decoded = ImageReader::open(&path)?.decode()?;
        Ok(PicTile {
            avg_colour: imagebuffer_average(&decoded.to_rgb8()),
            _cached_source: decoded,
            source_path: path,
        })
    }

    pub fn load_as_res(&self, res: u64) -> ImageResult<Vec<u8>> {
        if res == 1 {
            Ok((self.avg_colour).to_vec())
        } else {
            let decoded = &self._cached_source;
            let (x, y, w, h) = get_crop_offsets(decoded.width(), decoded.height());
            let cropped = decoded.crop_imm(x, y, w, h);
            Ok(cropped
                .resize(res as u32, res as u32, FilterType::Triangle)
                .into_rgb8()
                .into_raw())
        }
    }
}
