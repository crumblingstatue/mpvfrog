use crate::ansi_parser::{AnsiParser, TermCmd};

pub struct AnsiTerm {
    term_state: TermState,
    ansi_parser: AnsiParser,
}

struct TermState {
    width: u16,
    height: usize,
    cells: Vec<u8>,
    cursor: Cursor,
}

impl TermState {
    fn new(width: u16) -> Self {
        Self {
            width,
            height: 0,
            cells: Vec::new(),
            cursor: Cursor::default(),
        }
    }
    fn contents_to_string(&self) -> String {
        let mut s = String::new();
        for y in 0..self.height {
            s.push_str(std::str::from_utf8(self.line_slice(y)).unwrap().trim_end());
            s.push('\n');
        }
        s
    }
    fn line_slice(&self, y: usize) -> &[u8] {
        let from = y * self.width as usize;
        let to = from + self.width as usize;
        &self.cells[from..to]
    }
    fn put_char(&mut self, ch: u8) {
        self.extend_while_cursor_past();
        self.cells[self.cursor.index(self.width)] = ch;
        self.cursor.x += 1;
        if self.cursor.x >= self.width {
            self.cursor.x = 0;
            self.cursor.y += 1;
        }
    }
    fn extend(&mut self) {
        self.cells
            .extend(std::iter::repeat(b' ').take(self.width as usize));
        self.height += 1;
    }
    fn extend_while_cursor_past(&mut self) {
        while self.cursor.y >= self.height {
            self.extend();
        }
    }
    fn line_feed(&mut self) {
        self.cursor.x = 0;
        self.cursor.y += 1;
    }
    fn erase_from_cursor_to_eol(&mut self) {
        for x in self.cursor.x..self.width {
            let idx = self.cursor.y * self.width as usize + x as usize;
            if idx >= self.cells.len() {
                break;
            }
            self.cells[idx] = b' ';
        }
    }
}

#[derive(Default)]
struct Cursor {
    x: u16,
    y: usize,
}

impl Cursor {
    fn index(&self, width: u16) -> usize {
        self.y * width as usize + self.x as usize
    }
}

impl AnsiTerm {
    pub fn new(width: u16) -> Self {
        Self {
            term_state: TermState::new(width),
            ansi_parser: AnsiParser::default(),
        }
    }
    pub fn feed(&mut self, data: &[u8]) {
        self.ansi_parser.advance(data, |cmd| match cmd {
            TermCmd::PutChar(c) => self.term_state.put_char(c),
            TermCmd::CarriageReturn => self.term_state.cursor.x = 0,
            TermCmd::LineFeed => self.term_state.line_feed(),
            TermCmd::CursorUp(n) => {
                self.term_state.cursor.y = self.term_state.cursor.y.saturating_sub(n as usize)
            }
            TermCmd::EraseFromCursorToEol => self.term_state.erase_from_cursor_to_eol(),
        });
    }
    pub fn reset(&mut self) {
        self.term_state.cursor = Cursor::default();
        self.term_state.cells.clear();
        self.term_state.height = 0;
        self.ansi_parser = AnsiParser::default();
    }
    pub fn contents_to_string(&self) -> String {
        self.term_state.contents_to_string()
    }
}
