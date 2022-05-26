mod core;
mod playlist_behavior;
mod ui;

use crate::{config::Config, mpv_handler::MpvHandler};

use egui_sfml::egui::{self, Context, Event, Key};
use ksni::{menu::StandardItem, Tray, TrayService};

use self::core::Core;
pub use playlist_behavior::PlaylistBehavior;

pub struct App {
    core: Core,
    ui: ui::Ui,
    pub tray_handle: ksni::Handle<AppTray>,
}

#[derive(Default)]
pub struct AppTray {
    pub should_toggle_window: bool,
    pub should_quit: bool,
    pub paused: bool,
    pub should_pause_resume: bool,
    pub more_info_label: String,
}

impl Tray for AppTray {
    fn activate(&mut self, _x: i32, _y: i32) {
        self.should_toggle_window = true;
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            StandardItem {
                label: if self.paused { "▶" } else { " ⏸" }.into(),
                activate: Box::new(|this: &mut Self| {
                    this.should_pause_resume = true;
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|this: &mut Self| {
                    this.should_quit = true;
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![ksni::Icon {
            width: 32,
            height: 32,
            data: include_bytes!("../icon.argb32").to_vec(),
        }]
    }
    fn tool_tip(&self) -> ksni::ToolTip {
        let title = if !self.more_info_label.is_empty() {
            format!("mpv-egui\n{}", self.more_info_label)
        } else {
            "mpv-egui".into()
        };
        ksni::ToolTip {
            title,
            ..Default::default()
        }
    }
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
        let tray_service = TrayService::new(AppTray::default());
        let tray_handle = tray_service.handle();
        tray_service.spawn();
        App {
            ui: Default::default(),
            core: state,
            tray_handle,
        }
    }

    pub fn update(&mut self, ctx: &Context, toggle_pause: bool) {
        if toggle_pause {
            self.core.play_or_toggle_pause();
        }
        if !ctx.wants_keyboard_input() {
            self.handle_egui_input(ctx);
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
                    " " => self.core.play_or_toggle_pause(),
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

    pub(crate) fn paused_or_stopped(&self) -> bool {
        !self.core.mpv_handler.active() || self.core.mpv_handler.paused()
    }

    fn currently_playing_name(&self) -> Option<&str> {
        self.core.playlist[self.core.selected_song]
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
