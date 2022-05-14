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
    child_out: String,
    from_thread_recv: Option<ThreadRecv>,
    to_thread_send: Option<HostSend>,
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
                                &mut self.from_thread_recv,
                                &mut self.to_thread_send,
                                &mut self.ansi_parser,
                                self.volume,
                            );
                            self.playing_index = Some(i);
                        }
                    }
                });
            match &mut self.from_thread_recv {
                None => {}
                Some(recv) => {
                    for ev in &ctx.input().raw.events {
                        if let Event::Text(s) = ev {
                            self.to_thread_send
                                .as_mut()
                                .unwrap()
                                .send(HostMessage::Input(s.to_owned()))
                                .unwrap();
                        }
                    }

                    match recv.try_recv() {
                        Ok(msg) => match msg {
                            ThreadMessage::MpvOut { buf, n_read } => {
                                Self::update_child_out(
                                    &mut self.ansi_parser,
                                    &mut self.child_out,
                                    &buf[..n_read],
                                );
                            }
                            ThreadMessage::PlaybackStopped => {
                                eprintln!("Playback stopped!");
                                self.to_thread_send = None;
                                self.from_thread_recv = None;
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
                        Self::stop_music(&mut self.to_thread_send);
                    }
                }
            }
            ui.separator();
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
            child_out: String::new(),
            volume: 50,
            ansi_parser: Default::default(),
            cfg: Config::load_or_default(),
            song_paths: Vec::new(),
            playing_index: None,
            from_thread_recv: None,
            to_thread_send: None,
        };
        this.read_songs();
        this
    }
    fn play_music(
        path: &Path,
        child_out: &mut String,
        from_thread_recv: &mut Option<ThreadRecv>,
        to_thread_send: &mut Option<HostSend>,
        ansi_parser: &mut AnsiParser,
        volume: u8,
    ) {
        Self::stop_music(to_thread_send);
        if let Some(recv) = from_thread_recv {
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
        // Reset ansi parser and clear output
        *ansi_parser = AnsiParser::default();
        child_out.clear();
        let mut child = Command::new("mpv")
            .arg("--no-video")
            .arg(path)
            .arg(&format!("--volume={}", volume))
            .spawn_pty(Some(&pty_process::Size::new(30, 80)))
            .unwrap();
        // Thread sends, host receives
        let (t_send, h_recv) = mpsc::channel();
        // Host sends, thread receives
        let (h_send, t_recv) = mpsc::channel();
        *from_thread_recv = Some(h_recv);
        *to_thread_send = Some(h_send);
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
            let n_read = child.pty().read(&mut buf).unwrap();
            t_send
                .send(ThreadMessage::MpvOut {
                    buf: Box::new(buf),
                    n_read,
                })
                .unwrap();
        });
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
        self.sort_songs();
    }
    fn stop_music(to_thread_send: &mut Option<HostSend>) {
        if let Some(sender) = to_thread_send {
            sender.send(HostMessage::Stop).unwrap();
        }
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
