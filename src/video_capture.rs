use std::path::Path;

use camino::Utf8PathBuf;
use ffmpeg_next as ffmpeg;

//it is sometimes cheaper to fast forward a video to a given frame idx
//rather than explicitly seek to it
//depending on the gap between where the current frame is vs the target frame
//how big is that gap? not easily discoverable, changes per video and sometimes while playing
//so this is a best effort guess
const HARD_SEEK_THRESHOLD_SECONDS: i64 = 4;

pub struct VideoCaptureFrame {
    pub width: u64,
    pub height: u64,
    pub pixels: Vec<[u8; 3]>,
    pub frame_index: i64,
}

impl VideoCaptureFrame {
    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<(), image::ImageError> {
        image::save_buffer(
            path,
            self.pixels.as_flattened(),
            self.width as u32,
            self.height as u32,
            image::ColorType::Rgb8,
        )
    }
}

pub fn frame_idx_from_pts(
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

pub struct VideoCapture {
    best_video_stream_index: usize,
    stream_time_base: ffmpeg::Rational,
    stream_frame_rate: ffmpeg::Rational,
    stream_start_time: i64,

    last_decoded_frame_index: Option<i64>,
    cropped_square_size: Option<u64>, //initialise the scaler to output small frames directly, faster than scaling after the fact
    is_eof_sent: bool,

    path: Utf8PathBuf,
    input: ffmpeg::format::context::Input,
    decoder: ffmpeg::decoder::Video,
    scaler: ffmpeg::software::scaling::Context,
}

impl VideoCapture {
    pub fn open(
        path: Utf8PathBuf,
        cropped_square_size: Option<u64>,
    ) -> Result<Self, ffmpeg::Error> {
        let input = ffmpeg::format::input(&path)?;

        let stream = input
            .streams()
            .best(ffmpeg::media::Type::Video)
            .ok_or_else(|| {
                eprintln!("{path} - no valid streams found");
                ffmpeg::Error::StreamNotFound
            })?;
        let stream_index = stream.index();
        let stream_start_time = stream.start_time();
        let stream_frame_rate = stream.avg_frame_rate();
        let stream_time_base = stream.time_base();

        let context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
        let decoder = context.decoder().video()?;

        let in_format = decoder.format();
        let in_width = decoder.width();
        let in_height = decoder.height();

        let (out_w, out_h) = cropped_square_size
            .map(|s| {
                let scaling_factor = (in_width as f64 / s as f64).min(in_height as f64 / s as f64);
                (
                    (in_width as f64 / scaling_factor) as u32,
                    (in_height as f64 / scaling_factor) as u32,
                )
            })
            .unwrap_or((in_width, in_height));

        let scaler = ffmpeg::software::scaling::Context::get(
            in_format,
            in_width,
            in_height,
            ffmpeg::format::Pixel::RGB24,
            out_w,
            out_h,
            ffmpeg::software::scaling::Flags::FAST_BILINEAR,
        )?;

        Ok(Self {
            best_video_stream_index: stream_index,
            stream_time_base,
            stream_frame_rate,
            stream_start_time,

            last_decoded_frame_index: None,
            cropped_square_size,
            is_eof_sent: false,

            path,
            input,
            decoder,
            scaler,
        })
    }

    fn read_raw_frame(&mut self) -> Result<Option<(i64, ffmpeg::frame::Video)>, ffmpeg::Error> {
        let mut raw_frame = ffmpeg::frame::Video::empty();

        //one packet can represent 0..N frames... so we have to dance a little bit
        let res = 'decode: loop {
            //check if frame is buffered
            if self.decoder.receive_frame(&mut raw_frame).is_ok() {
                break 'decode Ok(Some(raw_frame));
            }

            //no buffered frame, were we at the end of the file?
            if self.is_eof_sent {
                break 'decode Ok(None);
            }

            //no we weren't, so let's read a bit more
            let mut found_packet = false;
            for (stream, packet) in self.input.packets() {
                if stream.index() == self.best_video_stream_index {
                    self.decoder.send_packet(&packet)?;
                    found_packet = true;
                    break;
                }
            }

            //now we're at the end of the file, notify the decoder
            if !found_packet {
                self.decoder.send_eof()?;
                self.is_eof_sent = true;
            }
        };

        let raw_frame = match res {
            Ok(None) => return Ok(None),
            Ok(Some(raw_frame)) => raw_frame,
            Err(e) => {
                eprintln!("{} - {e} while seeking", self.path);
                return Err(e);
            }
        };
        let Some(frame_idx) = raw_frame.timestamp().and_then(|ts| {
            frame_idx_from_pts(
                self.stream_time_base,
                self.stream_frame_rate,
                self.stream_start_time,
                ts,
            )
        }) else {
            eprintln!("{} - couldn't calculate frame_idx", self.path);
            return Err(ffmpeg::Error::InvalidData);
        };
        self.last_decoded_frame_index = Some(frame_idx);
        Ok(Some((frame_idx, raw_frame)))
    }

    pub fn read_frame(&mut self) -> Result<Option<VideoCaptureFrame>, ffmpeg::Error> {
        let (frame_idx, raw_frame) = match self.read_raw_frame() {
            Ok(None) => return Ok(None),
            Err(e) => return Err(e),
            Ok(Some(raw_frame)) => raw_frame,
        };
        let mut rgb_frame = ffmpeg::frame::Video::empty();
        self.scaler.run(&raw_frame, &mut rgb_frame)?;

        //stride = width + alignment padding
        let stride = rgb_frame.stride(0) as u64;
        let data = rgb_frame.data(0);
        let frame_width = rgb_frame.width() as u64;
        let frame_height = rgb_frame.height() as u64;

        let (out_width, out_height, x_start, y_start) = match self.cropped_square_size {
            Some(size) => {
                let size = size.min(frame_width).min(frame_height);
                let x_start = (frame_width - size) / 2;
                let y_start = (frame_height - size) / 2;
                (size, size, x_start, y_start)
            }
            None => (frame_width, frame_height, 0, 0),
        };

        let mut pixels = vec![[0u8; 3]; (out_width * out_height) as usize];

        for row in 0..out_height {
            let src_row = y_start + row;
            let row_byte_start = (src_row * stride + x_start * 3) as usize;
            let row_bytes = &data[row_byte_start..row_byte_start + (out_width * 3) as usize];

            for (col, chunk) in row_bytes.as_chunks::<3>().0.iter().enumerate() {
                pixels[(row * out_width) as usize + col] = [chunk[0], chunk[1], chunk[2]];
            }
        }

        Ok(Some(VideoCaptureFrame {
            width: out_width,
            height: out_height,
            pixels,
            frame_index: frame_idx,
        }))
    }

    ///this will seek such that calling the next read_frame() gives you the n=target_frame frame
    pub fn seek_to_frame(&mut self, target_frame: i64) -> Result<(), ffmpeg::Error> {
        if self
            .last_decoded_frame_index
            .is_some_and(|i| target_frame == i + 1)
            || self.last_decoded_frame_index.is_none() && target_frame == 0
        {
            return Ok(());
        }

        if self.last_decoded_frame_index.is_none_or(|i| {
            target_frame < i
                || (target_frame - i)
                    > HARD_SEEK_THRESHOLD_SECONDS * self.stream_frame_rate.numerator() as i64
                        / self.stream_frame_rate.denominator() as i64
        }) {
            let target_ts = target_frame * self.stream_frame_rate.denominator() as i64
                / self.stream_frame_rate.numerator() as i64;
            self.input
                .seek(target_ts * 1_000_000, ..target_ts * 1_000_000)?;
            self.decoder.flush();
            self.is_eof_sent = false;
        }

        if target_frame == 0 {
            self.last_decoded_frame_index = None;
            return Ok(());
        }

        loop {
            match self.read_raw_frame()? {
                Some((frame_idx, _)) if frame_idx + 1 > target_frame => {
                    eprintln!(
                        "{} has sought ahead from target frame {} to frame {}...",
                        self.path.clone(),
                        target_frame - 1,
                        frame_idx
                    );
                    return Err(ffmpeg::Error::Bug);
                }
                Some((frame_idx, _)) if frame_idx + 1 == target_frame => return Ok(()),
                Some(_) => {}
                //may be worth figuring out how to store video duration for early returning if we hit this constantly
                None => return Err(ffmpeg::Error::Eof),
            }
        }
    }
}
