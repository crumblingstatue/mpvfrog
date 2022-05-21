#![feature(let_else)]

mod app;
mod config;
mod mpv_handler;

use app::App;
use eframe::{emath::vec2, NativeOptions};

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
