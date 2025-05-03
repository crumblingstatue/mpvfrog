//! Application state management

mod core;
mod playlist_behavior;
pub mod tray;
pub mod ui;

pub use playlist_behavior::PlaylistBehavior;
use {
    self::{
        core::Core,
        tray::{AppToTrayMsg, AppTray},
    },
    crate::{
        config::Config,
        mpv_handler::{ActivePtyInput, MpvHandler},
    },
    egui_sf2g::egui::{self, Context, Event, Key},
    std::{fmt::Display, sync::Mutex, time::Instant},
    zbus::names::BusName,
};

pub static LOG: Mutex<String> = Mutex::new(String::new());

/// Log a line to the application logger. Uses [`std::fmt`] syntax.
#[macro_export]
macro_rules! logln {
    ($($arg:tt)*) => {{
        use std::fmt::Write;
        let mut log = $crate::app::LOG.lock().unwrap();
        writeln!(log, $($arg)*).unwrap();
    }}
}

pub struct App {
    pub core: Core,
    pub ui: ui::Ui,
    pub tray_handle: Option<AppTray>,
    last_tooltip_update: Instant,
    pub modal: ModalPopup,
}

#[derive(Default)]
pub struct ModalPopup {
    payload: Option<ModalPayload>,
}

struct ModalPayload {
    title: String,
    msg: String,
    kind: ModalPayloadKind,
}

enum ModalPayloadKind {
    Warning,
    Error,
}

impl ModalPopup {
    pub fn warn(&mut self, title: &str, msg: impl Display) {
        self.payload = Some(ModalPayload {
            title: title.into(),
            msg: msg.to_string(),
            kind: ModalPayloadKind::Warning,
        });
    }
    pub fn error(&mut self, title: &str, msg: impl Display) {
        self.payload = Some(ModalPayload {
            title: title.into(),
            msg: msg.to_string(),
            kind: ModalPayloadKind::Error,
        });
    }
}

pub trait ResultModalExt {
    fn err_popup(&self, title: &str, modal: &mut ModalPopup);
}

impl<T, E: Display> ResultModalExt for Result<T, E> {
    fn err_popup(&self, title: &str, modal: &mut ModalPopup) {
        if let Err(e) = self {
            modal.error(title, e);
        }
    }
}

impl App {
    pub fn new(ctx: &Context, args: &crate::Args) -> Self {
        ctx.set_visuals(egui::Visuals::dark());
        let cfg = Config::load_or_default();
        let mut core = Core {
            cfg,
            playlist: Vec::new(),
            selected_song: 0,
            mpv_handler: MpvHandler::default(),
            playlist_behavior: PlaylistBehavior::Continue,
            user_stopped: true,
            song_change: false,
        };
        // Handle path argument for opening a folder (and optionally play a file)
        let mut play_this = None;
        if let Some(path) = &args.path {
            if path.is_dir() {
                core.cfg.music_folder = Some(path.clone());
            } else if path.is_file() {
                if let Some(parent) = path.parent() {
                    core.cfg.music_folder = Some(parent.to_owned());
                    play_this = Some(path.strip_prefix(parent).unwrap());
                }
            }
        }
        core.read_songs();
        let mut ui: ui::Ui = Default::default();
        ui.recalc_filt_entries(&core);
        ui.apply_colorix_theme(core.cfg.theme.as_ref(), ctx);
        let tray_handle = match AppTray::establish() {
            Ok(handle) => Some(handle),
            Err(e) => {
                eprintln!("Failed to establish tray connection: {e}");
                None
            }
        };
        let mut app = Self {
            ui,
            core,
            tray_handle,
            last_tooltip_update: Instant::now(),
            modal: ModalPopup::default(),
        };
        if let Some(this) = play_this {
            if let Some(pos) = app
                .core
                .playlist
                .iter()
                .position(|play_path| play_path == this)
            {
                app.focus_and_play(pos);
            }
        }
        app
    }

    pub fn update(&mut self, ctx: &Context) {
        if !ctx.wants_keyboard_input() {
            self.handle_egui_input(ctx);
        }
        self.handle_mpv_events();
        self.core.mpv_handler.update(&mut self.modal);
        self.core.handle_mpv_not_active(&mut self.modal);
        // Do the ui
        self.ui.update(&mut self.core, ctx, &mut self.modal);
    }

    fn handle_mpv_events(&mut self) {
        while let Some(event) = self.core.mpv_handler.poll_event() {
            self.core.handle_event(event);
        }
    }

    /// Update when in the background (window not open)
    pub fn bg_update(&mut self) {
        self.handle_mpv_events();
        self.core.mpv_handler.update(&mut self.modal);
        self.core.handle_mpv_not_active(&mut self.modal);
    }

    /// Update when tray popup is open
    pub fn tray_popup_update(&mut self, ctx: &Context) {
        if !ctx.wants_keyboard_input() {
            self.handle_egui_input(ctx);
        }
        self.bg_update();
    }

    pub fn save(&self) {
        let vec = serde_json::to_vec_pretty(&self.core.cfg).unwrap();
        std::fs::write(Config::path(), vec).unwrap();
    }

    fn handle_egui_input(&mut self, ctx: &Context) {
        ctx.input(|input| {
            if input.key_pressed(Key::Space) && !self.core.mpv_handler.active() {
                self.core.play_selected_song(&mut self.modal);
                return;
            }
            for ev in &input.raw.events {
                let mpv_active =
                    matches!(self.core.mpv_handler.active_pty_input, ActivePtyInput::Mpv);
                match ev {
                    Event::Text(s) => match s.as_str() {
                        " " if mpv_active => self.core.play_or_toggle_pause(&mut self.modal),
                        "<" if mpv_active => self.core.play_prev(&mut self.modal),
                        ">" if mpv_active => self.core.play_next(&mut self.modal),
                        s => {
                            self.core.mpv_handler.send_input(s);
                        }
                    },
                    Event::Key {
                        key,
                        pressed: true,
                        modifiers: _,
                        repeat: _,
                        physical_key: _,
                    } => match key {
                        Key::ArrowUp => self.core.mpv_handler.send_input("\x1b[A"),
                        Key::ArrowDown => self.core.mpv_handler.send_input("\x1b[B"),
                        Key::ArrowRight => self.core.mpv_handler.send_input("\x1b[C"),
                        Key::ArrowLeft => self.core.mpv_handler.send_input("\x1b[D"),
                        Key::Backspace => {
                            self.core.cfg.speed = 1.0;
                            self.core.mpv_handler.send_input("\x08");
                        }
                        _ => (),
                    },
                    _ => (),
                }
            }
        });
    }

    pub(crate) fn paused_or_stopped(&self) -> bool {
        !self.core.mpv_handler.active() || self.core.mpv_handler.paused()
    }

    pub fn currently_playing_name(&self) -> Option<&str> {
        self.core
            .playlist
            .get(self.core.selected_song)?
            .file_name()
            .and_then(|name| name.to_str())
    }

    pub(crate) fn update_tooltip(&mut self) {
        // Don't spam DBus with updates every frame
        if self.last_tooltip_update.elapsed().as_secs() >= 1 {
            self.last_tooltip_update = Instant::now();
        } else {
            return;
        }
        let mut buf = String::new();
        if let Some(currently_playing) = self.currently_playing_name() {
            buf.push_str(currently_playing);
            buf.push('\n');
        }
        if let Some(last) = self.core.mpv_handler.mpv_output().lines().last() {
            buf.push_str(last);
        }
        if let Some(trhandle) = &self.tray_handle {
            trhandle
                .sender
                .send(AppToTrayMsg::UpdateHoverText(buf))
                .unwrap();
            trhandle
                .conn
                .emit_signal(
                    None::<BusName>,
                    "/StatusNotifierItem",
                    "org.kde.StatusNotifierItem",
                    "NewToolTip",
                    &(),
                )
                .unwrap();
        }
    }

    pub(crate) fn update_volume(&mut self) {
        if let Some(vol) = self.core.mpv_handler.volume() {
            self.core.cfg.volume = vol;
        }
    }

    pub(crate) fn focus_and_play(&mut self, idx: usize) {
        self.core.selected_song = idx;
        self.ui.focus_on = Some(idx);
        self.core.play_selected_song(&mut self.modal);
    }
}
