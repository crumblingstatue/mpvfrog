//! Handling of the spawned mpv process

use {
    crate::{
        app::ModalPopup,
        config::ArgType,
        ipc::{self, IpcEvent},
        logln,
        util::str_ext::trim_lines,
    },
    ansi_term_buf::Term,
    anyhow::Context,
    nonblock::NonBlockingReader,
    pty_process::blocking::{Command as PtyCommand, Pty},
    std::{
        ffi::{OsStr, OsString},
        io::{Read as _, Write as _},
        process::{Child, Stdio},
        sync::Arc,
        time::Duration,
    },
};

struct MpvHandlerInner {
    child: Child,
    demux: Option<DemuxState>,
    mpv_pty: Pty,
    ipc_bridge: ipc::Bridge,
}

const DEMUX_READ_CAPACITY: usize = 4096;

enum DemuxMsg {
    PtyRead {
        buf: [u8; DEMUX_READ_CAPACITY],
        len: usize,
    },
}

struct DemuxState {
    child: Child,
    recv: std::sync::mpsc::Receiver<DemuxMsg>,
    pty: Arc<Pty>,
}

pub struct MpvHandler {
    pub mpv_term: Term,
    pub demux_term: Term,
    /// Name of the demuxer command. This should be synchronized with the term, so the tab
    /// shows the same name as the command that produced the term output.
    pub demux_cmd_name: String,
    inner: Option<MpvHandlerInner>,
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
        self.stop_music();
        self.mpv_term.reset();
        self.demux_term.reset();
        self.demux_cmd_name.clear();
        let (mut pty, pts) = pty_process::blocking::open()?;
        let mut mpv_command = PtyCommand::new(mpv_cmd);
        let (demuxer_pty, demux_pts) = pty_process::blocking::open()?;
        let demuxer_pty = Arc::new(demuxer_pty);
        mpv_command = mpv_command.args(mpv_args);
        let mut demux = None;
        if let Some(demuxer) = custom_demuxer {
            logln!("Demuxer: {}, args: {:?}", demuxer.cmd, demuxer.args);
            self.demux_cmd_name = demuxer.cmd.clone();
            let mut demux_child = PtyCommand::new(demuxer.cmd)
                .args(demuxer.args)
                .stdout(Stdio::piped())
                .spawn(demux_pts)
                .context("Failed to spawn demuxer")?;
            mpv_command = mpv_command.stdin(demux_child.stdout.take().unwrap());
            let (msg_send, msg_recv) = std::sync::mpsc::channel();
            let demuxer_pty_clone = demuxer_pty.clone();
            std::thread::spawn(move || {
                let mut demux_buf = [0u8; DEMUX_READ_CAPACITY];
                loop {
                    match (&*demuxer_pty_clone).read(&mut demux_buf) {
                        Ok(len) => {
                            msg_send
                                .send(DemuxMsg::PtyRead {
                                    buf: demux_buf,
                                    len,
                                })
                                .unwrap();
                        }
                        Err(e) => {
                            logln!("Demuxer pty read error: {}", e);
                            return;
                        }
                    }
                }
            });
            demux = Some(DemuxState {
                child: demux_child,
                recv: msg_recv,
                pty: demuxer_pty,
            });
        }
        let mut child = mpv_command.spawn(pts)?;
        let attempts = 5;
        let ipc_bridge = 'connect: {
            for i in 0..attempts {
                std::thread::sleep(Duration::from_millis(100));
                match ipc::Bridge::connect() {
                    Ok(bridge) => break 'connect bridge,
                    Err(e) => {
                        if let Some(status) = child.try_wait()? {
                            let mut stderr = Vec::new();
                            let result = pty.read_to_end(&mut stderr);
                            if let Err(e) = result {
                                logln!("Failed to read mpv pty: {e}");
                            }
                            let mut term = Term::new(80);
                            term.feed(&stderr);
                            let stderr = trim_lines(term.contents_to_string());
                            anyhow::bail!("mpv exited with {status}.\nStderr:\n{stderr}");
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
            demux,
            ipc_bridge,
        });
        Ok(())
    }
    pub fn stop_music(&mut self) {
        let Some(inner) = &mut self.inner else { return };
        inner.mpv_pty.write_all(b"q").unwrap();
        inner.child.wait().unwrap();
        'wait_demuxer: {
            if let Some(mut demux) = inner.demux.take() {
                for i in 0..5 {
                    logln!("Wait for demuxer to exit (attempt {i})");
                    if let Some(status) = demux.child.try_wait().unwrap() {
                        logln!("Demuxer exited with status: {status}");
                        break 'wait_demuxer;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                demux.child.kill().unwrap();
                logln!("Killed demuxer");
            }
        }
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
        let mut nbr = NonBlockingReader::from_fd(&inner.mpv_pty).unwrap();
        match inner.child.try_wait() {
            Ok(None) => {}
            Ok(Some(status)) => {
                if !status.success() {
                    let mut remaining_data = Vec::new();
                    let result = nbr.read_available(&mut remaining_data);
                    if let Err(e) = result {
                        logln!("Failed to read mpv pty: {e}");
                    }
                    self.mpv_term.feed(&remaining_data);
                    let stderr = trim_lines(self.mpv_term.contents_to_string());
                    modal.error(
                        "Abnormal mpv termination",
                        format!("Mpv exited with status {status}\nStderr:\n{stderr}"),
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
        if let Some(demux) = &inner.demux {
            loop {
                match demux.recv.try_recv() {
                    Ok(msg) => match msg {
                        DemuxMsg::PtyRead { buf, len } => {
                            self.demux_term.feed(&buf[..len]);
                        }
                    },
                    Err(e) => match e {
                        std::sync::mpsc::TryRecvError::Empty => break,
                        std::sync::mpsc::TryRecvError::Disconnected => {
                            eprintln!("Chanel disconnected!");
                            break;
                        }
                    },
                }
            }
        }
    }

    pub fn send_input(&mut self, s: &str) {
        let Some(inner) = &mut self.inner else { return };
        let mut pty = match self.active_pty_input {
            ActivePtyInput::Mpv => &inner.mpv_pty,
            ActivePtyInput::Demuxer => match &inner.demux {
                Some(demux) => &demux.pty,
                None => {
                    logln!("Trying to write to demux while no demux");
                    return;
                }
            },
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
        trim_lines(self.mpv_term.contents_to_string())
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
        self.inner
            .as_ref()
            .is_some_and(|inner| inner.demux.is_some())
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
            mpv_term: Term::new(80),
            demux_term: Term::new(80),
            demux_cmd_name: String::new(),
            inner: None,
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
