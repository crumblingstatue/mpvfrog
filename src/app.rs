mod core;
mod playlist_behavior;
pub mod tray;
mod ui;

pub use playlist_behavior::PlaylistBehavior;
use {
    self::{core::Core, tray::AppTray},
    crate::{config::Config, mpv_handler::MpvHandler},
    egui_sfml::egui::{self, Context, Event, Key},
    std::sync::Mutex,
};

pub static LOG: Mutex<String> = Mutex::new(String::new());

#[macro_export]
macro_rules! logln {
    ($($arg:tt)*) => {{
        use std::fmt::Write;
        let mut log = $crate::app::LOG.lock().unwrap();
        writeln!(log, $($arg)*).unwrap();
    }}
}

pub struct App {
    core: Core,
    ui: ui::Ui,
    pub tray_handle: ksni::Handle<AppTray>,
}

impl App {
    pub fn new(ctx: &Context) -> Self {
        ctx.set_visuals(egui::Visuals::dark());

        let mut state = Core {
            cfg: Config::load_or_default(),
            playlist: Vec::new(),
            selected_song: 0,
            mpv_handler: MpvHandler::default(),
            playlist_behavior: PlaylistBehavior::Continue,
            user_stopped: true,
            song_change: false,
        };
        state.read_songs();
        App {
            ui: Default::default(),
            core: state,
            tray_handle: AppTray::spawn(),
        }
    }

    pub fn update(&mut self, ctx: &Context, toggle_pause: bool) {
        if toggle_pause {
            self.core.play_or_toggle_pause();
        }
        if !ctx.wants_keyboard_input() {
            self.handle_egui_input(ctx);
        }
        while let Some(event) = self.core.mpv_handler.poll_event() {
            self.core.handle_event(event);
        }
        self.core.mpv_handler.update();
        self.core.handle_mpv_not_active();
        // Do the ui
        self.ui.update(&mut self.core, ctx);
    }

    /// Update when in the background (window not open)
    pub fn bg_update(&mut self, toggle_pause: bool) {
        if toggle_pause {
            self.core.play_or_toggle_pause();
        }
        self.core.mpv_handler.update();
        self.core.handle_mpv_not_active();
    }

    pub fn save(&mut self) {
        let vec = serde_json::to_vec_pretty(&self.core.cfg).unwrap();
        std::fs::write(Config::path(), vec).unwrap();
    }

    fn handle_egui_input(&mut self, ctx: &Context) {
        ctx.input(|input| {
            if input.key_pressed(Key::Space) && !self.core.mpv_handler.active() {
                self.core.play_selected_song();
                return;
            }
            for ev in &input.raw.events {
                match ev {
                    Event::Text(s) => match s.as_str() {
                        " " => self.core.play_or_toggle_pause(),
                        "<" => self.core.play_prev(),
                        ">" => self.core.play_next(),
                        s => {
                            self.core.mpv_handler.input(s);
                        }
                    },
                    Event::Key {
                        key,
                        pressed: true,
                        modifiers: _,
                        repeat: _,
                    } => match key {
                        Key::ArrowUp => self.core.mpv_handler.input("\x1b[A"),
                        Key::ArrowDown => self.core.mpv_handler.input("\x1b[B"),
                        Key::ArrowRight => self.core.mpv_handler.input("\x1b[C"),
                        Key::ArrowLeft => self.core.mpv_handler.input("\x1b[D"),
                        Key::Backspace => {
                            self.core.cfg.speed = 1.0;
                            self.core.mpv_handler.input("\x08");
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

    fn currently_playing_name(&self) -> Option<&str> {
        self.core
            .playlist
            .get(self.core.selected_song)?
            .file_name()
            .and_then(|name| name.to_str())
    }

    pub(crate) fn write_more_info(&self, buf: &mut String) {
        buf.clear();
        if let Some(currently_playing) = self.currently_playing_name() {
            buf.push_str(currently_playing);
            buf.push('\n');
        }
        if let Some(last) = self.core.mpv_handler.mpv_output().lines().last() {
            buf.push_str(last);
        }
    }
}
