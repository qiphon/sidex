//! Breadcrumb navigation bar with clickable path segments.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, MouseButton, UiEvent, Widget};

/// A single breadcrumb segment.
#[derive(Clone, Debug)]
pub struct BreadcrumbSegment {
    pub label: String,
    pub icon: Option<String>,
}

impl BreadcrumbSegment {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
        }
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

/// A horizontal breadcrumb bar with clickable path segments.
#[allow(dead_code)]
pub struct Breadcrumbs<F: FnMut(usize)> {
    pub segments: Vec<BreadcrumbSegment>,
    pub on_select: F,

    height: f32,
    font_size: f32,
    separator_width: f32,
    hovered_index: Option<usize>,

    foreground: Color,
    hover_fg: Color,
    separator_color: Color,
    hover_bg: Color,
}

impl<F: FnMut(usize)> Breadcrumbs<F> {
    pub fn new(segments: Vec<BreadcrumbSegment>, on_select: F) -> Self {
        Self {
            segments,
            on_select,
            height: 22.0,
            font_size: 12.0,
            separator_width: 16.0,
            hovered_index: None,
            foreground: Color::from_hex("#cccccccc").unwrap_or(Color::WHITE),
            hover_fg: Color::from_hex("#e0e0e0").unwrap_or(Color::WHITE),
            separator_color: Color::from_hex("#cccccc66").unwrap_or(Color::WHITE),
            hover_bg: Color::from_hex("#2a2d2e").unwrap_or(Color::BLACK),
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn segment_rects(&self, rect: Rect) -> Vec<Rect> {
        let mut rects = Vec::new();
        let mut x = rect.x + 8.0;
        for seg in &self.segments {
            let w = seg.label.len() as f32 * self.font_size * 0.6 + 12.0;
            rects.push(Rect::new(x, rect.y, w, rect.height));
            x += w + self.separator_width;
        }
        rects
    }
}

impl<F: FnMut(usize)> Widget for Breadcrumbs<F> {
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Fixed(self.height),
            ..LayoutNode::default()
        }
    }

    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        let mut rr = sidex_gpu::RectRenderer::new();
        let seg_rects = self.segment_rects(rect);

        for (i, sr) in seg_rects.iter().enumerate() {
            if self.hovered_index == Some(i) {
                rr.draw_rect(sr.x, sr.y, sr.width, sr.height, self.hover_bg, 2.0);
            }

            if i + 1 < seg_rects.len() {
                let sep_x = sr.right() + (self.separator_width - 1.0) / 2.0;
                rr.draw_rect(
                    sep_x,
                    sr.y + 4.0,
                    1.0,
                    sr.height - 8.0,
                    self.separator_color,
                    0.0,
                );
            }
        }
        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        let seg_rects = self.segment_rects(rect);

        match event {
            UiEvent::MouseMove { x, y } => {
                self.hovered_index = seg_rects.iter().position(|r| r.contains(*x, *y));
                EventResult::Ignored
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => {
                if let Some(idx) = seg_rects.iter().position(|r| r.contains(*x, *y)) {
                    (self.on_select)(idx);
                    return EventResult::Handled;
                }
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }
}
