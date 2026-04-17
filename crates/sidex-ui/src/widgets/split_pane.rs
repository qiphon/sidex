//! Resizable split-pane container with draggable divider.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::layout::{Direction, LayoutNode, Rect, Size};
use crate::widget::{EventResult, MouseButton, UiEvent, Widget};

/// A split container with a draggable divider between child panes.
pub struct SplitPane<F: FnMut(&[f32])> {
    pub direction: Direction,
    /// Flex weights for each child pane.
    pub sizes: Vec<f32>,
    pub on_resize: F,
    /// Minimum size (pixels) for each pane.
    pub min_sizes: Vec<f32>,
    /// Maximum size (pixels) for each pane.
    pub max_sizes: Vec<Option<f32>>,

    divider_thickness: f32,
    divider_color: Color,
    divider_hover_color: Color,

    dragging_index: Option<usize>,
    hover_index: Option<usize>,
}

impl<F: FnMut(&[f32])> SplitPane<F> {
    pub fn new(sizes: Vec<f32>, on_resize: F) -> Self {
        let len = sizes.len();
        Self {
            direction: Direction::Row,
            sizes,
            on_resize,
            min_sizes: vec![100.0; len],
            max_sizes: vec![None; len],
            divider_thickness: 4.0,
            divider_color: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            divider_hover_color: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            dragging_index: None,
            hover_index: None,
        }
    }

    pub fn column(mut self) -> Self {
        self.direction = Direction::Column;
        self
    }

    /// Returns the pixel sizes of each pane given the total available space.
    fn pane_pixel_sizes(&self, total: f32) -> Vec<f32> {
        let dividers_total = (self.sizes.len().saturating_sub(1)) as f32 * self.divider_thickness;
        let available = (total - dividers_total).max(0.0);
        let weight_sum: f32 = self.sizes.iter().sum();
        if weight_sum <= 0.0 {
            return vec![0.0; self.sizes.len()];
        }
        self.sizes
            .iter()
            .enumerate()
            .map(|(i, &w)| {
                let raw = available * w / weight_sum;
                let clamped = raw.max(self.min_sizes.get(i).copied().unwrap_or(0.0));
                if let Some(max) = self.max_sizes.get(i).and_then(|m| *m) {
                    clamped.min(max)
                } else {
                    clamped
                }
            })
            .collect()
    }

    fn divider_rects(&self, rect: Rect) -> Vec<Rect> {
        let is_row = self.direction == Direction::Row;
        let total = if is_row { rect.width } else { rect.height };
        let pane_sizes = self.pane_pixel_sizes(total);
        let mut result = Vec::new();
        let mut cursor = if is_row { rect.x } else { rect.y };

        for (i, &ps) in pane_sizes.iter().enumerate() {
            cursor += ps;
            if i < pane_sizes.len() - 1 {
                let dr = if is_row {
                    Rect::new(cursor, rect.y, self.divider_thickness, rect.height)
                } else {
                    Rect::new(rect.x, cursor, rect.width, self.divider_thickness)
                };
                result.push(dr);
                cursor += self.divider_thickness;
            }
        }
        result
    }
}

impl<F: FnMut(&[f32])> Widget for SplitPane<F> {
    fn layout(&self) -> LayoutNode {
        let children = self
            .sizes
            .iter()
            .map(|&w| LayoutNode {
                size: Size::Flex(w),
                ..LayoutNode::default()
            })
            .collect();
        LayoutNode {
            direction: self.direction,
            size: Size::Flex(1.0),
            children,
            ..LayoutNode::default()
        }
    }

    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        let mut rr = sidex_gpu::RectRenderer::new();
        let dividers = self.divider_rects(rect);

        for (i, dr) in dividers.iter().enumerate() {
            let color = if self.dragging_index == Some(i) || self.hover_index == Some(i) {
                self.divider_hover_color
            } else {
                self.divider_color
            };
            rr.draw_rect(dr.x, dr.y, dr.width, dr.height, color, 0.0);
        }
        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        let dividers = self.divider_rects(rect);

        match event {
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => {
                for (i, dr) in dividers.iter().enumerate() {
                    if dr.contains(*x, *y) {
                        self.dragging_index = Some(i);
                        return EventResult::Handled;
                    }
                }
                EventResult::Ignored
            }
            UiEvent::MouseMove { x, y } => {
                if let Some(drag_idx) = self.dragging_index {
                    let is_row = self.direction == Direction::Row;
                    let total = if is_row { rect.width } else { rect.height };
                    let dividers_total =
                        (self.sizes.len().saturating_sub(1)) as f32 * self.divider_thickness;
                    let available = (total - dividers_total).max(0.0);
                    let weight_sum: f32 = self.sizes.iter().sum();

                    let pos = if is_row { *x - rect.x } else { *y - rect.y };
                    let pane_sizes = self.pane_pixel_sizes(total);
                    let prev_end: f32 = pane_sizes[..drag_idx].iter().sum::<f32>()
                        + drag_idx as f32 * self.divider_thickness;
                    let combined = pane_sizes[drag_idx] + pane_sizes[drag_idx + 1];

                    let new_left = (pos - prev_end).clamp(
                        self.min_sizes.get(drag_idx).copied().unwrap_or(0.0),
                        combined - self.min_sizes.get(drag_idx + 1).copied().unwrap_or(0.0),
                    );
                    let new_right = combined - new_left;

                    if available > 0.0 {
                        self.sizes[drag_idx] = new_left / available * weight_sum;
                        self.sizes[drag_idx + 1] = new_right / available * weight_sum;
                        (self.on_resize)(&self.sizes);
                    }
                    EventResult::Handled
                } else {
                    self.hover_index = dividers.iter().position(|dr| dr.contains(*x, *y));
                    EventResult::Ignored
                }
            }
            UiEvent::MouseUp { .. } if self.dragging_index.is_some() => {
                self.dragging_index = None;
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}
