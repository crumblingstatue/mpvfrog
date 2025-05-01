#![doc(html_no_source)]
#![forbid(unsafe_code)]
#![warn(
    unused_qualifications,
    clippy::redundant_closure_for_method_calls,
    clippy::manual_let_else,
    clippy::ptr_as_ptr,
    clippy::use_self
)]

use existing_instance::Msg;

mod app;
mod bool_ext;
mod config;
mod ipc;
mod mpv_handler;
mod rect_math;
mod runner;

const APP_LABEL: &str = "🐸 mpvfrog";

/// Entry point
fn main() {
    let listener = match existing_instance::establish_endpoint("mpvfrog-instance", true) {
        Ok(endpoint) => match endpoint {
            existing_instance::Endpoint::New(listener) => Some(listener),
            existing_instance::Endpoint::Existing(mut stream) => {
                stream.send(Msg::Nudge);
                return;
            }
        },
        Err(e) => {
            eprintln!("Failed to establish IPC endpoint: {e}\nContinuing.");
            None
        }
    };
    runner::run(700, 500, "mpvfrog", listener);
}
