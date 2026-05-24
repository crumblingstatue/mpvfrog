//! Persistent configuration for the application

use {
    directories::ProjectDirs,
    enum_kinds::EnumKind,
    serde::{Deserialize, Deserializer, Serialize},
    std::{
        fmt::Display,
        path::{Path, PathBuf},
    },
};

pub type ThemeColors = [[u8; 3]; 12];

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub music_folder: Option<PathBuf>,
    /// These should all wrap mpv, but could be different demuxers (like for midi)
    #[serde(default)]
    pub custom_demuxers: Vec<CustomDemuxerEntry>,
    #[serde(default = "default_volume")]
    pub volume: u8,
    #[serde(default = "default_speed")]
    pub speed: f64,
    #[serde(default)]
    pub video: bool,
    #[serde(default)]
    pub theme: Option<ThemeColors>,
    /// Follow symbolic links when loading files from a dir
    #[serde(default)]
    pub follow_symlinks: bool,
    /// Skip hidden files/folders
    #[serde(default)]
    pub skip_hidden: bool,
    /// Paths to fallback fonts to load on startup
    #[serde(default)]
    pub fallback_font_paths: Vec<String>,
    #[serde(default = "default_scan_max_depth")]
    pub scan_max_depth: u8,
}

fn default_scan_max_depth() -> u8 {
    4
}

impl Default for Config {
    fn default() -> Self {
        Self {
            music_folder: Default::default(),
            custom_demuxers: Default::default(),
            volume: default_volume(),
            speed: default_speed(),
            video: false,
            theme: None,
            follow_symlinks: false,
            skip_hidden: false,
            fallback_font_paths: Vec::new(),
            scan_max_depth: default_scan_max_depth(),
        }
    }
}

const fn default_volume() -> u8 {
    50
}

const fn default_speed() -> f64 {
    1.0
}

impl Config {
    pub fn load_if_exists() -> Option<anyhow::Result<Self>> {
        let path = Self::path();
        path.exists().then(|| Self::load(&path))
    }
    fn load(path: &Path) -> anyhow::Result<Self> {
        let string = std::fs::read_to_string(path)?;
        let this = serde_json::from_str(&string)?;
        Ok(this)
    }
    pub fn path() -> PathBuf {
        let proj_dirs = ProjectDirs::from("", "crumblingstatue", "mpvfrog").unwrap();
        let cfg_dir = proj_dirs.config_dir();
        std::fs::create_dir_all(cfg_dir).unwrap();
        cfg_dir.join("config.json")
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, EnumKind, Clone)]
#[enum_kind(PredicateKind)]
pub enum Predicate {
    BeginsWith(String),
    #[serde(alias = "HasExt")]
    HasExts(#[serde(deserialize_with = "HasExtsPredicate::deserialize")] HasExtsPredicate),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Default)]
pub struct HasExtsPredicate {
    /// Space separated list of file extensions
    pub ext_list: String,
    /// Whether the ext comparison should be case sensitive
    pub case_sensitive: bool,
}

impl HasExtsPredicate {
    fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Serialize, Deserialize)]
        #[serde(untagged)]
        enum Versions {
            // Used to be a single string
            V1(String),
            V2(HasExtsPredicate),
        }
        let obj = Versions::deserialize(d)?;
        Ok(match obj {
            Versions::V1(ext_list) => Self {
                ext_list,
                case_sensitive: false,
            },
            Versions::V2(pred) => pred,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CustomDemuxerEntry {
    pub predicates: Vec<Predicate>,
    pub reader_cmd: Command,
    pub extra_mpv_args: Vec<String>,
    #[serde(default)]
    pub name: String,
}
#[derive(thiserror::Error, Debug)]
#[error("parse error: {kind}")]
pub struct CommandParseError {
    kind: CommandParseErrorKind,
}

#[derive(thiserror::Error, Debug)]
enum CommandParseErrorKind {
    #[error("Expected {what}, but reached end.")]
    ExpectedButEnd { what: &'static str },
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct Command {
    pub name: String,
    pub args: Vec<ArgType>,
}

impl Command {
    pub fn to_string(&self) -> Result<String, std::fmt::Error> {
        use std::fmt::Write;
        let mut buf = String::new();
        write!(&mut buf, "{} ", self.name)?;
        for arg in &self.args {
            write!(&mut buf, "{arg} ")?;
        }
        Ok(buf)
    }
}

impl Command {
    pub fn from_str(src: &str) -> Result<Self, CommandParseError> {
        let mut tokens = src.split_whitespace();
        let cmd_name = tokens.next().ok_or(CommandParseError {
            kind: CommandParseErrorKind::ExpectedButEnd { what: "command" },
        })?;
        let mut args = Vec::new();
        for token in tokens {
            if token == "{}" {
                args.push(ArgType::SongPath);
            } else {
                args.push(ArgType::Custom(token.to_string()));
            }
        }
        Ok(Self {
            name: cmd_name.to_string(),
            args,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ArgType {
    Custom(String),
    SongPath,
}

impl Display for ArgType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Custom(string) => write!(f, "{string}"),
            Self::SongPath => write!(f, "{{}}"),
        }
    }
}
impl Predicate {
    pub(crate) fn matches(&self, path: &Path) -> bool {
        match self {
            Self::BeginsWith(fragment) => Self::matches_begin(fragment, path),
            Self::HasExts(HasExtsPredicate {
                ext_list,
                case_sensitive,
            }) => Self::matches_exts(ext_list, path, *case_sensitive),
        }
    }

    fn matches_begin(fragment: &str, path: &Path) -> bool {
        match path.file_name().and_then(|path| path.to_str()) {
            Some(path_str) => path_str.starts_with(fragment),
            None => false,
        }
    }
    /// `exts` is a list of space separated extensions
    fn matches_exts(exts: &str, path: &Path, sensitive: bool) -> bool {
        match path.extension() {
            Some(path_ext) => exts.split_whitespace().any(|ext| {
                if sensitive {
                    path_ext == ext
                } else {
                    path_ext.eq_ignore_ascii_case(ext)
                }
            }),
            None => false,
        }
    }
}

pub trait PredicateSliceExt {
    fn find_predicate_match(&self, path: &Path) -> bool;
}

impl PredicateSliceExt for [Predicate] {
    fn find_predicate_match(&self, path: &Path) -> bool {
        self.iter().any(|pred| pred.matches(path))
    }
}
