//! Hover information popup widget.
//!
//! Renders a tooltip showing hover information: markdown content, code blocks,
//! diagnostics with severity, and quick-fix action links. Supports pinning
//! so the popup stays visible when the mouse moves away.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::draw::{DrawContext, IconId};
use crate::layout::{Edges, LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

/// Severity level for diagnostic sections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

/// A code action available as a quick fix.
#[derive(Clone, Debug)]
pub struct CodeAction {
    pub title: String,
    pub kind: Option<String>,
    pub is_preferred: bool,
}

impl CodeAction {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            kind: None,
            is_preferred: false,
        }
    }
}

/// A section of content within the hover widget.
#[derive(Clone, Debug)]
pub enum HoverSection {
    /// Markdown-formatted text.
    Markdown(String),
    /// A fenced code block with language and source.
    CodeBlock { language: String, code: String },
    /// A diagnostic message with severity.
    Diagnostic {
        severity: DiagnosticSeverity,
        message: String,
        source: Option<String>,
    },
    /// Quick fix actions.
    QuickFix { actions: Vec<CodeAction> },
}

/// The hover popup showing information at a document position.
#[allow(dead_code)]
pub struct HoverWidget {
    content: Vec<HoverSection>,
    visible: bool,
    position: (f32, f32),
    pinned: bool,

    scroll_offset: f32,
    hovered: bool,

    font_size: f32,
    code_font_size: f32,
    max_width: f32,
    max_height: f32,
    line_spacing: f32,

    background: Color,
    border_color: Color,
    shadow_color: Color,
    foreground: Color,
    code_bg: Color,
    code_fg: Color,
    link_color: Color,
    error_color: Color,
    warning_color: Color,
    info_color: Color,
    hint_color: Color,
    separator_color: Color,
    pin_indicator_color: Color,
}

impl Default for HoverWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl HoverWidget {
    pub fn new() -> Self {
        Self {
            content: Vec::new(),
            visible: false,
            position: (0.0, 0.0),
            pinned: false,
            scroll_offset: 0.0,
            hovered: false,
            font_size: 13.0,
            code_font_size: 12.0,
            max_width: 500.0,
            max_height: 300.0,
            line_spacing: 4.0,
            background: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            border_color: Color::from_hex("#454545").unwrap_or(Color::BLACK),
            shadow_color: Color::from_hex("#00000060").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            code_bg: Color::from_hex("#1e1e1e").unwrap_or(Color::BLACK),
            code_fg: Color::from_hex("#d4d4d4").unwrap_or(Color::WHITE),
            link_color: Color::from_hex("#3794ff").unwrap_or(Color::WHITE),
            error_color: Color::from_hex("#f14c4c").unwrap_or(Color::WHITE),
            warning_color: Color::from_hex("#cca700").unwrap_or(Color::WHITE),
            info_color: Color::from_hex("#3794ff").unwrap_or(Color::WHITE),
            hint_color: Color::from_hex("#6b6b6b").unwrap_or(Color::WHITE),
            separator_color: Color::from_hex("#404040").unwrap_or(Color::BLACK),
            pin_indicator_color: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
        }
    }

    /// Shows the hover popup at the given position with the provided content.
    pub fn show(&mut self, content: Vec<HoverSection>, position: (f32, f32)) {
        self.content = content;
        self.position = position;
        self.visible = true;
        self.scroll_offset = 0.0;
        self.pinned = false;
    }

    /// Dismisses the hover popup.
    pub fn hide(&mut self) {
        if self.pinned {
            return;
        }
        self.force_hide();
    }

    /// Force-hides the popup even if pinned.
    pub fn force_hide(&mut self) {
        self.visible = false;
        self.content.clear();
        self.pinned = false;
        self.scroll_offset = 0.0;
    }

    /// Pins the hover popup so it stays visible.
    pub fn pin(&mut self) {
        self.pinned = true;
    }

    /// Unpins the hover popup.
    pub fn unpin(&mut self) {
        self.pinned = false;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn is_pinned(&self) -> bool {
        self.pinned
    }

    // ── Private helpers ──────────────────────────────────────────────────

    #[allow(clippy::cast_precision_loss)]
    fn content_height(&self) -> f32 {
        let mut h = 16.0; // top + bottom padding
        for section in &self.content {
            h += self.section_height(section);
        }
        h
    }

    #[allow(clippy::cast_precision_loss)]
    fn section_height(&self, section: &HoverSection) -> f32 {
        match section {
            HoverSection::Markdown(text) => {
                let lines = text.lines().count().max(1);
                lines as f32 * (self.font_size + self.line_spacing) + 4.0
            }
            HoverSection::CodeBlock { code, .. } => {
                let lines = code.lines().count().max(1);
                lines as f32 * (self.code_font_size + 2.0) + 16.0
            }
            HoverSection::Diagnostic { .. } => self.font_size + 12.0,
            HoverSection::QuickFix { actions } => {
                actions.len() as f32 * (self.font_size + 6.0) + 4.0
            }
        }
    }

    fn widget_rect(&self, editor_rect: Rect) -> Rect {
        let ch = self.content_height().min(self.max_height);
        let w = self.max_width;
        let (mut x, mut y) = self.position;

        // Position above the hovered word by default
        y -= ch + 4.0;

        // Shift below if near top of editor
        if y < editor_rect.y {
            y = self.position.1 + self.font_size + 4.0;
        }
        if x + w > editor_rect.x + editor_rect.width {
            x = (editor_rect.x + editor_rect.width - w).max(editor_rect.x);
        }
        x = x.max(editor_rect.x);
        y = y.max(editor_rect.y);

        Rect::new(x, y, w, ch)
    }

    fn severity_icon(severity: DiagnosticSeverity) -> IconId {
        match severity {
            DiagnosticSeverity::Error => IconId::Error,
            DiagnosticSeverity::Warning => IconId::Warning,
            DiagnosticSeverity::Info => IconId::Info,
            DiagnosticSeverity::Hint => IconId::Info,
        }
    }

    fn severity_color(&self, severity: DiagnosticSeverity) -> Color {
        match severity {
            DiagnosticSeverity::Error => self.error_color,
            DiagnosticSeverity::Warning => self.warning_color,
            DiagnosticSeverity::Info => self.info_color,
            DiagnosticSeverity::Hint => self.hint_color,
        }
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn render_draw(&self, ctx: &mut DrawContext, editor_rect: Rect) {
        if !self.visible || self.content.is_empty() {
            return;
        }
        let wr = self.widget_rect(editor_rect);

        // Shadow
        let shadow = Rect::new(wr.x + 3.0, wr.y + 3.0, wr.width, wr.height);
        ctx.draw_rect(shadow, self.shadow_color, 4.0);

        // Background
        ctx.draw_rect(wr, self.background, 4.0);
        ctx.draw_border(wr, self.border_color, 1.0, 4.0);

        // Pin indicator
        if self.pinned {
            let pin_r = Rect::new(wr.x + wr.width - 14.0, wr.y + 2.0, 10.0, 3.0);
            ctx.draw_rect(pin_r, self.pin_indicator_color, 1.0);
        }

        ctx.save();
        ctx.clip(wr);

        let pad = 8.0;
        let mut cy = wr.y + pad - self.scroll_offset;
        let inner_w = wr.width - pad * 2.0;

        for (i, section) in self.content.iter().enumerate() {
            if i > 0 {
                let sep = Rect::new(wr.x + pad, cy, inner_w, 1.0);
                ctx.draw_rect(sep, self.separator_color, 0.0);
                cy += 4.0;
            }

            match section {
                HoverSection::Markdown(text) => {
                    for line in text.lines() {
                        if cy > wr.y + wr.height {
                            break;
                        }
                        let is_heading = line.starts_with('#');
                        let display = if is_heading {
                            line.trim_start_matches('#').trim()
                        } else {
                            line
                        };
                        ctx.draw_text(
                            display,
                            (wr.x + pad, cy),
                            self.foreground,
                            self.font_size,
                            is_heading,
                            false,
                        );
                        cy += self.font_size + self.line_spacing;
                    }
                    cy += 4.0;
                }
                HoverSection::CodeBlock { code, .. } => {
                    let block_h = {
                        let lines = code.lines().count().max(1);
                        lines as f32 * (self.code_font_size + 2.0) + 12.0
                    };
                    let code_rect = Rect::new(wr.x + pad, cy, inner_w, block_h);
                    ctx.draw_rect(code_rect, self.code_bg, 3.0);
                    cy += 6.0;
                    for line in code.lines() {
                        if cy > wr.y + wr.height {
                            break;
                        }
                        ctx.draw_text(
                            line,
                            (wr.x + pad + 8.0, cy),
                            self.code_fg,
                            self.code_font_size,
                            false,
                            false,
                        );
                        cy += self.code_font_size + 2.0;
                    }
                    cy += 6.0;
                }
                HoverSection::Diagnostic {
                    severity,
                    message,
                    source,
                } => {
                    let icon = Self::severity_icon(*severity);
                    let color = self.severity_color(*severity);
                    ctx.draw_icon(icon, (wr.x + pad, cy), 14.0, color);

                    let msg = if let Some(src) = source {
                        format!("{message} [{src}]")
                    } else {
                        message.clone()
                    };
                    ctx.draw_text(
                        &msg,
                        (wr.x + pad + 20.0, cy),
                        self.foreground,
                        self.font_size,
                        false,
                        false,
                    );
                    cy += self.font_size + 12.0;
                }
                HoverSection::QuickFix { actions } => {
                    for action in actions {
                        if cy > wr.y + wr.height {
                            break;
                        }
                        let label = format!("Quick Fix... {}", action.title);
                        ctx.draw_text(
                            &label,
                            (wr.x + pad, cy),
                            self.link_color,
                            self.font_size,
                            false,
                            false,
                        );
                        cy += self.font_size + 6.0;
                    }
                    cy += 4.0;
                }
            }
        }

        ctx.restore();
    }
}

impl Widget for HoverWidget {
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Auto,
            padding: Edges::all(0.0),
            ..LayoutNode::default()
        }
    }

    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        if !self.visible || self.content.is_empty() {
            return;
        }
        let wr = self.widget_rect(rect);
        let mut rr = sidex_gpu::RectRenderer::new();
        rr.draw_rect(wr.x, wr.y, wr.width, wr.height, self.background, 4.0);
        rr.draw_border(wr.x, wr.y, wr.width, wr.height, self.border_color, 1.0);
        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        if !self.visible {
            return EventResult::Ignored;
        }
        match event {
            UiEvent::KeyPress { key: Key::Escape, .. } => {
                self.force_hide();
                EventResult::Handled
            }
            UiEvent::MouseMove { x, y } => {
                let wr = self.widget_rect(rect);
                self.hovered = wr.contains(*x, *y);
                EventResult::Ignored
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => {
                let wr = self.widget_rect(rect);
                if wr.contains(*x, *y) {
                    if !self.pinned {
                        self.pin();
                    }
                    EventResult::Handled
                } else if self.pinned {
                    self.force_hide();
                    EventResult::Handled
                } else {
                    EventResult::Ignored
                }
            }
            UiEvent::MouseScroll { dy, .. } if self.hovered => {
                let total = self.content_height();
                let max_scroll = (total - self.max_height).max(0.0);
                self.scroll_offset = (self.scroll_offset - dy * 20.0)
                    .max(0.0)
                    .min(max_scroll);
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_and_hide() {
        let mut w = HoverWidget::new();
        w.show(
            vec![HoverSection::Markdown("hello world".into())],
            (100.0, 200.0),
        );
        assert!(w.is_visible());

        w.hide();
        assert!(!w.is_visible());
    }

    #[test]
    fn pinned_hover_stays() {
        let mut w = HoverWidget::new();
        w.show(
            vec![HoverSection::Markdown("pinned".into())],
            (50.0, 50.0),
        );
        w.pin();
        assert!(w.is_pinned());

        w.hide();
        assert!(w.is_visible()); // stays because pinned

        w.force_hide();
        assert!(!w.is_visible());
    }

    #[test]
    fn diagnostic_section() {
        let w = HoverWidget::new();
        let section = HoverSection::Diagnostic {
            severity: DiagnosticSeverity::Error,
            message: "unused variable".into(),
            source: Some("rustc".into()),
        };
        assert!(w.section_height(&section) > 0.0);
    }
}
