#![feature(let_else)]

mod ansi_parser;

use ansi_parser::AnsiParser;
use directories::ProjectDirs;
use egui_inspect::inspect;
use pty_process::Command as _;
use serde::{Deserialize, Serialize};
use std::{
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    process::{Child, Command},
};
use walkdir::WalkDir;

use eframe::{
    egui::{CentralPanel, DragValue, Event, ScrollArea},
    CreationContext, NativeOptions,
};

#[derive(Serialize, Deserialize, Default)]
struct Config {
    music_folder: Option<PathBuf>,
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

type PtyChild = pty_process::Child<Child, <Command as pty_process::Command>::Pty>;

struct App {
    mpv_child: Option<PtyChild>,
    child_out: String,
    volume: u8,
    ansi_parser: AnsiParser,
    cfg: Config,
    song_paths: Vec<PathBuf>,
    playing_index: Option<usize>,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();
        CentralPanel::default().show(ctx, |ui| {
            inspect! {
                ui,
                self.ansi_parser
            }
            ui.horizontal(|ui| {
                ui.label("Volume");
                ui.add(DragValue::new(&mut self.volume));
            });
            ui.horizontal(|ui| {
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
            ScrollArea::vertical()
                .max_height(200.0)
                .id_source("song_scroll")
                .show(ui, |ui| {
                    for (i, path) in self.song_paths.iter().enumerate() {
                        if ui
                            .selectable_label(
                                self.playing_index == Some(i),
                                path.display().to_string(),
                            )
                            .clicked()
                        {
                            let path: PathBuf = self
                                .cfg
                                .music_folder
                                .as_ref()
                                .unwrap()
                                .join(path)
                                .to_owned();
                            Self::play_music(
                                &path,
                                &mut self.child_out,
                                &mut self.mpv_child,
                                self.volume,
                            );
                            self.playing_index = Some(i);
                        }
                    }
                });
            ui.separator();
            match &mut self.mpv_child {
                None => {}
                Some(child) => {
                    for ev in &ctx.input().raw.events {
                        if let Event::Text(s) = ev {
                            child.pty().write_all(s.as_bytes()).unwrap();
                        }
                    }
                    let mut buf = [0u8; 256];
                    let n_read = child.pty().read(&mut buf).unwrap();
                    Self::update_child_out(
                        &mut self.ansi_parser,
                        &mut self.child_out,
                        &buf[..n_read],
                    );
                    if ui.button("stop").clicked() {
                        Self::stop_music(&mut self.mpv_child);
                    }
                }
            }
            ScrollArea::vertical()
                .id_source("out_scroll")
                .stick_to_bottom()
                .show(ui, |ui| {
                    ui.label(&self.child_out);
                });
        });
    }
    fn on_exit_event(&mut self) -> bool {
        let vec = serde_json::to_vec_pretty(&self.cfg).unwrap();
        std::fs::write(cfg_path(), &vec).unwrap();
        true
    }
}

impl App {
    fn new(_cc: &CreationContext<'_>) -> Self {
        let mut this = App {
            mpv_child: None,
            child_out: String::new(),
            volume: 50,
            ansi_parser: Default::default(),
            cfg: Config::load_or_default(),
            song_paths: Vec::new(),
            playing_index: None,
        };
        this.read_songs();
        this
    }
    fn play_music(
        path: &Path,
        child_out: &mut String,
        mpv_child: &mut Option<PtyChild>,
        volume: u8,
    ) {
        Self::stop_music(mpv_child);
        child_out.clear();
        let child = Command::new("mpv")
            .arg("--no-video")
            .arg(path)
            .arg(&format!("--volume={}", volume))
            .spawn_pty(Some(&pty_process::Size::new(30, 80)))
            .unwrap();
        *mpv_child = Some(child);
    }
    fn update_child_out(ansi_parser: &mut AnsiParser, out: &mut String, buf: &[u8]) {
        ansi_parser.advance_and_write(buf, out);
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
    }
    fn stop_music(mpv_child: &mut Option<PtyChild>) {
        if let Some(child) = mpv_child {
            child.pty().write_all(b"q").unwrap();
            child.wait().unwrap();
            *mpv_child = None;
        }
    }
}

fn main() {
    let native_opts = NativeOptions::default();
    eframe::run_native(
        "mpv-egui-musicplayer",
        native_opts,
        Box::new(|cc| Box::new(App::new(cc))),
    );
}
