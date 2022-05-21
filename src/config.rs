use std::path::PathBuf;

use directories::ProjectDirs;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    pub music_folder: Option<PathBuf>,
    /// These should all wrap mpv, but could be different demuxers (like for midi)
    #[serde(default)]
    pub custom_players: Vec<CustomPlayerEntry>,
    #[serde(default = "default_volume")]
    pub volume: u8,
}

const fn default_volume() -> u8 {
    50
}

impl Config {
    pub fn load_or_default() -> Self {
        match std::fs::read_to_string(Self::path()) {
            Ok(string) => serde_json::from_str(&string).unwrap(),
            Err(e) => {
                eprintln!("{}", e);
                Default::default()
            }
        }
    }
    pub fn path() -> PathBuf {
        let proj_dirs = ProjectDirs::from("", "crumblingstatue", "mpv-egui-musicplayer").unwrap();
        let cfg_dir = proj_dirs.config_dir();
        std::fs::create_dir_all(cfg_dir).unwrap();
        cfg_dir.join("config.json")
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct CustomPlayerEntry {
    pub ext: String,
    pub cmd: String,
    pub args: Vec<String>,
}
