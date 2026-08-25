pub mod file_io;
pub mod matchmaker;
pub mod rendering;
pub mod source;
pub mod tiles;
pub mod video_capture;

use std::time::Instant;

use image::{ImageBuffer, Rgb};

pub type RgbBuffer = ImageBuffer<Rgb<u8>, Vec<u8>>;
pub const CACHE_DIR: &str = "cache/";

pub fn benchmark<T>(label: &str, mut function: impl FnMut() -> T) -> T {
    let start = Instant::now();
    let res = function();
    println!("{label} took {:#?}", Instant::now().duration_since(start));
    res
}
