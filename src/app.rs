mod core;
mod playlist_behavior;
mod ui;

use crate::{config::Config, mpv_handler::MpvHandler};

use eframe::{
    egui::{self, Context, Event, Key},
    CreationContext,
};

use self::core::Core;
pub use playlist_behavior::PlaylistBehavior;

pub struct App {
    core: Core,
    ui: ui::Ui,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if !ctx.wants_keyboard_input() {
            self.handle_egui_input(ctx);
        }
        // We need to constantly update in order to keep reading from mpv
        ctx.request_repaint();
        self.core.mpv_handler.update();
        self.core.handle_mpv_not_active();
        // Do the ui
        self.ui.update(&mut self.core, ctx);
    }
    fn on_exit_event(&mut self) -> bool {
        let vec = serde_json::to_vec_pretty(&self.core.cfg).unwrap();
        std::fs::write(Config::path(), &vec).unwrap();
        true
    }
}

impl App {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

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
        }
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
