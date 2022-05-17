#![feature(let_else)]

mod ansi_parser;
mod ansi_term;

use ansi_term::AnsiTerm;
use directories::ProjectDirs;
use pty_process::Command as _;
use serde::{Deserialize, Serialize};
use std::{
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc,
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

type ThreadRecv = mpsc::Receiver<ThreadMessage>;
type HostSend = mpsc::Sender<HostMessage>;

enum ThreadMessage {
    MpvOut { buf: Box<[u8; 256]>, n_read: usize },
    PlaybackStopped,
}

enum HostMessage {
    /// Keyboard string input
    Input(String),
    /// Stop playback
    Stop,
}

struct App {
    cfg: Config,
    song_paths: Vec<PathBuf>,
    playing_index: Option<usize>,
    mpv_handler: MpvHandler,
}

struct MpvHandler {
    from_thread_recv: Option<ThreadRecv>,
    to_thread_send: Option<HostSend>,
    ansi_term: AnsiTerm,
    volume: u8,
}

impl MpvHandler {
    fn play_music(&mut self, path: &Path) {
        self.stop_music();
        if let Some(recv) = &mut self.from_thread_recv {
            // Wait for mpv to exit
            eprintln!("Waiting for mpv to exit...");
            loop {
                let msg = recv.recv().unwrap();
                match msg {
                    ThreadMessage::MpvOut { .. } => eprintln!("skipped mpv out..."),
                    ThreadMessage::PlaybackStopped => {
                        eprintln!("Okay, playback stopped!");
                        break;
                    }
                }
            }
        }
        self.ansi_term.reset();
        let mut child = Command::new("mpv")
            .arg("--no-video")
            .arg(path)
            .arg(&format!("--volume={}", self.volume))
            .spawn_pty(Some(&pty_process::Size::new(30, 80)))
            .unwrap();
        // Thread sends, host receives
        let (t_send, h_recv) = mpsc::channel();
        // Host sends, thread receives
        let (h_send, t_recv) = mpsc::channel();
        self.from_thread_recv = Some(h_recv);
        self.to_thread_send = Some(h_send);
        std::thread::spawn(move || loop {
            match t_recv.try_recv() {
                Ok(msg) => match msg {
                    HostMessage::Input(s) => {
                        child.pty().write_all(s.as_bytes()).unwrap();
                    }
                    HostMessage::Stop => {
                        child.pty().write_all(b"q").unwrap();
                        child.wait().unwrap();
                        t_send.send(ThreadMessage::PlaybackStopped).unwrap();
                        return;
                    }
                },
                Err(e) => match e {
                    mpsc::TryRecvError::Empty => {}
                    mpsc::TryRecvError::Disconnected => panic!("Disconnected!"),
                },
            }
            let mut buf = [0u8; 256];
            match child.pty().read(&mut buf) {
                Ok(n_read) => {
                    t_send
                        .send(ThreadMessage::MpvOut {
                            buf: Box::new(buf),
                            n_read,
                        })
                        .unwrap();
                }
                Err(e) => {
                    eprintln!("error reading from mpv process: {}", e);
                    // Better terminate playback
                    child.wait().unwrap();
                    t_send.send(ThreadMessage::PlaybackStopped).unwrap();
                    return;
                }
            }
        });
    }
    fn stop_music(&mut self) {
        if let Some(sender) = &mut self.to_thread_send {
            sender.send(HostMessage::Stop).unwrap();
        }
    }
    fn update_child_out(&mut self, buf: &[u8]) {
        self.ansi_term.feed(buf)
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();
        CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Volume");
                ui.add(DragValue::new(&mut self.mpv_handler.volume));
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
                            self.mpv_handler.play_music(&path);
                            self.playing_index = Some(i);
                        }
                    }
                });
            match &mut self.mpv_handler.from_thread_recv {
                None => {}
                Some(recv) => {
                    for ev in &ctx.input().raw.events {
                        if let Event::Text(s) = ev {
                            self.mpv_handler
                                .to_thread_send
                                .as_mut()
                                .unwrap()
                                .send(HostMessage::Input(s.to_owned()))
                                .unwrap();
                        }
                    }

                    match recv.try_recv() {
                        Ok(msg) => match msg {
                            ThreadMessage::MpvOut { buf, n_read } => {
                                self.mpv_handler.update_child_out(&buf[..n_read]);
                            }
                            ThreadMessage::PlaybackStopped => {
                                eprintln!("Playback stopped!");
                                self.mpv_handler.to_thread_send = None;
                                self.mpv_handler.from_thread_recv = None;
                            }
                        },
                        Err(e) => match e {
                            mpsc::TryRecvError::Empty => {}
                            mpsc::TryRecvError::Disconnected => {
                                eprintln!("Disconnected!");
                            }
                        },
                    }
                    if ui.button("stop").clicked() {
                        self.mpv_handler.stop_music();
                    }
                }
            }
            ui.separator();
            ScrollArea::vertical()
                .id_source("out_scroll")
                .stick_to_bottom()
                .show(ui, |ui| {
                    ui.label(self.mpv_handler.ansi_term.contents_to_string());
                });
        });
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
            from_thread_recv: None,
            to_thread_send: None,
            ansi_term: AnsiTerm::new(80),
            volume: 50,
        }
    }
}

impl App {
    fn new(_cc: &CreationContext<'_>) -> Self {
        let mut this = App {
            cfg: Config::load_or_default(),
            song_paths: Vec::new(),
            playing_index: None,
            mpv_handler: MpvHandler::default(),
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
}

fn main() {
    let native_opts = NativeOptions::default();
    eframe::run_native(
        "mpv-egui-musicplayer",
        native_opts,
        Box::new(|cc| Box::new(App::new(cc))),
    );
}
