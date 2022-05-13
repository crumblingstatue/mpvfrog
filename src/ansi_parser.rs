use eframe::egui;
use egui_inspect::derive::Inspect;

const ESC: u8 = 0x1b;

#[derive(Inspect, Debug)]
pub struct AnsiParser {
    status: Status,
    params: Vec<u8>,
    cursor: usize,
    last_newline: usize,
    /// Carriange return (\r) was encountered
    carriage_return: bool,
}

impl Default for AnsiParser {
    fn default() -> Self {
        Self {
            status: Status::Init,
            params: Vec::new(),
            cursor: 0,
            last_newline: 0,
            carriage_return: false,
        }
    }
}

#[derive(Debug, Inspect, PartialEq)]
enum Status {
    Init,
    Esc,
    ControlSeqStart,
}

impl AnsiParser {
    pub fn advance_and_write(&mut self, bytes: &[u8], out_string: &mut String) {
        for &byte in bytes {
            match self.status {
                Status::Init => match byte {
                    ESC => self.status = Status::Esc,
                    b'\r' => {
                        self.carriage_return = true;
                    }
                    _ => {
                        if byte == b'\n' {
                            self.last_newline = self.cursor;
                        }
                        if self.carriage_return {
                            self.cursor = self.last_newline + 1;
                            out_string.truncate(self.last_newline + 1);
                            self.carriage_return = false;
                        }
                        out_string.push(byte as char);
                        self.cursor += 1;
                    }
                },
                Status::Esc => {
                    match byte {
                        b'=' => {
                            // Unknown, ignore
                            self.status = Status::Init;
                        }
                        b'[' => {
                            // Control sequence start
                            self.status = Status::ControlSeqStart;
                        }
                        _ => panic!("[{:x}]", byte),
                    }
                }
                Status::ControlSeqStart => {
                    match byte {
                        0x30..=0x3F => {
                            self.params.push(byte);
                        }
                        0x40..=0x7E => {
                            // Terminator byte
                            self.status = Status::Init;
                            self.params.clear();
                        }
                        _ => panic!("<{:x}>", byte),
                    }
                }
            }
        }
    }
}
