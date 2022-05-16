use crate::ansi_parser::{AnsiParser, TermCmd};

pub struct AnsiTermBuf {
    width: u16,
    height: usize,
    cells: Vec<u8>,
    cursor: Cursor,
    ansi_parser: AnsiParser,
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

impl AnsiTermBuf {
    pub fn new(width: u16) -> Self {
        Self {
            width,
            height: 0,
            cells: Vec::new(),
            cursor: Cursor::default(),
            ansi_parser: AnsiParser::default(),
        }
    }
    pub fn contents_to_string(&self) -> String {
        let mut s = String::new();
        for y in 0..self.height {
            s.push_str(std::str::from_utf8(self.line_slice(y)).unwrap());
            s.push('\n');
        }
        s
    }
    pub fn line_slice(&self, y: usize) -> &[u8] {
        let from = y * self.width as usize;
        let to = from + self.width as usize;
        &self.cells[from..to]
    }
    pub fn feed(&mut self, data: &[u8]) {
        self.ansi_parser.advance(data, |cmd| match cmd {
            TermCmd::PutChar(c) => Self::put_char(
                &mut self.cells,
                &self.width,
                &mut self.height,
                &mut self.cursor,
                c,
            ),
            TermCmd::CarriageReturn => self.cursor.x = 0,
            TermCmd::LineFeed => Self::line_feed(&mut self.cursor),
            TermCmd::CursorUp(n) => self.cursor.y = self.cursor.y.saturating_sub(n as usize),
            TermCmd::EraseFromCursorToEol => {
                for x in self.cursor.x..self.width {
                    let idx = self.cursor.y * self.width as usize + x as usize;
                    if idx >= self.cells.len() {
                        break;
                    }
                    self.cells[idx] = b' ';
                }
            }
        });
    }
    fn put_char(cells: &mut Vec<u8>, width: &u16, height: &mut usize, cursor: &mut Cursor, ch: u8) {
        Self::extend_while_cursor_past(cells, cursor, width, height);
        cells[cursor.index(*width)] = ch;
        cursor.x += 1;
        if cursor.x >= *width {
            cursor.x = 0;
            cursor.y += 1;
        }
    }
    fn extend(cells: &mut Vec<u8>, width: &u16, height: &mut usize) {
        cells.extend(std::iter::repeat(b' ').take(*width as usize));
        *height += 1;
    }
    fn extend_while_cursor_past(
        cells: &mut Vec<u8>,
        cursor: &mut Cursor,
        width: &u16,
        height: &mut usize,
    ) {
        while cursor.y >= *height {
            Self::extend(cells, width, height);
        }
    }
    fn line_feed(cursor: &mut Cursor) {
        cursor.x = 0;
        cursor.y += 1;
    }
    pub fn reset(&mut self) {
        self.cursor = Cursor::default();
        self.cells.clear();
        self.height = 0;
        self.ansi_parser = AnsiParser::default();
    }
}
