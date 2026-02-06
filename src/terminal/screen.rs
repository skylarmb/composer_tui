//! Terminal screen buffer and ANSI escape sequence handling.

use vte::{Params, Parser, Perform};

/// A terminal color value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

/// Visual attributes for a terminal cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellStyle {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl Default for CellStyle {
    fn default() -> Self {
        Self {
            fg: Color::Default,
            bg: Color::Default,
            bold: false,
            italic: false,
            underline: false,
        }
    }
}

/// A single terminal cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub style: CellStyle,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            style: CellStyle::default(),
        }
    }
}

/// 2D terminal screen state with cursor and ANSI parsing.
pub struct ScreenBuffer {
    cols: usize,
    rows: usize,
    cells: Vec<Vec<Cell>>,
    cursor_row: usize,
    cursor_col: usize,
    current_style: CellStyle,
    parser: Parser,
    /// Deferred wrap flag: set when a character is placed at the last column.
    /// The actual wrap (CR+LF) only happens when the *next* printable character
    /// arrives, matching real terminal behavior (DEC VT220 spec).
    wrap_pending: bool,
}

impl ScreenBuffer {
    /// Create an empty screen buffer with the given dimensions.
    pub fn new(cols: usize, rows: usize) -> Self {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let cells = (0..rows).map(|_| vec![Cell::default(); cols]).collect();
        Self {
            cols,
            rows,
            cells,
            cursor_row: 0,
            cursor_col: 0,
            current_style: CellStyle::default(),
            parser: Parser::new(),
            wrap_pending: false,
        }
    }

    /// Parse terminal bytes and apply them to the screen.
    pub fn write(&mut self, data: &[u8]) {
        let mut parser = std::mem::take(&mut self.parser);
        for &byte in data {
            parser.advance(self, byte);
        }
        self.parser = parser;
    }

    /// Current screen width (columns).
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Current screen height (rows).
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Current cursor position as `(row, col)` (0-based).
    pub fn cursor_position(&self) -> (usize, usize) {
        (self.cursor_row, self.cursor_col)
    }

    /// Get a cell by `(row, col)`.
    pub fn cell(&self, row: usize, col: usize) -> Option<&Cell> {
        self.cells.get(row).and_then(|line| line.get(col))
    }

    /// Get text content for a row.
    pub fn row_text(&self, row: usize) -> Option<String> {
        self.cells
            .get(row)
            .map(|line| line.iter().map(|cell| cell.ch).collect())
    }

    /// Get all cells in a row.
    pub fn row_cells(&self, row: usize) -> Option<&[Cell]> {
        self.cells.get(row).map(Vec::as_slice)
    }

    /// Resize the screen while preserving top-left content where possible.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        let cols = cols.max(1);
        let rows = rows.max(1);

        if cols == self.cols && rows == self.rows {
            return;
        }

        for row in &mut self.cells {
            row.resize(cols, Cell::default());
        }

        if rows > self.rows {
            self.cells
                .extend((0..(rows - self.rows)).map(|_| vec![Cell::default(); cols]));
        } else {
            self.cells.truncate(rows);
        }

        self.cols = cols;
        self.rows = rows;
        self.cursor_row = self.cursor_row.min(self.rows - 1);
        self.cursor_col = self.cursor_col.min(self.cols - 1);
    }

    fn reset(&mut self) {
        self.clear_all();
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.current_style = CellStyle::default();
        self.wrap_pending = false;
    }

    fn clear_all(&mut self) {
        for row in &mut self.cells {
            for cell in row {
                *cell = Cell::default();
            }
        }
    }

    fn clear_line(&mut self, row: usize, start_col: usize, end_col: usize) {
        if let Some(line) = self.cells.get_mut(row) {
            for cell in line
                .iter_mut()
                .take(end_col.saturating_add(1))
                .skip(start_col)
            {
                *cell = Cell::default();
            }
        }
    }

    fn clear_screen_mode(&mut self, mode: u16) {
        match mode {
            1 => {
                for row in 0..self.cursor_row {
                    self.clear_line(row, 0, self.cols.saturating_sub(1));
                }
                self.clear_line(self.cursor_row, 0, self.cursor_col);
            }
            2 => self.clear_all(),
            _ => {
                self.clear_line(
                    self.cursor_row,
                    self.cursor_col,
                    self.cols.saturating_sub(1),
                );
                for row in (self.cursor_row + 1)..self.rows {
                    self.clear_line(row, 0, self.cols.saturating_sub(1));
                }
            }
        }
    }

    fn clear_line_mode(&mut self, mode: u16) {
        match mode {
            1 => self.clear_line(self.cursor_row, 0, self.cursor_col),
            2 => self.clear_line(self.cursor_row, 0, self.cols.saturating_sub(1)),
            _ => self.clear_line(
                self.cursor_row,
                self.cursor_col,
                self.cols.saturating_sub(1),
            ),
        }
    }

    fn put_char(&mut self, ch: char) {
        // If a previous character triggered wrap_pending, perform the deferred
        // wrap now (before placing this character).
        if self.wrap_pending {
            self.wrap_pending = false;
            self.cursor_col = 0;
            self.line_feed();
        }

        self.cells[self.cursor_row][self.cursor_col] = Cell {
            ch,
            style: self.current_style,
        };

        if self.cursor_col + 1 >= self.cols {
            // Don't wrap yet — defer until the next printable character.
            self.wrap_pending = true;
        } else {
            self.cursor_col += 1;
        }
    }

    fn line_feed(&mut self) {
        if self.cursor_row + 1 >= self.rows {
            self.scroll_up(1);
            self.cursor_row = self.rows - 1;
        } else {
            self.cursor_row += 1;
        }
    }

    fn scroll_up(&mut self, lines: usize) {
        let lines = lines.min(self.rows);
        for _ in 0..lines {
            self.cells.remove(0);
            self.cells.push(vec![Cell::default(); self.cols]);
        }
    }

    fn scroll_down(&mut self, lines: usize) {
        let lines = lines.min(self.rows);
        for _ in 0..lines {
            self.cells.pop();
            self.cells.insert(0, vec![Cell::default(); self.cols]);
        }
    }

    fn set_cursor_position(&mut self, row: usize, col: usize) {
        self.cursor_row = row.min(self.rows - 1);
        self.cursor_col = col.min(self.cols - 1);
    }

    fn apply_sgr(&mut self, params: &Params) {
        let mut codes = params
            .iter()
            .map(|param| param.first().copied().unwrap_or(0))
            .peekable();

        if codes.peek().is_none() {
            self.current_style = CellStyle::default();
            return;
        }

        while let Some(code) = codes.next() {
            match code {
                0 => self.current_style = CellStyle::default(),
                1 => self.current_style.bold = true,
                3 => self.current_style.italic = true,
                4 => self.current_style.underline = true,
                22 => self.current_style.bold = false,
                23 => self.current_style.italic = false,
                24 => self.current_style.underline = false,
                30..=37 => self.current_style.fg = Color::Indexed(to_u8(code - 30)),
                39 => self.current_style.fg = Color::Default,
                40..=47 => self.current_style.bg = Color::Indexed(to_u8(code - 40)),
                49 => self.current_style.bg = Color::Default,
                90..=97 => self.current_style.fg = Color::Indexed(to_u8((code - 90) + 8)),
                100..=107 => self.current_style.bg = Color::Indexed(to_u8((code - 100) + 8)),
                38 | 48 => {
                    let is_fg = code == 38;
                    let Some(mode) = codes.next() else {
                        continue;
                    };
                    match mode {
                        5 => {
                            if let Some(index) = codes.next() {
                                let color = Color::Indexed(to_u8(index));
                                if is_fg {
                                    self.current_style.fg = color;
                                } else {
                                    self.current_style.bg = color;
                                }
                            }
                        }
                        2 => {
                            let (Some(r), Some(g), Some(b)) =
                                (codes.next(), codes.next(), codes.next())
                            else {
                                continue;
                            };
                            let color = Color::Rgb(to_u8(r), to_u8(g), to_u8(b));
                            if is_fg {
                                self.current_style.fg = color;
                            } else {
                                self.current_style.bg = color;
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
}

impl Perform for ScreenBuffer {
    fn print(&mut self, c: char) {
        self.put_char(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' | 0x0b | 0x0c => {
                // LF clears wrap_pending — the newline handles line advancement.
                self.wrap_pending = false;
                self.line_feed();
            }
            b'\r' => {
                // CR clears wrap_pending and returns cursor to column 0.
                self.wrap_pending = false;
                self.cursor_col = 0;
            }
            0x08 => {
                // Backspace clears wrap_pending.
                self.wrap_pending = false;
                if self.cursor_col > 0 {
                    self.cursor_col -= 1;
                }
            }
            b'\t' => {
                self.wrap_pending = false;
                let next_tab = ((self.cursor_col / 8) + 1) * 8;
                self.cursor_col = next_tab.min(self.cols - 1);
            }
            _ => {}
        }
    }

    fn hook(&mut self, _: &Params, _: &[u8], _: bool, _: char) {}

    fn put(&mut self, _: u8) {}

    fn unhook(&mut self) {}

    fn osc_dispatch(&mut self, _: &[&[u8]], _: bool) {}

    fn csi_dispatch(&mut self, params: &Params, _: &[u8], _: bool, action: char) {
        // Any CSI sequence that moves the cursor or modifies the screen
        // clears the deferred wrap state.
        match action {
            'A' => {
                self.wrap_pending = false;
                let count = csi_count(params, 1);
                self.cursor_row = self.cursor_row.saturating_sub(count);
            }
            'B' => {
                self.wrap_pending = false;
                let count = csi_count(params, 1);
                self.cursor_row = self.cursor_row.saturating_add(count).min(self.rows - 1);
            }
            'C' => {
                self.wrap_pending = false;
                let count = csi_count(params, 1);
                self.cursor_col = self.cursor_col.saturating_add(count).min(self.cols - 1);
            }
            'D' => {
                self.wrap_pending = false;
                let count = csi_count(params, 1);
                self.cursor_col = self.cursor_col.saturating_sub(count);
            }
            'G' => {
                self.wrap_pending = false;
                let col = csi_param(params, 0, 1).saturating_sub(1);
                self.set_cursor_position(self.cursor_row, usize::from(col));
            }
            'H' | 'f' => {
                self.wrap_pending = false;
                let row = csi_param(params, 0, 1).saturating_sub(1);
                let col = csi_param(params, 1, 1).saturating_sub(1);
                self.set_cursor_position(usize::from(row), usize::from(col));
            }
            'd' => {
                self.wrap_pending = false;
                let row = csi_param(params, 0, 1).saturating_sub(1);
                self.set_cursor_position(usize::from(row), self.cursor_col);
            }
            'J' => {
                self.wrap_pending = false;
                self.clear_screen_mode(csi_param(params, 0, 0));
            }
            'K' => {
                self.wrap_pending = false;
                self.clear_line_mode(csi_param(params, 0, 0));
            }
            'S' => {
                self.wrap_pending = false;
                self.scroll_up(csi_count(params, 1));
            }
            'T' => {
                self.wrap_pending = false;
                self.scroll_down(csi_count(params, 1));
            }
            'm' => self.apply_sgr(params),
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _: &[u8], _: bool, byte: u8) {
        if byte == b'c' {
            self.reset();
        }
    }
}

fn csi_param(params: &Params, index: usize, default: u16) -> u16 {
    let value = params
        .iter()
        .nth(index)
        .and_then(|param| param.first().copied())
        .unwrap_or(default);
    if value == 0 {
        default
    } else {
        value
    }
}

fn csi_count(params: &Params, default: u16) -> usize {
    usize::from(csi_param(params, 0, default))
}

fn to_u8(value: u16) -> u8 {
    u8::try_from(value).unwrap_or(u8::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_multiline_content_and_scrolls() {
        let mut screen = ScreenBuffer::new(4, 2);
        screen.write(b"ABCD1234xy");

        assert_eq!(screen.row_text(0).as_deref(), Some("1234"));
        assert_eq!(screen.row_text(1).as_deref(), Some("xy  "));
        assert_eq!(screen.cursor_position(), (1, 2));
    }

    #[test]
    fn applies_cursor_moves_and_clear_sequences() {
        let mut screen = ScreenBuffer::new(8, 3);
        screen.write(b"hello");
        screen.write(b"\x1b[2DXY");
        assert_eq!(screen.row_text(0).as_deref(), Some("helXY   "));

        screen.write(b"\r\x1b[K");
        assert_eq!(screen.row_text(0).as_deref(), Some("        "));
    }

    #[test]
    fn parses_ansi_colors_including_bright_and_256_palette() {
        let mut screen = ScreenBuffer::new(6, 1);
        screen.write(b"\x1b[31mR\x1b[94mB\x1b[38;5;196mX\x1b[0mN");

        assert_eq!(screen.cell(0, 0).map(|c| c.ch), Some('R'));
        assert_eq!(
            screen.cell(0, 0).map(|c| c.style.fg),
            Some(Color::Indexed(1))
        );

        assert_eq!(screen.cell(0, 1).map(|c| c.ch), Some('B'));
        assert_eq!(
            screen.cell(0, 1).map(|c| c.style.fg),
            Some(Color::Indexed(12))
        );

        assert_eq!(screen.cell(0, 2).map(|c| c.ch), Some('X'));
        assert_eq!(
            screen.cell(0, 2).map(|c| c.style.fg),
            Some(Color::Indexed(196))
        );

        assert_eq!(screen.cell(0, 3).map(|c| c.ch), Some('N'));
        assert_eq!(screen.cell(0, 3).map(|c| c.style.fg), Some(Color::Default));
    }

    #[test]
    fn deferred_wrap_prevents_double_spacing() {
        // Simulates a full-width line followed by \r\n. Without deferred wrap,
        // the auto-wrap at column end + the explicit \n would produce a blank
        // line between each content line.
        let mut screen = ScreenBuffer::new(4, 4);

        // Fill row 0 exactly (4 chars), then \r\n, then next line content.
        screen.write(b"ABCD\r\nEFGH\r\nIJ");

        // With deferred wrap: ABCD fills row 0, wrap_pending is set.
        // \r clears wrap_pending + sets col=0. \n does line_feed to row 1.
        // EFGH fills row 1, wrap_pending is set.
        // \r clears wrap_pending + sets col=0. \n does line_feed to row 2.
        // IJ starts at row 2.
        assert_eq!(screen.row_text(0).as_deref(), Some("ABCD"));
        assert_eq!(screen.row_text(1).as_deref(), Some("EFGH"));
        assert_eq!(screen.row_text(2).as_deref(), Some("IJ  "));
        assert_eq!(screen.row_text(3).as_deref(), Some("    ")); // no blank line gap
    }

    #[test]
    fn wrap_pending_cleared_by_cursor_movement() {
        let mut screen = ScreenBuffer::new(4, 3);

        // Fill row 0 exactly → wrap_pending set.
        screen.write(b"ABCD");
        assert_eq!(screen.cursor_position(), (0, 3)); // cursor stays at last col

        // CSI cursor movement clears wrap_pending without wrapping.
        screen.write(b"\x1b[1;1H"); // CUP to row 1, col 1 (0-indexed: 0,0)
        assert_eq!(screen.cursor_position(), (0, 0));

        // Writing should overwrite row 0, not advance to row 1.
        screen.write(b"X");
        assert_eq!(screen.row_text(0).as_deref(), Some("XBCD"));
    }

    #[test]
    fn resize_preserves_existing_content() {
        let mut screen = ScreenBuffer::new(4, 2);
        screen.write(b"ABCDxy");

        screen.resize(6, 3);

        assert_eq!(screen.row_text(0).as_deref(), Some("ABCD  "));
        assert_eq!(screen.row_text(1).as_deref(), Some("xy    "));
        assert_eq!(screen.row_text(2).as_deref(), Some("      "));
        assert_eq!(screen.rows(), 3);
        assert_eq!(screen.cols(), 6);
    }
}
