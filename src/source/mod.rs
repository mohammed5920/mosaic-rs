use std::num::NonZero;

use anyhow::{Context, anyhow};
use camino::Utf8PathBuf;

use crate::{
    file_io::{MediaType, check_supported_extension},
    source::pic_source::PicSource,
};

mod pic_source;
mod vid_source;

pub enum Source {
    Pic(PicSource),
    Vid,
}

impl Source {
    pub fn open(path: &Utf8PathBuf, dscale_factor: NonZero<u32>) -> anyhow::Result<Source> {
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
