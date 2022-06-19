#![feature(let_else, associated_type_defaults)]

mod app;
mod bool_ext;
mod config;
mod ipc;
mod mpv_handler;
mod runner;

fn main() {
    runner::run(700, 500, "mpv-egui-musicplayer");
}
