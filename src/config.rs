use std::{
    fs::{self, File},
    io::BufWriter,
    num::NonZero,
    str::FromStr,
    sync::LazyLock,
};

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

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

    pub(crate) synthetic_tile_count: Option<NonZero<u64>>,
    pub(crate) hard_seek_threshold: NonZero<u64>,
    pub(crate) zoom_steps_per_octave: NonZero<u64>,
    pub(crate) tile_base_res: NonZero<u64>,
    pub(crate) prefetch_multiplier: NonZero<u64>,
    pub(crate) ram_percent: u64,
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
                source_path: Utf8PathBuf::from_str("test/test2.jpg").unwrap(),
                tile_path: Utf8PathBuf::from_str("test/pic_tiles").unwrap(),
                cache_path: Utf8PathBuf::from_str("cache").unwrap(),

                is_vsync: true,
                end_after_init: false,
                force_static_tiles: false,
                backend: wgpu::Backend::Vulkan,

                synthetic_tile_count: None,
                difference_threshold: 300,
                ram_percent: 75,
                prefetch_multiplier: NonZero::new(4).unwrap(),
                hard_seek_threshold: NonZero::new(4).unwrap(),
                zoom_steps_per_octave: NonZero::new(30).unwrap(),
                tile_base_res: NonZero::new(64).unwrap(),
            };
            let writer = File::create(CONFIG_PATH).expect("could not create config writer");
            let writer = BufWriter::new(writer);
            serde_json::to_writer_pretty(writer, &config).expect("could not write config");
            config
        }
    };

    assert!(
        config
            .synthetic_tile_count
            .is_none_or(|c| c.get() <= 16_777_216),
        "synthetic_tile_count must be <= 16_777_216"
    );
    assert!(
        (config.prefetch_multiplier.get() as f64).log2().fract() == 0.0,
        "prefetch_multiplier must be a power of 2"
    );
    assert!(
        config.ram_percent <= 100,
        "cannot use more than 100% of free RAM (sadly)"
    );
    config
}
