use std::{collections::HashSet, fmt::Debug, fs, path::Path, sync::LazyLock};

use anyhow::{Context as _, anyhow};
use camino::Utf8PathBuf;

static PIC_EXTENSIONS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    ["bmp", "gif", "ico", "jpeg", "png", "webp", "jpg", "test"]
        .into_iter()
        .map(|s| s.to_string())
        .collect()
});
static VID_EXTENSIONS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    ["mp4", "mkv", "webm", "mov", "avi"]
        .into_iter()
        .map(|s| s.to_string())
        .collect()
});

pub(crate) enum MediaType {
    Vid,
    Pic,
    Etc,
}

//NOTE: all the supported extensions live here
pub(crate) fn check_supported_extension(path: &Utf8PathBuf) -> MediaType {
    match path.extension().map(|e| e.to_lowercase()) {
        Some(pic) if PIC_EXTENSIONS.contains(&pic) => MediaType::Pic,
        Some(vid) if VID_EXTENSIONS.contains(&vid) => MediaType::Vid,
        _ => MediaType::Etc,
    }
}

pub(crate) fn walk_dir(path: impl AsRef<Path> + Debug) -> anyhow::Result<Vec<Utf8PathBuf>> {
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
