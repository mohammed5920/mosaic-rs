use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender},
    },
};

use parking_lot::RwLock;
use rayon::prelude::*;
use rustc_hash::{FxBuildHasher, FxHashSet};

use crate::{
    config::CONFIG,
    mosaic::tiles::Tile,
    streamer::tile_stores::{TileStore, tile_size_to_store_index},
    types::DenseIndex,
    util::{benchmark, is_power_of_two},
};

#[derive(Debug)]
pub(crate) enum StreamingMessage {
    Init,
    CycleStart {
        tile_size: u64,
        before_ram_bytes: u64,
        ram_limit_bytes: u64,
    },
    CycleEnd {
        after_ram_bytes: u64,
        did_onscreen_tiles_change: bool,
    },
    Shutdown,
}

//create a dedicated struct to save locking and hashing the tracker sets more than once
struct CachedTileJob {
    idx: DenseIndex,
    supertile_size: u64,
}

///i'm sorry, clippy...
#[allow(clippy::too_many_arguments)]
pub(crate) fn streamer_thread(
    parent_receiver: Receiver<StreamingMessage>,
    child_sender: Sender<StreamingMessage>,
    kill_flag_ref: Arc<AtomicBool>,
    onscreen_set_ref: Arc<RwLock<FxHashSet<DenseIndex>>>,
    tile_stores_ref: Arc<[TileStore]>,
    tiles_ref: Arc<[Tile]>,
    fast_limit: u64,
    res_limit: u64,
) {
    let probe_supertile = |tile_idx: DenseIndex, min_size: u64| -> Option<u64> {
        for store in tile_stores_ref
            .iter()
            .filter(|ts| ts.tile_size() >= fast_limit.max(min_size * 2))
            .rev()
        {
            if store.read_tracker().contains(&tile_idx) {
                return Some(store.tile_size());
            }
        }
        None
    };
    let end_job = |msg: StreamingMessage| {
        child_sender
            .send(msg)
            .expect("streamer should keep bg channel open");
    };
    end_job(StreamingMessage::Init);

    loop {
        let (tile_size, before_ram_bytes, ram_limit_bytes) = match parent_receiver
            .recv()
            .expect("streamer should keep bg channel open")
        {
            StreamingMessage::CycleStart {
                tile_size,
                before_ram_bytes,
                ram_limit_bytes,
            } => (tile_size, before_ram_bytes, ram_limit_bytes),
            StreamingMessage::Shutdown => return,
            invalid_message => panic!("bg thread received {invalid_message:?} from streamer"),
        };
        println!("stream started");

        debug_assert!(
            is_power_of_two(tile_size),
            "tile size {tile_size} is not a power of two"
        );

        let difference = onscreen_set_ref
            .read()
            .difference(&tile_stores_ref[tile_size_to_store_index(tile_size)].read_tracker())
            .copied()
            .collect::<Vec<_>>();

        if difference.is_empty() {
            println!("stream ended - no difference");
            end_job(StreamingMessage::CycleEnd {
                after_ram_bytes: before_ram_bytes,
                did_onscreen_tiles_change: false,
            });
            continue;
        }

        let (mut cached_tiles, mut pic_tiles, mut vid_tiles) = (Vec::new(), Vec::new(), Vec::new());

        let super_res = (tile_size * CONFIG.prefetch_multiplier.get())
            .min(fast_limit)
            .max(res_limit);

        for idx in difference {
            if let Some(supertile) = probe_supertile(idx, tile_size) {
                cached_tiles.push(CachedTileJob {
                    idx,
                    supertile_size: supertile,
                });
            } else {
                match tiles_ref[idx.0 as usize] {
                    Tile::Pic(ref p) => pic_tiles.push((idx, p)),
                    Tile::Vid(ref v) => vid_tiles.push((idx, v)),
                }
            }
        }

        if kill_flag_ref.load(Ordering::Relaxed) {
            println!("stream ended - kill flag 1");
            end_job(StreamingMessage::CycleEnd {
                after_ram_bytes: before_ram_bytes,
                did_onscreen_tiles_change: false,
            });
        }

        if !cached_tiles.is_empty() {
            benchmark(
                &format!(
                    "downscaling {} tiles to {tile_size}x{tile_size}",
                    cached_tiles.len()
                ),
                || {
                    let mut grouped = HashMap::<_, Vec<DenseIndex>, _>::with_hasher(FxBuildHasher);
                    for job in cached_tiles.into_iter() {
                        grouped.entry(job.supertile_size).or_default().push(job.idx);
                    }
                    for (res, group) in grouped {
                        tile_stores_ref[tile_size_to_store_index(res)].downscale_tiles(
                            &group,
                            &tile_stores_ref[tile_size_to_store_index(tile_size)],
                        );
                    }
                },
            );
        }

        rayon::join(
            || {
                benchmark(
                    &format!("streaming in {} pictures at {super_res}x{super_res}", {
                        pic_tiles.len()
                    }),
                    || {
                        if pic_tiles.is_empty() {
                            return;
                        }

                        let pics = pic_tiles
                            .into_par_iter()
                            .filter_map(|(idx, p)| {
                                //skip decoding the image if asked to stop early
                                if kill_flag_ref.load(Ordering::Relaxed) {
                                    return None;
                                }
                                let res = (
                                    idx,
                                    p.stream_in(super_res)
                                        //maybe we could skip this tile, but if it was loaded in and processed before,
                                        //that means the image is fine and it's the streaming that's wonky...
                                        .unwrap_or_else(|_| {
                                            panic!("could not stream in pic tile {idx:?}")
                                        }),
                                );
                                Some(res)
                            })
                            .collect::<Vec<_>>();

                        let converted = pics
                            .iter()
                            .map(|(i, f)| (*i, std::slice::from_ref(f)))
                            .collect::<Vec<_>>();

                        tile_stores_ref[tile_size_to_store_index(super_res)]
                            .write_tiles(&converted);

                        //skip downscaling if asked to stop early
                        if kill_flag_ref.load(Ordering::Relaxed) || super_res == tile_size {
                            return;
                        }

                        tile_stores_ref[tile_size_to_store_index(super_res)].downscale_tiles(
                            &converted.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
                            &tile_stores_ref[tile_size_to_store_index(tile_size)],
                        );
                    },
                )
            },
            || {
                benchmark(
                    &format!("streaming in {} videos at {super_res}x{super_res}", {
                        vid_tiles.len()
                    }),
                    || {
                        if vid_tiles.is_empty() {
                            return;
                        }

                        //group videos
                        let mut videos: HashMap<Arc<str>, Vec<_>> = HashMap::new();
                        for (idx, v) in vid_tiles {
                            videos
                                .entry(v.source_path.clone())
                                .or_default()
                                .push((idx, v));
                        }
                    },
                )
            },
        );

        println!("stream ended - end of loop");
        end_job(StreamingMessage::CycleEnd {
            after_ram_bytes: before_ram_bytes,
            did_onscreen_tiles_change: true,
        });
    }
}
