use std::{
    io::{BufReader, Read as _},
    iter,
    process::{Command, Stdio},
};

use anyhow::{Context, bail};
use camino::Utf8PathBuf;

use crate::file_io::{MediaType, check_supported_extension};

pub fn stream_whole_video(
    path: &Utf8PathBuf,
    width: u64,
    height: u64,
) -> anyhow::Result<impl Iterator<Item = Vec<[u8; 3]>>> {
    if !matches!(check_supported_extension(path), MediaType::Vid) {
        bail!("Unsupported video extension")
    }

    #[rustfmt::skip]
    let mut child = Command::new("ffmpeg.exe")
    .args([
        "-i", path.as_str(),
        "-vf", format!("scale={width}:{height}").as_str(),
        "-pix_fmt", "rgb24",
        "-f", "rawvideo",
        "-"
    ])
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .spawn()?;

    let stdout = child
        .stdout
        .take()
        .context("couldn't take stdout for video stream")?;
    let frame_bytes = (width * height * 3) as usize;
    let mut reader = BufReader::new(stdout);
    let mut buf = vec![0u8; frame_bytes];

    Ok(iter::from_fn(move || {
        //keep the child alive :)
        let _child = &child;
        reader.read_exact(&mut buf).ok()?;
        Some(buf.as_chunks::<3>().0.into())
    }))
}
