mod core;
mod playlist_behavior;
mod ui;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::{config::Config, mpv_handler::MpvHandler};

use egui_sfml::egui::{self, Context, Event, Key};
use tray_item::TrayItem;

use self::core::Core;
pub use playlist_behavior::PlaylistBehavior;

pub struct App {
    core: Core,
    ui: ui::Ui,
    _tray_item: TrayItem,
    pub should_quit: Arc<AtomicBool>,
    pub should_toggle_window: Arc<AtomicBool>,
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
        let mut tray_item = TrayItem::new(
            "mpv-egui-musicplayer",
            tray_item::IconSource::Data {
                height: 32,
                width: 32,
                data: include_bytes!("../icon.argb32").to_vec(),
            },
        )
        .unwrap();
        let should_quit = Arc::new(AtomicBool::new(false));
        let should_quit_clone = should_quit.clone();
        let should_toggle_window = Arc::new(AtomicBool::new(false));
        let should_toggle_window_clone = should_toggle_window.clone();
        tray_item
            .add_menu_item("Toggle window", move || {
                should_toggle_window_clone.store(true, Ordering::Relaxed)
            })
            .unwrap();
        tray_item
            .add_menu_item("Quit", move || {
                should_quit_clone.store(true, Ordering::Relaxed)
            })
            .unwrap();
        App {
            ui: Default::default(),
            core: state,
            _tray_item: tray_item,
            should_quit,
            should_toggle_window,
        }
    }

    pub fn update(&mut self, ctx: &Context) {
        if !ctx.wants_keyboard_input() {
            self.handle_egui_input(ctx);
        }
        self.core.mpv_handler.update();
        self.core.handle_mpv_not_active();
        // Do the ui
        self.ui.update(&mut self.core, ctx);
    }

    /// Update when in the background (window not open)
    pub fn bg_update(&mut self) {
        self.core.mpv_handler.update();
        self.core.handle_mpv_not_active();
    }

    pub fn save(&mut self) {
        let vec = serde_json::to_vec_pretty(&self.core.cfg).unwrap();
        std::fs::write(Config::path(), &vec).unwrap();
    }

    fn handle_egui_input(&mut self, ctx: &Context) {
        let input = ctx.input();
        if input.key_pressed(Key::Space) && !self.core.mpv_handler.active() {
            self.core.play_selected_song();
            return;
        }
        for ev in &input.raw.events {
            match ev {
                Event::Text(s) => match s.as_str() {
                    " " => self.core.mpv_handler.toggle_pause(),
                    "<" => self.core.play_prev(),
                    ">" => self.core.play_next(),
                    s => {
                        match s {
                            "9" => {
                                self.core.cfg.volume -= 2;
                            }
                            "0" => {
                                self.core.cfg.volume += 2;
                            }
                            "[" => {
                                self.core.cfg.speed -= 0.1;
                            }
                            "]" => {
                                self.core.cfg.speed += 0.1;
                            }
                            "{" => {
                                self.core.cfg.speed -= 0.01;
                            }
                            "}" => {
                                self.core.cfg.speed += 0.01;
                            }
                            _ => {}
                        }
                        self.core.mpv_handler.input(s);
                    }
                },
                Event::Key {
                    key,
                    pressed: true,
                    modifiers: _,
                } => match *key {
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
    }
}
