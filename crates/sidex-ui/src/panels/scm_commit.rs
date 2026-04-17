//! SCM commit message input and action bar widget.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, Modifiers, MouseButton, UiEvent, Widget};

// ── Warning types ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarningSeverity {
    Warning,
    Error,
}

impl WarningSeverity {
    pub fn color(self) -> Color {
        match self {
            Self::Warning => Color::from_rgb(226, 192, 81),
            Self::Error => Color::from_rgb(193, 74, 74),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CommitMessageWarning {
    pub line: u32,
    pub message: String,
    pub severity: WarningSeverity,
}

const SUBJECT_WARN: usize = 72;
const SUBJECT_ERR: usize = 100;
const BODY_LIMIT: usize = 72;

pub fn validate_commit_message(msg: &str) -> Vec<CommitMessageWarning> {
    let mut w = Vec::new();
    let lines: Vec<&str> = msg.lines().collect();
    if lines.is_empty() {
        return w;
    }
    let slen = lines[0].len();
    if slen > SUBJECT_ERR {
        w.push(CommitMessageWarning {
            line: 0,
            message: format!("Subject is {slen} chars (max {SUBJECT_ERR})"),
            severity: WarningSeverity::Error,
        });
    } else if slen > SUBJECT_WARN {
        w.push(CommitMessageWarning {
            line: 0,
            message: format!("Subject is {slen} chars (recommended {SUBJECT_WARN})"),
            severity: WarningSeverity::Warning,
        });
    }
    if lines.len() >= 2 && !lines[1].is_empty() {
        w.push(CommitMessageWarning {
            line: 1,
            message: "Missing blank line after subject".into(),
            severity: WarningSeverity::Warning,
        });
    }
    for (i, line) in lines.iter().enumerate().skip(2) {
        if line.len() > BODY_LIMIT {
            w.push(CommitMessageWarning {
                line: i as u32,
                message: format!("Body line {} is {} chars (recommended {BODY_LIMIT})", i + 1, line.len()),
                severity: WarningSeverity::Warning,
            });
        }
    }
    w
}

// ── Commit options ───────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct CommitOptions {
    pub message: String,
    pub amend: bool,
    pub sign_off: bool,
    pub stage_all: bool,
}

// ── Widget ───────────────────────────────────────────────────────────────────

const MAX_HISTORY: usize = 50;
const MIN_LINES: usize = 3;
const LH: f32 = 18.0;
const ROW_H: f32 = 22.0;
const BTN_H: f32 = 28.0;
const CB_SZ: f32 = 14.0;
const PAD: f32 = 8.0;

fn hex(s: &str) -> Color { Color::from_hex(s).unwrap_or(Color::BLACK) }

/// Commit message input and SCM action bar.
///
/// Multi-line text input with auto-resize, validation indicators,
/// amend/sign-off checkboxes, and commit/stage-all buttons. Ctrl+Enter commits.
#[allow(dead_code)]
pub struct ScmCommitWidget<F: FnMut(CommitOptions)> {
    message: String,
    cursor_pos: usize,
    is_amend: bool,
    is_expanded: bool,
    sign_off: bool,
    on_commit: F,
    focused: bool,
    input_focused: bool,
    stage_all_pending: bool,
    history: Vec<String>,
    history_index: Option<usize>,
    draft_message: String,
}

impl<F: FnMut(CommitOptions)> ScmCommitWidget<F> {
    pub fn new(on_commit: F) -> Self {
        Self {
            message: String::new(), cursor_pos: 0,
            is_amend: false, is_expanded: false, sign_off: false,
            on_commit, focused: false, input_focused: false, stage_all_pending: false,
            history: Vec::new(), history_index: None, draft_message: String::new(),
        }
    }

    pub fn message(&self) -> &str { &self.message }

    pub fn set_message(&mut self, msg: impl Into<String>) {
        self.message = msg.into();
        self.cursor_pos = self.message.len();
    }

    pub fn toggle_amend(&mut self) { self.is_amend = !self.is_amend; }
    pub fn toggle_sign_off(&mut self) { self.sign_off = !self.sign_off; }

    pub fn push_history(&mut self, msg: String) {
        if msg.is_empty() || self.history.last().map_or(false, |l| l == &msg) {
            return;
        }
        self.history.push(msg);
        if self.history.len() > MAX_HISTORY { self.history.remove(0); }
        self.history_index = None;
    }

    fn recall(&mut self, fwd: bool) {
        if !fwd {
            if self.history.is_empty() { return; }
            let idx = match self.history_index {
                None => { self.draft_message = self.message.clone(); self.history.len() - 1 }
                Some(0) => return, Some(i) => i - 1,
            };
            self.history_index = Some(idx);
            self.message = self.history[idx].clone();
        } else {
            let idx = match self.history_index { None => return, Some(i) => i + 1 };
            if idx >= self.history.len() { self.history_index = None; self.message = self.draft_message.clone(); }
            else { self.history_index = Some(idx); self.message = self.history[idx].clone(); }
        }
        self.cursor_pos = self.message.len();
    }

    fn do_commit(&mut self) {
        if self.message.is_empty() { return; }
        let opts = CommitOptions {
            message: self.message.clone(), amend: self.is_amend,
            sign_off: self.sign_off, stage_all: self.stage_all_pending,
        };
        self.push_history(self.message.clone());
        (self.on_commit)(opts);
        self.message.clear();
        self.cursor_pos = 0;
        self.stage_all_pending = false;
    }

    fn input_height(&self) -> f32 {
        let mut n = self.message.lines().count().max(1);
        if self.message.ends_with('\n') { n += 1; }
        (n.max(MIN_LINES) as f32) * LH + 8.0
    }

    fn subject_color(&self) -> Color {
        let len = self.message.lines().next().map_or(0, str::len);
        if len > SUBJECT_ERR { Color::from_rgb(193, 74, 74) }
        else if len > SUBJECT_WARN { Color::from_rgb(226, 192, 81) }
        else { Color::from_hex("#969696").unwrap_or(Color::WHITE) }
    }

    fn total_height(&self) -> f32 {
        self.input_height() + ROW_H * 3.0 + BTN_H * 2.0 + PAD + 20.0
    }
}

impl<F: FnMut(CommitOptions)> Widget for ScmCommitWidget<F> {
    fn layout(&self) -> LayoutNode {
        LayoutNode { size: Size::Fixed(self.total_height()), ..LayoutNode::default() }
    }

    #[allow(clippy::cast_precision_loss)]
    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        let mut rr = sidex_gpu::RectRenderer::new();
        let w = rect.width - PAD * 2.0;
        let mut y = rect.y + 4.0;

        rr.draw_rect(rect.x, rect.y, rect.width, rect.height, hex("#252526"), 0.0);

        let ih = self.input_height();
        let bdr = if self.input_focused { hex("#007fd4") } else { hex("#3c3c3c") };
        rr.draw_rect(rect.x + PAD, y, w, ih, hex("#3c3c3c"), 2.0);
        rr.draw_border(rect.x + PAD, y, w, ih, bdr, 1.0);
        y += ih + 4.0;

        rr.draw_rect(rect.x + PAD, y, 6.0, ROW_H - 6.0, self.subject_color(), 2.0);
        y += ROW_H;

        let draw_cb = |rr: &mut sidex_gpu::RectRenderer, y: f32, checked: bool| {
            let cy = y + (ROW_H - CB_SZ) / 2.0;
            let bg = if checked { hex("#007fd4") } else { hex("#3c3c3c") };
            rr.draw_rect(rect.x + PAD, cy, CB_SZ, CB_SZ, bg, 2.0);
            rr.draw_border(rect.x + PAD, cy, CB_SZ, CB_SZ, hex("#6b6b6b"), 1.0);
        };
        draw_cb(&mut rr, y, self.is_amend);
        y += ROW_H;
        draw_cb(&mut rr, y, self.sign_off);
        y += ROW_H + 4.0;

        rr.draw_rect(rect.x + PAD, y, w, BTN_H, hex("#0e639c"), 3.0);
        y += BTN_H + 4.0;
        rr.draw_rect(rect.x + PAD, y, w, BTN_H, hex("#0e639c"), 3.0);

        let _ = renderer;
    }

    #[allow(clippy::cast_precision_loss)]
    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        match event {
            UiEvent::Focus => { self.focused = true; EventResult::Handled }
            UiEvent::Blur => { self.focused = false; self.input_focused = false; EventResult::Handled }

            UiEvent::KeyPress { key: Key::Enter, modifiers: Modifiers { ctrl: true, .. } }
                if self.focused => { self.do_commit(); EventResult::Handled }

            UiEvent::KeyPress { key: Key::ArrowUp, .. }
                if self.input_focused && !self.message.contains('\n') =>
                { self.recall(false); EventResult::Handled }
            UiEvent::KeyPress { key: Key::ArrowDown, .. }
                if self.input_focused && !self.message.contains('\n') =>
                { self.recall(true); EventResult::Handled }

            UiEvent::KeyPress { key: Key::Char(ch), .. } if self.input_focused => {
                self.message.insert(self.cursor_pos, *ch);
                self.cursor_pos += ch.len_utf8();
                self.history_index = None;
                EventResult::Handled
            }
            UiEvent::KeyPress { key: Key::Enter, .. } if self.input_focused => {
                self.message.insert(self.cursor_pos, '\n');
                self.cursor_pos += 1;
                EventResult::Handled
            }
            UiEvent::KeyPress { key: Key::Backspace, .. }
                if self.input_focused && self.cursor_pos > 0 =>
            {
                let prev = self.message[..self.cursor_pos]
                    .char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
                self.message.drain(prev..self.cursor_pos);
                self.cursor_pos = prev;
                EventResult::Handled
            }

            UiEvent::MouseDown { x, y, button: MouseButton::Left } if rect.contains(*x, *y) => {
                self.focused = true;
                let ih = self.input_height();
                let ib = rect.y + 4.0 + ih;
                if *y < ib { self.input_focused = true; return EventResult::Handled; }
                self.input_focused = false;

                let mut cy = ib + 4.0 + ROW_H;
                if *y >= cy && *y < cy + ROW_H { self.toggle_amend(); return EventResult::Handled; }
                cy += ROW_H;
                if *y >= cy && *y < cy + ROW_H { self.toggle_sign_off(); return EventResult::Handled; }
                cy += ROW_H + 4.0;
                let in_x = *x >= rect.x + PAD && *x <= rect.x + rect.width - PAD;
                if *y >= cy && *y < cy + BTN_H && in_x { self.do_commit(); return EventResult::Handled; }
                cy += BTN_H + 4.0;
                if *y >= cy && *y < cy + BTN_H && in_x {
                    self.stage_all_pending = true;
                    self.do_commit();
                    return EventResult::Handled;
                }
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}
