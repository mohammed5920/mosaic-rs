use std::{collections::HashMap, fs, panic, sync::Arc};

use bincode::{Decode, Encode};
use camino::Utf8Path;
use imohash::Hasher as ImoHasher;
use parking_lot::RwLock;
use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::{
    config::CONFIG,
    mosaic::tiles::{
        pic_tiles::PicTile,
        vid_tiles::{VidTile, get_start_end_frame_indices, process_video_for_vidtiles},
    },
};

#[derive(Encode, Decode)]
struct CachedPic {
    average_colour: [u8; 3],
}

#[derive(Encode, Decode)]
struct CachedVid {
    average_colours: Vec<[u8; 3]>,
}

#[derive(Encode, Decode)]
enum CacheValue<T> {
    Valid(T),
    Unreadable { reason: String },
}

pub(crate) struct Cache {
    pic_cache: RwLock<FxHashMap<u128, CacheValue<CachedPic>>>,
    vid_cache: RwLock<FxHashMap<u128, CacheValue<CachedVid>>>,
}

//written to file only
#[derive(Encode, Decode)]
enum CachedResult {
    Picture(CacheValue<CachedPic>),
    Video(CacheValue<CachedVid>),
}

#[derive(Encode, Decode)]
struct CachedEntry {
    hash: u128,
    inner: CachedResult,
}
//written to file only

pub(crate) fn load_cache() -> Cache {
    let cached_vec = match fs::read(&CONFIG.cache_path)
        .map_err(|e| ToString::to_string(&e))
        .and_then(|bytes| {
            bincode::decode_from_slice::<Vec<CachedEntry>, _>(&bytes, bincode::config::standard())
                .map_err(|e| ToString::to_string(&e))
        }) {
        Ok((v, _)) => v,
        Err(e) => {
            eprintln!("could not decode cache because {e}, recreating...");
            return Cache {
                pic_cache: HashMap::with_hasher(FxBuildHasher).into(),
                vid_cache: HashMap::with_hasher(FxBuildHasher).into(),
            };
        }
    };

    let (mut pics, mut vids) = (
        HashMap::with_hasher(FxBuildHasher),
        HashMap::with_hasher(FxBuildHasher),
    );
    for entry in cached_vec.into_iter() {
        match entry.inner {
            CachedResult::Picture(p) => {
                pics.insert(entry.hash, p);
            }
            CachedResult::Video(v) => {
                vids.insert(entry.hash, v);
            }
        }
    }

    Cache {
        pic_cache: pics.into(),
        vid_cache: vids.into(),
    }
}

fn hash_file(path: &str) -> u128 {
    let hasher = ImoHasher::new();
    //unwrapping here because this path is supposed to be valid from walking the directory earlier
    hasher
        .sum_file(path)
        .unwrap_or_else(|e| panic!("could not hash {path} because {e}"))
}

pub(crate) fn load_pic_tile(cache: &Cache, path: String) -> Option<PicTile> {
    let hash = hash_file(&path);
    if let Some(v) = cache.pic_cache.read().get(&hash) {
        match v {
            CacheValue::Unreadable { reason } => {
                eprintln!("{}", reason);
                return None;
            }
            CacheValue::Valid(pic) => {
                return Some(PicTile {
                    average_colour: pic.average_colour,
                    source_path: path,
                });
            }
        }
    };

    match PicTile::new(&path) {
        Err(e) => {
            eprintln!("{}", e);
            cache.pic_cache.write().insert(
                hash,
                CacheValue::Unreadable {
                    reason: e.to_string(),
                },
            );
            None
        }
        Ok(p) => {
            cache.pic_cache.write().insert(
                hash,
                CacheValue::Valid(CachedPic {
                    average_colour: p.average_colour,
                }),
            );
            Some(p)
        }
    }
}

pub(crate) fn load_vid_tiles(cache: &Cache, path: Arc<Utf8Path>) -> Option<Vec<VidTile>> {
    let hash = hash_file(path.as_str());
    if let Some(v) = cache.vid_cache.read().get(&hash) {
        match v {
            CacheValue::Unreadable { reason } => {
                eprintln!("{}", reason);
                return None;
            }
            CacheValue::Valid(vid) => {
                return Some(
                    get_start_end_frame_indices(&vid.average_colours)
                        .into_iter()
                        .map(|(start, end)| {
                            VidTile::new(
                                vid.average_colours[start],
                                path.clone(),
                                start as u32,
                                end as u32,
                            )
                        })
                        .collect(),
                );
            }
        }
    };

    match process_video_for_vidtiles(path.clone()) {
        Err(e) => {
            eprintln!("{:?}", e);
            cache.vid_cache.write().insert(
                hash,
                CacheValue::Unreadable {
                    reason: e.to_string(),
                },
            );
            None
        }
        Ok(colours) => {
            let res = get_start_end_frame_indices(&colours)
                .into_iter()
                .map(|(start, end)| {
                    VidTile::new(colours[start], path.clone(), start as u32, end as u32)
                })
                .collect();

            cache.vid_cache.write().insert(
                hash,
                CacheValue::Valid(CachedVid {
                    average_colours: colours,
                }),
            );
            Some(res)
        }
    }
}

pub(crate) fn flush_cache(cache: Cache) {
    let mut pic_read = cache.pic_cache.write();
    let mut vid_read = cache.vid_cache.write();

    let mut all: Vec<CachedEntry> = Vec::with_capacity(pic_read.len() + vid_read.len());
    for (hash, inner) in pic_read.drain() {
        all.push(CachedEntry {
            hash,
            inner: CachedResult::Picture(inner),
        });
    }
    for (hash, inner) in vid_read.drain() {
        all.push(CachedEntry {
            hash,
            inner: CachedResult::Video(inner),
        });
    }

    let encoded = bincode::encode_to_vec(&all, bincode::config::standard())
        .expect("cache should be encoded to vec");
    fs::write(&CONFIG.cache_path, encoded).expect("cache should be written");
}
