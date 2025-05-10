//! Interprocess comunication with spawned mpv process

mod command;
mod property;

use {
    crate::{logln, util::result_ext::LogErrExt as _},
    command::{AudioAdd, AudioRemove, Command, ObserveProperty, SetProperty},
    interprocess::local_socket::{
        GenericFilePath, Stream as LocalSocketStream, ToFsName, traits::Stream as _,
    },
    property::{PropValue, Property},
    std::{
        collections::{HashMap, VecDeque},
        io::{Read, Write},
        marker::PhantomData,
    },
};

pub enum IpcEvent {
    EndFile,
}

pub struct Bridge {
    ipc_stream: LocalSocketStream,
    pub observed: Properties,
    pub event_queue: VecDeque<IpcEvent>,
}

#[derive(Default)]
pub struct Properties {
    pub paused: bool,
    pub volume: u8,
    pub speed: f64,
    pub duration: f64,
    pub time_pos: f64,
    pub ab_loop_a: Option<f64>,
    pub ab_loop_b: Option<f64>,
    pub track_count: u8,
    pub lavfi_complex: String,
    pub loop_file: bool,
}

impl Bridge {
    pub fn connect() -> anyhow::Result<Self> {
        let ipc_stream = LocalSocketStream::connect(
            "/tmp/mpvfrog.sock".to_fs_name::<GenericFilePath>().unwrap(),
        )?;
        ipc_stream.set_nonblocking(true)?;
        let mut this = Self {
            ipc_stream,
            observed: Default::default(),
            event_queue: Default::default(),
        };
        this.observe_property::<property::Speed>()?;
        this.observe_property::<property::Volume>()?;
        this.observe_property::<property::TimePos>()?;
        this.observe_property::<property::Duration>()?;
        this.observe_property::<property::AbLoopA>()?;
        this.observe_property::<property::AbLoopB>()?;
        this.observe_property::<property::TrackListCount>()?;
        this.observe_property::<property::LavfiComplex>()?;
        this.observe_property::<property::LoopFile>()?;
        Ok(this)
    }
    pub fn observe_property<T: Property>(&mut self) -> anyhow::Result<()> {
        self.write_command(ObserveProperty::<T>(PhantomData))
    }
    pub fn toggle_pause(&mut self) -> anyhow::Result<()> {
        // We assume here that the pause command will succeed.
        //
        // Yeah, I don't know what else to do here, because mpv doesn't seem
        // to fire a pause event anymore when it gets paused.
        self.observed.paused ^= true;
        self.set_property::<property::Pause>(self.observed.paused)?;
        Ok(())
    }
    fn write_command<C: Command>(&mut self, command: C) -> anyhow::Result<()> {
        let command_json = command.to_command_json();
        let mut serialized = serde_json::to_vec(&command_json).unwrap();
        // Commands need to be terminated with newline
        serialized.push(b'\n');
        self.ipc_stream.write_all(&serialized)?;
        Ok(())
    }
    pub fn write_str(&mut self, text: &str) -> anyhow::Result<()> {
        self.ipc_stream.write_all(text.as_bytes())?;
        self.ipc_stream.write_all(b"\n")?;
        Ok(())
    }
    fn set_property<P: Property>(&mut self, value: P::Value) -> anyhow::Result<()>
    where
        P::Value: PropValue,
    {
        self.write_command(SetProperty::<P>(value))
    }
    pub fn handle_responses(&mut self) -> anyhow::Result<()> {
        loop {
            let mut buf = [0; 1000];
            match self.ipc_stream.read(&mut buf) {
                Ok(amount) => {
                    if amount == 0 {
                        // Assume EOF and return
                        return Ok(());
                    }
                    let string = std::str::from_utf8(&buf[..amount]).unwrap();
                    for line in string.lines() {
                        self.handle_response_line(line)
                    }
                }
                Err(e) => match e.kind() {
                    std::io::ErrorKind::WouldBlock => return Ok(()),
                    _ => anyhow::bail!("ipc bridge io error: {}", e),
                },
            }
        }
    }
    fn handle_response_line(&mut self, line: &str) {
        match serde_json::from_str::<HashMap<&str, serde_json::Value>>(line) {
            Ok(map) => {
                if let Some(event) = map.get("event") {
                    match event.as_str().unwrap() {
                        "pause" => self.observed.paused = true,
                        "unpause" => self.observed.paused = false,
                        "property-change" => {
                            let name = map.get("name").unwrap().as_str().unwrap();
                            let Some(data) = map.get("data") else {
                                logln!("data-less property change: {}", name);
                                return;
                            };
                            match name {
                                property::Speed::NAME => {
                                    self.observed.speed = data.as_f64().unwrap()
                                }
                                property::Volume::NAME => {
                                    self.observed.volume = data.as_f64().unwrap() as u8
                                }
                                property::Duration::NAME => {
                                    self.observed.duration = data.as_f64().unwrap()
                                }
                                property::TimePos::NAME => {
                                    self.observed.time_pos = data.as_f64().unwrap()
                                }
                                property::AbLoopA::NAME => self.observed.ab_loop_a = data.as_f64(),
                                property::AbLoopB::NAME => self.observed.ab_loop_b = data.as_f64(),
                                property::TrackListCount::NAME => {
                                    self.observed.track_count = data.as_u64().unwrap() as u8
                                }
                                property::LavfiComplex::NAME => {
                                    self.observed.lavfi_complex = data.as_str().unwrap().to_owned()
                                }
                                property::LoopFile::NAME => {
                                    self.observed.loop_file = data.as_str() == Some("inf")
                                }
                                name => logln!("Unhandled property: {} = {}", name, data),
                            }
                        }
                        "end-file" => {
                            self.event_queue.push_back(IpcEvent::EndFile);
                        }
                        _ => logln!("Unhandled event: {}", event),
                    }
                }
            }
            Err(e) => {
                logln!("Serialize error: {}", e);
                logln!("Unserialized event: {}", line);
            }
        }
    }
    pub fn set_volume(&mut self, vol: u8) -> anyhow::Result<()> {
        self.set_property::<property::Volume>(vol as f64)
    }
    pub fn set_speed(&mut self, speed: f64) -> anyhow::Result<()> {
        self.set_property::<property::Speed>(speed)
    }
    pub fn seek(&mut self, pos: f64) -> anyhow::Result<()> {
        self.set_property::<property::TimePos>(pos)
    }
    pub fn set_video(&mut self, show: bool) -> anyhow::Result<()> {
        self.set_property::<property::Video>(show.then_some("1"))
    }
    pub fn set_ab_loop(&mut self, a: Option<f64>, b: Option<f64>) -> anyhow::Result<()> {
        self.set_property::<property::AbLoopA>(a)?;
        self.set_property::<property::AbLoopB>(b)
    }
    pub fn add_audio(&mut self, path: &str) -> anyhow::Result<()> {
        self.write_command(AudioAdd(path))?;
        self.set_property::<property::LavfiComplex>("[aid1] [aid2] amix [ao]".into())
    }

    pub fn mix_t1_with_track(&mut self, track: u64) -> anyhow::Result<()> {
        self.set_property::<property::LavfiComplex>(format!("[aid1] [aid{track}] amix [ao]"))?;
        Ok(())
    }

    pub(crate) fn switch_to_track(&mut self, id: u64) -> anyhow::Result<()> {
        self.set_property::<property::LavfiComplex>("".into())?;
        self.set_property::<property::Aid>(id)?;
        Ok(())
    }

    pub(crate) fn remove_track(&mut self, track_num: u64) -> anyhow::Result<()> {
        self.write_command(AudioRemove(track_num))?;
        Ok(())
    }

    pub(crate) fn set_loop_file(&mut self, loop_file: bool) {
        self.set_property::<property::LoopFile>(if loop_file { Some("inf") } else { None })
            .log_err("Failed to set loop");
    }
}
