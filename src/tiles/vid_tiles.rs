use {
    crate::{CACHE_DIR, tiles::calc_average_colour},
    camino::Utf8PathBuf,
    imohash::Hasher as ImoHasher,
    std::{fs, str::FromStr},
};

pub struct VidTile {
    pub average_colour: [u8; 3],
    source_path: Utf8PathBuf,
    start_frame: u64,
    end_frame: u64,
}

fn read_colour_cache(hash: u128) -> Option<Vec<[u8; 3]>> {
    let mut cache_dir = Utf8PathBuf::from_str(CACHE_DIR).ok()?;
    cache_dir.push(hash.to_string());
    Some(
        fs::read(cache_dir.with_extension("bin"))
            .ok()?
            .as_chunks::<3>()
            .0
            .into(),
    )
}

pub fn vid_tiles_from_path(path: Utf8PathBuf, tile_base_res: u64) -> anyhow::Result<Vec<VidTile>> {
    let hasher = ImoHasher::new();
    let hash = hasher.sum_file(path.as_str())?;

    let colours = match read_colour_cache(hash) {
        Some(colours) => colours,
        // None => stream_whole_video(&path, tile_base_res, tile_base_res)?
        //     .map(|base_size_frame| calc_average_colour(&base_size_frame))
        //     .collect(),
        None => todo!(),
    };

    todo!()
}
