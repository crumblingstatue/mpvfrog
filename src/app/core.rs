use {
    super::{ModalPopup, PlaylistBehavior, playlist::Playlist},
    crate::{
        config::{Config, PredicateSliceExt},
        ipc::Bridge,
        logln,
        mpv_handler::{CustomDemuxer, MpvHandler},
        util::result_ext::ResultModalExt,
    },
    std::{ffi::OsStr, path::PathBuf},
};

pub struct Core {
    pub(crate) cfg: Config,
    pub(crate) playlist: Playlist,
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
        self.playlist.read_songs(&self.cfg);
    }

    pub(crate) fn play_selected_song(&mut self, modal: &mut ModalPopup) {
        self.save_mpv_values_to_cfg();
        self.user_stopped = false;
        let selection = self.selected_song;
        let Some(sel_item) = &self.playlist.get(selection) else {
            logln!("play_selected_song: Dangling index: {selection}");
            return;
        };
        let path: PathBuf = match &self.cfg.music_folder {
            Some(folder) => folder.join(&sel_item.path),
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

    pub(super) fn save_mpv_values_to_cfg(&mut self) {
        self.mpv_handler.ipc(|b| {
            self.cfg.volume = b.observed.volume;
            self.cfg.speed = b.observed.speed;
        });
    }

    /// Plays the selected song, or toggles the pause state if already playing
    pub fn play_or_toggle_pause(&mut self, modal: &mut ModalPopup) {
        if self.mpv_handler.active() {
            self.mpv_handler
                .ipc(Bridge::toggle_pause)
                .err_popup("Play error", modal);
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
        self.mpv_handler.ipc(|b| b.seek(pos)).unwrap_or(Ok(()))
    }

    pub(crate) fn handle_event(&mut self, event: crate::ipc::IpcEvent) {
        match event {
            crate::ipc::IpcEvent::EndFile => {
                self.save_mpv_values_to_cfg();
            }
        }
    }
}
