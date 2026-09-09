use std::fs;

use anyhow::bail;
use camino::Utf8PathBuf;
use image::ImageReader;
use imohash::Hasher as ImoHasher;

use crate::{config::CONFIG, mosaic::tiles::calc_average_colour};

#[derive(bincode::Decode, bincode::Encode)]
pub(crate) struct PicTile {
    pub(crate) average_colour: [u8; 3],
    source_path: String,
}

#[derive(bincode::Decode, bincode::Encode)]
enum CachedPicTile {
    Valid(PicTile),
    Unreadable { reason: String },
}

impl PicTile {
    pub(crate) fn open(path: Utf8PathBuf) -> anyhow::Result<PicTile> {
        let hasher = ImoHasher::new();
        let hash = hasher.sum_file(path.as_str())?;
        let mut cache_entry_path = CONFIG.cache_path.clone();
        cache_entry_path.push(hash.to_string());

        if let Some(cached_tile) = 'cache_check: {
            let Ok(cached_bytes) = fs::read(&cache_entry_path) else {
                break 'cache_check None;
            };
            let Ok((decoded, _)) = bincode::decode_from_slice::<CachedPicTile, _>(
                &cached_bytes,
                bincode::config::standard(),
            ) else {
                break 'cache_check None;
            };
            match decoded {
                CachedPicTile::Valid(pic_tile) => Some(pic_tile),
                CachedPicTile::Unreadable { reason } => bail!(reason),
            }
        } {
            return Ok(cached_tile);
        }

        let res = match ImageReader::open(&path)
            .map_err(|e| format!("Error opening {path} - {e}"))
            .and_then(|img| {
                img.with_guessed_format()
                    .map_err(|e| format!("Error guessing {path} - {e}"))
                    .and_then(|g| {
                        g.decode()
                            .map_err(|e| format!("Error decoding {path} - {e}"))
                    })
            }) {
            Ok(decoded) => CachedPicTile::Valid(PicTile {
                average_colour: calc_average_colour(&decoded.to_rgb8().into_raw()),
                source_path: path.to_string(),
            }),
            Err(e) => CachedPicTile::Unreadable { reason: e },
        };

        let encoded = bincode::encode_to_vec(&res, bincode::config::standard())
            .expect("cache should be encoded to vec");
        fs::write(&cache_entry_path, encoded).expect("cache should be written");

        match res {
            CachedPicTile::Valid(pic_tile) => Ok(pic_tile),
            CachedPicTile::Unreadable { reason } => bail!(reason),
        }
    }
}
