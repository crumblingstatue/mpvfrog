mod ui;

use crate::{config::Config, mpv_handler::MpvHandler};

use std::path::PathBuf;
use walkdir::WalkDir;

use eframe::{
    egui::{self, Event, Key},
    CreationContext,
};

pub struct App {
    state: AppState,
    ui: ui::Ui,
}

struct AppState {
    cfg: Config,
    playlist: Vec<PathBuf>,
    selected_song: Option<usize>,
    mpv_handler: MpvHandler,
    playlist_behavior: PlaylistBehavior,
    /// This is `true` when the user has initiated a stop, rather than just mpv exiting
    user_stopped: bool,
}

#[derive(PartialEq)]
enum PlaylistBehavior {
    Stop,
    Continue,
    RepeatOne,
    RepeatPlaylist,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Forward input to mpv child process
        if self.state.mpv_handler.active() {
            for ev in &ctx.input().raw.events {
                match ev {
                    Event::Text(s) => match s.as_str() {
                        " " => self.state.mpv_handler.toggle_pause(),
                        s => {
                            match s {
                                "9" => {
                                    self.state.cfg.volume -= 2;
                                }
                                "0" => {
                                    self.state.cfg.volume += 2;
                                }
                                "[" => {
                                    self.state.cfg.speed -= 0.1;
                                }
                                "]" => {
                                    self.state.cfg.speed += 0.1;
                                }
                                "{" => {
                                    self.state.cfg.speed -= 0.01;
                                }
                                "}" => {
                                    self.state.cfg.speed += 0.01;
                                }
                                _ => {}
                            }
                            self.state.mpv_handler.input(s);
                        }
                    },
                    Event::Key {
                        key,
                        pressed: true,
                        modifiers: _,
                    } => {
                        if *key == Key::Backspace {
                            self.state.cfg.speed = 1.0;
                            self.state.mpv_handler.input("\x08");
                        }
                    }
                    _ => (),
                }
            }
        }
        // We need to constantly update in order to keep reading from mpv
        ctx.request_repaint();
        self.state.mpv_handler.update();
        self.handle_mpv_not_active();
        // Do the ui
        self.ui.update(&mut self.state, ctx);
    }
    fn on_exit_event(&mut self) -> bool {
        let vec = serde_json::to_vec_pretty(&self.state.cfg).unwrap();
        std::fs::write(Config::path(), &vec).unwrap();
        true
    }
}

impl App {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let mut state = AppState {
            cfg: Config::load_or_default(),
            playlist: Vec::new(),
            selected_song: None,
            mpv_handler: MpvHandler::default(),
            playlist_behavior: PlaylistBehavior::Continue,
            user_stopped: false,
        };
        state.read_songs();
        App {
            ui: Default::default(),
            state,
        }
    }

    fn handle_mpv_not_active(&mut self) {
        if self.state.user_stopped {
            return;
        }
        if !self.state.mpv_handler.active() {
            let Some(sel) = &mut self.state.selected_song else { return; };
            match self.state.playlist_behavior {
                PlaylistBehavior::Stop => return,
                PlaylistBehavior::Continue => {
                    if *sel + 1 < self.state.playlist.len() {
                        *sel += 1;
                    } else {
                        return;
                    }
                }
                PlaylistBehavior::RepeatOne => {}
                PlaylistBehavior::RepeatPlaylist => {
                    *sel += 1;
                    if *sel >= self.state.playlist.len() {
                        *sel = 0;
                    }
                }
            }
            self.state.play_selected_song();
        }
    }
}

impl AppState {
    fn read_songs(&mut self) {
        let Some(music_folder) = &self.cfg.music_folder else {
            return;
        };
        for entry in WalkDir::new(music_folder)
            .into_iter()
            .filter_map(|e| e.ok())
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

    fn sort_songs(&mut self) {
        self.playlist.sort();
    }

    fn play_selected_song(&mut self) {
        self.user_stopped = false;
        let Some(selection) = self.selected_song else { return };
        let sel_path = &self.playlist[selection];
        let path: PathBuf = self.cfg.music_folder.as_ref().unwrap().join(sel_path);
        let ext_str = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        match self.cfg.custom_players.iter().find(|en| en.ext == ext_str) {
            Some(en) => self.mpv_handler.play_music(
                &en.cmd,
                std::iter::once(path.as_ref()).chain(en.args.iter().map(|s| s.as_ref())),
            ),
            None => self.mpv_handler.play_music(
                "mpv",
                [
                    path.as_ref(),
                    "--no-video".as_ref(),
                    format!("--volume={}", self.cfg.volume).as_ref(),
                    format!("--speed={}", self.cfg.speed).as_ref(),
                ],
            ),
        }
    }

    fn stop_music(&mut self) {
        self.mpv_handler.stop_music();
        self.user_stopped = true;
    }
}
