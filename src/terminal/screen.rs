//! Terminal screen buffer and ANSI escape sequence handling.

use std::collections::VecDeque;

use vte::{Params, Parser, Perform};

const DEFAULT_SCROLLBACK_LIMIT: usize = 1000;

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
    scrollback: VecDeque<Vec<Cell>>,
    scrollback_limit: usize,
    cursor_row: usize,
    cursor_col: usize,
    current_style: CellStyle,
    parser: Parser,
    /// Deferred wrap flag: set when a character is placed at the last column.
    /// The actual wrap (CR+LF) only happens when the *next* printable character
    /// arrives, matching real terminal behavior (DEC VT220 spec).
    wrap_pending: bool,
    /// Bytes to send back to the PTY in response to terminal capability queries.
    response_bytes: Vec<u8>,
}

impl ScreenBuffer {
    /// Create an empty screen buffer with the given dimensions.
    pub fn new(cols: usize, rows: usize) -> Self {
        Self::new_with_scrollback(cols, rows, DEFAULT_SCROLLBACK_LIMIT)
    }

    /// Create an empty screen buffer with the given dimensions and scrollback limit.
    pub fn new_with_scrollback(cols: usize, rows: usize, scrollback_limit: usize) -> Self {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let cells = (0..rows).map(|_| vec![Cell::default(); cols]).collect();
        Self {
            cols,
            rows,
            cells,
            scrollback: VecDeque::new(),
            scrollback_limit,
            cursor_row: 0,
            cursor_col: 0,
            current_style: CellStyle::default(),
            parser: Parser::new(),
            wrap_pending: false,
            response_bytes: Vec::new(),
        }
    }

    /// Parse terminal bytes and apply them to the screen.
    pub fn write(&mut self, data: &[u8]) {
        let _ = self.write_with_responses(data);
    }

    /// Parse terminal bytes and return any response bytes that should be sent
    /// back to the PTY (for example, CSI cursor-position queries).
    pub fn write_with_responses(&mut self, data: &[u8]) -> Vec<u8> {
        self.response_bytes.clear();
        let mut parser = std::mem::take(&mut self.parser);
        for &byte in data {
            parser.advance(self, byte);
        }
        self.parser = parser;
        std::mem::take(&mut self.response_bytes)
    }

    /// Current screen width (columns).
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Current screen height (rows).
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Number of rows currently held in scrollback history.
    pub fn scrollback_len(&self) -> usize {
        self.scrollback.len()
    }

    /// Maximum valid scroll offset from the live bottom of the terminal.
    pub fn max_scroll_offset(&self) -> usize {
        self.scrollback.len()
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

    /// Iterate over the current viewport rows for a given scroll offset.
    ///
    /// `scroll_offset = 0` returns the live bottom viewport (current screen rows).
    /// Larger offsets move the viewport upward into scrollback.
    pub fn viewport_rows(&self, scroll_offset: usize) -> impl Iterator<Item = &[Cell]> {
        let offset = scroll_offset.min(self.max_scroll_offset());
        let total_rows = self.scrollback.len() + self.cells.len();
        let start = total_rows.saturating_sub(self.rows + offset);
        let end = (start + self.rows).min(total_rows);

        (start..end).map(move |index| {
            if index < self.scrollback.len() {
                self.scrollback[index].as_slice()
            } else {
                self.cells[index - self.scrollback.len()].as_slice()
            }
        })
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
        for row in &mut self.scrollback {
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
            let removed = self.cells.remove(0);
            self.push_scrollback_line(removed);
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

    fn push_scrollback_line(&mut self, line: Vec<Cell>) {
        if self.scrollback_limit == 0 {
            return;
        }

        if self.scrollback.len() >= self.scrollback_limit {
            self.scrollback.pop_front();
        }
        self.scrollback.push_back(line);
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

    fn queue_response(&mut self, bytes: &[u8]) {
        self.response_bytes.extend_from_slice(bytes);
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

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _: bool, action: char) {
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
            'n' => {
                // DSR (Device Status Report) queries.
                // - CSI 5 n  => "terminal OK"
                // - CSI 6 n  => report cursor position (row;col are 1-based)
                // Some apps send private DSR as CSI ? 6 n.
                let query = csi_param(params, 0, 0);
                let is_private = intermediates.contains(&b'?');
                match query {
                    5 => self.queue_response(b"\x1b[0n"),
                    6 => {
                        let row = self.cursor_row + 1;
                        let col = self.cursor_col + 1;
                        let response = if is_private {
                            format!("\x1b[?{row};{col}R")
                        } else {
                            format!("\x1b[{row};{col}R")
                        };
                        self.queue_response(response.as_bytes());
                    }
                    _ => {}
                }
            }
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

    fn cells_to_text(cells: &[Cell]) -> String {
        cells.iter().map(|cell| cell.ch).collect()
    }

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

    #[test]
    fn scrollback_accumulates_scrolled_lines_with_limit() {
        let mut screen = ScreenBuffer::new_with_scrollback(4, 2, 2);
        screen.write(b"L1aa\r\nL2bb\r\nL3cc\r\nL4dd");

        assert_eq!(screen.scrollback_len(), 2);

        let top_viewport: Vec<String> = screen
            .viewport_rows(screen.max_scroll_offset())
            .map(cells_to_text)
            .collect();
        assert_eq!(top_viewport, vec!["L1aa".to_string(), "L2bb".to_string()]);
    }

    #[test]
    fn viewport_rows_follow_scroll_offset() {
        let mut screen = ScreenBuffer::new_with_scrollback(4, 2, 10);
        screen.write(b"L1aa\r\nL2bb\r\nL3cc\r\nL4dd");

        let live: Vec<String> = screen.viewport_rows(0).map(cells_to_text).collect();
        assert_eq!(live, vec!["L3cc".to_string(), "L4dd".to_string()]);

        let offset_one: Vec<String> = screen.viewport_rows(1).map(cells_to_text).collect();
        assert_eq!(offset_one, vec!["L2bb".to_string(), "L3cc".to_string()]);

        let clamped_top: Vec<String> = screen.viewport_rows(99).map(cells_to_text).collect();
        assert_eq!(clamped_top, vec!["L1aa".to_string(), "L2bb".to_string()]);
    }

    #[test]
    fn responds_to_dsr_cursor_position_query() {
        let mut screen = ScreenBuffer::new(10, 3);
        screen.write(b"abc");
        let response = screen.write_with_responses(b"\x1b[6n");
        assert_eq!(response, b"\x1b[1;4R".to_vec());
    }

    #[test]
    fn responds_to_private_dsr_cursor_position_query() {
        let mut screen = ScreenBuffer::new(10, 3);
        screen.write(b"abc");
        let response = screen.write_with_responses(b"\x1b[?6n");
        assert_eq!(response, b"\x1b[?1;4R".to_vec());
    }
}
