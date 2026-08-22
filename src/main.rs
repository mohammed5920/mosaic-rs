#[allow(clippy::all)]
#[allow(clippy::pedantic)]
use std::{num::NonZero, str::FromStr};

use camino::Utf8PathBuf;
use mosaic_rs::{benchmark, source::Source, tiles::load_tiles};

const SOURCE: &str = "test/source.jpg";
const SOURCE_DSCALE: NonZero<u32> = NonZero::new(1).unwrap();
const TILES: &str = "test/pic_tiles";
const TILE_BASE_RES: u64 = 64;

fn main() -> anyhow::Result<()> {
    let source = benchmark("loading source", || {
        Source::open(&Utf8PathBuf::from_str(SOURCE).unwrap(), SOURCE_DSCALE)
    })?;
    let tiles = benchmark("loading tiles", || {
        load_tiles(&Utf8PathBuf::from_str(TILES).unwrap(), TILE_BASE_RES)
    })?;
    Ok(())
}
