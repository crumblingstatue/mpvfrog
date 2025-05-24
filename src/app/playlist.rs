use {crate::config::Config, std::path::PathBuf, walkdir::WalkDir};

#[derive(Default)]
pub struct Playlist {
    items: Vec<Item>,
}

pub struct Item {
    pub path: PathBuf,
}

impl Item {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Playlist {
    pub fn read_songs(&mut self, cfg: &Config) {
        let Some(music_folder) = &cfg.music_folder else {
            return;
        };
        self.items.clear();
        for entry in WalkDir::new(music_folder)
            .follow_links(cfg.follow_symlinks)
            .into_iter()
            .filter_map(Result::ok)
        {
            if entry.file_type().is_file() {
                let en_path = entry.path();
                if let Some(ext) = en_path.extension().and_then(|ext| ext.to_str()) {
                    if ["jpg", "png", "txt"]
                        .into_iter()
                        .any(|filter_ext| filter_ext == ext)
                    {
                        continue;
                    }
                }
                let path = en_path.strip_prefix(music_folder).unwrap().to_owned();
                self.items.push(Item::new(path));
            }
        }
        self.sort();
    }
    pub fn sort(&mut self) {
        self.items.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    }
    pub fn get(&self, idx: usize) -> Option<&Item> {
        self.items.get(idx)
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn iter(&self) -> std::slice::Iter<'_, Item> {
        self.items.iter()
    }
}
