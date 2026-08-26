#[allow(clippy::all)]
#[allow(clippy::pedantic)]
use std::{num::NonZero, str::FromStr};

use anyhow::bail;
use camino::Utf8PathBuf;
use ffmpeg_next as ffmpeg;
use mosaic_rs::{
    benchmark, matchmaker::Matchmaker, rendering::to_file::save_to_file, source::Source,
    tiles::load_tiles, video_capture::VideoCapture,
};

const TILE_BASE_RES: u64 = 64;
const SOURCE: &str = "test/source.jpg";
const TILES: &str = "test/vid_tiles/S1";
const SOURCE_DSCALE: NonZero<u32> = NonZero::new(1).unwrap();
const TILE_DEBUG_SIZE: u64 = 2;

fn main() -> anyhow::Result<()> {
    ffmpeg::init()?;
    let Source::Pic(pic_source) = benchmark("loading source", || {
        Source::open(&Utf8PathBuf::from_str(SOURCE).unwrap(), SOURCE_DSCALE)
    })?
    else {
        bail!("video support cannot be tested with this script")
    };
    let tiles = benchmark("loading tiles", || {
        load_tiles(&Utf8PathBuf::from_str(TILES).unwrap(), TILE_BASE_RES)
    })?;
    let matchmaker = benchmark("generating match tree", || Matchmaker::from_tiles(&tiles));
    let made_matches = benchmark("making matches", || {
        matchmaker.matchmake(pic_source.as_pixels())
    });
    benchmark("rendering test image", || {
        save_to_file(
            &pic_source,
            &tiles,
            TILE_DEBUG_SIZE,
            &made_matches,
            "test/result.jpg",
        )
    })?;

    // let mut cap = benchmark("opening vidcap", || {
    //     VideoCapture::open("test/source.mkv".into(), Some(64))
    // })?;

    // for i in 0..10 {
    //     let frame = benchmark(&format!("saving frame {i}"), || cap.read_frame())?
    //         .unwrap_or_else(|| panic!("frame {i} was None"));
    //     image::save_buffer(
    //         format!("test/cap_rs/{i}.jpg"),
    //         frame.pixels.as_flattened(),
    //         frame.width as u32,
    //         frame.height as u32,
    //         image::ColorType::Rgb8,
    //     )?;
    // }

    // benchmark("seeking", || cap.seek_to_frame(86431))?;

    // for i in 86431..86531 {
    //     let frame = benchmark(&format!("saving frame {i}"), || cap.read_frame())?
    //         .unwrap_or_else(|| panic!("frame {i} was None"));
    //     assert!(i == frame.frame_index);
    //     image::save_buffer(
    //         format!("test/cap_rs/{i}.jpg"),
    //         frame.pixels.as_flattened(),
    //         frame.width as u32,
    //         frame.height as u32,
    //         image::ColorType::Rgb8,
    //     )?;
    // }

    Ok(())
}
