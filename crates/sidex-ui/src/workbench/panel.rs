//! Bottom panel: Terminal, Output, Problems, Debug Console.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, MouseButton, UiEvent, Widget};

/// A tab in the bottom panel.
#[derive(Clone, Debug)]
pub struct PanelTab {
    pub id: String,
    pub label: String,
    pub badge_count: Option<u32>,
}

impl PanelTab {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            badge_count: None,
        }
    }

    pub fn with_badge(mut self, count: u32) -> Self {
        self.badge_count = Some(count);
        self
    }
}

/// The bottom panel area of the workbench.
#[allow(dead_code)]
pub struct Panel<F: FnMut(usize)> {
    pub tabs: Vec<PanelTab>,
    pub active_tab: usize,
    pub on_tab_select: F,

    pub visible: bool,
    pub maximized: bool,
    height: f32,
    min_height: f32,
    tab_bar_height: f32,

    background: Color,
    border_color: Color,
    tab_active_fg: Color,
    tab_active_border: Color,
    tab_inactive_fg: Color,
    badge_bg: Color,
    badge_fg: Color,
    hovered_tab: Option<usize>,
}

impl<F: FnMut(usize)> Panel<F> {
    pub fn new(tabs: Vec<PanelTab>, on_tab_select: F) -> Self {
        Self {
            tabs,
            active_tab: 0,
            on_tab_select,
            visible: true,
            maximized: false,
            height: 250.0,
            min_height: 100.0,
            tab_bar_height: 35.0,
            background: Color::from_hex("#1e1e1e").unwrap_or(Color::BLACK),
            border_color: Color::from_hex("#80808059").unwrap_or(Color::BLACK),
            tab_active_fg: Color::from_hex("#e7e7e7").unwrap_or(Color::WHITE),
            tab_active_border: Color::from_hex("#e7e7e7").unwrap_or(Color::WHITE),
            tab_inactive_fg: Color::from_hex("#e7e7e799").unwrap_or(Color::WHITE),
            badge_bg: Color::from_hex("#4d4d4d").unwrap_or(Color::BLACK),
            badge_fg: Color::WHITE,
            hovered_tab: None,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn toggle_maximize(&mut self) {
        self.maximized = !self.maximized;
    }

    #[allow(clippy::cast_precision_loss)]
    fn tab_rects(&self, rect: Rect) -> Vec<Rect> {
        let mut x = rect.x + 8.0;
        let tab_w = 100.0;
        self.tabs
            .iter()
            .map(|_| {
                let r = Rect::new(x, rect.y, tab_w, self.tab_bar_height);
                x += tab_w;
                r
            })
            .collect()
    }
}

impl<F: FnMut(usize)> Widget for Panel<F> {
    fn layout(&self) -> LayoutNode {
        if self.visible {
            LayoutNode {
                size: Size::Fixed(self.height),
                min_size: Some(self.min_height),
                ..LayoutNode::default()
            }
        } else {
            LayoutNode {
                size: Size::Fixed(0.0),
                ..LayoutNode::default()
            }
        }
    }

    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        if !self.visible {
            return;
        }
        let mut rr = sidex_gpu::RectRenderer::new();

        rr.draw_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            self.background,
            0.0,
        );
        rr.draw_rect(rect.x, rect.y, rect.width, 1.0, self.border_color, 0.0);

        let tab_rects = self.tab_rects(rect);
        for (i, tr) in tab_rects.iter().enumerate() {
            let is_active = i == self.active_tab;
            if is_active {
                rr.draw_rect(
                    tr.x,
                    tr.y + tr.height - 2.0,
                    tr.width,
                    2.0,
                    self.tab_active_border,
                    0.0,
                );
            }

            if let Some(count) = self.tabs[i].badge_count {
                if count > 0 {
                    let badge_s = 16.0;
                    let bx = tr.x + tr.width - badge_s - 4.0;
                    let by = tr.y + (tr.height - badge_s) / 2.0;
                    rr.draw_rect(bx, by, badge_s, badge_s, self.badge_bg, badge_s / 2.0);
                }
            }
        }
        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        if !self.visible {
            return EventResult::Ignored;
        }
        let tab_rects = self.tab_rects(rect);

        match event {
            UiEvent::MouseMove { x, y } => {
                self.hovered_tab = tab_rects.iter().position(|r| r.contains(*x, *y));
                EventResult::Ignored
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => {
                if let Some(idx) = tab_rects.iter().position(|r| r.contains(*x, *y)) {
                    self.active_tab = idx;
                    (self.on_tab_select)(idx);
                    return EventResult::Handled;
                }
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }
}
