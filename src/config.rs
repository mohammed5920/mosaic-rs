use std::{
    fs::{self, File},
    io::BufWriter,
    num::NonZero,
    str::FromStr,
    sync::LazyLock,
};

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::util::is_power_of_two;

const CONFIG_PATH: &str = "config.json";
pub(crate) static CONFIG: LazyLock<AppConfig> = LazyLock::new(load_config);

#[derive(Serialize, Deserialize)]
pub(crate) struct AppConfig {
    pub(crate) source_path: Utf8PathBuf,
    pub(crate) tile_path: Utf8PathBuf,
    pub(crate) cache_path: Utf8PathBuf,

    pub(crate) backend: wgpu::Backend,
    pub(crate) is_vsync: bool,
    ///for profiling initialisation
    pub(crate) end_after_init: bool,
    ///don't make the tiles move
    pub(crate) force_static_tiles: bool,

    //it is sometimes cheaper to fast forward a video to a given frame idx
    //rather than explicitly seek to it
    //depending on the gap between where the current frame is vs the target frame
    //how big is that gap? not easily discoverable, changes per video and sometimes while playing
    //so this is a best effort guess
    pub(crate) hard_seek_threshold: u64,
    pub(crate) zoom_steps_per_octave: NonZero<u64>,
    pub(crate) tile_base_res: NonZero<u64>,
    pub(crate) prefetch_multiplier: NonZero<u64>,
    pub(crate) video_fps: f64,
    pub(crate) ram_percent: f64,
    ///can be zero to load every single video frame as a tile
    pub(crate) difference_threshold: u64,
}

pub(crate) fn load_config() -> AppConfig {
    let config = fs::read_to_string(CONFIG_PATH)
        .map_err(|e| format!("error reading config: {e}"))
        .and_then(|s| {
            serde_json::from_str::<AppConfig>(&s).map_err(|e| format!("error parsing config: {e}"))
        });

    let config = match config {
        Ok(config) => config,
        Err(e) => {
            eprintln!("{e} - loading default");
            let config = AppConfig {
                source_path: Utf8PathBuf::from_str("test/source.jpg").unwrap(),
                tile_path: Utf8PathBuf::from_str("test/tiles").unwrap(),
                cache_path: Utf8PathBuf::from_str("cache.bin").unwrap(),

                is_vsync: true,
                end_after_init: false,
                force_static_tiles: false,
                backend: wgpu::Backend::Vulkan,

                difference_threshold: 300,
                ram_percent: 75.0,
                prefetch_multiplier: NonZero::new(2).unwrap(),
                hard_seek_threshold: 3,
                zoom_steps_per_octave: NonZero::new(30).unwrap(),
                tile_base_res: NonZero::new(64).unwrap(),
                video_fps: 24.0,
            };
            let writer = File::create(CONFIG_PATH).expect("could not create config writer");
            let writer = BufWriter::new(writer);
            serde_json::to_writer_pretty(writer, &config).expect("could not write config");
            config
        }
    };

    assert!(
        is_power_of_two(config.prefetch_multiplier.get()),
        "prefetch_multiplier must be a power of 2"
    );
    assert!(
        config.ram_percent <= 100.0,
        "cannot use more than 100% of free RAM (sadly)"
    );
    config
}
