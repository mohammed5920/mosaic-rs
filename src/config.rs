use std::{
    fs::{self, File},
    io::BufWriter,
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

    pub(crate) vsync: bool,
    pub(crate) backend: wgpu::Backend,

    pub(crate) hard_seek_threshold: u64,
    pub(crate) difference_threshold: u64,
    pub(crate) zoom_steps_per_octave: u64,
    pub(crate) tile_base_res: u64,
}

pub(crate) fn load_config() -> AppConfig {
    let config = fs::read_to_string(CONFIG_PATH)
        .map_err(|e| format!("error reading config: {e}"))
        .and_then(|s| {
            serde_json::from_str::<AppConfig>(&s).map_err(|e| format!("error parsing config: {e}"))
        });

    match config {
        Ok(config) => config,
        Err(e) => {
            eprintln!("{e} - loading default");
            let config = AppConfig {
                source_path: Utf8PathBuf::from_str("test/test.jpg").unwrap(),
                tile_path: Utf8PathBuf::from_str("test/vid_tiles").unwrap(),
                cache_path: Utf8PathBuf::from_str("cache").unwrap(),
                vsync: true,
                backend: wgpu::Backend::Vulkan,
                hard_seek_threshold: 4,
                difference_threshold: 300,
                zoom_steps_per_octave: 30,
                tile_base_res: 64,
            };
            let writer = File::create(CONFIG_PATH).expect("could not create config writer");
            let writer = BufWriter::new(writer);
            serde_json::to_writer_pretty(writer, &config).expect("could not write config");
            config
        }
    }
}
