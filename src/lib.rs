pub mod file_util;
pub mod matchmaker;
pub mod renderer;
pub mod media_source;
pub mod tiles;
pub mod vidcap;

use std::{path::Path, time::Instant};

use image::{ImageBuffer, Rgb};

pub type RgbBuffer = ImageBuffer<Rgb<u8>, Vec<u8>>;

pub fn benchmark<T>(label: &str, mut function: impl FnMut() -> T) -> T {
    let start = Instant::now();
    let res = function();
    println!("{label} took {:#?}", Instant::now().duration_since(start));
    res
}

pub fn is_fixed_frame_rate(video_path: impl AsRef<Path>) -> Result<bool, ffprobe::FfProbeError> {
    fn parse_fps(fps_str: &str) -> f64 {
        let mut parts = fps_str.split('/');
        let (Some(num), Some(den)) = (parts.next(), parts.next()) else {
            return 0.0;
        };
        let (Ok(num), Ok(den)) = (num.parse::<f64>(), den.parse::<f64>()) else {
            return 0.0;
        };
        if den != 0.0 { num / den } else { 0.0 }
    }
    let info = ffprobe::ffprobe(video_path)?;
    let Some(stream) = info
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("video"))
    else {
        return Ok(false);
    };
    let r_fps = parse_fps(&stream.r_frame_rate);
    let avg_fps = parse_fps(&stream.avg_frame_rate);
    Ok((r_fps - avg_fps).abs() < 0.01)
}
