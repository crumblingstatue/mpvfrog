mod app;
mod bool_ext;
mod config;
mod ipc;
mod mpv_handler;
mod rect_math;
mod runner;

fn warn_dialog(title: &str, desc: &str) {
    rfd::MessageDialog::new()
        .set_title(title)
        .set_level(rfd::MessageLevel::Warning)
        .set_description(desc)
        .show();
}

fn main() {
    runner::run(740, 500, "mpv-egui-musicplayer");
}
