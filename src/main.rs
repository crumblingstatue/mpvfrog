#![doc(html_no_source)]
#![warn(
    unused_qualifications,
    clippy::redundant_closure_for_method_calls,
    clippy::manual_let_else,
    clippy::ptr_as_ptr
)]

use std::sync::Mutex;

mod app;
mod bool_ext;
mod config;
mod ipc;
mod mpv_handler;
mod rect_math;
mod runner;

/// Global egui modal dialog handle
static MODAL: Mutex<Option<egui_modal::Modal>> = Mutex::new(None);

/// Display a modal warning popup in the egui ui
fn warn_dialog(title: &str, desc: &str) {
    let Some(modal) = &mut *MODAL.lock().unwrap() else {
        eprintln!("Dialog not yet init. warn: {title}: {desc}");
        return;
    };
    modal
        .dialog()
        .with_title(title)
        .with_icon(egui_modal::Icon::Warning)
        .with_body(desc)
        .open();
}

/// Entry point
fn main() {
    runner::run(740, 500, "mpv-egui-musicplayer");
}
