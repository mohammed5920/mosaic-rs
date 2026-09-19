use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU16, Ordering},
        mpsc::{Receiver, Sender},
    },
};

use camino::Utf8Path;
use parking_lot::RwLock;
use rayon::prelude::*;
use rustc_hash::{FxBuildHasher, FxHashSet};

use crate::{
    config::CONFIG,
    mosaic::tiles::{Tile, vid_tiles::stream_tiles_from_video},
    streamer::tile_stores::{StoreFrame, TileStore, tile_size_to_store_index},
    types::DenseIndex,
    util::benchmark,
};

#[derive(Debug)]
pub(crate) enum StreamingMessage {
    Init,
    CycleStart { tile_size: u64 },
    CycleEnd { mark_atlas_dirty: bool },
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
    pool_size_bytes: i64,
) {
    //
    //closures
    //
    let probe_supertile = |tile_idx: DenseIndex, min_size: u64| -> Option<u64> {
        for store in tile_stores_ref
            .iter()
            .filter(|ts| ts.tile_size() >= fast_limit.max(min_size * 2))
            .rev()
        {
            if store.contains_tile(tile_idx) {
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
    let tile_ram_usage = |index: DenseIndex, tile_size: u64| -> u64 {
        if tile_size <= fast_limit {
            return 0;
        }
        ((tiles_ref[index.0 as usize].frame_count() * tile_size * tile_size) as f64 * 1.5) as u64
    };

    end_job(StreamingMessage::Init);
    //
    //closures
    //

    let smart_stores = tile_stores_ref
        .iter()
        .filter_map(|s| match s {
            TileStore::Fast(_) => None,
            TileStore::Smart(s) => Some(s),
        })
        .collect::<Vec<_>>();

    loop {
        //
        //command center
        //
        let request_tile_size = match parent_receiver
            .recv()
            .expect("streamer should keep bg channel open")
        {
            StreamingMessage::CycleStart { tile_size } => tile_size,
            StreamingMessage::Shutdown => return,
            invalid_message => panic!("bg thread received {invalid_message:?} from streamer"),
        };

        let difference = tile_stores_ref[tile_size_to_store_index(request_tile_size)]
            .difference(onscreen_set_ref.read().iter().copied());

        if difference.is_empty() {
            end_job(StreamingMessage::CycleEnd {
                mark_atlas_dirty: false,
            });
            continue;
        }
        //
        //command center
        //

        //
        //sorting request
        //
        let (mut cached_tiles, mut pic_tiles, mut vid_tiles) = (Vec::new(), Vec::new(), Vec::new());

        let super_res = (request_tile_size * CONFIG.prefetch_multiplier.get())
            .max(fast_limit)
            .min(res_limit);

        for idx in difference {
            if let Some(supertile) = probe_supertile(idx, request_tile_size) {
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
            end_job(StreamingMessage::CycleEnd {
                mark_atlas_dirty: false,
            });
            continue;
        }
        //
        //sorting request
        //

        //
        //freeing ram
        //
        let current_usage = smart_stores
            .iter()
            .map(|s| s.ram_usage_bytes())
            .sum::<u64>();
        let requested = cached_tiles
            .iter()
            .map(|i| tile_ram_usage(i.idx, request_tile_size))
            .sum::<u64>()
            + pic_tiles
                .iter()
                .map(|(i, _)| tile_ram_usage(*i, super_res))
                .sum::<u64>()
            + vid_tiles
                .iter()
                .map(|(i, _)| tile_ram_usage(*i, super_res))
                .sum::<u64>();

        if ((current_usage + requested) as i64) >= pool_size_bytes {
            let needed = (current_usage + requested) as i64 - pool_size_bytes;
            let mut freed = 0;
            let mut freeing_stage = 1;
            'freeing: loop {
                for ts in &smart_stores {
                    for idx in ts.get_least_used() {
                        // #1 - tile is not on screen at any size
                        // #2 - tile is on screen, but not at this size, and a higher res tile exists
                        // #3 - tile is on screen, but not at this size, and a higher res tile does not exist but tile will not be downscaled from
                        if (freeing_stage == 1 && !onscreen_set_ref.read().contains(&idx))
                            || (freeing_stage == 2
                                && (ts.tile_size != request_tile_size
                                    && probe_supertile(idx, ts.tile_size).is_some()))
                            || (freeing_stage == 3
                                && (ts.tile_size != request_tile_size
                                    && !cached_tiles.iter().any(|j| j.idx == idx)))
                        {
                            freed += ts.free_tile(idx);
                            if freed as i64 >= needed {
                                break 'freeing;
                            }
                        }
                    }
                }
                freeing_stage += 1;
                if freeing_stage >= 4 {
                    break;
                }
            }
            println!(
                "needed {} megabytes, reached stage {freeing_stage}, freed {} megabytes",
                (needed as f64) / 1024.0 / 1024.0,
                (freed as f64) / 1024.0 / 1024.0
            );
            if kill_flag_ref.load(Ordering::Relaxed) {
                end_job(StreamingMessage::CycleEnd {
                    mark_atlas_dirty: false,
                });
                continue;
            }
        }
        //
        //freeing ram
        //

        //
        //dispatching tiles
        //
        if !cached_tiles.is_empty() {
            benchmark(
                &format!(
                    "downscaling {} tiles to {request_tile_size}x{request_tile_size}",
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
                            &tile_stores_ref[tile_size_to_store_index(request_tile_size)],
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
                                let res = vec![
                                    p.stream_in(super_res)
                                        //maybe we could skip this tile, but if it was loaded in and processed before,
                                        //that means the image is fine and it's the streaming that's wonky...
                                        .unwrap_or_else(|e| {
                                            panic!("could not stream in pic tile {idx:?}: {e}")
                                        }),
                                ];
                                Some((idx, res))
                            })
                            .collect::<Vec<_>>();

                        let idxs = pics.iter().map(|(i, _)| *i).collect::<Vec<_>>();
                        tile_stores_ref[tile_size_to_store_index(super_res)].write_tiles(pics);

                        //skip downscaling if asked to stop early
                        if kill_flag_ref.load(Ordering::Relaxed) || super_res == request_tile_size {
                            return;
                        }

                        if super_res != request_tile_size {
                            tile_stores_ref[tile_size_to_store_index(super_res)].downscale_tiles(
                                &idxs,
                                &tile_stores_ref[tile_size_to_store_index(request_tile_size)],
                            );
                        }
                    },
                )
            },
            || {
                benchmark(
                    &format!("streaming in {} video tiles at {super_res}x{super_res}", {
                        vid_tiles.len()
                    }),
                    || {
                        if vid_tiles.is_empty() {
                            return;
                        }

                        //group videos
                        let mut videos: HashMap<Arc<Utf8Path>, Vec<_>> = HashMap::new();
                        for (idx, v) in vid_tiles {
                            videos
                                .entry(v.source_path.clone())
                                .or_default()
                                .push((idx, v));
                        }
                        let is_multithreaded =
                            videos.len() < std::thread::available_parallelism().unwrap().get() / 2;
                        let all_len = videos.len();
                        let prog = AtomicU16::new(0);
                        let all: Vec<(DenseIndex, Vec<StoreFrame>)> = videos
                            .into_par_iter()
                            .map(|(path, mut tiles)| {
                                if kill_flag_ref.load(Ordering::Relaxed) {
                                    return Vec::new();
                                }

                                tiles.sort_unstable_by_key(|(_, t)| t.start_frame_index);
                                let frames = stream_tiles_from_video(
                                    path.clone(),
                                    tiles.iter().map(|(_, v)| *v),
                                    super_res,
                                    is_multithreaded,
                                    kill_flag_ref.clone(),
                                );

                                let local_prog = prog.fetch_add(1, Ordering::Relaxed);
                                println!("streaming video {} / {all_len}", local_prog + 1);

                                frames
                                    .into_iter()
                                    .enumerate()
                                    .map(|(i, frames)| (tiles[i].0, frames))
                                    .collect::<Vec<_>>()
                            })
                            .flatten()
                            .collect::<Vec<_>>();

                        let idxs = all.iter().map(|(i, _)| *i).collect::<Vec<_>>();
                        tile_stores_ref[tile_size_to_store_index(super_res)].write_tiles(all);

                        //skip downscaling if asked to stop early
                        if kill_flag_ref.load(Ordering::Relaxed) || super_res == request_tile_size {
                            return;
                        }

                        if super_res != request_tile_size {
                            tile_stores_ref[tile_size_to_store_index(super_res)].downscale_tiles(
                                &idxs,
                                &tile_stores_ref[tile_size_to_store_index(request_tile_size)],
                            );
                        }
                    },
                )
            },
        );
        //
        //dispatching tiles
        //

        end_job(StreamingMessage::CycleEnd {
            mark_atlas_dirty: true,
        });
    }
}
