//! Sidebar container with collapsible view sections.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, MouseButton, UiEvent, Widget};

/// A collapsible section within the sidebar.
#[derive(Clone, Debug)]
pub struct SidebarSection {
    pub title: String,
    pub expanded: bool,
    /// Flex weight when expanded (controls proportional sizing).
    pub weight: f32,
}

impl SidebarSection {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            expanded: true,
            weight: 1.0,
        }
    }

    pub fn collapsed(mut self) -> Self {
        self.expanded = false;
        self
    }
}

/// The sidebar panel (file explorer, search, SCM, etc.).
#[allow(dead_code)]
pub struct Sidebar<F: FnMut(usize)> {
    pub sections: Vec<SidebarSection>,
    pub on_toggle_section: F,
    pub visible: bool,

    width: f32,
    section_header_height: f32,
    hovered_header: Option<usize>,

    background: Color,
    foreground: Color,
    header_bg: Color,
    header_fg: Color,
    border_color: Color,
}

impl<F: FnMut(usize)> Sidebar<F> {
    pub fn new(sections: Vec<SidebarSection>, on_toggle_section: F) -> Self {
        Self {
            sections,
            on_toggle_section,
            visible: true,
            width: 250.0,
            section_header_height: 22.0,
            hovered_header: None,
            background: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            header_bg: Color::from_hex("#383838").unwrap_or(Color::BLACK),
            header_fg: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            border_color: Color::from_hex("#2b2b2b").unwrap_or(Color::BLACK),
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    fn header_rects(&self, rect: Rect) -> Vec<Rect> {
        let mut y = rect.y;
        let mut result = Vec::new();
        for section in &self.sections {
            result.push(Rect::new(rect.x, y, rect.width, self.section_header_height));
            y += self.section_header_height;
            if section.expanded {
                let content_h = self.section_content_height(section, rect);
                y += content_h;
            }
        }
        result
    }

    fn section_content_height(&self, section: &SidebarSection, rect: Rect) -> f32 {
        if !section.expanded {
            return 0.0;
        }
        let header_total = self.sections.len() as f32 * self.section_header_height;
        let available = (rect.height - header_total).max(0.0);
        let expanded_weight: f32 = self
            .sections
            .iter()
            .filter(|s| s.expanded)
            .map(|s| s.weight)
            .sum();
        if expanded_weight > 0.0 {
            available * section.weight / expanded_weight
        } else {
            0.0
        }
    }
}

impl<F: FnMut(usize)> Widget for Sidebar<F> {
    fn layout(&self) -> LayoutNode {
        if self.visible {
            LayoutNode {
                size: Size::Fixed(self.width),
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

        let headers = self.header_rects(rect);
        for (i, hr) in headers.iter().enumerate() {
            rr.draw_rect(hr.x, hr.y, hr.width, hr.height, self.header_bg, 0.0);
            rr.draw_rect(
                hr.x,
                hr.y + hr.height - 1.0,
                hr.width,
                1.0,
                self.border_color,
                0.0,
            );

            if self.hovered_header == Some(i) {
                let hover_bg = Color::from_hex("#2a2d2e").unwrap_or(Color::BLACK);
                rr.draw_rect(hr.x, hr.y, hr.width, hr.height, hover_bg, 0.0);
            }
        }

        rr.draw_rect(
            rect.x + rect.width - 1.0,
            rect.y,
            1.0,
            rect.height,
            self.border_color,
            0.0,
        );

        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        if !self.visible {
            return EventResult::Ignored;
        }
        let headers = self.header_rects(rect);

        match event {
            UiEvent::MouseMove { x, y } => {
                self.hovered_header = headers.iter().position(|r| r.contains(*x, *y));
                EventResult::Ignored
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => {
                if let Some(idx) = headers.iter().position(|r| r.contains(*x, *y)) {
                    self.sections[idx].expanded = !self.sections[idx].expanded;
                    (self.on_toggle_section)(idx);
                    return EventResult::Handled;
                }
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }
}
