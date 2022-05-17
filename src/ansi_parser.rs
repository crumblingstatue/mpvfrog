const ESC: u8 = 0x1b;

#[derive(Debug)]
pub struct AnsiParser {
    status: Status,
    params: Vec<u8>,
}

impl Default for AnsiParser {
    fn default() -> Self {
        Self {
            status: Status::Init,
            params: Vec::new(),
        }
    }
}

#[derive(Debug, PartialEq)]
enum Status {
    Init,
    Esc,
    ControlSeqStart,
}

pub enum TermCmd {
    PutChar(u8),
    CarriageReturn,
    LineFeed,
    /// Move cursor up this many lines
    CursorUp(u8),
    /// Erase from cursor to the end of line
    EraseFromCursorToEol,
}

impl AnsiParser {
    pub fn advance(&mut self, bytes: &[u8], mut term_callback: impl FnMut(TermCmd)) {
        for &byte in bytes {
            match self.status {
                Status::Init => match byte {
                    ESC => self.status = Status::Esc,
                    b'\r' => {
                        term_callback(TermCmd::CarriageReturn);
                    }
                    b'\n' => term_callback(TermCmd::LineFeed),
                    c if c.is_ascii() => term_callback(TermCmd::PutChar(c)),
                    c => panic!("Unhandled byte: {}", c),
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
                        _ => eprintln!("Unexpected ansi [{:x}]", byte),
                    }
                }
                Status::ControlSeqStart => {
                    match byte {
                        0x30..=0x3F => {
                            self.params.push(byte);
                        }
                        0x40..=0x7E => {
                            // Terminator byte
                            match byte {
                                // color/etc, ignore
                                b'm' => {}
                                b'K' => {
                                    term_callback(TermCmd::EraseFromCursorToEol);
                                }
                                b'A' => {
                                    // Move cursor up N lines
                                    let n = self.params.get(0);
                                    term_callback(TermCmd::CursorUp(n.cloned().unwrap_or(1)));
                                }
                                _ => eprintln!("terminator byte {} ({})", byte, byte as char),
                            }
                            self.status = Status::Init;
                            self.params.clear();
                        }
                        _ => eprintln!("Unexpected ansi <{:x}>", byte),
                    }
                }
            }
        }
    }
}
