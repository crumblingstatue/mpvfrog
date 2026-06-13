#![doc(html_no_source)]
#![forbid(unsafe_code)]
#![warn(
    unused_qualifications,
    clippy::redundant_closure_for_method_calls,
    clippy::manual_let_else,
    clippy::ptr_as_ptr,
    clippy::use_self,
    clippy::ref_option,
    clippy::needless_pass_by_ref_mut
)]
// Annoying lints
#![allow(clippy::collapsible_if)]

use {clap::Parser, existing_instance::Msg, std::path::PathBuf};

mod app;
mod config;
mod ipc;
mod mpv_handler;
mod rect_math;
mod runner;
mod time_fmt;
mod util {
    pub mod egui_ext;
    pub mod result_ext;
    pub mod str_ext;
}

const APP_LABEL: &str = "🐸 mpvfrog";

#[derive(clap::Parser)]
struct Args {
    /// Path to file or directory
    ///
    /// - If it's a directory, mpvfrog will set the music directory to it
    ///
    /// - If it's a file, mpvfrog will set music directory to parent, and play the file
    path: Option<PathBuf>,
}

/// Entry point
fn main() {
    let mut args = Args::parse();
    if let Some(path) = &mut args.path {
        // Canonicalize the path argument, so we can get the parent even for relative paths
        match path.canonicalize() {
            Ok(canon) => {
                *path = canon;
            }
            Err(e) => {
                eprintln!("Failed to canonicalize path {path:?}: {e}");
            }
        }
    }
    let listener = match existing_instance::establish_endpoint("mpvfrog-instance", true) {
        Ok(endpoint) => match endpoint {
            existing_instance::Endpoint::New(listener) => Some(listener),
            existing_instance::Endpoint::Existing(mut stream) => {
                match &args.path {
                    Some(path) => {
                        stream.send(Msg::String(path.as_os_str().to_str().unwrap().to_owned()));
                    }
                    None => {
                        stream.send(Msg::Nudge);
                    }
                }
                return;
            }
        },
        Err(e) => {
            eprintln!("Failed to establish IPC endpoint: {e}\nContinuing.");
            None
        }
    };
    runner::run(700, 500, "mpvfrog", listener, args);
}
