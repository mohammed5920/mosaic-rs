use std::num::NonZero;

use anyhow::anyhow;
use camino::Utf8PathBuf;

use crate::{
    file_io::{MediaType, check_supported_extension},
    source::pic_source::PicSource,
};

pub mod pic_source;
pub mod vid_source;

pub enum Source {
    Pic(PicSource),
    Vid,
}

impl Source {
    pub fn open(path: &Utf8PathBuf, dscale_factor: NonZero<u32>) -> anyhow::Result<Source> {
        match check_supported_extension(path) {
            MediaType::Pic => Ok(Source::Pic(PicSource::open(path, dscale_factor)?)),
            MediaType::Vid => todo!(),
            MediaType::Etc => Err(anyhow!("Unrecognised source extension")),
        }
    }
}
