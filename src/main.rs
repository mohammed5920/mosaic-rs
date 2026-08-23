#[allow(clippy::all)]
#[allow(clippy::pedantic)]
use std::{num::NonZero, str::FromStr};

use anyhow::bail;
use camino::Utf8PathBuf;
use mosaic_rs::{
    benchmark, matchmaker::Matchmaker, rendering::to_file::save_as_jpeg, source::Source,
    tiles::load_tiles,
};

const SOURCE: &str = "test/source.jpg";
const TILES: &str = "test/pic_tiles";
const TILE_BASE_RES: u64 = 64;
const SOURCE_DSCALE: NonZero<u32> = NonZero::new(1).unwrap();
const TILE_DEBUG_SIZE: u64 = 32;

fn main() -> anyhow::Result<()> {
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
        save_as_jpeg(
            &pic_source,
            &tiles,
            TILE_DEBUG_SIZE,
            &made_matches,
            "test/result.jpg",
        )
    })?;
    Ok(())
}
