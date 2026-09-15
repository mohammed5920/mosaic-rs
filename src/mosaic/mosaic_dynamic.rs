use std::sync::Arc;

use camino::Utf8PathBuf;

use crate::{
    mosaic::{matchmaker::MatchMaker, tiles::Tile},
    types::DenseIndex,
    vidcap::VideoCapture,
};

pub(crate) struct DynamicMosaic {
    vid_cap: VideoCapture,
    matchmaker: MatchMaker,
    pub(crate) tiles: Arc<[Tile]>,
    pub(crate) dense_matches: Vec<DenseIndex>,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl DynamicMosaic {
    pub(crate) fn new(
        path: Utf8PathBuf,
        tiles: Vec<Tile>,
        matchmaker: MatchMaker,
    ) -> anyhow::Result<Self> {
        let vid_cap = VideoCapture::new(Arc::from(path), None, true)?;
        let mut res = Self {
            width: vid_cap.width,
            height: vid_cap.height,
            matchmaker,
            tiles: tiles.into(),
            dense_matches: Vec::new(),
            vid_cap,
        };
        res.advance_frame();
        Ok(res)
    }

    pub(crate) fn advance_frame(&mut self) {
        let new_frame = self
            .vid_cap
            .read_rgb_frame()
            .expect("could not stream source")
            .expect("end of source stream");
        self.dense_matches = self
            .matchmaker
            .matchmake(&new_frame.rgb)
            .into_iter()
            .map(|i| DenseIndex(i.0))
            .collect();
    }

    pub(crate) fn seek_to_frame(&mut self, frame_idx: u64) {
        todo!()
    }
}
