use std::{
    collections::HashMap,
    io::{Read, Write},
};

use interprocess::local_socket::LocalSocketStream;
use serde::Serialize;

pub struct Bridge {
    ipc_stream: LocalSocketStream,
    pub observed: Properties,
}

#[derive(Default)]
pub struct Properties {
    pub paused: bool,
    pub volume: u8,
    pub speed: f64,
}

enum Command<'a> {
    SetPaused(bool),
    ObserveProperty(&'a str),
}

impl<'a> Command<'a> {
    fn into_command_json(self) -> CommandJson {
        let vec = match self {
            Command::SetPaused(paused) => {
                vec!["set_property".into(), "pause".into(), paused.into()]
            }
            Command::ObserveProperty(which) => {
                vec!["observe_property".into(), 1.into(), which.into()]
            }
        };
        CommandJson { command: vec }
    }
}

#[derive(Serialize)]
struct CommandJson {
    command: Vec<serde_json::Value>,
}

impl Bridge {
    pub fn connect() -> Self {
        let ipc_stream = LocalSocketStream::connect("/tmp/mpv-egui-musicplayer.sock").unwrap();
        ipc_stream.set_nonblocking(true).unwrap();
        let mut this = Self {
            ipc_stream,
            observed: Default::default(),
        };
        this.write_command(Command::ObserveProperty("speed"));
        this.write_command(Command::ObserveProperty("volume"));
        this
    }
    pub fn toggle_pause(&mut self) {
        self.write_command(Command::SetPaused(!self.observed.paused));
    }
    fn write_command(&mut self, command: Command) {
        let command_json = command.into_command_json();
        let mut serialized = serde_json::to_vec(&command_json).unwrap();
        // Commands need to be terminated with newline
        serialized.push(b'\n');
        self.ipc_stream.write_all(&serialized).unwrap();
    }
    pub fn handle_responses(&mut self) {
        loop {
            let mut buf = [0; 1000];
            match self.ipc_stream.read(&mut buf) {
                Ok(amount) => {
                    let string = std::str::from_utf8(&buf[..amount]).unwrap();
                    for line in string.lines() {
                        self.handle_response_line(line)
                    }
                }
                Err(e) => match e.kind() {
                    std::io::ErrorKind::WouldBlock => return,
                    _ => panic!("ipc bridge io error: {}", e),
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
                            let data = map.get("data").unwrap();
                            match name {
                                "speed" => self.observed.speed = data.as_f64().unwrap(),
                                "volume" => self.observed.volume = data.as_f64().unwrap() as u8,
                                name => eprintln!("Unhandled property: {}", name),
                            }
                        }
                        _ => eprintln!("Unhandled event: {}", event),
                    }
                } else if let Some(req_id) = map.get("request_id") {
                    eprintln!("Request id: {}", req_id);
                }
            }
            Err(e) => {
                eprintln!("Serialize error: {}", e);
                eprintln!("Unserialized event: {}", line);
            }
        }
    }
}
