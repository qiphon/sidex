//! Editor tab bar widget with rendering, drag-reorder, overflow scroll,
//! pinned/preview tabs, dirty indicators, context menu, and configurable sizing.

use std::path::PathBuf;
use sidex_gpu::color::Color;
use crate::layout::Rect;

// ── Render output primitives ────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct RectInstance { pub rect: Rect, pub color: Color, pub corner_radius: f32 }

#[derive(Clone, Debug)]
pub struct TextInstance {
    pub text: String, pub x: f32, pub y: f32,
    pub color: Color, pub size: f32, pub bold: bool, pub italic: bool,
}

#[derive(Clone, Debug)]
pub struct IconInstance { pub icon: String, pub x: f32, pub y: f32, pub size: f32, pub color: Color }

#[derive(Clone, Debug, Default)]
pub struct TabBarRenderOutput {
    pub rects: Vec<RectInstance>,
    pub texts: Vec<TextInstance>,
    pub icons: Vec<IconInstance>,
}

impl TabBarRenderOutput {
    fn rect(&mut self, r: Rect, color: Color, cr: f32) {
        self.rects.push(RectInstance { rect: r, color, corner_radius: cr });
    }
    fn icon(&mut self, name: &str, x: f32, y: f32, size: f32, color: Color) {
        self.icons.push(IconInstance { icon: name.into(), x, y, size, color });
    }
    fn text(&mut self, t: &str, x: f32, y: f32, color: Color, size: f32, italic: bool) {
        self.texts.push(TextInstance { text: t.into(), x, y, color, size, bold: false, italic });
    }
}

// ── Tab sizing ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabSizing { #[default] Fit, Shrink, Fixed }

impl TabSizing {
    pub fn from_str(s: &str) -> Self {
        match s { "shrink" => Self::Shrink, "fixed" => Self::Fixed, _ => Self::Fit }
    }
}

// ── Tab descriptor ──────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Tab {
    pub id: String, pub label: String, pub path: Option<PathBuf>, pub icon: String,
    pub is_active: bool, pub is_dirty: bool, pub is_pinned: bool, pub is_preview: bool,
    pub description: Option<String>,
}

impl Tab {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(), label: label.into(), path: None, icon: "file".into(),
            is_active: false, is_dirty: false, is_pinned: false, is_preview: false,
            description: None,
        }
    }
}

// ── Context menu actions ────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabBarContextAction { Close, CloseOthers, CloseToRight, CloseAll, CloseSaved, Pin, Unpin, Split }

// ── Editor theme subset ─────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct EditorTheme {
    pub tab_active_bg: Color, pub tab_inactive_bg: Color, pub tab_hover_bg: Color,
    pub tab_active_fg: Color, pub tab_inactive_fg: Color, pub tab_border: Color,
    pub tab_active_top_border: Color, pub tab_dirty_dot: Color,
    pub tab_close_fg: Color, pub tab_close_hover_bg: Color, pub tab_drop_indicator: Color,
}

impl Default for EditorTheme {
    fn default() -> Self {
        let h = |s| Color::from_hex(s).unwrap_or(Color::BLACK);
        Self {
            tab_active_bg: h("#1e1e1e"), tab_inactive_bg: h("#2d2d2d"),
            tab_hover_bg: h("#2a2d2e"), tab_active_fg: Color::WHITE,
            tab_inactive_fg: h("#ffffff80"), tab_border: h("#252526"),
            tab_active_top_border: h("#007acc"), tab_dirty_dot: h("#e8e8e8"),
            tab_close_fg: h("#cccccc"), tab_close_hover_bg: h("#404040"),
            tab_drop_indicator: h("#007acc"),
        }
    }
}

// ── Tab bar ─────────────────────────────────────────────────────────────────

pub struct TabBar {
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    pub scroll_offset: f32,
    pub show_tab_close_button: bool,
    pub sizing: TabSizing,
    pub tab_height: f32,
    pub tab_min_width: f32,
    pub tab_max_width: f32,
    pub pinned_tab_width: f32,
    pub fixed_tab_width: f32,
    pub font_size: f32,
    hovered_tab: Option<usize>,
    hovered_close: Option<usize>,
    drag_source: Option<usize>,
    drag_x: f32,
}

impl TabBar {
    pub fn new(tabs: Vec<Tab>, active_tab: usize) -> Self {
        Self {
            tabs, active_tab, scroll_offset: 0.0, show_tab_close_button: true,
            sizing: TabSizing::Fit, tab_height: 35.0, tab_min_width: 80.0,
            tab_max_width: 200.0, pinned_tab_width: 42.0, fixed_tab_width: 120.0,
            font_size: 12.0, hovered_tab: None, hovered_close: None,
            drag_source: None, drag_x: 0.0,
        }
    }

    fn tab_width(&self, tab: &Tab, cw: f32) -> f32 {
        if tab.is_pinned { return self.pinned_tab_width; }
        match self.sizing {
            TabSizing::Fixed => self.fixed_tab_width,
            _ => {
                let np = self.tabs.iter().filter(|t| !t.is_pinned).count();
                let pt = self.tabs.iter().filter(|t| t.is_pinned).count() as f32
                    * self.pinned_tab_width;
                let per = if np > 0 { (cw - pt).max(0.0) / np as f32 } else { self.tab_max_width };
                if self.sizing == TabSizing::Shrink {
                    per.clamp(self.tab_min_width, self.tab_max_width)
                } else {
                    per.min(self.tab_max_width)
                }
            }
        }
    }

    fn total_width(&self, cw: f32) -> f32 {
        self.tabs.iter().map(|t| self.tab_width(t, cw)).sum()
    }

    pub fn has_overflow(&self, cw: f32) -> bool { self.total_width(cw) > cw }

    pub fn ensure_active_visible(&mut self, cw: f32) {
        let (start, w) = self.tab_x_and_width(self.active_tab, cw);
        if start < self.scroll_offset {
            self.scroll_offset = start;
        } else if start + w > self.scroll_offset + cw {
            self.scroll_offset = start + w - cw;
        }
    }

    fn tab_x_and_width(&self, index: usize, cw: f32) -> (f32, f32) {
        let mut x = 0.0_f32;
        for (i, tab) in self.tabs.iter().enumerate() {
            let w = self.tab_width(tab, cw);
            if i == index { return (x, w); }
            x += w;
        }
        (x, 0.0)
    }

    pub fn tab_index_at_x(&self, px: f32, cw: f32) -> Option<usize> {
        let adj = px + self.scroll_offset;
        let mut cursor = 0.0_f32;
        for (i, tab) in self.tabs.iter().enumerate() {
            let w = self.tab_width(tab, cw);
            if adj >= cursor && adj < cursor + w { return Some(i); }
            cursor += w;
        }
        None
    }

    pub fn begin_drag(&mut self, index: usize, x: f32) { self.drag_source = Some(index); self.drag_x = x; }
    pub fn update_drag(&mut self, x: f32) { self.drag_x = x; }

    pub fn end_drag(&mut self, cw: f32) -> Option<(usize, usize)> {
        let src = self.drag_source.take()?;
        let tgt = self.drop_index(cw)?;
        if tgt != src && tgt != src + 1 {
            Some((src, if tgt > src { tgt - 1 } else { tgt }))
        } else { None }
    }

    fn drop_index(&self, cw: f32) -> Option<usize> {
        let adj = self.drag_x + self.scroll_offset;
        let mut cursor = 0.0_f32;
        for (i, tab) in self.tabs.iter().enumerate() {
            let w = self.tab_width(tab, cw);
            if adj < cursor + w / 2.0 { return Some(i); }
            cursor += w;
        }
        Some(self.tabs.len())
    }

    pub fn set_hover(&mut self, tab: Option<usize>, close: Option<usize>) {
        self.hovered_tab = tab; self.hovered_close = close;
    }

    pub fn context_menu_actions(&self, index: usize) -> Vec<TabBarContextAction> {
        use TabBarContextAction::*;
        let pinned = self.tabs.get(index).map_or(false, |t| t.is_pinned);
        vec![Close, CloseOthers, CloseToRight, CloseAll, CloseSaved,
             if pinned { Unpin } else { Pin }, Split]
    }
}

// ── Rendering ───────────────────────────────────────────────────────────────

pub fn render_tab_bar(bar: &TabBar, bounds: Rect, theme: &EditorTheme) -> TabBarRenderOutput {
    let mut out = TabBarRenderOutput::default();
    out.rect(Rect::new(bounds.x, bounds.y, bounds.width, bar.tab_height), theme.tab_inactive_bg, 0.0);

    let mut x = bounds.x - bar.scroll_offset;
    for (i, tab) in bar.tabs.iter().enumerate() {
        let w = bar.tab_width(tab, bounds.width);
        let tr = Rect::new(x, bounds.y, w, bar.tab_height);
        x += w;
        if tr.x + tr.width < bounds.x || tr.x > bounds.x + bounds.width { continue; }

        let active = i == bar.active_tab;
        let hovered = bar.hovered_tab == Some(i) && !active;
        let bg = if active { theme.tab_active_bg } else if hovered { theme.tab_hover_bg } else { theme.tab_inactive_bg };
        let fg = if active { theme.tab_active_fg } else { theme.tab_inactive_fg };

        out.rect(tr, bg, 0.0);
        out.rect(Rect::new(tr.x + w - 1.0, tr.y + 4.0, 1.0, tr.height - 8.0), theme.tab_border, 0.0);
        if active {
            out.rect(Rect::new(tr.x, tr.y, w, 2.0), theme.tab_active_top_border, 0.0);
        }

        if tab.is_pinned {
            out.icon(&tab.icon, tr.x + (w - 14.0) / 2.0, tr.y + (tr.height - 14.0) / 2.0, 14.0, fg);
            if tab.is_dirty {
                out.rect(Rect::new(tr.x + w - 10.0, tr.y + 4.0, 6.0, 6.0), theme.tab_dirty_dot, 3.0);
            }
        } else {
            out.icon(&tab.icon, tr.x + 8.0, tr.y + (tr.height - 14.0) / 2.0, 14.0, fg);
            out.text(&tab.label, tr.x + 26.0, tr.y + (tr.height - bar.font_size) / 2.0, fg, bar.font_size, tab.is_preview);
            render_close_area(&mut out, bar, theme, i, tab, &tr, active);
        }
    }

    if bar.has_overflow(bounds.width) {
        let ar = Rect::new(bounds.x + bounds.width - 28.0, bounds.y, 28.0, bar.tab_height);
        out.rect(ar, theme.tab_active_bg, 0.0);
        out.icon("chevron-right", ar.x + 7.0, bounds.y + (bar.tab_height - 14.0) / 2.0, 14.0, theme.tab_active_fg);
    }

    if let Some(src) = bar.drag_source {
        if let Some(tgt) = bar.drop_index(bounds.width) {
            if tgt != src && tgt != src + 1 {
                let mut dx = bounds.x - bar.scroll_offset;
                for (i, t) in bar.tabs.iter().enumerate() {
                    if i == tgt { break; }
                    dx += bar.tab_width(t, bounds.width);
                }
                out.rect(Rect::new(dx - 1.0, bounds.y, 2.0, bar.tab_height), theme.tab_drop_indicator, 0.0);
            }
        }
    }
    out
}

fn render_close_area(
    out: &mut TabBarRenderOutput, bar: &TabBar, theme: &EditorTheme,
    i: usize, tab: &Tab, tr: &Rect, active: bool,
) {
    let cs = 16.0;
    let cx = tr.x + tr.width - cs - 8.0;
    let cy = tr.y + (tr.height - cs) / 2.0;
    let show = bar.show_tab_close_button && (active || bar.hovered_tab == Some(i));
    let close_hovered = bar.hovered_close == Some(i);

    if tab.is_dirty {
        if show && close_hovered {
            out.icon("close", cx + 2.0, cy + 2.0, 12.0, theme.tab_close_fg);
        } else {
            out.rect(Rect::new(cx + 4.0, cy + 4.0, 8.0, 8.0), theme.tab_dirty_dot, 4.0);
        }
    } else if show {
        if close_hovered { out.rect(Rect::new(cx, cy, cs, cs), theme.tab_close_hover_bg, 2.0); }
        out.icon("close", cx + 2.0, cy + 2.0, 12.0, theme.tab_close_fg);
    }
}
