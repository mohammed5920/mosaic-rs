use std::{sync::Arc, thread};

use camino::Utf8PathBuf;
use crossbeam::{
    channel::{Receiver, Sender, bounded},
    select,
};

use crate::{
    config::CONFIG,
    mosaic::{matchmaker::MatchMaker, tiles::Tile},
    types::DenseIndex,
    vidcap::VideoCapture,
};

pub(crate) struct DynamicMosaic {
    pub(crate) tiles: Arc<[Tile]>,
    pub(crate) dense_matches: Vec<DenseIndex>,
    pub(crate) width: u32,
    pub(crate) height: u32,
    seek_gen: i32,
    decoder_seek_tx: Sender<PipelineMessage>,
    matcher_seek_tx: Sender<PipelineMessage>,
    matcher_data_rx: Receiver<PipelineMessage>,
}

#[derive(Debug)]
enum PipelineMessage {
    Seek {
        new_pos: i64,
        new_gen: i32,
    },
    RawFrame {
        inner: Vec<u8>,
        generation: i32,
    },
    ProcessedFrame {
        inner: Vec<DenseIndex>,
        generation: i32,
    },
    Shutdown,
}

impl DynamicMosaic {
    pub(crate) fn new(
        path: Utf8PathBuf,
        tiles: Vec<Tile>,
        matchmaker: MatchMaker,
    ) -> anyhow::Result<Self> {
        let (decoder_seek_tx, decoder_seek_rx) = bounded::<PipelineMessage>(1);
        let (matcher_seek_tx, matcher_seek_rx) = bounded::<PipelineMessage>(1);
        let (decoder_data_tx, decoder_data_rx) =
            bounded::<PipelineMessage>(CONFIG.video_fps.ceil() as usize);
        let (matcher_data_tx, matcher_data_rx) =
            bounded::<PipelineMessage>(CONFIG.video_fps.ceil() as usize);

        let vid_cap = VideoCapture::new(Arc::from(path.clone()), None, true)?;
        thread::spawn(move || decode_thread(decoder_seek_rx, decoder_data_tx, path));
        thread::spawn(move || {
            matchmaker_thread(
                matcher_seek_rx,
                decoder_data_rx,
                matcher_data_tx,
                matchmaker,
            )
        });

        let mut res = Self {
            width: vid_cap.width,
            height: vid_cap.height,
            tiles: tiles.into(),
            dense_matches: Vec::new(),
            seek_gen: 0,
            decoder_seek_tx,
            matcher_seek_tx,
            matcher_data_rx,
        };
        res.advance_frame();
        Ok(res)
    }

    pub(crate) fn advance_frame(&mut self) {
        loop {
            match self.matcher_data_rx.recv() {
                Ok(PipelineMessage::ProcessedFrame { inner, generation }) => {
                    if generation >= self.seek_gen {
                        self.dense_matches = inner;
                        break;
                    }
                }
                something_else => panic!("main thread received {something_else:?} from matcher"),
            }
        }
    }

    pub(crate) fn seek_to_frame(&mut self, frame_idx: u64) {
        self.seek_gen += 1;
        self.decoder_seek_tx
            .send(PipelineMessage::Seek {
                new_pos: frame_idx as i64,
                new_gen: self.seek_gen,
            })
            .expect("main thread should be able to send to decoder");
        self.matcher_seek_tx
            .send(PipelineMessage::Seek {
                new_pos: frame_idx as i64,
                new_gen: self.seek_gen,
            })
            .expect("main thread should be able to send to matcher");
    }

    pub(crate) fn shutdown(&self) {
        let _ = self.decoder_seek_tx.send(PipelineMessage::Shutdown);
        let _ = self.matcher_seek_tx.send(PipelineMessage::Shutdown);
    }
}

fn decode_thread(
    mosaic_rx: Receiver<PipelineMessage>,
    matcher_tx: Sender<PipelineMessage>,
    path: Utf8PathBuf,
) {
    let mut curr_gen = 0;
    //recreating here because i can't pass VideoCapture between threads without unsafe or wrapping unnecessarily
    //it only takes single digit milliseconds anyway
    let mut vid_cap = VideoCapture::new(Arc::from(path.clone()), None, true)
        .unwrap_or_else(|e| panic!("could not create video capture for {path}: {e}"));
    loop {
        let pending = vid_cap
            .read_rgb_frame()
            .expect("error decoding source stream")
            .expect("source stream eof");

        select! {
            recv(mosaic_rx) -> msg => match msg {
                Ok(PipelineMessage::Seek { new_pos, new_gen }) => {
                    curr_gen = new_gen;
                    vid_cap.seek_to_frame(new_pos).expect("error seeking source stream");
                    continue;
                },
                Ok(PipelineMessage::Shutdown) => return,
                something_else => panic!("decode thread received unexpected {something_else:?}")
            },

            send(matcher_tx, PipelineMessage::RawFrame {inner: pending.rgb, generation: curr_gen}) -> res => {
                if res.is_err() {panic!("matchmaker thread threw unexpected {res:?}")};
                continue;
            }
        }
    }
}

fn matchmaker_thread(
    mosaic_rx: Receiver<PipelineMessage>,
    decode_rx: Receiver<PipelineMessage>,
    mosaic_tx: Sender<PipelineMessage>,
    matchmaker: MatchMaker,
) {
    let mut curr_gen = 0;
    let mut pending = None;
    loop {
        if pending.is_none() {
            select! {
                recv(mosaic_rx) -> msg => match msg {
                    Ok(PipelineMessage::Seek { new_gen, .. }) => {
                        curr_gen = new_gen;
                        continue;
                    },
                    Ok(PipelineMessage::Shutdown) => return,
                    something_else => panic!("matchmaker thread received unexpected {something_else:?}")
                },

                recv(decode_rx) -> msg => {
                    let Ok(PipelineMessage::RawFrame { inner, generation: frame_gen }) = msg else {
                        panic!("matchmaker thread received unexpected {msg:?}")
                    };
                    if curr_gen <= frame_gen {
                        pending = Some((frame_gen, matchmaker.matchmake(&inner).into_iter().map(|i| DenseIndex(i.0)).collect()));
                    }
                    continue;
                }
            }
        };

        let (frame_gen, inner) = pending.take().unwrap();
        select! {
            recv(mosaic_rx) -> msg => match msg {
                Ok(PipelineMessage::Seek { new_gen, .. }) => {
                    curr_gen = new_gen;
                    pending = None;
                    continue;
                },
                Ok(PipelineMessage::Shutdown) => return,
                something_else => panic!("matchmaker thread received unexpected {something_else:?}")
            },

            send(mosaic_tx, PipelineMessage::ProcessedFrame { inner, generation: frame_gen }) -> res => {
                if res.is_err() {panic!("matchmaker thread threw unexpected {res:?}")};
                continue;
            }
        };
    }
}
