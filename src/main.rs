#[allow(clippy::all)]
#[allow(clippy::pedantic)]
mod mosaic;
use std::num::NonZero;

use crate::mosaic::source::load_source;

const SOURCE: &str = "test/source.jpg";
const TILES: &str = "test/pic_tiles";
const DSCALE_FACTOR: NonZero<u32> = NonZero::new(1).unwrap();

fn main() -> anyhow::Result<()> {
    let source = load_source(SOURCE, DSCALE_FACTOR)?;
    Ok(())
}
