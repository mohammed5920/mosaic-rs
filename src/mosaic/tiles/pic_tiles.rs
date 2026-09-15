use anyhow::anyhow;
use fast_image_resize::{
    FilterType, PixelType, ResizeAlg::Convolution, ResizeOptions, images::Image as FRImage,
};
use image::{DynamicImage, ImageReader, RgbImage};
use yuv::{
    BufferStoreMut, YuvBiPlanarImageMut, YuvChromaSubsampling, YuvConversionMode, YuvRange,
    YuvStandardMatrix, rgb_to_yuv_nv12,
};

use crate::{
    mosaic::tiles::{RESIZER, calc_average_colour},
    streamer::tile_stores::StoreFrame,
};

#[derive(Clone)]
pub(crate) struct PicTile {
    pub(crate) average_colour: [u8; 3],
    pub(crate) source_path: String,
}

fn open_image(path: &str) -> Result<DynamicImage, String> {
    let res = ImageReader::open(path)
        .map_err(|e| format!("error opening {path}: {e:?}"))?
        .with_guessed_format()
        .map_err(|e| format!("error guessing {path}: {e:?}"))?
        .decode()
        .map_err(|e| format!("error decoding {path}: {e:?}"))?;
    Ok(res)
}

fn image_to_tile(rgb_image: &RgbImage, tile_size: u64) -> FRImage<'_> {
    let min_dim = rgb_image.width().min(rgb_image.height());
    let x_offset = (rgb_image.width() - min_dim) / 2;
    let y_offset = (rgb_image.height() - min_dim) / 2;

    let mut dst_image = FRImage::new(tile_size as u32, tile_size as u32, PixelType::U8x3);

    let mut options = ResizeOptions::new().crop(
        x_offset as f64,
        y_offset as f64,
        min_dim as f64,
        min_dim as f64,
    );
    options.algorithm = Convolution(FilterType::Bilinear);

    RESIZER
        .with_borrow_mut(|r| r.resize(rgb_image, &mut dst_image, &options))
        .expect("resizing options should be set correctly");

    dst_image
}

impl PicTile {
    pub(crate) fn new(path: &str) -> Result<PicTile, String> {
        Ok(PicTile {
            average_colour: calc_average_colour(&open_image(path)?.into_rgb8().into_raw()),
            source_path: path.to_string(),
        })
    }

    pub(crate) fn frame_count(&self) -> u64 {
        1
    }

    pub(crate) fn stream_in(&self, tile_size: u64) -> anyhow::Result<StoreFrame> {
        let rgb_image = open_image(&self.source_path)
            .map_err(|e| anyhow!(e))?
            .into_rgb8();
        let rgb_tile = image_to_tile(&rgb_image, tile_size);
        let mut planar_image = YuvBiPlanarImageMut::<u8>::alloc(
            tile_size as u32,
            tile_size as u32,
            YuvChromaSubsampling::Yuv420,
        );
        rgb_to_yuv_nv12(
            &mut planar_image,
            rgb_tile.buffer(),
            (tile_size as u32) * 3,
            YuvRange::Limited,
            YuvStandardMatrix::Bt601,
            YuvConversionMode::Fast,
        )?;

        let (BufferStoreMut::Owned(y_plane), BufferStoreMut::Owned(cbcr_plane)) =
            (planar_image.y_plane, planar_image.uv_plane)
        else {
            unreachable!("buffers allocated by YuvBiPlanarImageMut are always owned")
        };

        Ok(StoreFrame::new(y_plane, cbcr_plane, tile_size))
    }
}
