//! Window title bar with menu bar and window controls.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::draw::{CursorIcon, DrawContext, IconId};
use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, MouseButton, UiEvent, Widget};

/// A top-level menu in the menu bar.
#[derive(Clone, Debug)]
pub struct MenuBarItem {
    pub label: String,
}

impl MenuBarItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

/// The platform for determining window control style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Platform {
    #[default]
    MacOS,
    Windows,
    Linux,
}

/// The window title bar.
#[allow(dead_code)]
pub struct TitleBar<F: FnMut(usize)> {
    pub title: String,
    pub menus: Vec<MenuBarItem>,
    pub platform: Platform,
    pub on_menu_click: F,

    height: f32,
    font_size: f32,
    hovered_menu: Option<usize>,
    hovered_control: Option<usize>,

    active_bg: Color,
    active_fg: Color,
    inactive_bg: Color,
    inactive_fg: Color,
    border_color: Color,
    menu_hover_bg: Color,
    is_active: bool,

    // Window controls
    close_hover_bg: Color,
    minimize_hover_bg: Color,
    maximize_hover_bg: Color,
}

impl<F: FnMut(usize)> TitleBar<F> {
    pub fn new(title: impl Into<String>, on_menu_click: F) -> Self {
        Self {
            title: title.into(),
            menus: default_menus(),
            platform: Platform::default(),
            on_menu_click,
            height: 30.0,
            font_size: 12.0,
            hovered_menu: None,
            hovered_control: None,
            active_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            active_fg: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            inactive_bg: Color::from_hex("#3c3c3c99").unwrap_or(Color::BLACK),
            inactive_fg: Color::from_hex("#cccccc99").unwrap_or(Color::WHITE),
            border_color: Color::from_hex("#2b2b2b").unwrap_or(Color::BLACK),
            menu_hover_bg: Color::from_hex("#505050").unwrap_or(Color::BLACK),
            is_active: true,
            close_hover_bg: Color::from_hex("#e81123").unwrap_or(Color::BLACK),
            minimize_hover_bg: Color::from_hex("#505050").unwrap_or(Color::BLACK),
            maximize_hover_bg: Color::from_hex("#505050").unwrap_or(Color::BLACK),
        }
    }

    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
    }

    #[allow(clippy::cast_precision_loss)]
    fn menu_rects(&self, rect: Rect) -> Vec<Rect> {
        let start_x = if self.platform == Platform::MacOS {
            rect.x + 78.0
        } else {
            rect.x + 8.0
        };
        let mut x = start_x;
        self.menus
            .iter()
            .map(|m| {
                let w = m.label.len() as f32 * self.font_size * 0.6 + 16.0;
                let r = Rect::new(x, rect.y, w, rect.height);
                x += w;
                r
            })
            .collect()
    }

    fn traffic_light_rects(&self, rect: Rect) -> [Rect; 3] {
        let size = 12.0;
        let y = rect.y + (rect.height - size) / 2.0;
        let gap = 8.0;
        [
            Rect::new(rect.x + 12.0, y, size, size),
            Rect::new(rect.x + 12.0 + size + gap, y, size, size),
            Rect::new(rect.x + 12.0 + (size + gap) * 2.0, y, size, size),
        ]
    }

    fn windows_control_rects(&self, rect: Rect) -> [Rect; 3] {
        let w = 46.0;
        let h = rect.height;
        let base_x = rect.x + rect.width - w * 3.0;
        [
            Rect::new(base_x, rect.y, w, h),           // minimize
            Rect::new(base_x + w, rect.y, w, h),       // maximize
            Rect::new(base_x + w * 2.0, rect.y, w, h), // close
        ]
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn render_draw(&self, ctx: &mut DrawContext, rect: Rect) {
        let bg = if self.is_active {
            self.active_bg
        } else {
            self.inactive_bg
        };
        let fg = if self.is_active {
            self.active_fg
        } else {
            self.inactive_fg
        };

        // Background
        ctx.draw_rect(rect, bg, 0.0);

        // Bottom border
        let border = Rect::new(rect.x, rect.y + rect.height - 1.0, rect.width, 1.0);
        ctx.draw_rect(border, self.border_color, 0.0);

        match self.platform {
            Platform::MacOS => {
                // Traffic light buttons
                let lights = self.traffic_light_rects(rect);
                let colors = [
                    Color::from_hex("#ff5f57").unwrap_or(Color::WHITE),
                    Color::from_hex("#febc2e").unwrap_or(Color::WHITE),
                    Color::from_hex("#28c840").unwrap_or(Color::WHITE),
                ];
                for (light, color) in lights.iter().zip(colors.iter()) {
                    ctx.draw_rect(*light, *color, 6.0);
                }

                // Centered title
                let title_w = self.title.len() as f32 * self.font_size * 0.6;
                let title_x = rect.x + (rect.width - title_w) / 2.0;
                let title_y = rect.y + (rect.height - self.font_size) / 2.0;
                ctx.draw_text(
                    &self.title,
                    (title_x, title_y),
                    fg,
                    self.font_size,
                    false,
                    false,
                );
            }
            Platform::Windows | Platform::Linux => {
                // App icon
                ctx.draw_icon(IconId::Gear, (rect.x + 8.0, rect.y + 7.0), 16.0, fg);

                // Title (after menus)
                let menu_rects = self.menu_rects(rect);
                let last_menu_end = menu_rects.last().map_or(rect.x + 8.0, |r| r.right());
                let title_w = self.title.len() as f32 * self.font_size * 0.6;
                let available = rect.width - last_menu_end + rect.x;
                let title_x = last_menu_end + (available - title_w).max(0.0) / 2.0;
                let title_y = rect.y + (rect.height - self.font_size) / 2.0;
                ctx.draw_text(
                    &self.title,
                    (title_x, title_y),
                    fg,
                    self.font_size,
                    false,
                    false,
                );

                // Window controls (minimize, maximize, close)
                let controls = self.windows_control_rects(rect);
                let icons = [IconId::Close, IconId::Close, IconId::Close]; // Placeholder shapes

                for (ci, ctrl) in controls.iter().enumerate() {
                    let hover_bg = match ci {
                        0 => self.minimize_hover_bg,
                        1 => self.maximize_hover_bg,
                        2 => self.close_hover_bg,
                        _ => self.minimize_hover_bg,
                    };
                    if self.hovered_control == Some(ci) {
                        ctx.draw_rect(*ctrl, hover_bg, 0.0);
                    }
                    let icon_x = ctrl.x + (ctrl.width - 10.0) / 2.0;
                    let icon_y = ctrl.y + (ctrl.height - 10.0) / 2.0;
                    ctx.draw_icon(icons[ci], (icon_x, icon_y), 10.0, fg);
                }
            }
        }

        // Menu bar items
        let menu_rects = self.menu_rects(rect);
        for (i, mr) in menu_rects.iter().enumerate() {
            if self.hovered_menu == Some(i) {
                ctx.draw_rect(*mr, self.menu_hover_bg, 2.0);
            }
            let text_y = mr.y + (mr.height - self.font_size) / 2.0;
            let text_x = mr.x + 8.0;
            ctx.draw_text(
                &self.menus[i].label,
                (text_x, text_y),
                fg,
                self.font_size,
                false,
                false,
            );
        }

        if self.hovered_menu.is_some() {
            ctx.set_cursor(CursorIcon::Pointer);
        }
    }
}

impl<F: FnMut(usize)> Widget for TitleBar<F> {
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Fixed(self.height),
            ..LayoutNode::default()
        }
    }

    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        let mut rr = sidex_gpu::RectRenderer::new();
        let bg = if self.is_active {
            self.active_bg
        } else {
            self.inactive_bg
        };
        rr.draw_rect(rect.x, rect.y, rect.width, rect.height, bg, 0.0);
        rr.draw_rect(
            rect.x,
            rect.y + rect.height - 1.0,
            rect.width,
            1.0,
            self.border_color,
            0.0,
        );
        if self.platform == Platform::MacOS {
            let lights = self.traffic_light_rects(rect);
            let colors = [
                Color::from_hex("#ff5f57").unwrap_or(Color::WHITE),
                Color::from_hex("#febc2e").unwrap_or(Color::WHITE),
                Color::from_hex("#28c840").unwrap_or(Color::WHITE),
            ];
            for (light, color) in lights.iter().zip(colors.iter()) {
                rr.draw_rect(light.x, light.y, light.width, light.height, *color, 6.0);
            }
        }
        let menu_rects = self.menu_rects(rect);
        for (i, mr) in menu_rects.iter().enumerate() {
            if self.hovered_menu == Some(i) {
                rr.draw_rect(mr.x, mr.y, mr.width, mr.height, self.menu_hover_bg, 2.0);
            }
        }
        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        let menu_rects = self.menu_rects(rect);
        match event {
            UiEvent::MouseMove { x, y } => {
                self.hovered_menu = menu_rects.iter().position(|r| r.contains(*x, *y));
                if self.platform != Platform::MacOS {
                    let controls = self.windows_control_rects(rect);
                    self.hovered_control = controls.iter().position(|r| r.contains(*x, *y));
                }
                EventResult::Ignored
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => {
                if let Some(idx) = menu_rects.iter().position(|r| r.contains(*x, *y)) {
                    (self.on_menu_click)(idx);
                    return EventResult::Handled;
                }
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }
}

fn default_menus() -> Vec<MenuBarItem> {
    [
        "File",
        "Edit",
        "Selection",
        "View",
        "Go",
        "Run",
        "Terminal",
        "Help",
    ]
    .iter()
    .map(|&s| MenuBarItem::new(s))
    .collect()
}
