use ansi_term_buf::Term;
use nonblock::NonBlockingReader;
use pty_process::blocking::{Command as PtyCommand, Pty};
use std::{
    ffi::{OsStr, OsString},
    io::Write as _,
    os::unix::prelude::AsRawFd,
    process::{Child, Command, Stdio},
};

use crate::config::ArgType;

pub struct MpvHandler {
    ansi_term: Term,
    child: Option<Child>,
    pty: Option<Pty>,
    paused: bool,
}

pub struct CustomDemuxer {
    cmd: String,
    args: Vec<OsString>,
}

struct RawFdWrap {
    fd: std::os::unix::io::RawFd,
}

impl AsRawFd for RawFdWrap {
    fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
        self.fd
    }
}

impl MpvHandler {
    pub fn play_music<'a>(
        &mut self,
        mpv_cmd: &str,
        mpv_args: impl IntoIterator<Item = &'a OsStr>,
        custom_demuxer: Option<CustomDemuxer>,
    ) {
        self.stop_music();
        self.ansi_term.reset();
        let pty = Pty::new().unwrap();
        let pts = pty.pts().unwrap();
        self.pty = Some(pty);
        let mut mpv_command = PtyCommand::new(mpv_cmd);
        mpv_command.args(mpv_args);
        if let Some(demuxer) = custom_demuxer {
            eprintln!("Demuxer: {}, args: {:?}", demuxer.cmd, demuxer.args);
            let mut demux_child = Command::new(demuxer.cmd)
                .args(demuxer.args)
                .stdout(Stdio::piped())
                .stdin(Stdio::null())
                .spawn()
                .unwrap();
            mpv_command.stdin(demux_child.stdout.take().unwrap());
        }
        let child = mpv_command.spawn(&pts).unwrap();
        self.child = Some(child);
    }
    pub fn stop_music(&mut self) {
        let Some(child) = &mut self.child else { return };
        self.pty.as_mut().unwrap().write_all(b"q").unwrap();
        child.wait().unwrap();
        self.child = None;
    }
    fn update_child_out(&mut self, buf: &[u8]) {
        self.ansi_term.feed(buf)
    }
    pub fn update(&mut self) {
        let Some(pty) = &mut self.pty else { return; };
        let mut buf = Vec::new();
        let mut nbr = NonBlockingReader::from_fd(pty).unwrap();
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
        let Some(pty) = &mut self.pty else { return };
        pty.write_all(s.as_bytes()).unwrap();
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
            ansi_term: Term::new(100),
            child: None,
            paused: false,
            pty: None,
        }
    }
}
impl CustomDemuxer {
    pub(crate) fn from_config_cmd(reader_cmd: &crate::config::Command, song_path: &OsStr) -> Self {
        Self {
            cmd: reader_cmd.name.clone(),
            args: reader_cmd
                .args
                .iter()
                .map(|arg| config_cmd_arg_to_os_string(arg, song_path))
                .collect(),
        }
    }
}

fn config_cmd_arg_to_os_string(arg: &crate::config::ArgType, song_path: &OsStr) -> OsString {
    match arg {
        ArgType::Custom(string) => string.into(),
        ArgType::SongPath => song_path.to_owned(),
    }
}
