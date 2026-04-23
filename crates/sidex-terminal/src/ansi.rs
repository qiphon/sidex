//! Complete ANSI escape sequence parser.
//!
//! Provides a standalone state-machine parser for VT100/VT220/xterm escape
//! sequences. Emits [`AnsiAction`] values that can be interpreted by a
//! terminal emulator to drive a [`TerminalGrid`].

/// Parser states for the ANSI state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserState {
    Ground,
    Escape,
    EscapeIntermediate,
    CsiEntry,
    CsiParam,
    CsiIntermediate,
    OscString,
    DcsEntry,
    DcsParam,
    DcsPassthrough,
}

/// An OSC (Operating System Command) parsed from `\x1b]...\x07` or `\x1b]...\x1b\\`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OscCommand {
    SetTitle(String),
    SetIconName(String),
    SetTitleAndIcon(String),
    SetClipboard(String),
    SetHyperlink {
        uri: Option<String>,
        id: Option<String>,
    },
    SetColor {
        index: u8,
        color: String,
    },
    ResetColor(u8),
    SetForeground(String),
    SetBackground(String),
    ResetForeground,
    ResetBackground,
    Unknown(Vec<String>),
}

/// Actions emitted by the ANSI parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnsiAction {
    /// A printable character.
    Print(char),
    /// A C0 control code (BEL, BS, HT, LF, VT, FF, CR, etc.).
    Execute(u8),
    /// A CSI (Control Sequence Introducer) dispatch.
    CsiDispatch {
        params: Vec<u16>,
        intermediates: Vec<u8>,
        action: char,
    },
    /// An OSC (Operating System Command) dispatch.
    OscDispatch(OscCommand),
    /// An ESC dispatch.
    EscDispatch {
        intermediates: Vec<u8>,
        action: char,
    },
    /// A DCS (Device Control String) dispatch.
    DcsDispatch { params: Vec<u16>, data: Vec<u8> },
}

/// A standalone ANSI escape sequence parser.
///
/// Feed bytes via [`advance`](AnsiParser::advance) and collect the resulting
/// actions.
pub struct AnsiParser {
    pub state: ParserState,
    pub params: Vec<u16>,
    pub intermediates: Vec<u8>,
    pub osc_buffer: String,
    current_param: u16,
    has_param: bool,
    dcs_data: Vec<u8>,
}

impl Default for AnsiParser {
    fn default() -> Self {
        Self::new()
    }
}

impl AnsiParser {
    pub fn new() -> Self {
        Self {
            state: ParserState::Ground,
            params: Vec::new(),
            intermediates: Vec::new(),
            osc_buffer: String::new(),
            current_param: 0,
            has_param: false,
            dcs_data: Vec::new(),
        }
    }

    /// Feeds a single byte into the parser and returns an optional action.
    pub fn advance(&mut self, byte: u8) -> Option<AnsiAction> {
        match self.state {
            ParserState::Ground => self.ground(byte),
            ParserState::Escape => self.escape(byte),
            ParserState::EscapeIntermediate => self.escape_intermediate(byte),
            ParserState::CsiEntry => self.csi_entry(byte),
            ParserState::CsiParam => self.csi_param(byte),
            ParserState::CsiIntermediate => self.csi_intermediate(byte),
            ParserState::OscString => self.osc_string(byte),
            ParserState::DcsEntry => self.dcs_entry(byte),
            ParserState::DcsParam => self.dcs_param(byte),
            ParserState::DcsPassthrough => self.dcs_passthrough(byte),
        }
    }

    /// Feeds a slice of bytes, collecting all actions.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<AnsiAction> {
        let mut actions = Vec::new();
        for &b in bytes {
            if let Some(action) = self.advance(b) {
                actions.push(action);
            }
        }
        actions
    }

    fn reset(&mut self) {
        self.state = ParserState::Ground;
        self.params.clear();
        self.intermediates.clear();
        self.osc_buffer.clear();
        self.current_param = 0;
        self.has_param = false;
        self.dcs_data.clear();
    }

    fn finish_param(&mut self) {
        if self.has_param {
            self.params.push(self.current_param);
        }
        self.current_param = 0;
        self.has_param = false;
    }

    // --- State handlers ---

    fn ground(&mut self, byte: u8) -> Option<AnsiAction> {
        match byte {
            0x1B => {
                self.state = ParserState::Escape;
                self.params.clear();
                self.intermediates.clear();
                None
            }
            0x00..=0x1A | 0x1C..=0x1F => Some(AnsiAction::Execute(byte)),
            0x20..=0x7E => Some(AnsiAction::Print(byte as char)),
            0x80..=0xFF => {
                // UTF-8 high bytes — treat as printable for simplicity.
                // A real implementation would accumulate multi-byte sequences.
                Some(AnsiAction::Print(byte as char))
            }
            _ => None,
        }
    }

    fn escape(&mut self, byte: u8) -> Option<AnsiAction> {
        match byte {
            b'[' => {
                self.state = ParserState::CsiEntry;
                self.params.clear();
                self.current_param = 0;
                self.has_param = false;
                None
            }
            b']' => {
                self.state = ParserState::OscString;
                self.osc_buffer.clear();
                None
            }
            b'P' => {
                self.state = ParserState::DcsEntry;
                self.params.clear();
                self.current_param = 0;
                self.has_param = false;
                self.dcs_data.clear();
                None
            }
            0x20..=0x2F => {
                self.intermediates.push(byte);
                self.state = ParserState::EscapeIntermediate;
                None
            }
            0x30..=0x7E => {
                let action = AnsiAction::EscDispatch {
                    intermediates: self.intermediates.clone(),
                    action: byte as char,
                };
                self.reset();
                Some(action)
            }
            0x1B => None, // ESC ESC — stay in escape
            _ => {
                self.reset();
                None
            }
        }
    }

    fn escape_intermediate(&mut self, byte: u8) -> Option<AnsiAction> {
        match byte {
            0x20..=0x2F => {
                self.intermediates.push(byte);
                None
            }
            0x30..=0x7E => {
                let action = AnsiAction::EscDispatch {
                    intermediates: self.intermediates.clone(),
                    action: byte as char,
                };
                self.reset();
                Some(action)
            }
            _ => {
                self.reset();
                None
            }
        }
    }

    fn csi_entry(&mut self, byte: u8) -> Option<AnsiAction> {
        match byte {
            b'0'..=b'9' => {
                self.current_param = u16::from(byte - b'0');
                self.has_param = true;
                self.state = ParserState::CsiParam;
                None
            }
            b';' => {
                self.params.push(0);
                self.state = ParserState::CsiParam;
                None
            }
            b'?' | b'>' | b'!' => {
                self.intermediates.push(byte);
                self.state = ParserState::CsiParam;
                None
            }
            0x20..=0x2F => {
                self.intermediates.push(byte);
                self.state = ParserState::CsiIntermediate;
                None
            }
            0x40..=0x7E => {
                let action = AnsiAction::CsiDispatch {
                    params: self.params.clone(),
                    intermediates: self.intermediates.clone(),
                    action: byte as char,
                };
                self.reset();
                Some(action)
            }
            _ => {
                self.reset();
                None
            }
        }
    }

    fn csi_param(&mut self, byte: u8) -> Option<AnsiAction> {
        match byte {
            b'0'..=b'9' => {
                self.current_param = self
                    .current_param
                    .saturating_mul(10)
                    .saturating_add(u16::from(byte - b'0'));
                self.has_param = true;
                None
            }
            b';' => {
                self.finish_param();
                None
            }
            b':' => {
                // Sub-parameter separator (used in SGR extended underline, etc.)
                self.finish_param();
                None
            }
            0x20..=0x2F => {
                self.finish_param();
                self.intermediates.push(byte);
                self.state = ParserState::CsiIntermediate;
                None
            }
            0x40..=0x7E => {
                self.finish_param();
                let action = AnsiAction::CsiDispatch {
                    params: self.params.clone(),
                    intermediates: self.intermediates.clone(),
                    action: byte as char,
                };
                self.reset();
                Some(action)
            }
            _ => {
                self.reset();
                None
            }
        }
    }

    fn csi_intermediate(&mut self, byte: u8) -> Option<AnsiAction> {
        match byte {
            0x20..=0x2F => {
                self.intermediates.push(byte);
                None
            }
            0x40..=0x7E => {
                let action = AnsiAction::CsiDispatch {
                    params: self.params.clone(),
                    intermediates: self.intermediates.clone(),
                    action: byte as char,
                };
                self.reset();
                Some(action)
            }
            _ => {
                self.reset();
                None
            }
        }
    }

    fn osc_string(&mut self, byte: u8) -> Option<AnsiAction> {
        match byte {
            0x1B => {
                // Might be ST (\x1b\\)
                // For simplicity, treat ESC as terminator too
                let cmd = parse_osc(&self.osc_buffer);
                self.reset();
                Some(AnsiAction::OscDispatch(cmd))
            }
            0x07 | 0x9C => {
                let cmd = parse_osc(&self.osc_buffer);
                self.reset();
                Some(AnsiAction::OscDispatch(cmd))
            }
            _ => {
                self.osc_buffer.push(byte as char);
                None
            }
        }
    }

    fn dcs_entry(&mut self, byte: u8) -> Option<AnsiAction> {
        match byte {
            b'0'..=b'9' => {
                self.current_param = u16::from(byte - b'0');
                self.has_param = true;
                self.state = ParserState::DcsParam;
                None
            }
            b';' => {
                self.params.push(0);
                self.state = ParserState::DcsParam;
                None
            }
            _ => {
                self.state = ParserState::DcsPassthrough;
                self.dcs_data.push(byte);
                None
            }
        }
    }

    fn dcs_param(&mut self, byte: u8) -> Option<AnsiAction> {
        match byte {
            b'0'..=b'9' => {
                self.current_param = self
                    .current_param
                    .saturating_mul(10)
                    .saturating_add(u16::from(byte - b'0'));
                self.has_param = true;
                None
            }
            b';' => {
                self.finish_param();
                None
            }
            _ => {
                self.finish_param();
                self.state = ParserState::DcsPassthrough;
                self.dcs_data.push(byte);
                None
            }
        }
    }

    fn dcs_passthrough(&mut self, byte: u8) -> Option<AnsiAction> {
        match byte {
            0x1B | 0x9C => {
                let action = AnsiAction::DcsDispatch {
                    params: self.params.clone(),
                    data: self.dcs_data.clone(),
                };
                self.reset();
                Some(action)
            }
            _ => {
                self.dcs_data.push(byte);
                None
            }
        }
    }
}

fn parse_osc(buffer: &str) -> OscCommand {
    let parts: Vec<&str> = buffer.splitn(2, ';').collect();
    let code = parts.first().copied().unwrap_or("");
    let payload = parts.get(1).copied().unwrap_or("");

    match code {
        "0" => OscCommand::SetTitleAndIcon(payload.to_string()),
        "1" => OscCommand::SetIconName(payload.to_string()),
        "2" => OscCommand::SetTitle(payload.to_string()),
        "8" => {
            let osc8_parts: Vec<&str> = payload.splitn(2, ';').collect();
            let params_str = osc8_parts.first().copied().unwrap_or("");
            let uri_str = osc8_parts.get(1).copied().unwrap_or("");
            let id = params_str
                .split(':')
                .find_map(|p| p.strip_prefix("id="))
                .map(String::from);
            let uri = if uri_str.is_empty() {
                None
            } else {
                Some(uri_str.to_string())
            };
            OscCommand::SetHyperlink { uri, id }
        }
        "10" => OscCommand::SetForeground(payload.to_string()),
        "11" => OscCommand::SetBackground(payload.to_string()),
        "52" => OscCommand::SetClipboard(payload.to_string()),
        "110" => OscCommand::ResetForeground,
        "111" => OscCommand::ResetBackground,
        _ => {
            if let Ok(idx) = code.parse::<u8>() {
                if (4..=19).contains(&idx) {
                    if payload.is_empty() || payload == "?" {
                        return OscCommand::ResetColor(idx);
                    }
                    return OscCommand::SetColor {
                        index: idx,
                        color: payload.to_string(),
                    };
                }
            }
            let all: Vec<String> = buffer.split(';').map(String::from).collect();
            OscCommand::Unknown(all)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_text() {
        let mut parser = AnsiParser::new();
        let actions = parser.feed(b"Hello");
        assert_eq!(actions.len(), 5);
        assert_eq!(actions[0], AnsiAction::Print('H'));
        assert_eq!(actions[4], AnsiAction::Print('o'));
    }

    #[test]
    fn parse_csi_cursor_up() {
        let mut parser = AnsiParser::new();
        let actions = parser.feed(b"\x1b[3A");
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            AnsiAction::CsiDispatch {
                params,
                intermediates,
                action,
            } => {
                assert_eq!(params, &[3]);
                assert!(intermediates.is_empty());
                assert_eq!(*action, 'A');
            }
            other => panic!("expected CsiDispatch, got {other:?}"),
        }
    }

    #[test]
    fn parse_csi_sgr_multiple_params() {
        let mut parser = AnsiParser::new();
        let actions = parser.feed(b"\x1b[1;31;42m");
        assert_eq!(actions.len(), 1);
        if let AnsiAction::CsiDispatch { params, action, .. } = &actions[0] {
            assert_eq!(params, &[1, 31, 42]);
            assert_eq!(*action, 'm');
        }
    }

    #[test]
    fn parse_csi_private_mode() {
        let mut parser = AnsiParser::new();
        let actions = parser.feed(b"\x1b[?25l");
        assert_eq!(actions.len(), 1);
        if let AnsiAction::CsiDispatch {
            params,
            intermediates,
            action,
        } = &actions[0]
        {
            assert_eq!(params, &[25]);
            assert_eq!(intermediates, &[b'?']);
            assert_eq!(*action, 'l');
        }
    }

    #[test]
    fn parse_osc_set_title() {
        let mut parser = AnsiParser::new();
        let actions = parser.feed(b"\x1b]0;My Title\x07");
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            AnsiAction::OscDispatch(OscCommand::SetTitleAndIcon(title)) => {
                assert_eq!(title, "My Title");
            }
            other => panic!("expected OscDispatch SetTitleAndIcon, got {other:?}"),
        }
    }

    #[test]
    fn parse_osc_hyperlink() {
        let mut parser = AnsiParser::new();
        let actions = parser.feed(b"\x1b]8;;https://example.com\x07");
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            AnsiAction::OscDispatch(OscCommand::SetHyperlink { uri, id }) => {
                assert_eq!(uri.as_deref(), Some("https://example.com"));
                assert!(id.is_none());
            }
            other => panic!("expected SetHyperlink, got {other:?}"),
        }
    }

    #[test]
    fn parse_esc_save_cursor() {
        let mut parser = AnsiParser::new();
        let actions = parser.feed(b"\x1b7");
        assert_eq!(actions.len(), 1);
        if let AnsiAction::EscDispatch { action, .. } = &actions[0] {
            assert_eq!(*action, '7');
        }
    }

    #[test]
    fn parse_c0_controls() {
        let mut parser = AnsiParser::new();
        let actions = parser.feed(b"\r\n\x08\t");
        assert_eq!(actions.len(), 4);
        assert_eq!(actions[0], AnsiAction::Execute(b'\r'));
        assert_eq!(actions[1], AnsiAction::Execute(b'\n'));
        assert_eq!(actions[2], AnsiAction::Execute(0x08));
        assert_eq!(actions[3], AnsiAction::Execute(b'\t'));
    }

    #[test]
    fn parse_csi_cup() {
        let mut parser = AnsiParser::new();
        let actions = parser.feed(b"\x1b[10;20H");
        assert_eq!(actions.len(), 1);
        if let AnsiAction::CsiDispatch { params, action, .. } = &actions[0] {
            assert_eq!(params, &[10, 20]);
            assert_eq!(*action, 'H');
        }
    }
}
