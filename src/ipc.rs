//! Interprocess comunication with spawned mpv process

use {
    crate::logln,
    interprocess::local_socket::{
        GenericFilePath, Stream as LocalSocketStream, ToFsName, traits::Stream as _,
    },
    serde::Serialize,
    std::{
        collections::{HashMap, VecDeque},
        io::{Read, Write},
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
}

trait Command {
    type R: Serialize;
    fn json_values(&self) -> Self::R;
    fn to_command_json(&self) -> CommandJson<Self::R> {
        CommandJson {
            command: self.json_values(),
        }
    }
}

trait SetProperty {
    const NAME: &'static str;
    type Value: Into<serde_json::Value>;
    fn value(&self) -> Self::Value;
}

impl<T: SetProperty> Command for T {
    type R = [serde_json::Value; 3];

    fn json_values(&self) -> Self::R {
        [
            "set_property".into(),
            Self::NAME.into(),
            self.value().into(),
        ]
    }
}

macro_rules! set_property_impl {
    ($t:ty, $name:literal, $v:ty) => {
        impl SetProperty for $t {
            const NAME: &'static str = $name;
            type Value = $v;
            fn value(&self) -> Self::Value {
                self.0
            }
        }
    };
}

struct SetPaused(bool);
set_property_impl!(SetPaused, "pause", bool);

struct ObserveProperty<'a>(&'a str);

impl Command for ObserveProperty<'_> {
    type R = [serde_json::Value; 3];
    fn json_values(&self) -> Self::R {
        ["observe_property".into(), 1.into(), self.0.into()]
    }
}

struct SetVolume(u8);
set_property_impl!(SetVolume, "volume", u8);

struct SetSpeed(f64);
set_property_impl!(SetSpeed, "speed", f64);

struct Seek(f64);
set_property_impl!(Seek, "time-pos", f64);

struct SetVideo(bool);

impl SetProperty for SetVideo {
    const NAME: &'static str = "vid";

    type Value = serde_json::Value;

    fn value(&self) -> Self::Value {
        if self.0 { 1.into() } else { false.into() }
    }
}

#[derive(Serialize)]
struct CommandJson<T: Serialize> {
    command: T,
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
        this.write_command(ObserveProperty("speed"))?;
        this.write_command(ObserveProperty("volume"))?;
        this.write_command(ObserveProperty("time-pos"))?;
        this.write_command(ObserveProperty("duration"))?;
        Ok(this)
    }
    pub fn toggle_pause(&mut self) -> anyhow::Result<()> {
        // We assume here that the pause command will succeed.
        //
        // Yeah, I don't know what else to do here, because mpv doesn't seem
        // to fire a pause event anymore when it gets paused.
        self.observed.paused ^= true;
        self.write_command(SetPaused(self.observed.paused))?;
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
                                "speed" => self.observed.speed = data.as_f64().unwrap(),
                                "volume" => self.observed.volume = data.as_f64().unwrap() as u8,
                                "duration" => self.observed.duration = data.as_f64().unwrap(),
                                "time-pos" => self.observed.time_pos = data.as_f64().unwrap(),
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
        self.write_command(SetVolume(vol))
    }
    pub fn set_speed(&mut self, speed: f64) -> anyhow::Result<()> {
        self.write_command(SetSpeed(speed))
    }
    pub fn seek(&mut self, pos: f64) -> anyhow::Result<()> {
        self.write_command(Seek(pos))
    }
    pub fn set_video(&mut self, show: bool) -> anyhow::Result<()> {
        self.write_command(SetVideo(show))
    }
}
