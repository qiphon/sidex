//! Terminal emulator — ANSI escape sequence parser using the `vte` crate.
//!
//! Processes byte streams from a PTY and updates a [`TerminalGrid`] accordingly.
//! Implements the `vte::Perform` trait to handle characters, control codes,
//! CSI sequences (cursor movement, erase, SGR), OSC sequences, and ESC sequences.

use crate::grid::{Cell, CellAttributes, Color, NamedColor, TerminalGrid};

/// A terminal emulator that feeds PTY output bytes through a VTE parser
/// and updates the backing grid.
#[allow(clippy::struct_excessive_bools)]
pub struct TerminalEmulator {
    grid: TerminalGrid,
    parser: vte::Parser,
    pen: Cell,
    saved_cursor: (u16, u16),
    title: String,
    cursor_visible: bool,
    auto_wrap: bool,
    origin_mode: bool,
    application_cursor_keys: bool,
    bracketed_paste: bool,
    mouse_tracking: MouseTracking,
    focus_events: bool,
    alternate_grid: Option<TerminalGrid>,
    alternate_saved_cursor: (u16, u16),
    current_hyperlink: Option<String>,
}

/// Mouse tracking modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseTracking {
    None,
    X10,
    Normal,
    ButtonEvent,
    AnyEvent,
}

/// Mouse encoding modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEncoding {
    Default,
    Sgr,
}

impl TerminalEmulator {
    pub fn new(grid: TerminalGrid) -> Self {
        Self {
            grid,
            parser: vte::Parser::new(),
            pen: Cell::default(),
            saved_cursor: (0, 0),
            title: String::new(),
            cursor_visible: true,
            auto_wrap: true,
            origin_mode: false,
            application_cursor_keys: false,
            bracketed_paste: false,
            mouse_tracking: MouseTracking::None,
            focus_events: false,
            alternate_grid: None,
            alternate_saved_cursor: (0, 0),
            current_hyperlink: None,
        }
    }

    pub fn process(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            let mut performer = Performer {
                grid: &mut self.grid,
                pen: &mut self.pen,
                saved_cursor: &mut self.saved_cursor,
                title: &mut self.title,
                cursor_visible: &mut self.cursor_visible,
                auto_wrap: &mut self.auto_wrap,
                origin_mode: &mut self.origin_mode,
                application_cursor_keys: &mut self.application_cursor_keys,
                bracketed_paste: &mut self.bracketed_paste,
                mouse_tracking: &mut self.mouse_tracking,
                focus_events: &mut self.focus_events,
                alternate_grid: &mut self.alternate_grid,
                alternate_saved_cursor: &mut self.alternate_saved_cursor,
                current_hyperlink: &mut self.current_hyperlink,
            };
            self.parser.advance(&mut performer, byte);
        }
    }

    pub fn grid(&self) -> &TerminalGrid {
        &self.grid
    }
    pub fn grid_mut(&mut self) -> &mut TerminalGrid {
        &mut self.grid
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn pen(&self) -> &Cell {
        &self.pen
    }
    pub fn cursor_visible(&self) -> bool {
        self.cursor_visible
    }
    pub fn auto_wrap(&self) -> bool {
        self.auto_wrap
    }
    pub fn origin_mode(&self) -> bool {
        self.origin_mode
    }
    pub fn application_cursor_keys(&self) -> bool {
        self.application_cursor_keys
    }
    pub fn bracketed_paste(&self) -> bool {
        self.bracketed_paste
    }
    pub fn mouse_tracking(&self) -> MouseTracking {
        self.mouse_tracking
    }
    pub fn focus_events(&self) -> bool {
        self.focus_events
    }
    pub fn is_alternate_screen(&self) -> bool {
        self.alternate_grid.is_some()
    }
}

struct Performer<'a> {
    grid: &'a mut TerminalGrid,
    pen: &'a mut Cell,
    saved_cursor: &'a mut (u16, u16),
    title: &'a mut String,
    cursor_visible: &'a mut bool,
    auto_wrap: &'a mut bool,
    origin_mode: &'a mut bool,
    application_cursor_keys: &'a mut bool,
    bracketed_paste: &'a mut bool,
    mouse_tracking: &'a mut MouseTracking,
    focus_events: &'a mut bool,
    alternate_grid: &'a mut Option<TerminalGrid>,
    alternate_saved_cursor: &'a mut (u16, u16),
    current_hyperlink: &'a mut Option<String>,
}

#[allow(clippy::cast_possible_truncation)]
fn u16_to_u8(v: u16) -> u8 {
    (v & 0xFF) as u8
}

fn named_from_index(idx: u8) -> NamedColor {
    match idx {
        0 => NamedColor::Black,
        1 => NamedColor::Red,
        2 => NamedColor::Green,
        3 => NamedColor::Yellow,
        4 => NamedColor::Blue,
        5 => NamedColor::Magenta,
        6 => NamedColor::Cyan,
        7 => NamedColor::White,
        8 => NamedColor::BrightBlack,
        9 => NamedColor::BrightRed,
        10 => NamedColor::BrightGreen,
        11 => NamedColor::BrightYellow,
        12 => NamedColor::BrightBlue,
        13 => NamedColor::BrightMagenta,
        14 => NamedColor::BrightCyan,
        _ => NamedColor::BrightWhite,
    }
}

impl Performer<'_> {
    fn handle_sgr(&mut self, params: &vte::Params) {
        let mut iter = params.iter();
        while let Some(param) = iter.next() {
            let code = param[0];
            match code {
                0 => {
                    *self.pen = Cell::default();
                    self.pen.hyperlink.clone_from(self.current_hyperlink);
                }
                1 => self.pen.attrs |= CellAttributes::BOLD,
                2 => self.pen.attrs |= CellAttributes::DIM,
                3 => self.pen.attrs |= CellAttributes::ITALIC,
                4 => {
                    self.pen.attrs |= CellAttributes::UNDERLINE;
                    self.pen.attrs.remove(
                        CellAttributes::DOUBLE_UNDERLINE
                            | CellAttributes::CURLY_UNDERLINE
                            | CellAttributes::DOTTED_UNDERLINE
                            | CellAttributes::DASHED_UNDERLINE,
                    );
                    if param.len() > 1 {
                        match param[1] {
                            2 => {
                                self.pen.attrs.remove(CellAttributes::UNDERLINE);
                                self.pen.attrs |= CellAttributes::DOUBLE_UNDERLINE;
                            }
                            3 => {
                                self.pen.attrs.remove(CellAttributes::UNDERLINE);
                                self.pen.attrs |= CellAttributes::CURLY_UNDERLINE;
                            }
                            4 => {
                                self.pen.attrs.remove(CellAttributes::UNDERLINE);
                                self.pen.attrs |= CellAttributes::DOTTED_UNDERLINE;
                            }
                            5 => {
                                self.pen.attrs.remove(CellAttributes::UNDERLINE);
                                self.pen.attrs |= CellAttributes::DASHED_UNDERLINE;
                            }
                            _ => {}
                        }
                    }
                }
                5 => self.pen.attrs |= CellAttributes::BLINK,
                7 => self.pen.attrs |= CellAttributes::INVERSE,
                8 => self.pen.attrs |= CellAttributes::HIDDEN,
                9 => self.pen.attrs |= CellAttributes::STRIKETHROUGH,
                21 => {
                    self.pen.attrs.remove(CellAttributes::UNDERLINE);
                    self.pen.attrs |= CellAttributes::DOUBLE_UNDERLINE;
                }
                22 => self
                    .pen
                    .attrs
                    .remove(CellAttributes::BOLD | CellAttributes::DIM),
                23 => self.pen.attrs.remove(CellAttributes::ITALIC),
                24 => self.pen.attrs.remove(
                    CellAttributes::UNDERLINE
                        | CellAttributes::DOUBLE_UNDERLINE
                        | CellAttributes::CURLY_UNDERLINE
                        | CellAttributes::DOTTED_UNDERLINE
                        | CellAttributes::DASHED_UNDERLINE,
                ),
                25 => self.pen.attrs.remove(CellAttributes::BLINK),
                27 => self.pen.attrs.remove(CellAttributes::INVERSE),
                28 => self.pen.attrs.remove(CellAttributes::HIDDEN),
                29 => self.pen.attrs.remove(CellAttributes::STRIKETHROUGH),
                30..=37 => self.pen.fg = Color::Named(named_from_index(u16_to_u8(code - 30))),
                38 => self.parse_extended_color(&mut iter, true),
                39 => self.pen.fg = Color::Default,
                40..=47 => self.pen.bg = Color::Named(named_from_index(u16_to_u8(code - 40))),
                48 => self.parse_extended_color(&mut iter, false),
                49 => self.pen.bg = Color::Default,
                53 => self.pen.attrs |= CellAttributes::OVERLINE,
                55 => self.pen.attrs.remove(CellAttributes::OVERLINE),
                90..=97 => self.pen.fg = Color::Named(named_from_index(u16_to_u8(code - 90 + 8))),
                100..=107 => {
                    self.pen.bg = Color::Named(named_from_index(u16_to_u8(code - 100 + 8)));
                }
                _ => {}
            }
        }
    }

    fn parse_extended_color<'b>(
        &mut self,
        iter: &mut impl Iterator<Item = &'b [u16]>,
        foreground: bool,
    ) {
        let Some(kind) = iter.next() else { return };
        match kind[0] {
            5 => {
                if let Some(idx) = iter.next() {
                    let color = Color::Indexed(u16_to_u8(idx[0]));
                    if foreground {
                        self.pen.fg = color;
                    } else {
                        self.pen.bg = color;
                    }
                }
            }
            2 => {
                let r = iter.next().map_or(0, |p| u16_to_u8(p[0]));
                let g = iter.next().map_or(0, |p| u16_to_u8(p[0]));
                let b = iter.next().map_or(0, |p| u16_to_u8(p[0]));
                let color = Color::Rgb(r, g, b);
                if foreground {
                    self.pen.fg = color;
                } else {
                    self.pen.bg = color;
                }
            }
            _ => {}
        }
    }

    fn enter_alternate_screen(&mut self) {
        if self.alternate_grid.is_some() {
            return;
        }
        let rows = self.grid.rows();
        let cols = self.grid.cols();
        let main = std::mem::replace(self.grid, TerminalGrid::new(rows, cols));
        *self.alternate_grid = Some(main);
        *self.alternate_saved_cursor =
            (self.grid.cursor_position().0, self.grid.cursor_position().1);
    }

    fn exit_alternate_screen(&mut self) {
        if let Some(main) = self.alternate_grid.take() {
            *self.grid = main;
            let (r, c) = *self.alternate_saved_cursor;
            self.grid.set_cursor(r, c);
        }
    }

    fn handle_dec_set(&mut self, params: &vte::Params) {
        for param in params {
            match param[0] {
                1 => *self.application_cursor_keys = true,
                6 => *self.origin_mode = true,
                7 => *self.auto_wrap = true,
                25 => *self.cursor_visible = true,
                1000 => *self.mouse_tracking = MouseTracking::Normal,
                1002 => *self.mouse_tracking = MouseTracking::ButtonEvent,
                1003 => *self.mouse_tracking = MouseTracking::AnyEvent,
                1004 => *self.focus_events = true,
                1049 => {
                    self.enter_alternate_screen();
                    *self.saved_cursor = self.grid.cursor_position();
                }
                2004 => *self.bracketed_paste = true,
                9 => *self.mouse_tracking = MouseTracking::X10,
                47 | 1047 => self.enter_alternate_screen(),
                _ => {}
            }
        }
    }

    fn handle_dec_rst(&mut self, params: &vte::Params) {
        for param in params {
            match param[0] {
                1 => *self.application_cursor_keys = false,
                6 => *self.origin_mode = false,
                7 => *self.auto_wrap = false,
                25 => *self.cursor_visible = false,
                1000 | 1002 | 1003 | 9 => *self.mouse_tracking = MouseTracking::None,
                1004 => *self.focus_events = false,
                1049 => {
                    self.exit_alternate_screen();
                }
                2004 => *self.bracketed_paste = false,
                47 | 1047 => self.exit_alternate_screen(),
                _ => {}
            }
        }
    }
}

impl vte::Perform for Performer<'_> {
    fn print(&mut self, c: char) {
        self.grid.write_char(c, self.pen);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => {
                let (row, _col) = self.grid.cursor_position();
                let (_top, bottom) = self.grid.scroll_region();
                if row >= bottom {
                    self.grid.scroll_up();
                } else {
                    self.grid.set_cursor(row + 1, self.grid.cursor_position().1);
                }
            }
            b'\r' => {
                let (row, _col) = self.grid.cursor_position();
                self.grid.set_cursor(row, 0);
            }
            0x08 => {
                let (row, col) = self.grid.cursor_position();
                if col > 0 {
                    self.grid.set_cursor(row, col - 1);
                }
            }
            b'\t' => {
                let (row, col) = self.grid.cursor_position();
                let next_tab = self.grid.next_tab_stop(col);
                self.grid.set_cursor(row, next_tab);
            }
            0x07 => {
                log::trace!("BEL");
            }
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        if intermediates == [b'?'] {
            match action {
                'h' => {
                    self.handle_dec_set(params);
                    return;
                }
                'l' => {
                    self.handle_dec_rst(params);
                    return;
                }
                _ => {
                    return;
                }
            }
        }

        let first = params.iter().next().map_or(1, |p| p[0].max(1));
        let (row, col) = self.grid.cursor_position();

        match action {
            'A' => self.grid.set_cursor(row.saturating_sub(first), col), // CUU
            'B' => self.grid.set_cursor(row + first, col),               // CUD
            'C' => self.grid.set_cursor(row, col + first),               // CUF
            'D' => self.grid.set_cursor(row, col.saturating_sub(first)), // CUB
            'G' => {
                // CHA
                let c = params.iter().next().map_or(1, |p| p[0].max(1));
                self.grid.set_cursor(row, c - 1);
            }
            'd' => {
                // VPA
                let r = params.iter().next().map_or(1, |p| p[0].max(1));
                self.grid.set_cursor(r - 1, col);
            }
            'H' | 'f' => {
                // CUP / HVP
                let mut piter = params.iter();
                let r = piter.next().map_or(1, |p| p[0].max(1));
                let c = piter.next().map_or(1, |p| p[0].max(1));
                self.grid.set_cursor(r - 1, c - 1);
            }
            'J' => {
                // ED
                let mode = params.iter().next().map_or(0, |p| p[0]);
                match mode {
                    0 => self.grid.clear_below(),
                    1 => self.grid.clear_above(),
                    2 | 3 => self.grid.clear(),
                    _ => {}
                }
            }
            'K' => {
                // EL
                let mode = params.iter().next().map_or(0, |p| p[0]);
                match mode {
                    0 => self.grid.clear_line_from_cursor(),
                    1 => self.grid.clear_line_to_cursor(),
                    2 => self.grid.clear_line(row),
                    _ => {}
                }
            }
            'X' => self.grid.erase_chars(first),  // ECH
            '@' => self.grid.insert_chars(first), // ICH
            'P' => self.grid.delete_chars(first), // DCH
            'L' => self.grid.insert_lines(first), // IL
            'M' => self.grid.delete_lines(first), // DL
            'm' => self.handle_sgr(params),       // SGR
            'S' => {
                // SU
                for _ in 0..first {
                    self.grid.scroll_up();
                }
            }
            'T' => {
                // SD
                for _ in 0..first {
                    self.grid.scroll_down();
                }
            }
            'r' => {
                // DECSTBM
                let mut piter = params.iter();
                let top = piter.next().map_or(1, |p| p[0].max(1));
                let bottom = piter.next().map_or(self.grid.rows(), |p| p[0].max(1));
                self.grid.set_scroll_region(top - 1, bottom - 1);
                self.grid.set_cursor(0, 0);
            }
            _ => {
                log::trace!("unhandled CSI: {action}");
            }
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.is_empty() {
            return;
        }
        let code = params[0];
        match code {
            b"0" | b"2" if params.len() >= 2 => {
                *self.title = String::from_utf8_lossy(params[1]).into_owned();
            }
            b"8" if params.len() >= 3 => {
                let url = String::from_utf8_lossy(params[2]).into_owned();
                if url.is_empty() {
                    *self.current_hyperlink = None;
                    self.pen.hyperlink = None;
                } else {
                    *self.current_hyperlink = Some(url.clone());
                    self.pen.hyperlink = Some(url);
                }
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        match (byte, intermediates) {
            (b'7', _) => {
                *self.saved_cursor = self.grid.cursor_position();
            }
            (b'8', _) => {
                let (r, c) = *self.saved_cursor;
                self.grid.set_cursor(r, c);
            }
            (b'M', _) => {
                let (row_, _col) = self.grid.cursor_position();
                let (top, _bottom) = self.grid.scroll_region();
                if row_ == top {
                    self.grid.scroll_down();
                } else {
                    self.grid
                        .set_cursor(row_.saturating_sub(1), self.grid.cursor_position().1);
                }
            }
            _ => {
                log::trace!("unhandled ESC: 0x{byte:02x}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_emulator(rows: u16, cols: u16) -> TerminalEmulator {
        TerminalEmulator::new(TerminalGrid::new(rows, cols))
    }

    #[test]
    fn simple_text_output() {
        let mut emu = make_emulator(24, 80);
        emu.process(b"Hello, world!");
        assert_eq!(emu.grid().row_text(0), "Hello, world!");
        assert_eq!(emu.grid().cursor_position(), (0, 13));
    }

    #[test]
    fn newline_and_carriage_return() {
        let mut emu = make_emulator(24, 80);
        emu.process(b"Line1\r\nLine2");
        assert_eq!(emu.grid().row_text(0), "Line1");
        assert_eq!(emu.grid().row_text(1), "Line2");
    }

    #[test]
    fn cursor_movement_csi() {
        let mut emu = make_emulator(24, 80);
        emu.process(b"\x1b[3;5H*");
        assert_eq!(emu.grid().cell(2, 4).c, '*');
    }

    #[test]
    fn erase_display_below() {
        let mut emu = make_emulator(4, 10);
        emu.process(b"AAAAAAAAAA");
        emu.process(b"\r\nBBBBBBBBBB");
        emu.process(b"\x1b[1;6H\x1b[0J");
        assert_eq!(emu.grid().row_text(0), "AAAAA");
        assert_eq!(emu.grid().row_text(1), "");
    }

    #[test]
    fn erase_entire_line() {
        let mut emu = make_emulator(4, 10);
        emu.process(b"Hello");
        emu.process(b"\x1b[2K");
        assert_eq!(emu.grid().row_text(0), "");
    }

    #[test]
    fn sgr_bold_and_reset() {
        let mut emu = make_emulator(24, 80);
        emu.process(b"\x1b[1mBold\x1b[0m");
        assert!(emu.grid().cell(0, 0).bold());
        assert_eq!(emu.grid().cell(0, 0).c, 'B');
        assert!(!emu.pen().bold());
    }

    #[test]
    fn sgr_italic_underline_strikethrough() {
        let mut emu = make_emulator(24, 80);
        emu.process(b"\x1b[3;4;9mX");
        let cell = emu.grid().cell(0, 0);
        assert!(cell.italic());
        assert!(cell.underline());
        assert!(cell.strikethrough());
    }

    #[test]
    fn sgr_dim_blink_inverse_hidden_overline() {
        let mut emu = make_emulator(24, 80);
        emu.process(b"\x1b[2;5;7;8;53mX");
        let cell = emu.grid().cell(0, 0);
        assert!(cell.dim());
        assert!(cell.blink());
        assert!(cell.inverse());
        assert!(cell.hidden());
        assert!(cell.attrs.contains(CellAttributes::OVERLINE));
    }

    #[test]
    fn sgr_standard_foreground_color() {
        let mut emu = make_emulator(24, 80);
        emu.process(b"\x1b[31mR");
        assert_eq!(emu.grid().cell(0, 0).fg, Color::Named(NamedColor::Red));
    }

    #[test]
    fn sgr_256_color() {
        let mut emu = make_emulator(24, 80);
        emu.process(b"\x1b[38;5;200mX");
        assert_eq!(emu.grid().cell(0, 0).fg, Color::Indexed(200));
    }

    #[test]
    fn sgr_truecolor() {
        let mut emu = make_emulator(24, 80);
        emu.process(b"\x1b[38;2;100;150;200mX");
        assert_eq!(emu.grid().cell(0, 0).fg, Color::Rgb(100, 150, 200));
    }

    #[test]
    fn sgr_background_truecolor() {
        let mut emu = make_emulator(24, 80);
        emu.process(b"\x1b[48;2;10;20;30mX");
        assert_eq!(emu.grid().cell(0, 0).bg, Color::Rgb(10, 20, 30));
    }

    #[test]
    fn sgr_bright_colors() {
        let mut emu = make_emulator(24, 80);
        emu.process(b"\x1b[91mX");
        assert_eq!(
            emu.grid().cell(0, 0).fg,
            Color::Named(NamedColor::BrightRed)
        );
    }

    #[test]
    fn osc_set_title() {
        let mut emu = make_emulator(24, 80);
        emu.process(b"\x1b]0;My Terminal\x07");
        assert_eq!(emu.title(), "My Terminal");
    }

    #[test]
    fn backspace_moves_cursor_back() {
        let mut emu = make_emulator(24, 80);
        emu.process(b"AB\x08C");
        assert_eq!(emu.grid().row_text(0), "AC");
    }

    #[test]
    fn tab_advances_to_next_stop() {
        let mut emu = make_emulator(24, 80);
        emu.process(b"A\tB");
        assert_eq!(emu.grid().cursor_position().1, 9);
        assert_eq!(emu.grid().cell(0, 8).c, 'B');
    }

    #[test]
    fn save_restore_cursor() {
        let mut emu = make_emulator(24, 80);
        emu.process(b"\x1b[5;10H");
        emu.process(b"\x1b7");
        emu.process(b"\x1b[1;1H");
        emu.process(b"\x1b8");
        assert_eq!(emu.grid().cursor_position(), (4, 9));
    }

    #[test]
    fn scroll_region_and_scroll_up() {
        let mut emu = make_emulator(5, 10);
        emu.process(b"\x1b[2;4r");
        assert_eq!(emu.grid().scroll_region(), (1, 3));
    }

    #[test]
    fn clear_display_mode_2() {
        let mut emu = make_emulator(4, 10);
        emu.process(b"Hello");
        emu.process(b"\r\nWorld");
        emu.process(b"\x1b[2J");
        assert_eq!(emu.grid().row_text(0), "");
        assert_eq!(emu.grid().row_text(1), "");
    }

    #[test]
    fn scrollback_on_scroll() {
        let mut emu = make_emulator(3, 10);
        emu.process(b"Line1\r\nLine2\r\nLine3\r\nLine4");
        assert!(emu.grid().scrollback_len() >= 1);
    }

    #[test]
    fn cursor_visibility_dec_mode() {
        let mut emu = make_emulator(24, 80);
        assert!(emu.cursor_visible());
        emu.process(b"\x1b[?25l");
        assert!(!emu.cursor_visible());
        emu.process(b"\x1b[?25h");
        assert!(emu.cursor_visible());
    }

    #[test]
    fn bracketed_paste_mode() {
        let mut emu = make_emulator(24, 80);
        assert!(!emu.bracketed_paste());
        emu.process(b"\x1b[?2004h");
        assert!(emu.bracketed_paste());
        emu.process(b"\x1b[?2004l");
        assert!(!emu.bracketed_paste());
    }

    #[test]
    fn alternate_screen_buffer() {
        let mut emu = make_emulator(24, 80);
        emu.process(b"Main screen");
        assert!(!emu.is_alternate_screen());
        emu.process(b"\x1b[?1049h");
        assert!(emu.is_alternate_screen());
        emu.process(b"Alt screen");
        assert_eq!(emu.grid().row_text(0), "Alt screen");
        emu.process(b"\x1b[?1049l");
        assert!(!emu.is_alternate_screen());
        assert_eq!(emu.grid().row_text(0), "Main screen");
    }

    #[test]
    fn csi_cha_and_vpa() {
        let mut emu = make_emulator(24, 80);
        emu.process(b"\x1b[5;5H");
        emu.process(b"\x1b[10G");
        assert_eq!(emu.grid().cursor_position(), (4, 9));
        emu.process(b"\x1b[3d");
        assert_eq!(emu.grid().cursor_position(), (2, 9));
    }

    #[test]
    fn insert_delete_chars() {
        let mut emu = make_emulator(4, 10);
        emu.process(b"ABCDE");
        emu.process(b"\x1b[1;3H");
        emu.process(b"\x1b[1P");
        assert_eq!(emu.grid().row_text(0), "ABDE");
    }

    #[test]
    fn erase_chars() {
        let mut emu = make_emulator(4, 10);
        emu.process(b"ABCDE");
        emu.process(b"\x1b[1;2H");
        emu.process(b"\x1b[2X");
        assert_eq!(emu.grid().row_text(0), "A  DE");
    }
}
