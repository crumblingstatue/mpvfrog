use ansi_term_buf::Term;
use nonblock::NonBlockingReader;
use pty_process::blocking::{Command as PtyCommand, Pty};
use std::{
    ffi::{OsStr, OsString},
    io::Write as _,
    os::unix::prelude::AsRawFd,
    process::{Child, Command, Stdio},
};

use crate::{config::ArgType, ipc};

struct MpvHandlerInner {
    child: Child,
    pty: Pty,
    ipc_bridge: ipc::Bridge,
}

pub struct MpvHandler {
    ansi_term: Term,
    inner: Option<MpvHandlerInner>,
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
        // Wait for socket (todo: Find better solution)
        std::thread::sleep(std::time::Duration::from_millis(100));
        self.inner = Some(MpvHandlerInner {
            child,
            pty,
            ipc_bridge: ipc::Bridge::connect(),
        });
    }
    pub fn stop_music(&mut self) {
        let Some(inner) = &mut self.inner else { return };
        inner.pty.write_all(b"q").unwrap();
        inner.child.wait().unwrap();
        self.inner = None;
    }
    fn update_child_out(&mut self, buf: &[u8]) {
        self.ansi_term.feed(buf)
    }
    pub fn update(&mut self) {
        let Some(inner) = &mut self.inner else { return; };
        inner.ipc_bridge.handle_responses();
        let mut buf = Vec::new();
        let mut nbr = NonBlockingReader::from_fd(&mut inner.pty).unwrap();
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
        let Some(inner) = &mut self.inner else { return };
        inner.pty.write_all(s.as_bytes()).unwrap();
    }

    pub fn active(&self) -> bool {
        self.inner.is_some()
    }

    pub fn toggle_pause(&mut self) {
        if let Some(inner) = &mut self.inner {
            inner.ipc_bridge.toggle_pause();
        }
    }

    pub fn paused(&self) -> bool {
        match &self.inner {
            Some(inner) => inner.ipc_bridge.observed.paused,
            None => true,
        }
    }
    pub fn mpv_output(&self) -> String {
        self.ansi_term.contents_to_string()
    }
    pub fn volume(&self) -> Option<u8> {
        self.inner
            .as_ref()
            .map(|inner| inner.ipc_bridge.observed.volume)
    }
    pub fn speed(&self) -> Option<f64> {
        self.inner
            .as_ref()
            .map(|inner| inner.ipc_bridge.observed.speed)
    }
    pub fn set_volume(&mut self, vol: u8) {
        if let Some(inner) = &mut self.inner {
            inner.ipc_bridge.set_volume(vol);
        }
    }
    pub fn set_speed(&mut self, speed: f64) {
        if let Some(inner) = &mut self.inner {
            inner.ipc_bridge.set_speed(speed);
        }
    }

    pub(crate) fn time_info(&self) -> Option<TimeInfo> {
        self.inner.as_ref().map(|inner| TimeInfo {
            pos: inner.ipc_bridge.observed.time_pos,
            duration: inner.ipc_bridge.observed.duration,
        })
    }

    pub(crate) fn seek(&mut self, pos: f64) {
        if let Some(inner) = &mut self.inner {
            inner.ipc_bridge.seek(pos);
        }
    }
}

pub struct TimeInfo {
    pub pos: f64,
    pub duration: f64,
}

impl Default for MpvHandler {
    fn default() -> Self {
        Self {
            ansi_term: Term::new(100),
            inner: None,
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
