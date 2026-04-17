//! Notification center with toast stacking, auto-dismiss, progress tracking,
//! Do Not Disturb mode, and notification history.

use std::time::{Duration, Instant};

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::draw::{CursorIcon, DrawContext, IconId};
use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

// ── Severity ────────────────────────────────────────────────────────────────

/// Severity level of a notification.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NotificationSeverity {
    #[default]
    Info,
    Warning,
    Error,
}

// ── Action / Progress ───────────────────────────────────────────────────────

/// A clickable action button on a notification.
#[derive(Clone, Debug)]
pub struct NotificationAction {
    pub label: String,
    pub command: String,
    pub is_primary: bool,
}

impl NotificationAction {
    pub fn new(label: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            command: command.into(),
            is_primary: false,
        }
    }

    pub fn primary(mut self) -> Self {
        self.is_primary = true;
        self
    }
}

/// Progress state attached to a notification.
#[derive(Clone, Debug)]
pub struct NotificationProgress {
    pub message: Option<String>,
    pub percentage: Option<f32>,
    pub infinite: bool,
}

impl Default for NotificationProgress {
    fn default() -> Self {
        Self {
            message: None,
            percentage: None,
            infinite: true,
        }
    }
}

// ── Handle returned to callers ──────────────────────────────────────────────

/// Opaque handle for a progress notification, allowing later updates.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProgressHandle(pub String);

// ── Notification ────────────────────────────────────────────────────────────

/// A single notification entry.
#[derive(Clone, Debug)]
pub struct Notification {
    pub id: String,
    pub severity: NotificationSeverity,
    pub message: String,
    pub source: Option<String>,
    pub actions: Vec<NotificationAction>,
    pub progress: Option<NotificationProgress>,
    pub is_sticky: bool,
    pub created_at: Instant,
    pub auto_dismiss_after: Option<Duration>,
    pub expanded: bool,
    pub dismissed: bool,

    fade_in: f32,
    fade_out: f32,
    hovered: bool,
}

impl Notification {
    fn new(id: String, severity: NotificationSeverity, message: String) -> Self {
        let auto_dismiss = match severity {
            NotificationSeverity::Info => Some(Duration::from_secs(5)),
            NotificationSeverity::Warning => Some(Duration::from_secs(10)),
            NotificationSeverity::Error => None,
        };
        let is_sticky = severity == NotificationSeverity::Error;
        Self {
            id,
            severity,
            message,
            source: None,
            actions: Vec::new(),
            progress: None,
            is_sticky,
            created_at: Instant::now(),
            auto_dismiss_after: auto_dismiss,
            expanded: false,
            dismissed: false,
            fade_in: 0.0,
            fade_out: 0.0,
            hovered: false,
        }
    }

    fn display_message(&self) -> String {
        let mut msg = String::new();
        if let Some(ref src) = self.source {
            msg.push_str(&format!("[{src}] "));
        }
        msg.push_str(&self.message);
        msg
    }

    fn should_auto_dismiss(&self) -> bool {
        if self.is_sticky || self.hovered {
            return false;
        }
        if let Some(dur) = self.auto_dismiss_after {
            self.created_at.elapsed() >= dur
        } else {
            false
        }
    }
}

// ── NotificationCenter ──────────────────────────────────────────────────────

/// Manages a stack of toast notifications and a history list.
#[allow(dead_code)]
pub struct NotificationCenter {
    pub notifications: Vec<Notification>,
    pub history: Vec<Notification>,
    pub do_not_disturb: bool,
    pub max_visible: usize,
    pub history_open: bool,

    next_id: u64,

    toast_width: f32,
    toast_min_height: f32,
    font_size: f32,
    margin: f32,
    gap: f32,

    hovered_notification: Option<usize>,
    hovered_action: Option<(usize, usize)>,
    hovered_close: Option<usize>,
    hovered_expand: Option<usize>,

    background: Color,
    foreground: Color,
    border_color: Color,
    shadow_color: Color,
    info_accent: Color,
    warning_accent: Color,
    error_accent: Color,
    action_fg: Color,
    action_primary_bg: Color,
    action_hover_bg: Color,
    close_hover_bg: Color,
    progress_bar_bg: Color,
    progress_bar_fg: Color,
    history_bg: Color,
    history_border: Color,
}

impl NotificationCenter {
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
            history: Vec::new(),
            do_not_disturb: false,
            max_visible: 3,
            history_open: false,
            next_id: 1,
            toast_width: 400.0,
            toast_min_height: 64.0,
            font_size: 13.0,
            margin: 12.0,
            gap: 8.0,
            hovered_notification: None,
            hovered_action: None,
            hovered_close: None,
            hovered_expand: None,
            background: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            border_color: Color::from_hex("#454545").unwrap_or(Color::BLACK),
            shadow_color: Color::from_hex("#00000060").unwrap_or(Color::BLACK),
            info_accent: Color::from_hex("#3794ff").unwrap_or(Color::WHITE),
            warning_accent: Color::from_hex("#cca700").unwrap_or(Color::WHITE),
            error_accent: Color::from_hex("#f14c4c").unwrap_or(Color::WHITE),
            action_fg: Color::from_hex("#3794ff").unwrap_or(Color::WHITE),
            action_primary_bg: Color::from_hex("#0e639c").unwrap_or(Color::BLACK),
            action_hover_bg: Color::from_hex("#04395e").unwrap_or(Color::BLACK),
            close_hover_bg: Color::from_hex("#404040").unwrap_or(Color::BLACK),
            progress_bar_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            progress_bar_fg: Color::from_hex("#0e70c0").unwrap_or(Color::WHITE),
            history_bg: Color::from_hex("#1e1e1e").unwrap_or(Color::BLACK),
            history_border: Color::from_hex("#454545").unwrap_or(Color::BLACK),
        }
    }

    // ── Public API ──────────────────────────────────────────────────────

    /// Show a toast notification. Returns its unique id.
    pub fn show_notification(
        &mut self,
        message: &str,
        severity: NotificationSeverity,
        actions: Vec<NotificationAction>,
    ) -> String {
        let id = format!("notif-{}", self.next_id);
        self.next_id += 1;

        let mut n = Notification::new(id.clone(), severity, message.to_string());
        n.actions = actions;

        if !self.do_not_disturb {
            self.notifications.push(n.clone());
        }
        self.history.push(n);
        id
    }

    /// Show a progress notification. Returns a handle for later updates.
    pub fn show_progress(&mut self, title: &str, cancellable: bool) -> ProgressHandle {
        let id = format!("progress-{}", self.next_id);
        self.next_id += 1;

        let mut n = Notification::new(id.clone(), NotificationSeverity::Info, title.to_string());
        n.progress = Some(NotificationProgress::default());
        n.is_sticky = true;
        n.auto_dismiss_after = None;
        if cancellable {
            n.actions.push(NotificationAction::new("Cancel", "notifications.cancel"));
        }

        if !self.do_not_disturb {
            self.notifications.push(n.clone());
        }
        self.history.push(n);
        ProgressHandle(id)
    }

    /// Update a progress notification's message and percentage.
    pub fn update_progress(&mut self, handle: &str, message: &str, percentage: f32) {
        for n in self.notifications.iter_mut().chain(self.history.iter_mut()) {
            if n.id == handle {
                if let Some(ref mut p) = n.progress {
                    p.message = Some(message.to_string());
                    p.percentage = Some(percentage.clamp(0.0, 100.0));
                    p.infinite = false;
                }
            }
        }
    }

    /// Finish and dismiss a progress notification.
    pub fn finish_progress(&mut self, handle: &str) {
        self.dismiss(handle);
    }

    /// Dismiss a notification by id.
    pub fn dismiss(&mut self, id: &str) {
        self.notifications.retain(|n| n.id != id);
    }

    /// Dismiss all visible notifications.
    pub fn dismiss_all(&mut self) {
        for n in self.notifications.drain(..) {
            let mut archived = n;
            archived.dismissed = true;
            // Already in history from show_notification
            let _ = archived;
        }
    }

    /// Toggle Do Not Disturb mode.
    pub fn set_do_not_disturb(&mut self, enabled: bool) {
        self.do_not_disturb = enabled;
        if enabled {
            self.notifications.clear();
        }
    }

    /// Toggle the notification history panel.
    pub fn toggle_history(&mut self) {
        self.history_open = !self.history_open;
    }

    /// Clear all history.
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Returns the count of unread notifications in history.
    pub fn unread_count(&self) -> usize {
        self.history.iter().filter(|n| !n.dismissed).count()
    }

    /// Advance timers, auto-dismiss expired notifications.
    pub fn tick(&mut self, _dt: f32) {
        let mut to_remove = Vec::new();
        for (i, n) in self.notifications.iter().enumerate() {
            if n.should_auto_dismiss() {
                to_remove.push(i);
            }
        }
        for i in to_remove.into_iter().rev() {
            self.notifications.remove(i);
        }
    }

    // ── Layout helpers ──────────────────────────────────────────────────

    fn accent_color(&self, severity: NotificationSeverity) -> Color {
        match severity {
            NotificationSeverity::Info => self.info_accent,
            NotificationSeverity::Warning => self.warning_accent,
            NotificationSeverity::Error => self.error_accent,
        }
    }

    fn severity_icon(severity: NotificationSeverity) -> IconId {
        match severity {
            NotificationSeverity::Info => IconId::Info,
            NotificationSeverity::Warning => IconId::Warning,
            NotificationSeverity::Error => IconId::Error,
        }
    }

    fn toast_height(&self, n: &Notification) -> f32 {
        let action_row = if n.actions.is_empty() { 0.0 } else { 28.0 };
        let progress_row = if n.progress.is_some() { 20.0 } else { 0.0 };
        let expand_extra = if n.expanded && n.message.len() > 80 { 40.0 } else { 0.0 };
        self.toast_min_height + action_row + progress_row + expand_extra
    }

    fn toast_rect(&self, index: usize, viewport: Rect) -> Rect {
        let mut y_offset = 0.0_f32;
        let visible = self.visible_toasts();
        for i in 0..index.min(visible.len()) {
            y_offset += self.toast_height(&visible[i]) + self.gap;
        }
        let n = &visible[index.min(visible.len().saturating_sub(1))];
        let h = self.toast_height(n);
        Rect::new(
            viewport.x + viewport.width - self.toast_width - self.margin,
            viewport.y + viewport.height - h - self.margin - y_offset,
            self.toast_width,
            h,
        )
    }

    fn close_rect(&self, toast: Rect) -> Rect {
        Rect::new(toast.x + toast.width - 24.0, toast.y + 4.0, 20.0, 20.0)
    }

    fn expand_rect(&self, toast: Rect) -> Rect {
        Rect::new(toast.x + toast.width - 48.0, toast.y + 4.0, 20.0, 20.0)
    }

    fn visible_toasts(&self) -> Vec<Notification> {
        self.notifications
            .iter()
            .rev()
            .take(self.max_visible)
            .cloned()
            .collect()
    }

    // ── Rendering ───────────────────────────────────────────────────────

    #[allow(clippy::cast_precision_loss)]
    pub fn render_draw(&self, ctx: &mut DrawContext, viewport: Rect) {
        let visible = self.visible_toasts();
        for (i, n) in visible.iter().enumerate() {
            let tr = self.toast_rect(i, viewport);
            let accent = self.accent_color(n.severity);

            // Shadow
            let shadow = Rect::new(tr.x + 2.0, tr.y + 2.0, tr.width, tr.height);
            ctx.draw_rect(shadow, self.shadow_color, 4.0);

            // Background
            ctx.draw_rect(tr, self.background, 4.0);
            ctx.draw_border(tr, self.border_color, 1.0, 4.0);

            // Severity stripe
            let stripe = Rect::new(tr.x, tr.y, 3.0, tr.height);
            ctx.draw_rect(stripe, accent, 0.0);

            // Icon
            ctx.draw_icon(
                Self::severity_icon(n.severity),
                (tr.x + 12.0, tr.y + 12.0),
                16.0,
                accent,
            );

            // Message (with source attribution)
            let display = n.display_message();
            let max_chars = if n.expanded { 200 } else { 60 };
            let truncated: String = display.chars().take(max_chars).collect();
            let show_text = if display.len() > max_chars && !n.expanded {
                format!("{truncated}...")
            } else {
                truncated
            };
            ctx.draw_text(
                &show_text,
                (tr.x + 36.0, tr.y + 12.0),
                self.foreground,
                self.font_size,
                false,
                false,
            );

            // Close button
            let cr = self.close_rect(tr);
            if self.hovered_close == Some(i) {
                ctx.draw_rect(cr, self.close_hover_bg, 2.0);
            }
            ctx.draw_icon(IconId::Close, (cr.x + 4.0, cr.y + 4.0), 12.0, self.foreground);

            // Expand toggle for long messages
            if n.message.len() > 60 {
                let er = self.expand_rect(tr);
                if self.hovered_expand == Some(i) {
                    ctx.draw_rect(er, self.close_hover_bg, 2.0);
                }
                let icon = if n.expanded {
                    IconId::ChevronDown
                } else {
                    IconId::ChevronRight
                };
                ctx.draw_icon(icon, (er.x + 4.0, er.y + 4.0), 12.0, self.foreground);
            }

            // Progress bar
            if let Some(ref p) = n.progress {
                let bar_y = tr.y + self.toast_min_height - 8.0;
                let bar_w = tr.width - 48.0;
                let bar_rect = Rect::new(tr.x + 36.0, bar_y, bar_w, 4.0);
                ctx.draw_rect(bar_rect, self.progress_bar_bg, 2.0);

                if let Some(pct) = p.percentage {
                    let fill_w = bar_w * (pct / 100.0);
                    let fill = Rect::new(tr.x + 36.0, bar_y, fill_w, 4.0);
                    ctx.draw_rect(fill, self.progress_bar_fg, 2.0);
                }

                if let Some(ref msg) = p.message {
                    ctx.draw_text(
                        msg,
                        (tr.x + 36.0, bar_y + 6.0),
                        self.foreground,
                        11.0,
                        false,
                        false,
                    );
                }
            }

            // Action buttons
            if !n.actions.is_empty() {
                let action_y = tr.y + self.toast_height(n) - 28.0;
                let mut ax = tr.x + 36.0;
                for (ai, action) in n.actions.iter().enumerate() {
                    let w = action.label.len() as f32 * self.font_size * 0.6 + 16.0;
                    let ar = Rect::new(ax, action_y, w, 24.0);

                    if action.is_primary {
                        ctx.draw_rect(ar, self.action_primary_bg, 2.0);
                    }
                    if self.hovered_action == Some((i, ai)) {
                        ctx.draw_rect(ar, self.action_hover_bg, 2.0);
                    }
                    let fg = if action.is_primary {
                        Color::WHITE
                    } else {
                        self.action_fg
                    };
                    ctx.draw_text(
                        &action.label,
                        (ax + 8.0, action_y + 4.0),
                        fg,
                        self.font_size,
                        false,
                        false,
                    );
                    ax += w + 4.0;
                }
            }
        }

        // History panel
        if self.history_open {
            self.render_history(ctx, viewport);
        }

        if self.hovered_notification.is_some()
            || self.hovered_close.is_some()
            || self.hovered_action.is_some()
        {
            ctx.set_cursor(CursorIcon::Pointer);
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn render_history(&self, ctx: &mut DrawContext, viewport: Rect) {
        let panel_w = 380.0;
        let panel_h = 300.0_f32.min(viewport.height * 0.6);
        let panel_x = viewport.x + viewport.width - panel_w - self.margin;
        let panel_y = viewport.y + viewport.height - panel_h - 30.0;
        let panel = Rect::new(panel_x, panel_y, panel_w, panel_h);

        ctx.draw_rect(panel, self.history_bg, 4.0);
        ctx.draw_border(panel, self.history_border, 1.0, 4.0);

        // Header
        let header = format!("Notifications ({})", self.history.len());
        ctx.draw_text(
            &header,
            (panel_x + 12.0, panel_y + 8.0),
            self.foreground,
            self.font_size,
            true,
            false,
        );

        // History items
        let mut y = panel_y + 32.0;
        for n in self.history.iter().rev().take(10) {
            if y + 28.0 > panel_y + panel_h {
                break;
            }
            let accent = self.accent_color(n.severity);
            ctx.draw_icon(
                Self::severity_icon(n.severity),
                (panel_x + 12.0, y + 4.0),
                14.0,
                accent,
            );

            let display: String = n.display_message().chars().take(45).collect();
            ctx.draw_text(
                &display,
                (panel_x + 32.0, y + 4.0),
                self.foreground,
                12.0,
                false,
                false,
            );
            y += 28.0;
        }
    }
}

impl Default for NotificationCenter {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for NotificationCenter {
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Auto,
            ..LayoutNode::default()
        }
    }

    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        let visible = self.visible_toasts();
        let mut rr = sidex_gpu::RectRenderer::new();
        for (i, n) in visible.iter().enumerate() {
            let tr = self.toast_rect(i, rect);
            rr.draw_rect(tr.x, tr.y, tr.width, tr.height, self.background, 4.0);
            rr.draw_border(tr.x, tr.y, tr.width, tr.height, self.border_color, 1.0);
            let accent = self.accent_color(n.severity);
            rr.draw_rect(tr.x, tr.y, 3.0, tr.height, accent, 0.0);
        }
        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, viewport: Rect) -> EventResult {
        let visible = self.visible_toasts();
        match event {
            UiEvent::MouseMove { x, y } => {
                self.hovered_notification = None;
                self.hovered_close = None;
                self.hovered_expand = None;
                self.hovered_action = None;

                for (i, n) in visible.iter().enumerate() {
                    let tr = self.toast_rect(i, viewport);
                    if !tr.contains(*x, *y) {
                        continue;
                    }
                    self.hovered_notification = Some(i);

                    if self.close_rect(tr).contains(*x, *y) {
                        self.hovered_close = Some(i);
                    } else if n.message.len() > 60
                        && self.expand_rect(tr).contains(*x, *y)
                    {
                        self.hovered_expand = Some(i);
                    } else if !n.actions.is_empty() {
                        let action_y = tr.y + self.toast_height(n) - 28.0;
                        let mut ax = tr.x + 36.0;
                        for (ai, action) in n.actions.iter().enumerate() {
                            let w = action.label.len() as f32 * self.font_size * 0.6 + 16.0;
                            let ar = Rect::new(ax, action_y, w, 24.0);
                            if ar.contains(*x, *y) {
                                self.hovered_action = Some((i, ai));
                                break;
                            }
                            ax += w + 4.0;
                        }
                    }

                    // Mark hovered so auto-dismiss pauses
                    if let Some(real_idx) = self.notification_real_index(i) {
                        self.notifications[real_idx].hovered = true;
                    }
                    return EventResult::Handled;
                }

                // Un-hover all
                for n in &mut self.notifications {
                    n.hovered = false;
                }
                EventResult::Ignored
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => {
                for (i, _n) in visible.iter().enumerate() {
                    let tr = self.toast_rect(i, viewport);
                    if !tr.contains(*x, *y) {
                        continue;
                    }
                    if self.hovered_close == Some(i) {
                        if let Some(real_idx) = self.notification_real_index(i) {
                            self.notifications.remove(real_idx);
                        }
                        return EventResult::Handled;
                    }
                    if self.hovered_expand == Some(i) {
                        if let Some(real_idx) = self.notification_real_index(i) {
                            self.notifications[real_idx].expanded =
                                !self.notifications[real_idx].expanded;
                        }
                        return EventResult::Handled;
                    }
                    return EventResult::Handled;
                }
                EventResult::Ignored
            }
            UiEvent::KeyPress {
                key: Key::Escape, ..
            } => {
                if self.history_open {
                    self.history_open = false;
                    return EventResult::Handled;
                }
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }
}

impl NotificationCenter {
    fn notification_real_index(&self, visible_index: usize) -> Option<usize> {
        let visible: Vec<_> = self
            .notifications
            .iter()
            .enumerate()
            .rev()
            .take(self.max_visible)
            .collect();
        visible.get(visible_index).map(|(i, _)| *i)
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_notification_returns_id() {
        let mut nc = NotificationCenter::new();
        let id = nc.show_notification("Hello", NotificationSeverity::Info, vec![]);
        assert!(id.starts_with("notif-"));
        assert_eq!(nc.notifications.len(), 1);
        assert_eq!(nc.history.len(), 1);
    }

    #[test]
    fn dismiss_removes_notification() {
        let mut nc = NotificationCenter::new();
        let id = nc.show_notification("test", NotificationSeverity::Info, vec![]);
        nc.dismiss(&id);
        assert!(nc.notifications.is_empty());
    }

    #[test]
    fn do_not_disturb_suppresses() {
        let mut nc = NotificationCenter::new();
        nc.set_do_not_disturb(true);
        nc.show_notification("quiet", NotificationSeverity::Error, vec![]);
        assert!(nc.notifications.is_empty());
        assert_eq!(nc.history.len(), 1);
    }

    #[test]
    fn progress_handle_updates() {
        let mut nc = NotificationCenter::new();
        let handle = nc.show_progress("Installing...", true);
        nc.update_progress(&handle.0, "45%", 45.0);
        let n = nc.notifications.iter().find(|n| n.id == handle.0).unwrap();
        let p = n.progress.as_ref().unwrap();
        assert_eq!(p.percentage, Some(45.0));
    }

    #[test]
    fn max_visible_respected() {
        let mut nc = NotificationCenter::new();
        nc.max_visible = 2;
        nc.show_notification("a", NotificationSeverity::Info, vec![]);
        nc.show_notification("b", NotificationSeverity::Info, vec![]);
        nc.show_notification("c", NotificationSeverity::Info, vec![]);
        assert_eq!(nc.visible_toasts().len(), 2);
    }

    #[test]
    fn unread_count() {
        let mut nc = NotificationCenter::new();
        nc.show_notification("one", NotificationSeverity::Info, vec![]);
        nc.show_notification("two", NotificationSeverity::Warning, vec![]);
        assert_eq!(nc.unread_count(), 2);
    }
}
