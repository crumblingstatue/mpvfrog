use ansi_term_buf::Term;
use nonblock::NonBlockingReader;
use pty_process::{std::Child, Command as _};
use std::{ffi::OsStr, io::Write as _, process::Command};

pub struct MpvHandler {
    ansi_term: Term,
    child: Option<Child>,
    paused: bool,
}

impl MpvHandler {
    pub fn play_music<'a>(&mut self, mpv_cmd: &str, args: impl IntoIterator<Item = &'a OsStr>) {
        self.stop_music();
        self.ansi_term.reset();
        let child = Command::new(mpv_cmd)
            .args(args)
            .spawn_pty(Some(&pty_process::Size::new(30, 80)))
            .unwrap();
        self.child = Some(child);
    }
    pub fn stop_music(&mut self) {
        let Some(child) = &mut self.child else { return };
        child.pty().write_all(b"q").unwrap();
        child.wait().unwrap();
        self.child = None;
    }
    fn update_child_out(&mut self, buf: &[u8]) {
        self.ansi_term.feed(buf)
    }
    pub fn update(&mut self) {
        let Some(child) = &mut self.child else { return; };
        let mut buf = Vec::new();
        let mut nbr = NonBlockingReader::from_fd((*child.pty()).try_clone().unwrap()).unwrap();
        match nbr.read_available(&mut buf) {
            Ok(n_read) => {
                if n_read != 0 {
                    self.update_child_out(&buf);
                }
            }
            Err(e) => {
                eprintln!("error reading from mpv process: {}", e);
                // Better terminate playback
                self.stop_music();
            }
        }
    }

    pub fn input(&mut self, s: &str) {
        let Some(child) = &mut self.child else { return };
        child.pty().write_all(s.as_bytes()).unwrap();
    }

    pub fn active(&self) -> bool {
        self.child.is_some()
    }

    pub fn toggle_pause(&mut self) {
        self.input(" ");
        self.paused ^= true;
    }

    pub fn paused(&self) -> bool {
        self.paused
    }
    pub fn mpv_output(&self) -> String {
        self.ansi_term.contents_to_string()
    }
}

impl Default for MpvHandler {
    fn default() -> Self {
        Self {
            ansi_term: Term::new(80),
            child: None,
            paused: false,
        }
    }
}
