#![feature(let_else)]

mod app;
mod bool_ext;
mod config;
mod ipc;
mod mpv_handler;
mod runner;

fn main() {
    runner::run(620, 440, "mpv-egui-musicplayer");
}
