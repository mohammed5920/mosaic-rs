use {
    crate::{
        config::CONFIG, mosaic::tiles::calc_average_colour, util::vid_util::is_fixed_frame_rate,
        vidcap::VideoCapture,
    },
    camino::Utf8Path,
    rustc_hash::{FxBuildHasher, FxHashMap},
    std::{collections::HashMap, sync::Arc},
};

#[derive(Clone)]
pub(crate) struct VidTile {
    pub(crate) average_colour: [u8; 3],
    pub(crate) start_frame_index: u32,
    //reference counted string because many tiles can come from the same video
    pub(crate) source_path: Arc<Utf8Path>,
    end_frame_index: u32,
}

impl VidTile {
    pub(crate) fn new(
        average_colour: [u8; 3],
        source_path: Arc<Utf8Path>,
        start_frame_index: u32,
        end_frame_index: u32,
    ) -> Self {
        Self {
            average_colour,
            start_frame_index,
            source_path,
            end_frame_index,
        }
    }

    pub(crate) fn end_frame_index(&self) -> u32 {
        if CONFIG.force_static_tiles {
            1
        } else {
            self.end_frame_index
        }
    }
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
pub(crate) fn get_start_end_frame_indices(colours: &[[u8; 3]]) -> FxHashMap<usize, usize> {
    let mut res = HashMap::with_hasher(FxBuildHasher);
    if colours.is_empty() {
        return res;
    }
    let mut last_start_index = 0;
    let mut last_start_colour = colours[0];
    for (offset, current_colour) in colours[1..].iter().enumerate() {
        let i = offset + 1;
        if mse(last_start_colour, *current_colour) >= CONFIG.difference_threshold as u32 {
            res.insert(last_start_index, i);
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

pub(crate) fn process_video_for_vidtiles(
    source_path: Arc<Utf8Path>,
) -> Result<Vec<[u8; 3]>, String> {
    match is_fixed_frame_rate(source_path.as_str()) {
        Ok(false) => return Err("Video is variable refresh-rate".to_owned()),
        Err(e) => return Err(format!("Could not probe video because {e}")),
        Ok(true) => {}
    }

    let mut cap = match VideoCapture::new(source_path.clone(), Some(CONFIG.tile_base_res.get())) {
        Ok(cap) => cap,
        Err(e) => return Err(format!("{e} while opening as capture")),
    };

    let mut colours = Vec::new();
    loop {
        let curr_frame = match cap.read_rgb_frame() {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return Err(format!("{e} while streaming capture")),
        };

        let curr_colour = calc_average_colour(&curr_frame.rgb);
        colours.push(curr_colour);
    }

    Ok(colours)
}
