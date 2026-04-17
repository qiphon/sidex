//! GPU-accelerated UI widget renderer.
//!
//! Renders all editor UI widgets (buttons, inputs, tabs, tree items, tooltips,
//! scrollbars, notifications, progress bars, etc.) into a [`RenderBatch`].
//! Each widget is drawn as a combination of rectangles and text primitives.

use crate::editor_compositor::Rect;
use crate::renderer::RenderBatch;
use crate::texture_cache::TextureId;

// ── Orientation ─────────────────────────────────────────────────────────────

/// Layout orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

/// Scrollbar orientation for UI rendering (distinct from the compositor type).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiScrollbarOrientation {
    Vertical,
    Horizontal,
}

// ── Widget states ───────────────────────────────────────────────────────────

/// Interactive state of a button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    Normal,
    Hovered,
    Pressed,
    Disabled,
}

// ── Style types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ButtonStyle {
    pub bg: [f32; 4],
    pub fg: [f32; 4],
    pub border: [f32; 4],
    pub corner_radius: f32,
    pub padding: f32,
}

impl Default for ButtonStyle {
    fn default() -> Self {
        Self {
            bg: [0.25, 0.25, 0.25, 1.0],
            fg: [0.9, 0.9, 0.9, 1.0],
            border: [0.4, 0.4, 0.4, 1.0],
            corner_radius: 4.0,
            padding: 6.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InputStyle {
    pub bg: [f32; 4],
    pub fg: [f32; 4],
    pub border: [f32; 4],
    pub cursor_color: [f32; 4],
    pub selection_color: [f32; 4],
    pub placeholder_color: [f32; 4],
}

impl Default for InputStyle {
    fn default() -> Self {
        Self {
            bg: [0.15, 0.15, 0.15, 1.0],
            fg: [0.9, 0.9, 0.9, 1.0],
            border: [0.35, 0.35, 0.35, 1.0],
            cursor_color: [0.9, 0.9, 0.9, 1.0],
            selection_color: [0.17, 0.34, 0.56, 0.6],
            placeholder_color: [0.5, 0.5, 0.5, 0.6],
        }
    }
}

#[derive(Debug, Clone)]
pub struct CheckboxStyle {
    pub bg: [f32; 4],
    pub check_color: [f32; 4],
    pub border: [f32; 4],
    pub size: f32,
}

impl Default for CheckboxStyle {
    fn default() -> Self {
        Self {
            bg: [0.15, 0.15, 0.15, 1.0],
            check_color: [0.4, 0.6, 1.0, 1.0],
            border: [0.4, 0.4, 0.4, 1.0],
            size: 16.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DropdownStyle {
    pub bg: [f32; 4],
    pub fg: [f32; 4],
    pub border: [f32; 4],
    pub item_hover_bg: [f32; 4],
    pub corner_radius: f32,
}

impl Default for DropdownStyle {
    fn default() -> Self {
        Self {
            bg: [0.18, 0.18, 0.18, 1.0],
            fg: [0.9, 0.9, 0.9, 1.0],
            border: [0.35, 0.35, 0.35, 1.0],
            item_hover_bg: [0.25, 0.25, 0.25, 1.0],
            corner_radius: 4.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScrollbarStyle {
    pub track_color: [f32; 4],
    pub thumb_color: [f32; 4],
    pub thumb_hover_color: [f32; 4],
    pub corner_radius: f32,
}

impl Default for ScrollbarStyle {
    fn default() -> Self {
        Self {
            track_color: [0.15, 0.15, 0.15, 0.5],
            thumb_color: [0.45, 0.45, 0.45, 0.5],
            thumb_hover_color: [0.55, 0.55, 0.55, 0.7],
            corner_radius: 4.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TabStyle {
    pub active_bg: [f32; 4],
    pub inactive_bg: [f32; 4],
    pub active_fg: [f32; 4],
    pub inactive_fg: [f32; 4],
    pub border: [f32; 4],
    pub close_icon_color: [f32; 4],
    pub dirty_dot_color: [f32; 4],
}

impl Default for TabStyle {
    fn default() -> Self {
        Self {
            active_bg: [0.12, 0.12, 0.12, 1.0],
            inactive_bg: [0.18, 0.18, 0.18, 1.0],
            active_fg: [1.0, 1.0, 1.0, 1.0],
            inactive_fg: [0.6, 0.6, 0.6, 1.0],
            border: [0.25, 0.25, 0.25, 1.0],
            close_icon_color: [0.6, 0.6, 0.6, 1.0],
            dirty_dot_color: [0.9, 0.9, 0.9, 1.0],
        }
    }
}

#[derive(Debug, Clone)]
pub struct TreeStyle {
    pub indent_width: f32,
    pub selected_bg: [f32; 4],
    pub hover_bg: [f32; 4],
    pub fg: [f32; 4],
    pub chevron_color: [f32; 4],
}

impl Default for TreeStyle {
    fn default() -> Self {
        Self {
            indent_width: 16.0,
            selected_bg: [0.17, 0.34, 0.56, 0.4],
            hover_bg: [0.2, 0.2, 0.2, 0.5],
            fg: [0.85, 0.85, 0.85, 1.0],
            chevron_color: [0.6, 0.6, 0.6, 1.0],
        }
    }
}

#[derive(Debug, Clone)]
pub struct TooltipStyle {
    pub bg: [f32; 4],
    pub fg: [f32; 4],
    pub border: [f32; 4],
    pub corner_radius: f32,
    pub padding: f32,
    pub max_width: f32,
}

impl Default for TooltipStyle {
    fn default() -> Self {
        Self {
            bg: [0.22, 0.22, 0.22, 1.0],
            fg: [0.9, 0.9, 0.9, 1.0],
            border: [0.35, 0.35, 0.35, 1.0],
            corner_radius: 4.0,
            padding: 6.0,
            max_width: 400.0,
        }
    }
}

/// Data describing a notification to render.
#[derive(Debug, Clone)]
pub struct NotificationRenderData {
    pub title: String,
    pub message: String,
    pub severity: NotificationSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct NotificationStyle {
    pub bg: [f32; 4],
    pub fg: [f32; 4],
    pub border: [f32; 4],
    pub info_accent: [f32; 4],
    pub warning_accent: [f32; 4],
    pub error_accent: [f32; 4],
    pub corner_radius: f32,
}

impl Default for NotificationStyle {
    fn default() -> Self {
        Self {
            bg: [0.2, 0.2, 0.2, 0.98],
            fg: [0.9, 0.9, 0.9, 1.0],
            border: [0.35, 0.35, 0.35, 1.0],
            info_accent: [0.2, 0.6, 1.0, 1.0],
            warning_accent: [0.9, 0.7, 0.1, 1.0],
            error_accent: [0.9, 0.25, 0.25, 1.0],
            corner_radius: 6.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProgressStyle {
    pub track_color: [f32; 4],
    pub fill_color: [f32; 4],
    pub corner_radius: f32,
    pub height: f32,
}

impl Default for ProgressStyle {
    fn default() -> Self {
        Self {
            track_color: [0.2, 0.2, 0.2, 1.0],
            fill_color: [0.2, 0.6, 1.0, 1.0],
            corner_radius: 2.0,
            height: 4.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SeparatorStyle {
    pub color: [f32; 4],
    pub thickness: f32,
}

impl Default for SeparatorStyle {
    fn default() -> Self {
        Self {
            color: [0.3, 0.3, 0.3, 1.0],
            thickness: 1.0,
        }
    }
}

/// An action button rendered in a panel header.
#[derive(Debug, Clone)]
pub struct ActionButton {
    pub icon: String,
    pub tooltip: String,
}

#[derive(Debug, Clone)]
pub struct PanelHeaderStyle {
    pub bg: [f32; 4],
    pub fg: [f32; 4],
    pub border: [f32; 4],
    pub height: f32,
}

impl Default for PanelHeaderStyle {
    fn default() -> Self {
        Self {
            bg: [0.18, 0.18, 0.18, 1.0],
            fg: [0.85, 0.85, 0.85, 1.0],
            border: [0.3, 0.3, 0.3, 1.0],
            height: 30.0,
        }
    }
}

// ── UiRenderer ──────────────────────────────────────────────────────────────

/// Stateless renderer for all UI widgets. Every method takes a mutable
/// `RenderBatch` and appends the geometry needed to draw the widget.
pub struct UiRenderer;

impl UiRenderer {
    pub fn render_button(
        batch: &mut RenderBatch,
        bounds: Rect,
        _label: &str,
        style: &ButtonStyle,
        state: ButtonState,
    ) {
        let bg = match state {
            ButtonState::Hovered => [
                style.bg[0] + 0.05, style.bg[1] + 0.05,
                style.bg[2] + 0.05, style.bg[3],
            ],
            ButtonState::Pressed => [
                style.bg[0] - 0.03, style.bg[1] - 0.03,
                style.bg[2] - 0.03, style.bg[3],
            ],
            ButtonState::Disabled => [
                style.bg[0], style.bg[1], style.bg[2], style.bg[3] * 0.5,
            ],
            ButtonState::Normal => style.bg,
        };
        batch.draw_rect_bordered(
            bounds.x, bounds.y, bounds.width, bounds.height,
            bg, style.corner_radius, 1.0, style.border,
        );
    }

    pub fn render_text_input(
        batch: &mut RenderBatch,
        bounds: Rect,
        _text: &str,
        cursor: usize,
        style: &InputStyle,
    ) {
        batch.draw_rect_bordered(
            bounds.x, bounds.y, bounds.width, bounds.height,
            style.bg, 3.0, 1.0, style.border,
        );
        let cursor_x = bounds.x + 4.0 + cursor as f32 * 8.0;
        batch.draw_rect(cursor_x, bounds.y + 3.0, 1.5, bounds.height - 6.0, style.cursor_color, 0.0);
    }

    pub fn render_checkbox(
        batch: &mut RenderBatch,
        bounds: Rect,
        checked: bool,
        _label: &str,
        style: &CheckboxStyle,
    ) {
        let s = style.size;
        let bx = bounds.x;
        let by = bounds.y + (bounds.height - s) / 2.0;
        batch.draw_rect_bordered(bx, by, s, s, style.bg, 2.0, 1.0, style.border);
        if checked {
            let inset = s * 0.25;
            batch.draw_rect(
                bx + inset, by + inset, s - inset * 2.0, s - inset * 2.0,
                style.check_color, 1.0,
            );
        }
    }

    pub fn render_dropdown(
        batch: &mut RenderBatch,
        bounds: Rect,
        _selected: &str,
        open: bool,
        items: &[&str],
        style: &DropdownStyle,
    ) {
        batch.draw_rect_bordered(
            bounds.x, bounds.y, bounds.width, bounds.height,
            style.bg, style.corner_radius, 1.0, style.border,
        );
        if open {
            let item_h = 24.0;
            let menu_h = items.len() as f32 * item_h;
            batch.draw_rect_bordered(
                bounds.x, bounds.y + bounds.height,
                bounds.width, menu_h,
                style.bg, style.corner_radius, 1.0, style.border,
            );
        }
    }

    pub fn render_scrollbar(
        batch: &mut RenderBatch,
        bounds: Rect,
        thumb_pos: f32,
        thumb_size: f32,
        orientation: UiScrollbarOrientation,
        style: &ScrollbarStyle,
    ) {
        batch.draw_rect(
            bounds.x, bounds.y, bounds.width, bounds.height,
            style.track_color, style.corner_radius,
        );
        match orientation {
            UiScrollbarOrientation::Vertical => {
                let thumb_y = bounds.y + thumb_pos * (bounds.height - thumb_size);
                batch.draw_rect(
                    bounds.x + 2.0, thumb_y,
                    bounds.width - 4.0, thumb_size,
                    style.thumb_color, style.corner_radius,
                );
            }
            UiScrollbarOrientation::Horizontal => {
                let thumb_x = bounds.x + thumb_pos * (bounds.width - thumb_size);
                batch.draw_rect(
                    thumb_x, bounds.y + 2.0,
                    thumb_size, bounds.height - 4.0,
                    style.thumb_color, style.corner_radius,
                );
            }
        }
    }

    pub fn render_tab(
        batch: &mut RenderBatch,
        bounds: Rect,
        _label: &str,
        _icon: Option<TextureId>,
        is_active: bool,
        is_dirty: bool,
        style: &TabStyle,
    ) {
        let bg = if is_active { style.active_bg } else { style.inactive_bg };
        batch.draw_rect(bounds.x, bounds.y, bounds.width, bounds.height, bg, 0.0);

        batch.draw_rect(bounds.x + bounds.width - 1.0, bounds.y, 1.0, bounds.height, style.border, 0.0);

        if is_active {
            batch.draw_rect(bounds.x, bounds.y + bounds.height - 2.0, bounds.width, 2.0, style.active_fg, 0.0);
        }

        if is_dirty {
            batch.draw_rect(
                bounds.x + bounds.width - 14.0,
                bounds.y + bounds.height / 2.0 - 3.0,
                6.0, 6.0,
                style.dirty_dot_color, 3.0,
            );
        }
    }

    pub fn render_tree_item(
        batch: &mut RenderBatch,
        bounds: Rect,
        indent: u32,
        _label: &str,
        _icon: Option<TextureId>,
        expanded: Option<bool>,
        selected: bool,
        style: &TreeStyle,
    ) {
        if selected {
            batch.draw_rect(bounds.x, bounds.y, bounds.width, bounds.height, style.selected_bg, 0.0);
        }

        let indent_px = indent as f32 * style.indent_width;

        if let Some(exp) = expanded {
            let chev_x = bounds.x + indent_px + 2.0;
            let chev_y = bounds.y + bounds.height / 2.0 - 3.0;
            let chev_size = 6.0;
            if exp {
                batch.draw_rect(chev_x, chev_y, chev_size, chev_size * 0.5, style.chevron_color, 1.0);
            } else {
                batch.draw_rect(chev_x, chev_y, chev_size * 0.5, chev_size, style.chevron_color, 1.0);
            }
        }
    }

    pub fn render_tooltip(
        batch: &mut RenderBatch,
        position: (f32, f32),
        _text: &str,
        style: &TooltipStyle,
    ) {
        let estimated_w = style.max_width.min(200.0);
        let h = 28.0;
        batch.draw_shadow(position.0, position.1, estimated_w, h, 4.0, [0.0, 0.0, 0.0, 0.3]);
        batch.draw_rect_bordered(
            position.0, position.1, estimated_w, h,
            style.bg, style.corner_radius, 1.0, style.border,
        );
    }

    pub fn render_notification(
        batch: &mut RenderBatch,
        bounds: Rect,
        notification: &NotificationRenderData,
        style: &NotificationStyle,
    ) {
        batch.draw_shadow(bounds.x, bounds.y, bounds.width, bounds.height, 6.0, [0.0, 0.0, 0.0, 0.3]);
        batch.draw_rect_bordered(
            bounds.x, bounds.y, bounds.width, bounds.height,
            style.bg, style.corner_radius, 1.0, style.border,
        );
        let accent = match notification.severity {
            NotificationSeverity::Info => style.info_accent,
            NotificationSeverity::Warning => style.warning_accent,
            NotificationSeverity::Error => style.error_accent,
        };
        batch.draw_rect(bounds.x, bounds.y, 3.0, bounds.height, accent, 0.0);
    }

    pub fn render_progress_bar(
        batch: &mut RenderBatch,
        bounds: Rect,
        progress: f32,
        style: &ProgressStyle,
    ) {
        let track_y = bounds.y + (bounds.height - style.height) / 2.0;
        batch.draw_rect(bounds.x, track_y, bounds.width, style.height, style.track_color, style.corner_radius);
        let fill_w = bounds.width * progress.clamp(0.0, 1.0);
        if fill_w > 0.0 {
            batch.draw_rect(bounds.x, track_y, fill_w, style.height, style.fill_color, style.corner_radius);
        }
    }

    pub fn render_badge(
        batch: &mut RenderBatch,
        position: (f32, f32),
        _text: &str,
        color: [f32; 4],
    ) {
        let w = 18.0;
        let h = 16.0;
        batch.draw_rect(position.0, position.1, w, h, color, h / 2.0);
    }

    pub fn render_icon(
        batch: &mut RenderBatch,
        position: (f32, f32),
        size: f32,
        _icon: &str,
        color: [f32; 4],
    ) {
        batch.draw_rect(position.0, position.1, size, size, color, 2.0);
    }

    pub fn render_separator(
        batch: &mut RenderBatch,
        bounds: Rect,
        orientation: Orientation,
        style: &SeparatorStyle,
    ) {
        match orientation {
            Orientation::Horizontal => {
                let y = bounds.y + bounds.height / 2.0;
                batch.draw_rect(bounds.x, y, bounds.width, style.thickness, style.color, 0.0);
            }
            Orientation::Vertical => {
                let x = bounds.x + bounds.width / 2.0;
                batch.draw_rect(x, bounds.y, style.thickness, bounds.height, style.color, 0.0);
            }
        }
    }

    pub fn render_panel_header(
        batch: &mut RenderBatch,
        bounds: Rect,
        _title: &str,
        _actions: &[ActionButton],
        style: &PanelHeaderStyle,
    ) {
        batch.draw_rect(bounds.x, bounds.y, bounds.width, style.height, style.bg, 0.0);
        batch.draw_rect(bounds.x, bounds.y + style.height - 1.0, bounds.width, 1.0, style.border, 0.0);
    }
}
