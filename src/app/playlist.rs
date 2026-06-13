use {
    crate::{config::Config, logln},
    std::{
        cmp::Ordering,
        path::{Path, PathBuf},
    },
    walkdir::WalkDir,
};

#[derive(Default)]
pub struct Playlist {
    items: Vec<Item>,
    walkdir_recv: Option<std::sync::mpsc::Receiver<Vec<Item>>>,
}

pub struct Item {
    pub path: PathBuf,
}

impl Item {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
}

impl Playlist {
    pub fn start_scan(&mut self, cfg: &Config) {
        let Some(music_folder) = cfg.music_folder.clone() else {
            return;
        };
        self.items.clear();
        let skip_hidden = cfg.skip_hidden;
        let follow_symlinks = cfg.follow_symlinks;
        let mut items = Vec::new();
        let (send, recv) = std::sync::mpsc::channel();
        self.walkdir_recv = Some(recv);
        let max_depth: usize = cfg.scan_max_depth.into();
        std::thread::spawn(move || {
            let entries = WalkDir::new(&music_folder)
                // Make sure we yield files before dirs
                // This also avoids descending into a dir before we yielded all files
                .sort_by(
                    |a, b| match (a.file_type().is_dir(), b.file_type().is_dir()) {
                        (false, true) => Ordering::Less,
                        (true, false) => Ordering::Greater,
                        _ => a.file_name().cmp(b.file_name()),
                    },
                )
                .max_depth(max_depth)
                .follow_links(follow_symlinks)
                .into_iter()
                .filter_entry(|en| if skip_hidden { !is_hidden(en) } else { true });
            let mut counter: u32 = 0;
            for entry in entries.filter_map(Result::ok) {
                if entry.file_type().is_file() {
                    let en_path = entry.path();
                    if let Some(ext) = en_path.extension().and_then(|ext| ext.to_str())
                        && ["jpg", "png", "txt"]
                            .into_iter()
                            .any(|filter_ext| filter_ext == ext)
                    {
                        continue;
                    }
                    let path = en_path.strip_prefix(&music_folder).unwrap().to_owned();
                    items.push(Item::new(path));
                    // We don't want to send too often, so we use a counter to limit send frequency
                    if counter.is_multiple_of(500) {
                        // If we can't send, we abort the scanning
                        if send.send(std::mem::take(&mut items)).is_err() {
                            return;
                        }
                    }
                }
                counter += 1;
            }
            send.send(std::mem::take(&mut items)).unwrap();
        });
    }
    /// Returns true if there was an update, and filter needs to be recomputed
    #[must_use]
    pub fn update(&mut self) -> bool {
        let mut update_happened = false;
        if let Some(recv) = &self.walkdir_recv {
            loop {
                match recv.try_recv() {
                    Ok(items) => {
                        self.items.extend(items);
                        update_happened = true;
                        // Safety limit for playlist items.
                        // Too large playlist causes issues.
                        if self.items.len() > 25_000 {
                            logln!("Error: Aborting playlist update due to too many items.");
                            self.walkdir_recv = None;
                            break;
                        }
                    }
                    Err(e) => match e {
                        std::sync::mpsc::TryRecvError::Empty => {
                            break;
                        }
                        std::sync::mpsc::TryRecvError::Disconnected => {
                            self.walkdir_recv = None;
                            break;
                        }
                    },
                }
            }
        }
        if update_happened {
            self.sort();
        }
        update_happened
    }
    pub fn is_scanning(&self) -> bool {
        self.walkdir_recv.is_some()
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

    pub(crate) fn cancel_scan(&mut self) {
        // This relies on the scan thread stopping if it fails to send
        self.walkdir_recv = None;
    }
    pub fn pos_of_path(&self, path: &Path) -> Option<usize> {
        self.iter().position(|item| item.path == path)
    }
}
