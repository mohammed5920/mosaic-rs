use {
    crate::{is_fixed_frame_rate, tiles::calc_average_colour, video_capture::VideoCapture},
    anyhow::Context,
    camino::Utf8PathBuf,
    imohash::Hasher as ImoHasher,
    rustc_hash::{FxBuildHasher, FxHashMap},
    std::{collections::HashMap, fs},
};

const CACHE_DIR: &str = "cache/tiles/";
const DIFFERENCE_THRESHOLD: u32 = 300;

//
// serialisation
//
#[derive(bincode::Decode, bincode::Encode)]
struct VidTilePreview {
    //which frame in the original video is this tile from?
    from_frame_index: u32,
    width: u64,
    height: u64,
    pixels: Vec<[u8; 3]>,
}

#[derive(bincode::Decode, bincode::Encode)]
struct VidTilesCacheEntry {
    //used just for debugging, not as source of truth for anything
    from_path: String,
    colours: Vec<[u8; 3]>, //avg color of every frame in the video
    tile_previews: Vec<VidTilePreview>,
}

#[derive(bincode::Decode, bincode::Encode)]
enum MaybeVidTilesCacheEntry {
    CachedEntry(VidTilesCacheEntry),
    UncacheableEntry { from_path: String, reason: String },
}
//
// serialisation
//

//
// util
//
fn mse(a: &[u8; 3], b: &[u8; 3]) -> u32 {
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
        if mse(&last_start_colour, current_colour) >= DIFFERENCE_THRESHOLD {
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

//
// cache check
//
enum CacheEvaluationResult {
    MissingOrCorrupt,
    ///invalid entry = the video itself cannot be used for vidtiles (vfr, corrupt, etc), don't waste time trying to reparse it every time
    Unreadable {
        reason: String,
    },
    //pass colours so they're cached with the fresh frames
    Stale {
        colors: Vec<[u8; 3]>,
        fresh_preview_frame_indices: FxHashMap<usize, usize>,
    },
    Valid {
        tiles: Vec<VidTile>,
    },
}

fn evaluate_vidtile_cache(
    source_path: &Utf8PathBuf,
    cache_path: &Utf8PathBuf,
) -> CacheEvaluationResult {
    let Ok(cached_bytes) = fs::read(cache_path) else {
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
    let cached = match decoded {
        MaybeVidTilesCacheEntry::CachedEntry(cached) => cached,
        MaybeVidTilesCacheEntry::UncacheableEntry {
            reason,
            from_path: _,
        } => {
            return CacheEvaluationResult::Unreadable { reason };
        }
    };

    //stale preview check
    let mut mapped_cached_previews: FxHashMap<_, _> = cached
        .tile_previews
        .into_iter()
        .map(|t| (t.from_frame_index, t))
        .collect();
    let true_indices = get_start_end_frame_indices(&cached.colours);
    for start in true_indices.keys() {
        if !mapped_cached_previews.contains_key(&(*start as u32)) {
            return CacheEvaluationResult::Stale {
                colors: cached.colours,
                fresh_preview_frame_indices: true_indices,
            };
        }
    }
    CacheEvaluationResult::Valid {
        tiles: true_indices
            .into_iter()
            .map(|(first, last)| VidTile {
                average_colour: cached.colours[first],
                source_path: source_path.clone(),
                start_frame_index: first as u32,
                end_frame_index: last as u32,
                first_frame: mapped_cached_previews
                    .remove(&(first as u32))
                    .expect("Index math should be correct"),
            })
            .collect(),
    }
}
//
// cache check
//

//
// video processing
//
fn extract_vidtile_previews(
    source_path: &Utf8PathBuf,
    fresh_preview_frame_indices: &FxHashMap<usize, usize>,
    tile_base_res: u64,
) -> anyhow::Result<Vec<VidTilePreview>> {
    let mut cap = VideoCapture::open(source_path.clone(), Some(tile_base_res))
        .with_context(|| format!("{source_path} - (update pass) cannot open capture"))?;
    let mut res = Vec::new();
    for i in fresh_preview_frame_indices.keys() {
        let i = *i;
        cap.seek_to_frame(i as i64)
            .with_context(|| format!("{source_path} - (update pass) cannot seek capture"))?;
        let frame = cap
            .read_frame()
            .with_context(|| format!("{source_path} - (update pass) cannot read from capture"))?
            .with_context(|| format!("{source_path} - (update pass) capture ended prematurely"))?;
        res.push(VidTilePreview {
            from_frame_index: i as u32,
            width: frame.width,
            height: frame.height,
            pixels: frame.pixels,
        });
    }
    Ok(res)
}

fn process_video_for_vidtiles(
    source_path: &Utf8PathBuf,
    tile_base_res: u64,
) -> (MaybeVidTilesCacheEntry, Option<FxHashMap<usize, usize>>) {
    match is_fixed_frame_rate(source_path) {
        Ok(true) => {}
        Ok(false) => {
            return (
                MaybeVidTilesCacheEntry::UncacheableEntry {
                    from_path: source_path.to_string(),
                    reason: "Video is variable refresh-rate".to_owned(),
                },
                None,
            );
        }
        Err(e) => {
            return (
                MaybeVidTilesCacheEntry::UncacheableEntry {
                    from_path: source_path.to_string(),
                    reason: format!("Could not probe video because {e}"),
                },
                None,
            );
        }
    }

    let mut cap = match VideoCapture::open(source_path.clone(), Some(tile_base_res)) {
        Ok(cap) => cap,
        Err(e) => {
            return (
                MaybeVidTilesCacheEntry::UncacheableEntry {
                    from_path: source_path.to_string(),
                    reason: format!("{e} while opening as capture"),
                },
                None,
            );
        }
    };

    let mut tile_previews = Vec::new();
    let mut colours = Vec::new();
    let mut last_tile_avg_colour = [0; 3];

    loop {
        let curr_frame = match cap.read_frame() {
            Err(e) => {
                return (
                    MaybeVidTilesCacheEntry::UncacheableEntry {
                        from_path: source_path.to_string(),
                        reason: format!("{e} while streaming capture"),
                    },
                    None,
                );
            }
            Ok(None) => break,
            Ok(Some(f)) => f,
        };

        let curr_colour = calc_average_colour(&curr_frame.pixels);
        colours.push(curr_colour);

        if tile_previews.is_empty()
            || mse(&last_tile_avg_colour, &curr_colour) >= DIFFERENCE_THRESHOLD
        {
            tile_previews.push(VidTilePreview {
                from_frame_index: curr_frame.frame_index as u32,
                width: curr_frame.width,
                height: curr_frame.height,
                pixels: curr_frame.pixels,
            });
            last_tile_avg_colour = curr_colour;
        }
    }

    let mut res2: FxHashMap<usize, usize> = tile_previews
        .windows(2)
        .map(|w| {
            (
                w[0].from_frame_index as usize,
                w[1].from_frame_index as usize - 1,
            )
        })
        .collect();
    if let Some(last) = tile_previews.last() {
        res2.insert(last.from_frame_index as usize, colours.len() - 1);
    }
    let res1: MaybeVidTilesCacheEntry = MaybeVidTilesCacheEntry::CachedEntry(VidTilesCacheEntry {
        from_path: source_path.to_string(),
        colours,
        tile_previews,
    });

    (res1, Some(res2))
}
//
// video processing
//

pub struct VidTile {
    pub average_colour: [u8; 3],
    source_path: Utf8PathBuf,
    start_frame_index: u32,
    end_frame_index: u32,
    first_frame: VidTilePreview,
}

pub fn vid_tiles_from_path(
    source_path: Utf8PathBuf,
    tile_base_res: u64,
) -> anyhow::Result<Vec<VidTile>> {
    let hasher = ImoHasher::new();
    let hash = hasher.sum_file(source_path.as_str())?;
    let mut cache_path = Utf8PathBuf::from(CACHE_DIR);
    cache_path.push(hash.to_string());

    let res = match evaluate_vidtile_cache(&source_path, &cache_path) {
        CacheEvaluationResult::Valid { tiles } => return Ok(tiles),
        CacheEvaluationResult::Stale {
            colors,
            fresh_preview_frame_indices,
        } => {
            let previews = extract_vidtile_previews(
                &source_path,
                &fresh_preview_frame_indices,
                tile_base_res,
            )?;
            (
                MaybeVidTilesCacheEntry::CachedEntry(VidTilesCacheEntry {
                    from_path: source_path.to_string(),
                    colours: colors,
                    tile_previews: previews,
                }),
                Some(fresh_preview_frame_indices),
            )
        }
        CacheEvaluationResult::Unreadable { reason } => anyhow::bail!(reason),
        CacheEvaluationResult::MissingOrCorrupt => {
            process_video_for_vidtiles(&source_path, tile_base_res)
        }
    };

    let encoded = bincode::encode_to_vec(&res.0, bincode::config::standard())
        .expect("Cache should be encoded to vec");
    fs::write(&cache_path, encoded)?;

    match res {
        (MaybeVidTilesCacheEntry::CachedEntry(vid_tiles_cache_entry), Some(frame_index_map)) => {
            Ok(vid_tiles_cache_entry
                .tile_previews
                .into_iter()
                .map(|t| VidTile {
                    average_colour: vid_tiles_cache_entry.colours[t.from_frame_index as usize],
                    source_path: source_path.clone(),
                    start_frame_index: t.from_frame_index,
                    end_frame_index: (*frame_index_map
                        .get(&(t.from_frame_index as usize))
                        .expect("Index math should be correct"))
                        as u32,
                    first_frame: t,
                })
                .collect())
        }
        (
            MaybeVidTilesCacheEntry::UncacheableEntry {
                from_path: _,
                reason,
            },
            _,
        ) => anyhow::bail!(reason),
        (MaybeVidTilesCacheEntry::CachedEntry(_), None) => unreachable!(),
    }
}
