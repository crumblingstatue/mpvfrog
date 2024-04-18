use {
    crate::{
        app::LOG,
        config::ArgType,
        ipc::{self, IpcEvent},
        logln, warn_dialog,
    },
    ansi_term_buf::Term,
    anyhow::Context,
    nonblock::NonBlockingReader,
    pty_process::blocking::{Command as PtyCommand, Pty},
    std::{
        ffi::{OsStr, OsString},
        io::Write as _,
        process::{Child, Stdio},
    },
};

struct MpvHandlerInner {
    child: Child,
    pty: Pty,
    demuxer_pty: Pty,
    ipc_bridge: ipc::Bridge,
}

pub struct MpvHandler {
    mpv_term: Term,
    pub demux_term: Term,
    inner: Option<MpvHandlerInner>,
    read_demuxer: bool,
}

pub struct CustomDemuxer {
    cmd: String,
    args: Vec<OsString>,
}

impl MpvHandler {
    pub fn play_music<'a>(
        &mut self,
        mpv_cmd: &str,
        mpv_args: impl IntoIterator<Item = &'a OsStr>,
        custom_demuxer: Option<CustomDemuxer>,
    ) -> anyhow::Result<()> {
        LOG.lock().unwrap().clear();
        self.read_demuxer = true;
        self.stop_music();
        self.mpv_term.reset();
        self.demux_term.reset();
        let pty = Pty::new().unwrap();
        let pts = pty.pts().unwrap();
        let mut mpv_command = PtyCommand::new(mpv_cmd);
        let demuxer_pty = Pty::new().unwrap();
        let demux_pts = demuxer_pty.pts().unwrap();
        mpv_command.args(mpv_args);
        if let Some(demuxer) = custom_demuxer {
            logln!("Demuxer: {}, args: {:?}", demuxer.cmd, demuxer.args);
            let mut demux_child = PtyCommand::new(demuxer.cmd)
                .args(demuxer.args)
                .stdout(Stdio::piped())
                .spawn(&demux_pts)
                .context("Failed to spawn demuxer")?;
            mpv_command.stdin(demux_child.stdout.take().unwrap());
        }
        let child = mpv_command.spawn(&pts).unwrap();
        // Wait for socket (todo: Find better solution)
        std::thread::sleep(std::time::Duration::from_millis(100));
        let ipc_bridge =
            ipc::Bridge::connect().context("Failed to establish connection with mpv")?;
        self.inner = Some(MpvHandlerInner {
            child,
            pty,
            demuxer_pty,
            ipc_bridge,
        });
        Ok(())
    }
    pub fn stop_music(&mut self) {
        let Some(inner) = &mut self.inner else { return };
        inner.pty.write_all(b"q").unwrap();
        inner.child.wait().unwrap();
        self.inner = None;
    }
    pub fn update(&mut self) {
        let Some(inner) = &mut self.inner else {
            return;
        };
        if let Err(e) = inner.ipc_bridge.handle_responses() {
            warn_dialog("IPC error", &format!("Mpv IPC error: {e}"));
        }
        let mut buf = Vec::new();
        let mut nbr = NonBlockingReader::from_fd(&mut inner.pty).unwrap();
        let mut demux_nbr = NonBlockingReader::from_fd(&mut inner.demuxer_pty).unwrap();
        match nbr.read_available(&mut buf) {
            Ok(n_read) => {
                if n_read != 0 {
                    self.mpv_term.feed(&buf);
                }
            }
            Err(e) => {
                logln!("error reading from mpv process: {}", e);
                // Better terminate playback
                self.stop_music();
                return;
            }
        }
        if self.read_demuxer {
            match demux_nbr.read_available(&mut buf) {
                Ok(n_read) => {
                    if n_read != 0 {
                        self.demux_term.feed(&buf);
                    }
                }
                Err(e) => {
                    logln!("Demuxer pty read error: {}", e);
                    self.read_demuxer = false;
                }
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
        self.mpv_term.contents_to_string()
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

    pub(crate) fn set_video(&mut self, show: bool) {
        if let Some(inner) = &mut self.inner {
            inner.ipc_bridge.set_video(show);
        }
    }

    pub(crate) fn poll_event(&mut self) -> Option<IpcEvent> {
        match &mut self.inner {
            Some(inner) => inner.ipc_bridge.event_queue.pop_front(),
            None => None,
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
            mpv_term: Term::new(100),
            demux_term: Term::new(100),
            inner: None,
            read_demuxer: true,
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
