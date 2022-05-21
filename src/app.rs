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
    song_paths: Vec<PathBuf>,
    selected_song: Option<usize>,
    mpv_handler: MpvHandler,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Forward input to mpv child process
        if self.state.mpv_handler.active() {
            for ev in &ctx.input().raw.events {
                match ev {
                    Event::Text(s) => match s.as_str() {
                        " " => self.state.mpv_handler.toggle_pause(),
                        _ => self.state.mpv_handler.input(s),
                    },
                    Event::Key {
                        key,
                        pressed: true,
                        modifiers: _,
                    } => {
                        if *key == Key::Backspace {
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
            song_paths: Vec::new(),
            selected_song: None,
            mpv_handler: MpvHandler::default(),
        };
        state.read_songs();
        App {
            ui: Default::default(),
            state,
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
                self.song_paths.push(path);
            }
        }
        self.sort_songs();
    }

    fn sort_songs(&mut self) {
        self.song_paths.sort();
    }

    fn play_selected_song(&mut self) {
        let Some(selection) = self.selected_song else { return };
        let sel_path = &self.song_paths[selection];
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
                ],
            ),
        }
    }
}
