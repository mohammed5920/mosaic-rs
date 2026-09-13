use crate::mosaic::{matchmaker::MatchMaker, media_source::vid_source::VidSource, tiles::Tile};

pub(crate) struct DynamicMosaic {}

impl DynamicMosaic {
    pub(crate) fn new(vid_source: VidSource, tiles: Vec<Tile>, matchmaker: MatchMaker) -> Self {
        todo!()
    }

    pub(crate) fn advance_frame(&mut self) {
        todo!()
    }

    pub(crate) fn seek_to_frame(&mut self, frame_idx: u64) {
        todo!()
    }
}
