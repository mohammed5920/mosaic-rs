use camino::Utf8PathBuf;
use image::{DynamicImage, ImageReader, ImageResult, imageops::FilterType};

use crate::tiles::calc_average_colour;

pub struct PicTile {
    pub average_colour: [u8; 3],
    source_path: Utf8PathBuf,
    //NOTE: remove this when renderer is started, to save RAM
    _cached_source: DynamicImage,
}

impl PicTile {
    pub fn open(path: Utf8PathBuf) -> ImageResult<PicTile> {
        let decoded = ImageReader::open(&path)?.decode()?;
        Ok(PicTile {
            average_colour: calc_average_colour(decoded.to_rgb8().into_raw().as_chunks::<3>().0),
            _cached_source: decoded,
            source_path: path,
        })
    }

    pub fn load_as_res(&self, res: u64) -> ImageResult<Vec<u8>> {
        if res == 1 {
            Ok((self.average_colour).to_vec())
        } else {
            let decoded = &self._cached_source;
            let (x, y, w, h) = {
                let (w, h) = (decoded.width(), decoded.height());
                let min_dim = w.min(h);
                let crop_x_offset = (w - min_dim) / 2;
                let crop_y_offset = (h - min_dim) / 2;
                (crop_x_offset, crop_y_offset, min_dim, min_dim)
            };
            let cropped = decoded.crop_imm(x, y, w, h);
            Ok(cropped
                .resize(res as u32, res as u32, FilterType::Triangle)
                .into_rgb8()
                .into_raw())
        }
    }
}
