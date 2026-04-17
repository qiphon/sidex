//! Collapsible tree widget with indent guides, icons, and virtual scrolling.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::draw::{DrawContext, IconId};
use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

/// A node in the tree data model.
pub struct TreeNode<T> {
    pub data: T,
    pub children: Vec<TreeNode<T>>,
    pub children_loaded: bool,
}

impl<T> TreeNode<T> {
    pub fn leaf(data: T) -> Self {
        Self {
            data,
            children: Vec::new(),
            children_loaded: true,
        }
    }

    pub fn branch(data: T, children: Vec<TreeNode<T>>) -> Self {
        Self {
            data,
            children,
            children_loaded: true,
        }
    }

    pub fn lazy(data: T) -> Self {
        Self {
            data,
            children: Vec::new(),
            children_loaded: false,
        }
    }

    pub fn is_leaf(&self) -> bool {
        self.children_loaded && self.children.is_empty()
    }
}

/// Flat representation of a visible tree row for rendering.
struct FlatRow {
    path: Vec<usize>,
    depth: usize,
    has_children: bool,
    is_expanded: bool,
}

/// Pre-rendered description of a tree row.
pub struct TreeRow {
    pub text: String,
    pub icon: Option<String>,
}

/// A tree view with collapsible nodes, indent guides, and keyboard navigation.
#[allow(dead_code)]
pub struct Tree<T, R, E, S>
where
    R: Fn(&T, usize) -> TreeRow,
    E: FnMut(&[usize]),
    S: FnMut(&[usize]),
{
    pub root: Vec<TreeNode<T>>,
    pub render_item: R,
    pub on_toggle: E,
    pub on_select: S,

    expanded: std::collections::HashSet<Vec<usize>>,
    selected_path: Option<Vec<usize>>,

    row_height: f32,
    indent_width: f32,
    scroll_offset: f32,
    focused: bool,
    hovered_index: Option<usize>,
    font_size: f32,

    guide_color: Color,
    selected_bg: Color,
    selected_fg: Color,
    hover_bg: Color,
    foreground: Color,
    chevron_color: Color,
    scrollbar_thumb: Color,
}

impl<T, R, E, S> Tree<T, R, E, S>
where
    R: Fn(&T, usize) -> TreeRow,
    E: FnMut(&[usize]),
    S: FnMut(&[usize]),
{
    pub fn new(root: Vec<TreeNode<T>>, render_item: R, on_toggle: E, on_select: S) -> Self {
        Self {
            root,
            render_item,
            on_toggle,
            on_select,
            expanded: std::collections::HashSet::new(),
            selected_path: None,
            row_height: 22.0,
            indent_width: 16.0,
            scroll_offset: 0.0,
            focused: false,
            hovered_index: None,
            font_size: 13.0,
            guide_color: Color::from_hex("#404040").unwrap_or(Color::WHITE),
            selected_bg: Color::from_hex("#04395e").unwrap_or(Color::BLACK),
            selected_fg: Color::WHITE,
            hover_bg: Color::from_hex("#2a2d2e").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            chevron_color: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            scrollbar_thumb: Color::from_hex("#79797966").unwrap_or(Color::WHITE),
        }
    }

    fn flatten(&self) -> Vec<FlatRow> {
        let mut rows = Vec::new();
        self.flatten_children(&self.root, &mut vec![], 0, &mut rows);
        rows
    }

    fn flatten_children(
        &self,
        nodes: &[TreeNode<T>],
        parent_path: &mut Vec<usize>,
        depth: usize,
        out: &mut Vec<FlatRow>,
    ) {
        for (i, node) in nodes.iter().enumerate() {
            parent_path.push(i);
            let path = parent_path.clone();
            let has_children = !node.is_leaf();
            let is_expanded = self.expanded.contains(&path);
            out.push(FlatRow {
                path: path.clone(),
                depth,
                has_children,
                is_expanded,
            });
            if is_expanded && has_children {
                self.flatten_children(&node.children, parent_path, depth + 1, out);
            }
            parent_path.pop();
        }
    }

    fn node_at_path(&self, path: &[usize]) -> Option<&TreeNode<T>> {
        let mut nodes = &self.root;
        let mut result = None;
        for &idx in path {
            let node = nodes.get(idx)?;
            result = Some(node);
            nodes = &node.children;
        }
        result
    }

    fn toggle_expanded(&mut self, path: &[usize]) {
        let p = path.to_vec();
        if self.expanded.contains(&p) {
            self.expanded.remove(&p);
        } else {
            self.expanded.insert(p);
        }
        (self.on_toggle)(path);
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn render_draw(&self, ctx: &mut DrawContext, rect: Rect) {
        ctx.save();
        ctx.clip(rect);

        let rows = self.flatten();
        let total_height = rows.len() as f32 * self.row_height;

        let first = (self.scroll_offset / self.row_height).floor() as usize;
        let count = (rect.height / self.row_height).ceil() as usize + 1;
        let last = (first + count).min(rows.len());

        for i in first..last {
            let row = &rows[i];
            let y = rect.y + i as f32 * self.row_height - self.scroll_offset;
            let row_rect = Rect::new(rect.x, y, rect.width, self.row_height);

            if y + self.row_height < rect.y || y > rect.y + rect.height {
                continue;
            }

            let is_selected = self.selected_path.as_deref() == Some(&row.path);
            let is_hovered = self.hovered_index == Some(i);

            // Row background
            if is_selected {
                ctx.draw_rect(row_rect, self.selected_bg, 0.0);
            } else if is_hovered {
                ctx.draw_rect(row_rect, self.hover_bg, 0.0);
            }

            // Indent guide lines
            for d in 0..row.depth {
                let guide_x = rect.x + d as f32 * self.indent_width + self.indent_width / 2.0;
                let guide_rect = Rect::new(guide_x, y, 1.0, self.row_height);
                ctx.draw_rect(guide_rect, self.guide_color, 0.0);
            }

            let content_x = rect.x + row.depth as f32 * self.indent_width;

            // Chevron icon for expandable nodes
            if row.has_children {
                let chevron = if row.is_expanded {
                    IconId::ChevronDown
                } else {
                    IconId::ChevronRight
                };
                let cy = y + (self.row_height - 12.0) / 2.0;
                ctx.draw_icon(chevron, (content_x, cy), 12.0, self.chevron_color);
            }

            let text_x = content_x + 16.0;

            // File/folder icon
            if let Some(node) = self.node_at_path(&row.path) {
                let rendered = (self.render_item)(&node.data, row.depth);
                let icon = if row.has_children {
                    if row.is_expanded {
                        IconId::FolderOpen
                    } else {
                        IconId::Folder
                    }
                } else {
                    IconId::File
                };
                let iy = y + (self.row_height - 14.0) / 2.0;
                ctx.draw_icon(icon, (text_x, iy), 14.0, self.foreground);

                // Text label
                let text_color = if is_selected {
                    self.selected_fg
                } else {
                    self.foreground
                };
                let ty = y + (self.row_height - self.font_size) / 2.0;
                ctx.draw_text(
                    &rendered.text,
                    (text_x + 18.0, ty),
                    text_color,
                    self.font_size,
                    false,
                    false,
                );
            }
        }

        // Scrollbar
        if total_height > rect.height {
            let thumb_ratio = rect.height / total_height;
            let thumb_h = (rect.height * thumb_ratio).max(20.0);
            let scroll_ratio = if total_height - rect.height > 0.0 {
                self.scroll_offset / (total_height - rect.height)
            } else {
                0.0
            };
            let thumb_y = rect.y + scroll_ratio * (rect.height - thumb_h);
            let sb_rect = Rect::new(rect.x + rect.width - 10.0, thumb_y, 10.0, thumb_h);
            ctx.draw_rect(sb_rect, self.scrollbar_thumb, 3.0);
        }

        ctx.restore();
    }
}

impl<T, R, E, S> Widget for Tree<T, R, E, S>
where
    R: Fn(&T, usize) -> TreeRow,
    E: FnMut(&[usize]),
    S: FnMut(&[usize]),
{
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Flex(1.0),
            ..LayoutNode::default()
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        let rows = self.flatten();
        let mut rr = sidex_gpu::RectRenderer::new();
        for (i, row) in rows.iter().enumerate() {
            let y = rect.y + i as f32 * self.row_height - self.scroll_offset;
            if y + self.row_height < rect.y || y > rect.y + rect.height {
                continue;
            }
            let is_selected = self.selected_path.as_deref() == Some(&row.path);
            if is_selected {
                rr.draw_rect(
                    rect.x,
                    y,
                    rect.width,
                    self.row_height,
                    self.selected_bg,
                    0.0,
                );
            }
            for d in 0..row.depth {
                let guide_x = rect.x + d as f32 * self.indent_width + self.indent_width / 2.0;
                rr.draw_rect(guide_x, y, 1.0, self.row_height, self.guide_color, 0.0);
            }
            if let Some(node) = self.node_at_path(&row.path) {
                let _rendered = (self.render_item)(&node.data, row.depth);
            }
        }
        let _ = renderer;
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        match event {
            UiEvent::Focus => {
                self.focused = true;
                EventResult::Handled
            }
            UiEvent::Blur => {
                self.focused = false;
                EventResult::Handled
            }
            UiEvent::MouseMove { x, y } => {
                if rect.contains(*x, *y) {
                    let rows = self.flatten();
                    let idx =
                        ((y - rect.y + self.scroll_offset) / self.row_height).floor() as usize;
                    self.hovered_index = if idx < rows.len() { Some(idx) } else { None };
                } else {
                    self.hovered_index = None;
                }
                EventResult::Ignored
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } if rect.contains(*x, *y) => {
                self.focused = true;
                let rows = self.flatten();
                let index = ((y - rect.y + self.scroll_offset) / self.row_height).floor() as usize;
                if let Some(row) = rows.get(index) {
                    let path = row.path.clone();
                    if row.has_children {
                        self.toggle_expanded(&path);
                    }
                    self.selected_path = Some(path.clone());
                    (self.on_select)(&path);
                }
                EventResult::Handled
            }
            UiEvent::MouseScroll { dy, .. } => {
                let rows = self.flatten();
                let total = rows.len() as f32 * self.row_height;
                let max = (total - rect.height).max(0.0);
                self.scroll_offset = (self.scroll_offset - dy * 40.0).clamp(0.0, max);
                EventResult::Handled
            }
            UiEvent::KeyPress { key, .. } if self.focused => {
                let rows = self.flatten();
                let current_idx = self
                    .selected_path
                    .as_ref()
                    .and_then(|p| rows.iter().position(|r| r.path == *p))
                    .unwrap_or(0);
                match key {
                    Key::ArrowDown => {
                        let next = (current_idx + 1).min(rows.len().saturating_sub(1));
                        if let Some(row) = rows.get(next) {
                            let path = row.path.clone();
                            self.selected_path = Some(path.clone());
                            (self.on_select)(&path);
                        }
                        EventResult::Handled
                    }
                    Key::ArrowUp => {
                        let next = current_idx.saturating_sub(1);
                        if let Some(row) = rows.get(next) {
                            let path = row.path.clone();
                            self.selected_path = Some(path.clone());
                            (self.on_select)(&path);
                        }
                        EventResult::Handled
                    }
                    Key::ArrowRight => {
                        if let Some(row) = rows.get(current_idx) {
                            if row.has_children && !row.is_expanded {
                                let path = row.path.clone();
                                self.toggle_expanded(&path);
                            }
                        }
                        EventResult::Handled
                    }
                    Key::ArrowLeft => {
                        if let Some(row) = rows.get(current_idx) {
                            if row.has_children && row.is_expanded {
                                let path = row.path.clone();
                                self.toggle_expanded(&path);
                            }
                        }
                        EventResult::Handled
                    }
                    Key::Enter | Key::Space => {
                        if let Some(row) = rows.get(current_idx) {
                            if row.has_children {
                                let path = row.path.clone();
                                self.toggle_expanded(&path);
                            }
                        }
                        EventResult::Handled
                    }
                    _ => EventResult::Ignored,
                }
            }
            _ => EventResult::Ignored,
        }
    }
}
