use {
    super::{ModalPopup, PlaylistBehavior},
    crate::{
        config::{Config, PredicateSliceExt},
        logln,
        mpv_handler::{CustomDemuxer, MpvHandler},
    },
    std::{ffi::OsStr, path::PathBuf},
    walkdir::WalkDir,
};

pub struct Core {
    pub(crate) cfg: Config,
    pub(crate) playlist: Vec<PathBuf>,
    pub(crate) selected_song: usize,
    pub(crate) mpv_handler: MpvHandler,
    pub(super) playlist_behavior: PlaylistBehavior,
    /// This is `true` when the user has initiated a stop, rather than just mpv exiting
    pub(super) user_stopped: bool,
    /// True if a method of AppState caused the song to be changed
    ///
    /// We can use this to scroll to the changed song in the ui for example.
    pub(super) song_change: bool,
}

impl Core {
    pub(crate) fn read_songs(&mut self) {
        let Some(music_folder) = &self.cfg.music_folder else {
            return;
        };
        self.playlist.clear();
        for entry in WalkDir::new(music_folder)
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
                self.playlist.push(path);
            }
        }
        self.sort_songs();
    }

    pub(super) fn sort_songs(&mut self) {
        self.playlist.sort();
    }

    pub(crate) fn play_selected_song(&mut self, modal: &mut ModalPopup) {
        self.save_mpv_values_to_cfg();
        self.user_stopped = false;
        let selection = self.selected_song;
        let sel_path = &self.playlist[selection];
        let path: PathBuf = match &self.cfg.music_folder {
            Some(folder) => folder.join(sel_path),
            None => {
                logln!("Can't play song, there is no music folder");
                return;
            }
        };
        let vol_arg = format!("--volume={}", self.cfg.volume);
        let speed_arg = format!("--speed={}", self.cfg.speed);
        let mut mpv_args = vec![
            path.as_ref(),
            "--input-ipc-server=/tmp/mpvfrog.sock".as_ref(),
            vol_arg.as_ref(),
            speed_arg.as_ref(),
        ];
        if !self.cfg.video {
            mpv_args.push("--no-video".as_ref());
        }
        let demuxer = match self
            .cfg
            .custom_players
            .iter()
            .find(|en| en.predicates.find_predicate_match(&path))
        {
            Some(en) => {
                mpv_args.remove(0);
                mpv_args.extend(en.extra_mpv_args.iter().map(<_ as AsRef<OsStr>>::as_ref));
                Some(CustomDemuxer::from_config_cmd(
                    &en.reader_cmd,
                    path.as_ref(),
                ))
            }
            None => None,
        };
        crate::app::LOG.lock().unwrap().clear();
        logln!("Mpv args: {mpv_args:?}");
        if let Err(e) = self.mpv_handler.play_music("mpv", mpv_args, demuxer) {
            modal.error("Play error", e);
            self.playlist_behavior = PlaylistBehavior::Stop;
        }
    }
    pub fn play_prev(&mut self, modal: &mut ModalPopup) {
        if self.selected_song == 0 {
            self.selected_song = self.playlist.len() - 1;
        } else {
            self.selected_song -= 1;
        }
        self.play_selected_song(modal);
        self.song_change = true;
    }

    pub fn play_next(&mut self, modal: &mut ModalPopup) {
        self.selected_song += 1;
        if self.selected_song >= self.playlist.len() {
            self.selected_song = 0;
        }
        self.play_selected_song(modal);
        self.song_change = true;
    }

    pub fn stop_music(&mut self) {
        self.save_mpv_values_to_cfg();
        self.mpv_handler.stop_music();
        self.user_stopped = true;
    }

    fn save_mpv_values_to_cfg(&mut self) {
        if let Some(vol) = self.mpv_handler.volume() {
            self.cfg.volume = vol;
        }
        if let Some(speed) = self.mpv_handler.speed() {
            self.cfg.speed = speed;
        }
    }

    /// Plays the selected song, or toggles the pause state if already playing
    pub fn play_or_toggle_pause(&mut self, modal: &mut ModalPopup) {
        if self.mpv_handler.active() {
            if let Err(e) = self.mpv_handler.toggle_pause() {
                modal.error("Play error", e);
            }
        } else {
            self.play_selected_song(modal);
        }
    }

    pub(super) fn handle_mpv_not_active(&mut self, modal: &mut ModalPopup) {
        if self.user_stopped {
            return;
        }
        if !self.mpv_handler.active() {
            match self.playlist_behavior {
                PlaylistBehavior::Stop => return,
                PlaylistBehavior::Continue => {
                    if self.selected_song + 1 < self.playlist.len() {
                        self.selected_song += 1;
                    } else {
                        return;
                    }
                }
                PlaylistBehavior::RepeatOne => {}
                PlaylistBehavior::RepeatPlaylist => {
                    self.selected_song += 1;
                    if self.selected_song >= self.playlist.len() {
                        self.selected_song = 0;
                    }
                }
            }
            // If we reached this point, we can take this as the song having been changed
            self.song_change = true;
            self.play_selected_song(modal);
        }
    }

    pub(crate) fn seek(&mut self, pos: f64) -> anyhow::Result<()> {
        self.mpv_handler.seek(pos)
    }

    pub fn set_video(&mut self, show: bool) -> anyhow::Result<()> {
        self.mpv_handler.set_video(show)
    }

    pub(crate) fn handle_event(&mut self, event: crate::ipc::IpcEvent) {
        match event {
            crate::ipc::IpcEvent::EndFile => {
                self.save_mpv_values_to_cfg();
            }
        }
    }
}
