//! Output panel — multi-channel log viewer.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

// ── Output channel ───────────────────────────────────────────────────────────

/// A named output channel (e.g. "Extension Host", "Git", "Tasks").
#[derive(Clone, Debug)]
pub struct OutputChannel {
    pub id: String,
    pub name: String,
    pub lines: Vec<OutputLine>,
    pub auto_scroll: bool,
}

impl OutputChannel {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            lines: Vec::new(),
            auto_scroll: true,
        }
    }

    pub fn append(&mut self, text: impl Into<String>, level: OutputLevel) {
        self.lines.push(OutputLine {
            text: text.into(),
            level,
            timestamp: None,
        });
    }

    pub fn append_with_timestamp(
        &mut self,
        text: impl Into<String>,
        level: OutputLevel,
        timestamp: impl Into<String>,
    ) {
        self.lines.push(OutputLine {
            text: text.into(),
            level,
            timestamp: Some(timestamp.into()),
        });
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }
}

/// A single line in an output channel.
#[derive(Clone, Debug)]
pub struct OutputLine {
    pub text: String,
    pub level: OutputLevel,
    pub timestamp: Option<String>,
}

/// Severity level for an output line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputLevel {
    Info,
    Warning,
    Error,
    Debug,
    Trace,
}

impl OutputLevel {
    pub fn color(self) -> Color {
        match self {
            Self::Info => Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            Self::Warning => Color::from_rgb(205, 173, 0),
            Self::Error => Color::from_rgb(244, 71, 71),
            Self::Debug => Color::from_rgb(160, 160, 160),
            Self::Trace => Color::from_rgb(100, 100, 100),
        }
    }
}

// ── Output panel ─────────────────────────────────────────────────────────────

/// The Output bottom panel.
///
/// Shows output from extension hosts, language servers, Git, tasks, etc.
/// Supports channel selection, scrolling log output, and clearing.
#[allow(dead_code)]
pub struct OutputPanel<OnChannelSelect>
where
    OnChannelSelect: FnMut(&str),
{
    pub channels: Vec<OutputChannel>,
    pub active_channel: usize,
    pub on_channel_select: OnChannelSelect,

    scroll_offset: f32,
    focused: bool,
    word_wrap: bool,
    show_timestamps: bool,

    selector_height: f32,
    line_height: f32,
    dropdown_width: f32,
    dropdown_open: bool,

    background: Color,
    selector_bg: Color,
    separator_color: Color,
    dropdown_bg: Color,
    dropdown_hover: Color,
    foreground: Color,
    timestamp_fg: Color,
}

impl<OnChannelSelect> OutputPanel<OnChannelSelect>
where
    OnChannelSelect: FnMut(&str),
{
    pub fn new(channels: Vec<OutputChannel>, on_channel_select: OnChannelSelect) -> Self {
        Self {
            channels,
            active_channel: 0,
            on_channel_select,

            scroll_offset: 0.0,
            focused: false,
            word_wrap: true,
            show_timestamps: false,

            selector_height: 28.0,
            line_height: 18.0,
            dropdown_width: 200.0,
            dropdown_open: false,

            background: Color::from_hex("#1e1e1e").unwrap_or(Color::BLACK),
            selector_bg: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            separator_color: Color::from_hex("#2b2b2b").unwrap_or(Color::BLACK),
            dropdown_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            dropdown_hover: Color::from_hex("#04395e").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            timestamp_fg: Color::from_hex("#969696").unwrap_or(Color::WHITE),
        }
    }

    pub fn active(&self) -> Option<&OutputChannel> {
        self.channels.get(self.active_channel)
    }

    pub fn active_mut(&mut self) -> Option<&mut OutputChannel> {
        self.channels.get_mut(self.active_channel)
    }

    pub fn select_channel(&mut self, index: usize) {
        if index < self.channels.len() {
            self.active_channel = index;
            self.scroll_offset = 0.0;
            let id = self.channels[index].id.clone();
            (self.on_channel_select)(&id);
        }
    }

    pub fn clear_active(&mut self) {
        if let Some(ch) = self.active_mut() {
            ch.clear();
        }
        self.scroll_offset = 0.0;
    }

    pub fn toggle_word_wrap(&mut self) {
        self.word_wrap = !self.word_wrap;
    }

    pub fn toggle_timestamps(&mut self) {
        self.show_timestamps = !self.show_timestamps;
    }

    pub fn add_channel(&mut self, channel: OutputChannel) {
        self.channels.push(channel);
    }

    fn scroll_to_bottom(&mut self, rect: Rect) {
        if let Some(ch) = self.active() {
            let total = ch.lines.len() as f32 * self.line_height;
            let content_h = rect.height - self.selector_height;
            self.scroll_offset = (total - content_h).max(0.0);
        }
    }
}

impl<OnChannelSelect> Widget for OutputPanel<OnChannelSelect>
where
    OnChannelSelect: FnMut(&str),
{
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Flex(1.0),
            ..LayoutNode::default()
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        let mut rr = sidex_gpu::RectRenderer::new();
        rr.draw_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            self.background,
            0.0,
        );

        // Channel selector bar
        rr.draw_rect(
            rect.x,
            rect.y,
            rect.width,
            self.selector_height,
            self.selector_bg,
            0.0,
        );
        rr.draw_rect(
            rect.x,
            rect.y + self.selector_height - 1.0,
            rect.width,
            1.0,
            self.separator_color,
            0.0,
        );

        // Dropdown selector
        let dd_x = rect.x + 8.0;
        let dd_y = rect.y + 3.0;
        let dd_h = self.selector_height - 6.0;
        rr.draw_rect(dd_x, dd_y, self.dropdown_width, dd_h, self.dropdown_bg, 3.0);

        // Clear button
        let btn_size = 20.0;
        let clear_x = rect.x + rect.width - btn_size - 8.0;
        let clear_y = rect.y + (self.selector_height - btn_size) / 2.0;
        rr.draw_rect(clear_x, clear_y, btn_size, btn_size, self.dropdown_bg, 3.0);

        // Dropdown menu
        if self.dropdown_open {
            let menu_y = rect.y + self.selector_height;
            let menu_h = self.channels.len() as f32 * 24.0;
            rr.draw_rect(
                dd_x,
                menu_y,
                self.dropdown_width,
                menu_h,
                self.dropdown_bg,
                2.0,
            );
            for (i, _ch) in self.channels.iter().enumerate() {
                let item_y = menu_y + i as f32 * 24.0;
                if i == self.active_channel {
                    rr.draw_rect(
                        dd_x,
                        item_y,
                        self.dropdown_width,
                        24.0,
                        self.dropdown_hover,
                        0.0,
                    );
                }
            }
        }

        // Log lines
        if let Some(channel) = self.active() {
            let content_top = rect.y + self.selector_height;
            let content_h = rect.height - self.selector_height;
            let first_line = (self.scroll_offset / self.line_height).floor() as usize;
            let visible_lines = (content_h / self.line_height).ceil() as usize + 1;

            for i in first_line..first_line + visible_lines {
                if let Some(line) = channel.lines.get(i) {
                    let ly = content_top + i as f32 * self.line_height - self.scroll_offset;
                    if ly + self.line_height < content_top || ly > rect.y + rect.height {
                        continue;
                    }
                    // Level color indicator
                    rr.draw_rect(rect.x, ly, 3.0, self.line_height, line.level.color(), 0.0);
                }
            }
        }

        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        match event {
            UiEvent::Focus => {
                self.focused = true;
                EventResult::Handled
            }
            UiEvent::Blur => {
                self.focused = false;
                self.dropdown_open = false;
                EventResult::Handled
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } if rect.contains(*x, *y) => {
                self.focused = true;

                // Dropdown toggle
                let dd_x = rect.x + 8.0;
                if *y < rect.y + self.selector_height
                    && *x >= dd_x
                    && *x < dd_x + self.dropdown_width
                {
                    self.dropdown_open = !self.dropdown_open;
                    return EventResult::Handled;
                }

                // Clear button
                let btn_size = 20.0;
                let clear_x = rect.x + rect.width - btn_size - 8.0;
                if *y < rect.y + self.selector_height && *x >= clear_x {
                    self.clear_active();
                    return EventResult::Handled;
                }

                // Dropdown menu selection
                if self.dropdown_open {
                    let menu_y = rect.y + self.selector_height;
                    let idx = ((*y - menu_y) / 24.0) as usize;
                    if idx < self.channels.len() {
                        self.select_channel(idx);
                    }
                    self.dropdown_open = false;
                    return EventResult::Handled;
                }

                self.dropdown_open = false;
                EventResult::Handled
            }
            UiEvent::MouseScroll { dy, .. } => {
                if let Some(ch) = self.active() {
                    let total = ch.lines.len() as f32 * self.line_height;
                    let content_h = rect.height - self.selector_height;
                    let max = (total - content_h).max(0.0);
                    self.scroll_offset = (self.scroll_offset - dy * 40.0).clamp(0.0, max);
                }
                EventResult::Handled
            }
            UiEvent::KeyPress { key: Key::End, .. } if self.focused => {
                self.scroll_to_bottom(rect);
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}
