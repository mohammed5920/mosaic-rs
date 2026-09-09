use {
    crate::{
        config::CONFIG, mosaic::tiles::calc_average_colour, util::vid_util::is_fixed_frame_rate,
        vidcap::VideoCapture,
    },
    camino::Utf8PathBuf,
    imohash::Hasher as ImoHasher,
    rustc_hash::{FxBuildHasher, FxHashMap},
    std::{collections::HashMap, fs, sync::Arc},
};

#[derive(bincode::Decode, bincode::Encode)]
struct VidTilesCacheEntry {
    ///used just for debugging, not as source of truth for anything
    from_path: String,
    ///avg color of every frame in the video
    colours: Vec<[u8; 3]>,
}

#[derive(bincode::Decode, bincode::Encode)]
enum MaybeVidTilesCacheEntry {
    CachedEntry(VidTilesCacheEntry),
    ///invalid entry = the video itself cannot be used for vidtiles (vfr, corrupt, etc), don't waste time trying to reparse it every time
    UncacheableEntry {
        from_path: String,
        reason: String,
    },
}

enum CacheEvaluationResult {
    MissingOrCorrupt,
    ///invalid entry = the video itself cannot be used for vidtiles (vfr, corrupt, etc), don't waste time trying to reparse it every time
    Unreadable {
        reason: String,
    },
    Valid {
        tiles: Vec<VidTile>,
    },
}

pub(crate) struct VidTile {
    pub(crate) average_colour: [u8; 3],
    pub(crate) start_frame_index: u32,
    pub(crate) end_frame_index: u32,
    //reference counted string because many tiles can come from the same video
    source_path: Arc<str>,
}

//
// util
//
fn mse(a: [u8; 3], b: [u8; 3]) -> u32 {
    let (ar, ag, ab) = (a[0] as i32, a[1] as i32, a[2] as i32);
    let (br, bg, bb) = (b[0] as i32, b[1] as i32, b[2] as i32);
    let (cr, cg, cb) = ((ar - br), (ag - bg), (ab - bb));
    (cr * cr + cg * cg + cb * cb) as u32
}

///the end of one tile will always be i-1 the start of the next, but making it a map makes it easier to query
fn get_start_end_frame_indices(colours: &[[u8; 3]]) -> FxHashMap<usize, usize> {
    let mut res = HashMap::with_hasher(FxBuildHasher);
    if colours.is_empty() {
        return res;
    }
    let mut last_start_index = 0;
    let mut last_start_colour = colours[0];
    for (offset, current_colour) in colours[1..].iter().enumerate() {
        let i = offset + 1;
        if mse(last_start_colour, *current_colour) >= CONFIG.difference_threshold as u32 {
            res.insert(last_start_index, i - 1);
            last_start_index = i;
            last_start_colour = *current_colour;
        };
    }
    //ensure the end of the video is added as a tile as well
    res.insert(last_start_index, colours.len() - 1);
    res
}
//
// util
//

fn evaluate_vidtile_cache(
    source_path: &Utf8PathBuf,
    cache_entry_path: &Utf8PathBuf,
) -> CacheEvaluationResult {
    let Ok(cached_bytes) = fs::read(cache_entry_path) else {
        //cache entry doesn't exist
        return CacheEvaluationResult::MissingOrCorrupt;
    };
    let Ok((decoded, _)) = bincode::decode_from_slice::<MaybeVidTilesCacheEntry, _>(
        &cached_bytes,
        bincode::config::standard(),
    ) else {
        eprintln!("cache entry for {source_path} is corrupt, reprocessing...");
        return CacheEvaluationResult::MissingOrCorrupt;
    };
    match decoded {
        MaybeVidTilesCacheEntry::CachedEntry(cached) => {
            let parent_path: Arc<str> = source_path.as_str().into();
            CacheEvaluationResult::Valid {
                tiles: get_start_end_frame_indices(&cached.colours)
                    .into_iter()
                    .map(|(start, end)| VidTile {
                        average_colour: cached.colours[start],
                        source_path: parent_path.clone(),
                        start_frame_index: start as u32,
                        end_frame_index: end as u32,
                    })
                    .collect(),
            }
        }
        MaybeVidTilesCacheEntry::UncacheableEntry {
            reason,
            from_path: _,
        } => CacheEvaluationResult::Unreadable { reason },
    }
}

fn process_video_for_vidtiles(source_path: &Utf8PathBuf) -> MaybeVidTilesCacheEntry {
    let uncacheable = |reason| -> _ {
        MaybeVidTilesCacheEntry::UncacheableEntry {
            from_path: source_path.to_string(),
            reason,
        }
    };

    match is_fixed_frame_rate(source_path) {
        Ok(false) => return uncacheable("Video is variable refresh-rate".to_owned()),
        Err(e) => return uncacheable(format!("Could not probe video because {e}")),
        Ok(true) => {}
    }

    let mut cap = match VideoCapture::open(source_path.clone(), Some(CONFIG.tile_base_res.get())) {
        Ok(cap) => cap,
        Err(e) => return uncacheable(format!("{e} while opening as capture")),
    };

    let mut colours = Vec::new();
    loop {
        let curr_frame = match cap.read_rgb_frame() {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return uncacheable(format!("{e} while streaming capture")),
        };

        let curr_colour = calc_average_colour(&curr_frame.rgb);
        colours.push(curr_colour);
    }

    MaybeVidTilesCacheEntry::CachedEntry(VidTilesCacheEntry {
        from_path: source_path.to_string(),
        colours,
    })
}

pub(crate) fn vid_tiles_from_path(source_path: Utf8PathBuf) -> anyhow::Result<Vec<VidTile>> {
    let hasher = ImoHasher::new();
    let hash = hasher.sum_file(source_path.as_str())?;
    let mut cache_entry_path = CONFIG.cache_path.clone();
    cache_entry_path.push(hash.to_string());

    let res = match evaluate_vidtile_cache(&source_path, &cache_entry_path) {
        CacheEvaluationResult::Valid { tiles } => return Ok(tiles),
        CacheEvaluationResult::Unreadable { reason } => anyhow::bail!(reason),
        CacheEvaluationResult::MissingOrCorrupt => process_video_for_vidtiles(&source_path),
    };

    let encoded = bincode::encode_to_vec(&res, bincode::config::standard())
        .expect("cache should be encoded to vec");
    fs::write(&cache_entry_path, encoded).expect("cache should be written");

    let parent_path: Arc<str> = source_path.as_str().into();
    match res {
        MaybeVidTilesCacheEntry::UncacheableEntry { reason, .. } => anyhow::bail!(reason),
        MaybeVidTilesCacheEntry::CachedEntry(entry) => {
            Ok(get_start_end_frame_indices(&entry.colours)
                .into_iter()
                .map(|(start, end)| VidTile {
                    average_colour: entry.colours[start],
                    source_path: parent_path.clone(),
                    start_frame_index: start as u32,
                    end_frame_index: end as u32,
                })
                .collect())
        }
    }
}
