use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, Sender},
};

use parking_lot::Mutex;
use rayon::prelude::*;
use rustc_hash::FxHashSet;

use crate::{
    config::CONFIG,
    mosaic::tiles::Tile,
    streamer::tile_stores::TileStore,
    types::DenseIndex,
    util::{benchmark, is_power_of_two},
};

pub(crate) enum StreamingMessage {
    Init,
    CycleStart {
        tile_size: u64,
        before_ram_bytes: u64,
        ram_limit_bytes: u64,
    },
    CycleEnd {
        after_ram_bytes: u64,
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
    parent_reciever: Receiver<StreamingMessage>,
    child_sender: Sender<StreamingMessage>,
    kill_flag_ref: Arc<AtomicBool>,
    onscreen_set_ref: Arc<Mutex<FxHashSet<DenseIndex>>>,
    tile_stores_ref: Arc<[TileStore]>,
    tiles_ref: Arc<[Tile]>,
    fast_limit: u64,
    res_limit: u64,
) {
    let ts_to_si = |tile_size: u64| (tile_size / 2).ilog2() as usize;
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
        let (tile_size, before_ram_bytes, ram_limit_bytes) = match parent_reciever
            .recv()
            .expect("streamer should keep bg channel open")
        {
            StreamingMessage::CycleStart {
                tile_size,
                before_ram_bytes,
                ram_limit_bytes,
            } => (tile_size, before_ram_bytes, ram_limit_bytes),
            StreamingMessage::Shutdown => return,
            _ => panic!("bg thread recieved invalid message from streamer"),
        };
        println!("stream started");

        debug_assert!(
            is_power_of_two(tile_size),
            "tile size {tile_size} is not a power of two"
        );

        let difference = onscreen_set_ref
            .lock()
            .difference(&tile_stores_ref[ts_to_si(tile_size)].read_tracker())
            .copied()
            .collect::<Vec<_>>();

        if difference.is_empty() {
            println!("stream ended - no difference");
            end_job(StreamingMessage::CycleEnd {
                after_ram_bytes: before_ram_bytes,
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
                    Tile::Pic(_) => pic_tiles.push(idx),
                    Tile::Vid(_) => vid_tiles.push(idx),
                }
                cached_tiles.push(CachedTileJob {
                    idx,
                    supertile_size: super_res,
                })
            }
        }

        if kill_flag_ref.load(Ordering::Relaxed) {
            println!("stream ended - kill flag 1");
            end_job(StreamingMessage::CycleEnd {
                after_ram_bytes: before_ram_bytes,
            });
        }

        let (a, b) = rayon::join(
            || {
                benchmark(
                    &format!("streaming in {} pictures at {super_res}x{super_res}", {
                        pic_tiles.len()
                    }),
                    || {
                        pic_tiles
                            .par_iter()
                            .map(|idx| {
                                tiles_ref[idx.0 as usize]
                                    .as_pic()
                                    .stream_in(super_res)
                                    .expect("temp")
                            })
                            .collect::<Vec<_>>()
                    },
                )
            },
            || {},
        );

        println!("stream ended - end of loop");
        end_job(StreamingMessage::CycleEnd {
            after_ram_bytes: before_ram_bytes,
        });
    }
}
