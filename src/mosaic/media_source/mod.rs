use anyhow::{Context, anyhow};
use camino::Utf8PathBuf;

use crate::{
    mosaic::media_source::pic_source::PicSource,
    util::file_util::{MediaType, check_supported_extension},
};

pub(crate) mod pic_source;
pub(crate) mod vid_source;

pub(crate) enum Source {
    Pic(PicSource),
    Vid,
}

impl Source {
    pub(crate) fn open(path: &Utf8PathBuf, dscale_factor: u32) -> anyhow::Result<Source> {
        match check_supported_extension(path) {
            MediaType::Pic => Ok(Source::Pic(
                PicSource::open(path, dscale_factor)
                    .with_context(|| format!("Error while opening {path} as source"))?,
            )),
            MediaType::Vid => todo!(),
            MediaType::Etc => Err(anyhow!("Unrecognised source extension")),
        }
    }
}
