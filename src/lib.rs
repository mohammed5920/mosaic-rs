pub mod matchmaker;
pub mod rendering;
pub mod source;
pub mod tiles;

use std::{collections::HashSet, fmt::Debug, fs, path::Path, sync::LazyLock, time::Instant};

use anyhow::{Context, anyhow};
use camino::Utf8PathBuf;
use image::{ImageBuffer, Rgb};

pub type RgbBuffer = ImageBuffer<Rgb<u8>, Vec<u8>>;

static PIC_EXTENSIONS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    ["bmp", "dds", "gif", "ico", "jpeg", "png", "webp", "jpg"]
        .into_iter()
        .map(|s| s.to_string())
        .collect()
});
static VID_EXTENSIONS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    [
        "mkv", "flv", "vob", "ogv", "rrc", "gifv", "mng", "mov", "avi", "qt", "wmv", "yuv", "rm",
        "asf", "amv", "mp4", "m4p", "m4v", "mpg", "mp2", "mpeg", "mpe", "mpv", "m4v", "svi", "3gp",
        "3g2", "mxf", "roq", "nsv", "flv", "f4v", "f4p", "f4a", "f4b", "mod", "webm", "bik",
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect()
});

pub enum MediaType {
    Vid,
    Pic,
    Etc,
}

//NOTE: all the supported extensions live here
pub fn check_supported_extension(path: &Utf8PathBuf) -> MediaType {
    match path.extension().map(|e| e.to_lowercase()) {
        Some(pic) if PIC_EXTENSIONS.contains(&pic) => MediaType::Pic,
        Some(vid) if VID_EXTENSIONS.contains(&vid) => MediaType::Vid,
        _ => MediaType::Etc,
    }
}

pub fn walk_dir(path: impl AsRef<Path> + Debug) -> anyhow::Result<Vec<Utf8PathBuf>> {
    let mut res = Vec::new();
    for entry in fs::read_dir(&path).with_context(|| format!("{path:?}"))? {
        let entry = entry.with_context(|| format!("{path:?}"))?;
        if entry
            .file_type()
            .with_context(|| format!("{path:?}"))?
            .is_dir()
        {
            res.append(&mut walk_dir(entry.path())?);
        } else {
            res.push(Utf8PathBuf::from_path_buf(entry.path()).map_err(|e| anyhow!("{e:?}"))?);
        }
    }
    Ok(res)
}

pub fn benchmark<T>(label: &str, function: impl Fn() -> T) -> T {
    let start = Instant::now();
    let res = function();
    println!("{label} took {:#?}", Instant::now().duration_since(start));
    res
}
