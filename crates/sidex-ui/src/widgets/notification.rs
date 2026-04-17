//! Notification toast widget with auto-dismiss, severity levels, and animations.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::draw::{DrawContext, IconId};
use crate::layout::{Edges, LayoutNode, Rect, Size};
use crate::widget::{EventResult, MouseButton, UiEvent, Widget};

/// Severity level of a notification.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Severity {
    #[default]
    Info,
    Warning,
    Error,
}

/// An action button on a notification.
#[derive(Clone, Debug)]
pub struct NotificationAction {
    pub label: String,
}

impl NotificationAction {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

/// A notification toast that appears in a corner of the screen.
#[allow(dead_code)]
pub struct NotificationToast<F: FnMut()> {
    pub message: String,
    pub severity: Severity,
    pub actions: Vec<NotificationAction>,
    pub on_dismiss: F,
    pub stack_index: usize,

    pub auto_dismiss_secs: f32,
    pub elapsed: f32,
    /// Fade-in animation progress [0, 1].
    pub fade_in: f32,
    /// Fade-out animation progress [0, 1].
    pub fade_out: f32,

    visible: bool,
    hovered: bool,
    hovered_action: Option<usize>,
    hovered_close: bool,

    toast_width: f32,
    toast_min_height: f32,
    font_size: f32,
    background: Color,
    foreground: Color,
    border_color: Color,
    shadow_color: Color,
    info_accent: Color,
    warning_accent: Color,
    error_accent: Color,
    action_fg: Color,
    action_hover_bg: Color,
    close_hover_bg: Color,
}

impl<F: FnMut()> NotificationToast<F> {
    pub fn new(message: impl Into<String>, severity: Severity, on_dismiss: F) -> Self {
        Self {
            message: message.into(),
            severity,
            actions: Vec::new(),
            on_dismiss,
            stack_index: 0,
            auto_dismiss_secs: 8.0,
            elapsed: 0.0,
            fade_in: 0.0,
            fade_out: 0.0,
            visible: true,
            hovered: false,
            hovered_action: None,
            hovered_close: false,
            toast_width: 400.0,
            toast_min_height: 64.0,
            font_size: 13.0,
            background: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            border_color: Color::from_hex("#454545").unwrap_or(Color::BLACK),
            shadow_color: Color::from_hex("#00000060").unwrap_or(Color::BLACK),
            info_accent: Color::from_hex("#3794ff").unwrap_or(Color::WHITE),
            warning_accent: Color::from_hex("#cca700").unwrap_or(Color::WHITE),
            error_accent: Color::from_hex("#f14c4c").unwrap_or(Color::WHITE),
            action_fg: Color::from_hex("#3794ff").unwrap_or(Color::WHITE),
            action_hover_bg: Color::from_hex("#04395e").unwrap_or(Color::BLACK),
            close_hover_bg: Color::from_hex("#404040").unwrap_or(Color::BLACK),
        }
    }

    pub fn with_actions(mut self, actions: Vec<NotificationAction>) -> Self {
        self.actions = actions;
        self
    }

    pub fn with_auto_dismiss(mut self, secs: f32) -> Self {
        self.auto_dismiss_secs = secs;
        self
    }

    pub fn with_stack_index(mut self, index: usize) -> Self {
        self.stack_index = index;
        self
    }

    /// Advances timers. Returns `true` if dismissed.
    pub fn tick(&mut self, dt: f32) -> bool {
        // Fade in
        if self.fade_in < 1.0 {
            self.fade_in = (self.fade_in + dt * 4.0).min(1.0);
        }

        if !self.visible || self.hovered {
            return false;
        }
        self.elapsed += dt;
        if self.auto_dismiss_secs > 0.0 && self.elapsed >= self.auto_dismiss_secs {
            self.fade_out = (self.fade_out + dt * 4.0).min(1.0);
            if self.fade_out >= 1.0 {
                self.visible = false;
                (self.on_dismiss)();
                return true;
            }
        }
        false
    }

    fn accent_color(&self) -> Color {
        match self.severity {
            Severity::Info => self.info_accent,
            Severity::Warning => self.warning_accent,
            Severity::Error => self.error_accent,
        }
    }

    fn severity_icon(&self) -> IconId {
        match self.severity {
            Severity::Info => IconId::Info,
            Severity::Warning => IconId::Warning,
            Severity::Error => IconId::Error,
        }
    }

    fn toast_height(&self) -> f32 {
        let action_row = if self.actions.is_empty() { 0.0 } else { 28.0 };
        self.toast_min_height + action_row
    }

    fn toast_rect(&self, viewport: Rect) -> Rect {
        let margin = 12.0;
        let h = self.toast_height();
        let y_offset = self.stack_index as f32 * (h + 8.0);
        Rect::new(
            viewport.x + viewport.width - self.toast_width - margin,
            viewport.y + viewport.height - h - margin - y_offset,
            self.toast_width,
            h,
        )
    }

    fn close_rect(&self, toast: Rect) -> Rect {
        Rect::new(toast.x + toast.width - 24.0, toast.y + 4.0, 20.0, 20.0)
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn render_draw(&self, ctx: &mut DrawContext, viewport: Rect) {
        if !self.visible {
            return;
        }
        let tr = self.toast_rect(viewport);
        let alpha = self.fade_in * (1.0 - self.fade_out);
        let _ = alpha; // Would be used for opacity blending

        // Shadow
        let shadow = Rect::new(tr.x + 2.0, tr.y + 2.0, tr.width, tr.height);
        ctx.draw_rect(shadow, self.shadow_color, 4.0);

        // Background card
        ctx.draw_rect(tr, self.background, 4.0);
        ctx.draw_border(tr, self.border_color, 1.0, 4.0);

        // Severity color stripe (left edge)
        let accent = self.accent_color();
        let stripe = Rect::new(tr.x, tr.y, 3.0, tr.height);
        ctx.draw_rect(stripe, accent, 0.0);

        // Severity icon
        let icon_y = tr.y + 12.0;
        ctx.draw_icon(self.severity_icon(), (tr.x + 12.0, icon_y), 16.0, accent);

        // Message text
        let text_x = tr.x + 36.0;
        let text_y = tr.y + 12.0;
        let max_text_w = tr.width - 60.0;
        let _ = max_text_w; // Would be used for text wrapping
        ctx.draw_text(
            &self.message,
            (text_x, text_y),
            self.foreground,
            self.font_size,
            false,
            false,
        );

        // Close button (X)
        let cr = self.close_rect(tr);
        if self.hovered_close {
            ctx.draw_rect(cr, self.close_hover_bg, 2.0);
        }
        ctx.draw_icon(
            IconId::Close,
            (cr.x + 4.0, cr.y + 4.0),
            12.0,
            self.foreground,
        );

        // Action buttons (inline)
        if !self.actions.is_empty() {
            let action_y = tr.y + self.toast_min_height - 4.0;
            let mut ax = text_x;
            for (i, action) in self.actions.iter().enumerate() {
                let w = action.label.len() as f32 * self.font_size * 0.6 + 16.0;
                let action_rect = Rect::new(ax, action_y, w, 24.0);
                if self.hovered_action == Some(i) {
                    ctx.draw_rect(action_rect, self.action_hover_bg, 2.0);
                }
                let label_y = action_y + (24.0 - self.font_size) / 2.0;
                ctx.draw_text(
                    &action.label,
                    (ax + 8.0, label_y),
                    self.action_fg,
                    self.font_size,
                    false,
                    false,
                );
                ax += w + 4.0;
            }
        }
    }
}

impl<F: FnMut()> Widget for NotificationToast<F> {
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Fixed(self.toast_height()),
            padding: Edges::symmetric(12.0, 8.0),
            ..LayoutNode::default()
        }
    }

    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        if !self.visible {
            return;
        }
        let tr = self.toast_rect(rect);
        let mut rr = sidex_gpu::RectRenderer::new();
        rr.draw_rect(tr.x, tr.y, tr.width, tr.height, self.background, 4.0);
        rr.draw_border(tr.x, tr.y, tr.width, tr.height, self.border_color, 1.0);
        let accent = self.accent_color();
        rr.draw_rect(tr.x, tr.y, 3.0, tr.height, accent, 0.0);
        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        if !self.visible {
            return EventResult::Ignored;
        }
        let tr = self.toast_rect(rect);
        match event {
            UiEvent::MouseMove { x, y } => {
                self.hovered = tr.contains(*x, *y);
                self.hovered_close = self.close_rect(tr).contains(*x, *y);
                if self.hovered && !self.actions.is_empty() {
                    let action_y = tr.y + self.toast_min_height - 4.0;
                    let mut ax = tr.x + 36.0;
                    self.hovered_action = None;
                    for (i, action) in self.actions.iter().enumerate() {
                        let w = action.label.len() as f32 * self.font_size * 0.6 + 16.0;
                        let ar = Rect::new(ax, action_y, w, 24.0);
                        if ar.contains(*x, *y) {
                            self.hovered_action = Some(i);
                            break;
                        }
                        ax += w + 4.0;
                    }
                } else {
                    self.hovered_action = None;
                }
                EventResult::Ignored
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } if tr.contains(*x, *y) => {
                if self.hovered_close {
                    self.visible = false;
                    (self.on_dismiss)();
                }
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}
