mod ui;

use crate::{config::Config, mpv_handler::MpvHandler};

use std::path::PathBuf;
use walkdir::WalkDir;

use eframe::{
    egui::{self, Context, Event, Key},
    CreationContext,
};

pub struct App {
    state: AppState,
    ui: ui::Ui,
}

struct AppState {
    cfg: Config,
    playlist: Vec<PathBuf>,
    selected_song: usize,
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
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.handle_egui_input(ctx);
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
            selected_song: 0,
            mpv_handler: MpvHandler::default(),
            playlist_behavior: PlaylistBehavior::Continue,
            user_stopped: true,
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
            match self.state.playlist_behavior {
                PlaylistBehavior::Stop => return,
                PlaylistBehavior::Continue => {
                    if self.state.selected_song + 1 < self.state.playlist.len() {
                        self.state.selected_song += 1;
                    } else {
                        return;
                    }
                }
                PlaylistBehavior::RepeatOne => {}
                PlaylistBehavior::RepeatPlaylist => {
                    self.state.selected_song += 1;
                    if self.state.selected_song >= self.state.playlist.len() {
                        self.state.selected_song = 0;
                    }
                }
            }
            self.state.play_selected_song();
        }
    }

    fn handle_egui_input(&mut self, ctx: &Context) {
        let input = ctx.input();
        if input.key_pressed(Key::Space) && !self.state.mpv_handler.active() {
            self.state.play_selected_song();
            return;
        }
        for ev in &input.raw.events {
            match ev {
                Event::Text(s) => match s.as_str() {
                    " " => self.state.mpv_handler.toggle_pause(),
                    "<" => self.state.play_prev(),
                    ">" => self.state.play_next(),
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
}

impl AppState {
    fn read_songs(&mut self) {
        let Some(music_folder) = &self.cfg.music_folder else {
            return;
        };
        self.playlist.clear();
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
        let selection = self.selected_song;
        let sel_path = &self.playlist[selection];
        let path: PathBuf = match &self.cfg.music_folder {
            Some(folder) => folder.join(sel_path),
            None => {
                eprintln!("Can't play song, there is no music folder");
                return;
            }
        };
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

    fn play_prev(&mut self) {
        if self.selected_song == 0 {
            self.selected_song = self.playlist.len() - 1;
        } else {
            self.selected_song -= 1;
        }
        self.play_selected_song();
    }

    fn play_next(&mut self) {
        self.selected_song += 1;
        if self.selected_song >= self.playlist.len() {
            self.selected_song = 0;
        }
        self.play_selected_song();
    }

    fn stop_music(&mut self) {
        self.mpv_handler.stop_music();
        self.user_stopped = true;
    }
}
