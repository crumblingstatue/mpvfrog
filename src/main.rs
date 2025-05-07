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

use {clap::Parser, existing_instance::Msg, std::path::PathBuf};

mod app;
mod bool_ext;
mod config;
mod ipc;
mod mpv_handler;
mod rect_math;
mod runner;

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
    let args = Args::parse();
    let listener = match existing_instance::establish_endpoint("mpvfrog-instance", true) {
        Ok(endpoint) => match endpoint {
            existing_instance::Endpoint::New(listener) => Some(listener),
            existing_instance::Endpoint::Existing(mut stream) => {
                match &args.path {
                    Some(path) => match path.canonicalize() {
                        Ok(canon) => {
                            stream
                                .send(Msg::String(canon.as_os_str().to_str().unwrap().to_owned()));
                        }
                        Err(e) => {
                            eprintln!("Failed to canonicalize path: {e}");
                        }
                    },
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
