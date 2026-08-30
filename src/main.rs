#[allow(clippy::all)]
#[allow(clippy::pedantic)]
use std::{num::NonZero, str::FromStr};

use anyhow::bail;
use camino::Utf8PathBuf;
use ffmpeg_next as ffmpeg;
use mosaic_rs::{
    benchmark, matchmaker::Matchmaker, rendering::event_loop::start_mosaic_loop, source::Source,
    tiles::load_tiles,
};

const TILE_BASE_RES: u64 = 64;
const SOURCE: &str = "test/source.jpg";
const TILES: &str = "test/vid_tiles/S1";
const SOURCE_DSCALE: NonZero<u32> = NonZero::new(1).unwrap();

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

    start_mosaic_loop()
}
