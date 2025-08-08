//! Handling of the spawned mpv process

use {
    crate::{
        app::ModalPopup,
        config::ArgType,
        ipc::{self, IpcEvent},
        logln,
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
    mpv_pty: Pty,
    demuxer_pty: Pty,
    ipc_bridge: ipc::Bridge,
}

pub struct MpvHandler {
    pub mpv_term: Term,
    pub demux_term: Term,
    inner: Option<MpvHandlerInner>,
    read_demuxer: bool,
    pub active_pty_input: ActivePtyInput,
}

pub enum ActivePtyInput {
    Mpv,
    Demuxer,
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
        self.read_demuxer = true;
        self.stop_music();
        self.mpv_term.reset();
        self.demux_term.reset();
        let (pty, pts) = pty_process::blocking::open()?;
        let mut mpv_command = PtyCommand::new(mpv_cmd);
        let (demuxer_pty, demux_pts) = pty_process::blocking::open()?;
        mpv_command = mpv_command.args(mpv_args);
        if let Some(demuxer) = custom_demuxer {
            logln!("Demuxer: {}, args: {:?}", demuxer.cmd, demuxer.args);
            let mut demux_child = PtyCommand::new(demuxer.cmd)
                .args(demuxer.args)
                .stdout(Stdio::piped())
                .spawn(demux_pts)
                .context("Failed to spawn demuxer")?;
            mpv_command = mpv_command.stdin(demux_child.stdout.take().unwrap());
        }
        let mut child = mpv_command.spawn(pts)?;
        let attempts = 5;
        let ipc_bridge = 'connect: {
            for i in 0..attempts {
                std::thread::sleep(std::time::Duration::from_millis(100));
                match ipc::Bridge::connect() {
                    Ok(bridge) => break 'connect bridge,
                    Err(e) => {
                        if let Some(status) = child.try_wait()? {
                            anyhow::bail!("mpv exited with {status}");
                        }
                        logln!("mpv connection attempt #{i}: {e}");
                    }
                }
            }
            anyhow::bail!("Failed connect to mpv");
        };
        self.inner = Some(MpvHandlerInner {
            child,
            mpv_pty: pty,
            demuxer_pty,
            ipc_bridge,
        });
        Ok(())
    }
    pub fn stop_music(&mut self) {
        let Some(inner) = &mut self.inner else { return };
        inner.mpv_pty.write_all(b"q").unwrap();
        inner.child.wait().unwrap();
        self.inner = None;
    }
    pub fn update(&mut self, modal: &mut ModalPopup) {
        let Some(inner) = &mut self.inner else {
            return;
        };
        if let Err(e) = inner.ipc_bridge.handle_responses() {
            modal.warn("Mpv IPC error", e);
        }
        let mut buf = Vec::new();
        let mut demux_buf = Vec::new();
        let mut nbr = NonBlockingReader::from_fd(&inner.mpv_pty).unwrap();
        let mut demux_nbr = NonBlockingReader::from_fd(&inner.demuxer_pty).unwrap();
        match inner.child.try_wait() {
            Ok(None) => {}
            Ok(Some(status)) => {
                if !status.success() {
                    modal.error(
                        "Abnormal mpv termination",
                        format!("Mpv exited with status {status}"),
                    );
                }
            }
            Err(e) => {
                modal.error(
                    "Abnormal mpv termination",
                    format!("Error waiting on mpv: {e}"),
                );
            }
        }
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
            match demux_nbr.read_available(&mut demux_buf) {
                Ok(n_read) => {
                    if n_read != 0 {
                        self.demux_term.feed(&demux_buf);
                    }
                }
                Err(e) => {
                    logln!("Demuxer pty read error: {}", e);
                    self.read_demuxer = false;
                }
            }
        }
    }

    pub fn send_input(&mut self, s: &str) {
        let Some(inner) = &mut self.inner else { return };
        let pty = match self.active_pty_input {
            ActivePtyInput::Mpv => &mut inner.mpv_pty,
            ActivePtyInput::Demuxer => &mut inner.demuxer_pty,
        };
        pty.write_all(s.as_bytes()).unwrap();
    }

    pub fn active(&self) -> bool {
        self.inner.is_some()
    }

    pub fn paused(&self) -> bool {
        match &self.inner {
            Some(inner) => inner.ipc_bridge.observed.paused,
            None => true,
        }
    }
    pub fn mpv_output(&self) -> String {
        let contents = self.mpv_term.contents_to_string();
        let mut out = String::new();
        // Replace trailing whitespace of each line with single line terminator
        //
        // This prevents the text viewer wrapping long lines
        for line in contents.lines() {
            out.push_str(line.trim_end());
            out.push('\n');
        }
        out
    }
    pub fn ab_loop(&self) -> Option<(Option<f64>, Option<f64>)> {
        self.inner.as_ref().map(|inner| {
            (
                inner.ipc_bridge.observed.ab_loop_a,
                inner.ipc_bridge.observed.ab_loop_b,
            )
        })
    }

    pub(crate) fn time_info(&self) -> Option<TimeInfo> {
        self.inner.as_ref().map(|inner| TimeInfo {
            pos: inner.ipc_bridge.observed.time_pos,
            duration: inner.ipc_bridge.observed.duration,
        })
    }

    pub(crate) fn poll_event(&mut self) -> Option<IpcEvent> {
        match &mut self.inner {
            Some(inner) => inner.ipc_bridge.event_queue.pop_front(),
            None => None,
        }
    }

    pub(crate) fn demuxer_active(&self) -> bool {
        self.read_demuxer
    }
    /// Send a command to the IPC bridge, if it exists
    pub(crate) fn ipc<'br, T, F>(&'br mut self, fun: F) -> Option<T>
    where
        F: FnOnce(&'br mut ipc::Bridge) -> T,
    {
        match &mut self.inner {
            Some(inner) => Some(fun(&mut inner.ipc_bridge)),
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
            active_pty_input: ActivePtyInput::Mpv,
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

fn config_cmd_arg_to_os_string(arg: &ArgType, song_path: &OsStr) -> OsString {
    match arg {
        ArgType::Custom(string) => string.into(),
        ArgType::SongPath => song_path.to_owned(),
    }
}
