//! Focus management — widget focus tracking, Tab/Shift+Tab navigation, focus
//! zones, focus traps, focus restoration, and focus ring rendering.

use std::collections::HashMap;

use sidex_gpu::color::Color;

use crate::layout::Rect;

/// Unique identifier for a widget in the focus system.
pub type WidgetId = u64;

// ── Focus direction ──────────────────────────────────────────────────────────

/// Navigation direction within a focus zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusDirection {
    Horizontal,
    Vertical,
    Both,
}

// ── Focusable element ────────────────────────────────────────────────────────

/// A single element that can receive focus inside a [`FocusZone`].
#[derive(Clone, Debug)]
pub struct FocusableElement {
    pub id: String,
    pub widget_id: WidgetId,
    pub tab_index: i32,
    pub auto_focus: bool,
    pub disabled: bool,
}

impl FocusableElement {
    pub fn new(id: impl Into<String>, widget_id: WidgetId) -> Self {
        Self {
            id: id.into(),
            widget_id,
            tab_index: 0,
            auto_focus: false,
            disabled: false,
        }
    }

    pub fn with_tab_index(mut self, index: i32) -> Self {
        self.tab_index = index;
        self
    }

    pub fn with_auto_focus(mut self) -> Self {
        self.auto_focus = true;
        self
    }

    pub fn is_focusable(&self) -> bool {
        self.tab_index >= 0 && !self.disabled
    }
}

// ── Focus zone ───────────────────────────────────────────────────────────────

/// A named region that groups focusable elements (e.g. a dialog, sidebar panel,
/// or editor group). Tab cycles within the zone when a trap is active.
#[derive(Clone, Debug)]
pub struct FocusZone {
    pub id: String,
    pub elements: Vec<FocusableElement>,
    pub wrap: bool,
    pub direction: FocusDirection,
    focused_index: Option<usize>,
}

impl FocusZone {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            elements: Vec::new(),
            wrap: true,
            direction: FocusDirection::Both,
            focused_index: None,
        }
    }

    pub fn with_direction(mut self, direction: FocusDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn with_wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    pub fn add_element(&mut self, element: FocusableElement) {
        self.elements.push(element);
    }

    pub fn remove_element(&mut self, id: &str) {
        self.elements.retain(|e| e.id != id);
        if let Some(idx) = self.focused_index {
            if idx >= self.elements.len() {
                self.focused_index = if self.elements.is_empty() {
                    None
                } else {
                    Some(self.elements.len() - 1)
                };
            }
        }
    }

    fn focusable_indices(&self) -> Vec<usize> {
        self.elements
            .iter()
            .enumerate()
            .filter(|(_, e)| e.is_focusable())
            .map(|(i, _)| i)
            .collect()
    }

    pub fn focus_first(&mut self) -> Option<WidgetId> {
        let indices = self.focusable_indices();
        if let Some(&idx) = indices.first() {
            self.focused_index = Some(idx);
            Some(self.elements[idx].widget_id)
        } else {
            None
        }
    }

    pub fn focus_next(&mut self) -> Option<WidgetId> {
        let indices = self.focusable_indices();
        if indices.is_empty() {
            return None;
        }

        let current_pos = self
            .focused_index
            .and_then(|fi| indices.iter().position(|&i| i == fi))
            .unwrap_or(0);

        let next_pos = if current_pos + 1 < indices.len() {
            current_pos + 1
        } else if self.wrap {
            0
        } else {
            return None;
        };

        let idx = indices[next_pos];
        self.focused_index = Some(idx);
        Some(self.elements[idx].widget_id)
    }

    pub fn focus_prev(&mut self) -> Option<WidgetId> {
        let indices = self.focusable_indices();
        if indices.is_empty() {
            return None;
        }

        let current_pos = self
            .focused_index
            .and_then(|fi| indices.iter().position(|&i| i == fi))
            .unwrap_or(0);

        let prev_pos = if current_pos > 0 {
            current_pos - 1
        } else if self.wrap {
            indices.len() - 1
        } else {
            return None;
        };

        let idx = indices[prev_pos];
        self.focused_index = Some(idx);
        Some(self.elements[idx].widget_id)
    }

    pub fn current_widget_id(&self) -> Option<WidgetId> {
        self.focused_index.map(|i| self.elements[i].widget_id)
    }
}

// ── Focus service ────────────────────────────────────────────────────────────

/// High-level focus manager built on top of zones. Provides focus trapping,
/// focus restoration, and workbench-level keyboard shortcuts.
#[derive(Debug)]
pub struct FocusService {
    zones: Vec<FocusZone>,
    current_focus: Option<String>,
    trap: Option<String>,
    restore_stack: Vec<String>,
}

impl Default for FocusService {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusService {
    pub fn new() -> Self {
        Self {
            zones: Vec::new(),
            current_focus: None,
            trap: None,
            restore_stack: Vec::new(),
        }
    }

    // ── Zone management ──────────────────────────────────────────────────

    pub fn register_zone(&mut self, zone: FocusZone) {
        self.zones.retain(|z| z.id != zone.id);
        self.zones.push(zone);
    }

    pub fn unregister_zone(&mut self, id: &str) {
        self.zones.retain(|z| z.id != id);
        if self.current_focus.as_deref() == Some(id) {
            self.current_focus = None;
        }
        if self.trap.as_deref() == Some(id) {
            self.trap = None;
        }
    }

    pub fn zone(&self, id: &str) -> Option<&FocusZone> {
        self.zones.iter().find(|z| z.id == id)
    }

    pub fn zone_mut(&mut self, id: &str) -> Option<&mut FocusZone> {
        self.zones.iter_mut().find(|z| z.id == id)
    }

    // ── Focus operations ─────────────────────────────────────────────────

    /// Focuses the given zone, optionally auto-focusing its first element.
    pub fn focus_zone(&mut self, zone_id: &str) -> Option<WidgetId> {
        if let Some(prev) = self.current_focus.take() {
            self.restore_stack.push(prev);
        }
        self.current_focus = Some(zone_id.to_string());

        self.zones
            .iter_mut()
            .find(|z| z.id == zone_id)
            .and_then(|z| z.focus_first())
    }

    /// Moves focus to the next element within the currently focused zone.
    pub fn focus_next(&mut self) -> Option<WidgetId> {
        let zone_id = self.active_zone_id()?;
        self.zones
            .iter_mut()
            .find(|z| z.id == zone_id)
            .and_then(|z| z.focus_next())
    }

    /// Moves focus to the previous element within the currently focused zone.
    pub fn focus_prev(&mut self) -> Option<WidgetId> {
        let zone_id = self.active_zone_id()?;
        self.zones
            .iter_mut()
            .find(|z| z.id == zone_id)
            .and_then(|z| z.focus_prev())
    }

    pub fn current_zone_id(&self) -> Option<&str> {
        self.current_focus.as_deref()
    }

    pub fn current_widget_id(&self) -> Option<WidgetId> {
        let zone_id = self.active_zone_id()?;
        self.zones
            .iter()
            .find(|z| z.id == zone_id)
            .and_then(|z| z.current_widget_id())
    }

    // ── Focus trap ───────────────────────────────────────────────────────

    /// Traps focus within the named zone. Tab/Shift+Tab cannot leave.
    pub fn trap_focus(&mut self, zone_id: &str) {
        self.trap = Some(zone_id.to_string());
        self.focus_zone(zone_id);
    }

    /// Releases the focus trap and restores focus to the previous zone.
    pub fn release_focus_trap(&mut self) {
        self.trap = None;
        if let Some(prev) = self.restore_stack.pop() {
            self.current_focus = Some(prev);
        }
    }

    pub fn is_trapped(&self) -> bool {
        self.trap.is_some()
    }

    fn active_zone_id(&self) -> Option<String> {
        self.trap
            .clone()
            .or_else(|| self.current_focus.clone())
    }

    // ── Focus restoration ────────────────────────────────────────────────

    /// Returns focus to the previously focused zone (e.g. when closing a panel).
    pub fn restore_previous_focus(&mut self) -> Option<WidgetId> {
        let prev = self.restore_stack.pop()?;
        self.focus_zone(&prev)
    }

    // ── Workbench shortcuts ──────────────────────────────────────────────

    /// Focus a specific editor group by 1-based index (Ctrl+1, Ctrl+2, Ctrl+3).
    pub fn focus_editor_group(&mut self, group: usize) -> Option<WidgetId> {
        let zone_id = format!("editor-group-{group}");
        self.focus_zone(&zone_id)
    }

    /// Focus the sidebar section: Explorer (Ctrl+Shift+E).
    pub fn focus_explorer(&mut self) -> Option<WidgetId> {
        self.focus_zone("sidebar-explorer")
    }

    /// Focus the sidebar section: Search (Ctrl+Shift+F).
    pub fn focus_search(&mut self) -> Option<WidgetId> {
        self.focus_zone("sidebar-search")
    }

    /// Focus the sidebar section: Source Control (Ctrl+Shift+G).
    pub fn focus_source_control(&mut self) -> Option<WidgetId> {
        self.focus_zone("sidebar-scm")
    }

    /// Focus the sidebar section: Debug (Ctrl+Shift+D).
    pub fn focus_debug(&mut self) -> Option<WidgetId> {
        self.focus_zone("sidebar-debug")
    }

    /// Focus the sidebar section: Extensions (Ctrl+Shift+X).
    pub fn focus_extensions(&mut self) -> Option<WidgetId> {
        self.focus_zone("sidebar-extensions")
    }

    /// Focus the integrated terminal (Ctrl+`).
    pub fn focus_terminal(&mut self) -> Option<WidgetId> {
        self.focus_zone("terminal")
    }
}

// ── Focus manager (low-level, widget-id based) ──────────────────────────────

/// Low-level widget focus tracker with Tab/Shift+Tab navigation and focus ring.
#[derive(Debug)]
pub struct FocusManager {
    focused: Option<WidgetId>,
    tab_order: Vec<WidgetId>,
    widget_rects: HashMap<WidgetId, Rect>,
    focus_ring: FocusRing,
    trap_stack: Vec<FocusTrapEntry>,
}

#[derive(Debug, Clone)]
struct FocusTrapEntry {
    widget_ids: Vec<WidgetId>,
    restore_focus: Option<WidgetId>,
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusManager {
    pub fn new() -> Self {
        Self {
            focused: None,
            tab_order: Vec::new(),
            widget_rects: HashMap::new(),
            focus_ring: FocusRing::new(),
            trap_stack: Vec::new(),
        }
    }

    pub fn register(&mut self, widget_id: WidgetId, rect: Rect) {
        if !self.tab_order.contains(&widget_id) {
            self.tab_order.push(widget_id);
        }
        self.widget_rects.insert(widget_id, rect);
    }

    pub fn unregister(&mut self, widget_id: WidgetId) {
        self.tab_order.retain(|&id| id != widget_id);
        self.widget_rects.remove(&widget_id);
        if self.focused == Some(widget_id) {
            self.focused = None;
        }
    }

    pub fn focus(&mut self, widget_id: WidgetId) {
        self.focused = Some(widget_id);
        self.focus_ring.show = true;
    }

    pub fn blur(&mut self, widget_id: WidgetId) {
        if self.focused == Some(widget_id) {
            self.focused = None;
            self.focus_ring.show = false;
        }
    }

    pub fn focus_next(&mut self) {
        let order = self.active_tab_order();
        if order.is_empty() {
            return;
        }
        let next = match self.focused {
            Some(current) => {
                let pos = order.iter().position(|&id| id == current).unwrap_or(0);
                order[(pos + 1) % order.len()]
            }
            None => order[0],
        };
        self.focus(next);
    }

    pub fn focus_prev(&mut self) {
        let order = self.active_tab_order();
        if order.is_empty() {
            return;
        }
        let prev = match self.focused {
            Some(current) => {
                let pos = order.iter().position(|&id| id == current).unwrap_or(0);
                if pos == 0 {
                    order[order.len() - 1]
                } else {
                    order[pos - 1]
                }
            }
            None => order[order.len() - 1],
        };
        self.focus(prev);
    }

    pub fn focused(&self) -> Option<WidgetId> {
        self.focused
    }

    pub fn is_focused(&self, widget_id: WidgetId) -> bool {
        self.focused == Some(widget_id)
    }

    pub fn focused_rect(&self) -> Option<Rect> {
        let id = self.focused?;
        self.widget_rects.get(&id).copied()
    }

    pub fn focus_ring(&self) -> &FocusRing {
        &self.focus_ring
    }

    pub fn focus_ring_mut(&mut self) -> &mut FocusRing {
        &mut self.focus_ring
    }

    // ── Focus traps ──────────────────────────────────────────────────────

    pub fn push_trap(&mut self, widget_ids: Vec<WidgetId>) {
        let restore = self.focused;
        self.trap_stack.push(FocusTrapEntry {
            widget_ids,
            restore_focus: restore,
        });
        self.focus_next();
    }

    pub fn pop_trap(&mut self) {
        if let Some(entry) = self.trap_stack.pop() {
            if let Some(restore) = entry.restore_focus {
                self.focus(restore);
            }
        }
    }

    pub fn is_trapped(&self) -> bool {
        !self.trap_stack.is_empty()
    }

    fn active_tab_order(&self) -> Vec<WidgetId> {
        if let Some(trap) = self.trap_stack.last() {
            trap.widget_ids.clone()
        } else {
            self.tab_order.clone()
        }
    }
}

// ── Focus ring ───────────────────────────────────────────────────────────────

/// Visual configuration for the focus ring highlight drawn around focused widgets.
#[derive(Clone, Debug)]
pub struct FocusRing {
    pub show: bool,
    pub color: Color,
    pub thickness: f32,
    pub corner_radius: f32,
    pub offset: f32,
    pub opacity: f32,
}

impl FocusRing {
    pub fn new() -> Self {
        Self {
            show: false,
            color: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            thickness: 1.0,
            corner_radius: 2.0,
            offset: 1.0,
            opacity: 1.0,
        }
    }

    /// Returns the rect for the focus ring given the widget rect.
    pub fn ring_rect(&self, widget_rect: Rect) -> Rect {
        Rect::new(
            widget_rect.x - self.offset,
            widget_rect.y - self.offset,
            widget_rect.width + self.offset * 2.0,
            widget_rect.height + self.offset * 2.0,
        )
    }
}

impl Default for FocusRing {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── FocusManager tests ───────────────────────────────────────────────

    #[test]
    fn focus_manager_tab_cycle() {
        let mut mgr = FocusManager::new();
        mgr.register(1, Rect::new(0.0, 0.0, 10.0, 10.0));
        mgr.register(2, Rect::new(20.0, 0.0, 10.0, 10.0));
        mgr.register(3, Rect::new(40.0, 0.0, 10.0, 10.0));

        mgr.focus_next();
        assert_eq!(mgr.focused(), Some(1));
        mgr.focus_next();
        assert_eq!(mgr.focused(), Some(2));
        mgr.focus_next();
        assert_eq!(mgr.focused(), Some(3));
        mgr.focus_next();
        assert_eq!(mgr.focused(), Some(1));
    }

    #[test]
    fn focus_manager_trap() {
        let mut mgr = FocusManager::new();
        mgr.register(1, Rect::ZERO);
        mgr.register(2, Rect::ZERO);
        mgr.register(3, Rect::ZERO);
        mgr.focus(1);

        mgr.push_trap(vec![2, 3]);
        assert!(mgr.is_trapped());
        assert_eq!(mgr.focused(), Some(2));

        mgr.focus_next();
        assert_eq!(mgr.focused(), Some(3));
        mgr.focus_next();
        assert_eq!(mgr.focused(), Some(2));

        mgr.pop_trap();
        assert!(!mgr.is_trapped());
        assert_eq!(mgr.focused(), Some(1));
    }

    // ── FocusZone tests ──────────────────────────────────────────────────

    #[test]
    fn focus_zone_navigation() {
        let mut zone = FocusZone::new("test");
        zone.add_element(FocusableElement::new("a", 10));
        zone.add_element(FocusableElement::new("b", 20));
        zone.add_element(FocusableElement::new("c", 30));

        assert_eq!(zone.focus_first(), Some(10));
        assert_eq!(zone.focus_next(), Some(20));
        assert_eq!(zone.focus_next(), Some(30));
        assert_eq!(zone.focus_next(), Some(10));
    }

    #[test]
    fn focus_zone_no_wrap() {
        let mut zone = FocusZone::new("test").with_wrap(false);
        zone.add_element(FocusableElement::new("a", 10));
        zone.add_element(FocusableElement::new("b", 20));

        zone.focus_first();
        assert_eq!(zone.focus_next(), Some(20));
        assert_eq!(zone.focus_next(), None);
    }

    #[test]
    fn focus_zone_skips_disabled() {
        let mut zone = FocusZone::new("test");
        zone.add_element(FocusableElement::new("a", 10));
        zone.add_element(FocusableElement {
            id: "b".into(),
            widget_id: 20,
            tab_index: 0,
            auto_focus: false,
            disabled: true,
        });
        zone.add_element(FocusableElement::new("c", 30));

        zone.focus_first();
        assert_eq!(zone.focus_next(), Some(30));
    }

    // ── FocusService tests ───────────────────────────────────────────────

    #[test]
    fn focus_service_zone_switching() {
        let mut svc = FocusService::new();

        let mut z1 = FocusZone::new("sidebar");
        z1.add_element(FocusableElement::new("explorer", 100));
        svc.register_zone(z1);

        let mut z2 = FocusZone::new("editor-group-1");
        z2.add_element(FocusableElement::new("tab1", 200));
        svc.register_zone(z2);

        assert_eq!(svc.focus_zone("sidebar"), Some(100));
        assert_eq!(svc.current_zone_id(), Some("sidebar"));

        assert_eq!(svc.focus_zone("editor-group-1"), Some(200));
        assert_eq!(svc.current_zone_id(), Some("editor-group-1"));
    }

    #[test]
    fn focus_service_trap_and_restore() {
        let mut svc = FocusService::new();

        let mut zone = FocusZone::new("main");
        zone.add_element(FocusableElement::new("btn", 1));
        svc.register_zone(zone);

        let mut dialog = FocusZone::new("dialog");
        dialog.add_element(FocusableElement::new("ok", 2));
        dialog.add_element(FocusableElement::new("cancel", 3));
        svc.register_zone(dialog);

        svc.focus_zone("main");
        svc.trap_focus("dialog");
        assert!(svc.is_trapped());
        assert_eq!(svc.current_zone_id(), Some("dialog"));

        svc.release_focus_trap();
        assert!(!svc.is_trapped());
        assert_eq!(svc.current_zone_id(), Some("main"));
    }

    #[test]
    fn focus_service_workbench_shortcuts() {
        let mut svc = FocusService::new();

        let mut z = FocusZone::new("editor-group-2");
        z.add_element(FocusableElement::new("tab", 42));
        svc.register_zone(z);

        assert_eq!(svc.focus_editor_group(2), Some(42));

        let mut term = FocusZone::new("terminal");
        term.add_element(FocusableElement::new("term1", 99));
        svc.register_zone(term);

        assert_eq!(svc.focus_terminal(), Some(99));
    }

    #[test]
    fn focus_ring_rect() {
        let ring = FocusRing::new();
        let widget = Rect::new(10.0, 20.0, 100.0, 50.0);
        let r = ring.ring_rect(widget);
        assert!((r.x - 9.0).abs() < 0.01);
        assert!((r.width - 102.0).abs() < 0.01);
    }
}
