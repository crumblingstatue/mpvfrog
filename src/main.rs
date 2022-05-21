#![feature(let_else)]

mod ansi_parser;
mod ansi_term;

use ansi_term::AnsiTerm;
use directories::ProjectDirs;
use nonblock::NonBlockingReader;
use pty_process::{std::Child, Command as _};
use serde::{Deserialize, Serialize};
use std::{ffi::OsStr, io::Write as _, path::PathBuf, process::Command};
use walkdir::WalkDir;

use eframe::{
    egui::{
        self, Button, CentralPanel, DragValue, Event, ScrollArea, TextEdit, TextStyle,
        TopBottomPanel, Window,
    },
    emath::vec2,
    CreationContext, NativeOptions,
};

#[derive(Serialize, Deserialize, Default)]
struct Config {
    music_folder: Option<PathBuf>,
    /// These should all wrap mpv, but could be different demuxers (like for midi)
    #[serde(default)]
    custom_players: Vec<CustomPlayerEntry>,
    #[serde(default = "default_volume")]
    volume: u8,
}

const fn default_volume() -> u8 {
    50
}

#[derive(Serialize, Deserialize, Default)]
struct CustomPlayerEntry {
    ext: String,
    cmd: String,
    args: Vec<String>,
}

fn cfg_path() -> PathBuf {
    let proj_dirs = ProjectDirs::from("", "crumblingstatue", "mpv-egui-musicplayer").unwrap();
    let cfg_dir = proj_dirs.config_dir();
    std::fs::create_dir_all(cfg_dir).unwrap();
    cfg_dir.join("config.json")
}

impl Config {
    fn load_or_default() -> Self {
        match std::fs::read_to_string(cfg_path()) {
            Ok(string) => serde_json::from_str(&string).unwrap(),
            Err(e) => {
                eprintln!("{}", e);
                Default::default()
            }
        }
    }
}

struct App {
    cfg: Config,
    song_paths: Vec<PathBuf>,
    selected_song: Option<usize>,
    mpv_handler: MpvHandler,
    custom_players_window_show: bool,
}

struct MpvHandler {
    ansi_term: AnsiTerm,
    child: Option<Child>,
    paused: bool,
}

impl MpvHandler {
    fn play_music<'a>(&mut self, mpv_cmd: &str, args: impl IntoIterator<Item = &'a OsStr>) {
        self.stop_music();
        self.ansi_term.reset();
        let child = Command::new(mpv_cmd)
            .args(args)
            .spawn_pty(Some(&pty_process::Size::new(30, 80)))
            .unwrap();
        self.child = Some(child);
    }
    fn stop_music(&mut self) {
        let Some(child) = &mut self.child else { return };
        child.pty().write_all(b"q").unwrap();
        child.wait().unwrap();
        self.child = None;
    }
    fn update_child_out(&mut self, buf: &[u8]) {
        self.ansi_term.feed(buf)
    }
    fn update(&mut self) {
        let Some(child) = &mut self.child else { return; };
        let mut buf = Vec::new();
        let mut nbr = NonBlockingReader::from_fd((*child.pty()).try_clone().unwrap()).unwrap();
        match nbr.read_available(&mut buf) {
            Ok(n_read) => {
                if n_read != 0 {
                    self.update_child_out(&buf);
                }
            }
            Err(e) => {
                eprintln!("error reading from mpv process: {}", e);
                // Better terminate playback
                self.stop_music();
            }
        }
    }

    fn input(&mut self, s: &str) {
        let Some(child) = &mut self.child else { return };
        child.pty().write_all(s.as_bytes()).unwrap();
    }

    fn active(&self) -> bool {
        self.child.is_some()
    }

    fn toggle_pause(&mut self) {
        self.input(" ");
        self.paused ^= true;
    }

    fn paused(&self) -> bool {
        self.paused
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();
        self.mpv_handler.update();
        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.group(|ui| {
                    if ui.button("Music folder").clicked() {
                        self.cfg.music_folder = rfd::FileDialog::new().pick_folder();
                        self.read_songs();
                    }
                    match &self.cfg.music_folder {
                        Some(folder) => {
                            ui.label(&folder.display().to_string());
                        }
                        None => {
                            ui.label("<none>");
                        }
                    }
                });
                if ui.button("Custom players...").clicked() {
                    self.custom_players_window_show ^= true;
                }
            });
        });
        CentralPanel::default().show(ctx, |ui| {
            ScrollArea::vertical()
                .max_height(200.0)
                .id_source("song_scroll")
                .show(ui, |ui| {
                    for (i, path) in self.song_paths.iter().enumerate() {
                        if ui
                            .selectable_label(
                                self.selected_song == Some(i),
                                path.display().to_string(),
                            )
                            .clicked()
                        {
                            self.selected_song = Some(i);
                            self.play_selected_song();
                            break;
                        }
                    }
                });
            if self.mpv_handler.active() {
                for ev in &ctx.input().raw.events {
                    if let Event::Text(s) = ev {
                        match s.as_str() {
                            " " => self.mpv_handler.toggle_pause(),
                            _ => self.mpv_handler.input(s),
                        }
                    }
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.group(|ui| {
                    if ui
                        .add_enabled(self.selected_song.is_some(), Button::new("⏪"))
                        .clicked()
                    {
                        if let Some(sel) = &mut self.selected_song {
                            *sel = sel.saturating_sub(1);
                            self.play_selected_song();
                        }
                    }
                    let active = self.mpv_handler.active();
                    let icon = if active && !self.mpv_handler.paused() {
                        "⏸"
                    } else {
                        "▶"
                    };
                    if ui
                        .add_enabled(self.selected_song.is_some(), Button::new(icon))
                        .clicked()
                    {
                        if active {
                            self.mpv_handler.toggle_pause();
                        } else {
                            self.play_selected_song();
                        }
                    }
                    if ui.add_enabled(active, Button::new("⏹")).clicked() {
                        self.mpv_handler.stop_music();
                    }
                    let can_forward = self
                        .selected_song
                        .map_or(false, |sel| sel + 1 < self.song_paths.len());
                    if ui.add_enabled(can_forward, Button::new("⏩")).clicked() {
                        if let Some(sel) = &mut self.selected_song {
                            *sel += 1;
                            self.play_selected_song();
                        }
                    }
                });
                ui.group(|ui| {
                    ui.label("🔈");
                    ui.add(DragValue::new(&mut self.cfg.volume));
                });
            });
            ui.separator();
            ScrollArea::vertical()
                .id_source("out_scroll")
                .stick_to_bottom()
                .show(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(
                            &mut self.mpv_handler.ansi_term.contents_to_string().as_str(),
                        )
                        .desired_width(620.0)
                        .font(TextStyle::Monospace),
                    );
                });
        });
        self.custom_players_window_ui(ctx);
    }
    fn on_exit_event(&mut self) -> bool {
        let vec = serde_json::to_vec_pretty(&self.cfg).unwrap();
        std::fs::write(cfg_path(), &vec).unwrap();
        true
    }
}

impl Default for MpvHandler {
    fn default() -> Self {
        Self {
            ansi_term: AnsiTerm::new(80),
            child: None,
            paused: false,
        }
    }
}

impl App {
    fn new(cc: &CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let mut this = App {
            cfg: Config::load_or_default(),
            song_paths: Vec::new(),
            selected_song: None,
            mpv_handler: MpvHandler::default(),
            custom_players_window_show: false,
        };
        this.read_songs();
        this
    }
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

    fn custom_players_window_ui(&mut self, ctx: &eframe::egui::Context) {
        Window::new("Custom players")
            .open(&mut self.custom_players_window_show)
            .show(ctx, |ui| {
                for en in &mut self.cfg.custom_players {
                    ui.group(|ui| {
                        ui.label("extension");
                        ui.text_edit_singleline(&mut en.ext);
                        ui.label("command");
                        ui.text_edit_singleline(&mut en.cmd);
                        ui.label("args");
                        for arg in &mut en.args {
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(arg);
                                if ui.button("...").clicked() {
                                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                                        *arg = path.to_string_lossy().into_owned();
                                    }
                                }
                            });
                        }
                        if ui.button("+").clicked() {
                            en.args.push(String::new());
                        }
                    });
                }
                if ui.button("add new custom player").clicked() {
                    self.cfg.custom_players.push(CustomPlayerEntry::default());
                }
            });
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

fn main() {
    let native_opts = NativeOptions {
        initial_window_size: Some(vec2(620., 440.)),
        ..Default::default()
    };
    eframe::run_native(
        "mpv-egui-musicplayer",
        native_opts,
        Box::new(|cc| Box::new(App::new(cc))),
    );
}
