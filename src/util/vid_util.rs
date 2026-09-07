use ffmpeg_next as ffmpeg;
use std::path::Path;

pub(crate) fn is_fixed_frame_rate(
    video_path: impl AsRef<Path>,
) -> Result<bool, ffprobe::FfProbeError> {
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

///returns None on division by zero (invalid rational bases)
pub(crate) fn frame_idx_from_pts(
    stream_time_base: ffmpeg::Rational,
    stream_frame_rate: ffmpeg::Rational,
    stream_start_time: i64,
    frame_pts: i64,
) -> Option<i64> {
    if frame_pts == ffmpeg::ffi::AV_NOPTS_VALUE {
        return None;
    }
    let tb_num = stream_time_base.numerator() as f64;
    let tb_den = stream_time_base.denominator() as f64;
    let fr_num = stream_frame_rate.numerator() as f64;
    let fr_den = stream_frame_rate.denominator() as f64;
    if tb_den == 0.0 || fr_num == 0.0 {
        return None;
    }
    let start_time = if stream_start_time == ffmpeg::ffi::AV_NOPTS_VALUE {
        0.0
    } else {
        stream_start_time as f64
    };
    let relative_pts = frame_pts as f64 - start_time;
    let frame_index_exact = (relative_pts * tb_num * fr_num) / (tb_den * fr_den);
    Some(frame_index_exact.round() as i64)
}

///take a full size video frame plane, crop out the extra padding and make it match out_w * out_h * bpp exactly
pub(crate) fn extract_plane(
    data: &[u8],
    stride: u64,
    x_start: u64,
    y_start: u64,
    out_w: u64,
    out_h: u64,
    bpp: u64,
) -> Vec<u8> {
    let mut out = vec![0u8; (out_w * out_h * bpp) as usize];
    for row in 0..out_h {
        let src_row = y_start + row;
        let row_byte_start = (src_row * stride + x_start * bpp) as usize;
        let row_len = (out_w * bpp) as usize;
        out[(row * out_w * bpp) as usize..][..row_len]
            .copy_from_slice(&data[row_byte_start..row_byte_start + row_len]);
    }
    out
}
