//! Accessibility support — ARIA roles, screen reader announcements, focus traps,
//! accessibility tree construction, high-contrast and reduced-motion detection.

use std::collections::VecDeque;

use crate::layout::LayoutNode;

// ── ARIA roles ───────────────────────────────────────────────────────────────

/// ARIA roles mapping to standard WAI-ARIA widget roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AriaRole {
    Application,
    Button,
    Checkbox,
    Combobox,
    Dialog,
    Grid,
    GridCell,
    Group,
    Heading,
    Link,
    List,
    ListItem,
    Menu,
    MenuBar,
    MenuItem,
    MenuItemCheckbox,
    MenuItemRadio,
    Option,
    ProgressBar,
    Radio,
    Scrollbar,
    Separator,
    Slider,
    Status,
    Tab,
    TabList,
    TabPanel,
    TextBox,
    Toolbar,
    Tooltip,
    Tree,
    TreeItem,
    Alert,
    AlertDialog,
    Switch,
    Region,
    Complementary,
    Navigation,
    Main,
    ContentInfo,
    Banner,
    Form,
    Search,
    Document,
    Presentation,
    None,
}

impl AriaRole {
    /// Returns the WAI-ARIA role string for use in accessibility tree output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Application => "application",
            Self::Button => "button",
            Self::Checkbox => "checkbox",
            Self::Combobox => "combobox",
            Self::Dialog => "dialog",
            Self::Grid => "grid",
            Self::GridCell => "gridcell",
            Self::Group => "group",
            Self::Heading => "heading",
            Self::Link => "link",
            Self::List => "list",
            Self::ListItem => "listitem",
            Self::Menu => "menu",
            Self::MenuBar => "menubar",
            Self::MenuItem => "menuitem",
            Self::MenuItemCheckbox => "menuitemcheckbox",
            Self::MenuItemRadio => "menuitemradio",
            Self::Option => "option",
            Self::ProgressBar => "progressbar",
            Self::Radio => "radio",
            Self::Scrollbar => "scrollbar",
            Self::Separator => "separator",
            Self::Slider => "slider",
            Self::Status => "status",
            Self::Tab => "tab",
            Self::TabList => "tablist",
            Self::TabPanel => "tabpanel",
            Self::TextBox => "textbox",
            Self::Toolbar => "toolbar",
            Self::Tooltip => "tooltip",
            Self::Tree => "tree",
            Self::TreeItem => "treeitem",
            Self::Alert => "alert",
            Self::AlertDialog => "alertdialog",
            Self::Switch => "switch",
            Self::Region => "region",
            Self::Complementary => "complementary",
            Self::Navigation => "navigation",
            Self::Main => "main",
            Self::ContentInfo => "contentinfo",
            Self::Banner => "banner",
            Self::Form => "form",
            Self::Search => "search",
            Self::Document => "document",
            Self::Presentation => "presentation",
            Self::None => "none",
        }
    }
}

// ── Accessible state ─────────────────────────────────────────────────────────

/// ARIA state properties for an accessible element.
#[derive(Clone, Debug, Default)]
pub struct AccessibleState {
    pub checked: Option<bool>,
    pub disabled: bool,
    pub expanded: Option<bool>,
    pub focused: bool,
    pub hidden: bool,
    pub pressed: Option<bool>,
    pub selected: bool,
    pub level: Option<u32>,
    pub position_in_set: Option<u32>,
    pub set_size: Option<u32>,
}

// ── Accessible action ────────────────────────────────────────────────────────

/// An action that assistive technology can trigger on an element.
#[derive(Clone, Debug)]
pub struct AccessibleAction {
    pub label: String,
    pub command: String,
}

impl AccessibleAction {
    pub fn new(label: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            command: command.into(),
        }
    }
}

// ── ARIA live regions ────────────────────────────────────────────────────────

/// ARIA live region politeness level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AriaLive {
    Off,
    Polite,
    Assertive,
}

impl AriaLive {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Polite => "polite",
            Self::Assertive => "assertive",
        }
    }
}

// ── Accessible element (tree node) ───────────────────────────────────────────

/// An element in the accessibility tree with ARIA properties.
#[derive(Clone, Debug)]
pub struct AccessibleElement {
    pub role: AriaRole,
    pub label: String,
    pub description: Option<String>,
    pub value: Option<String>,
    pub state: AccessibleState,
    pub actions: Vec<AccessibleAction>,
    pub children: Vec<AccessibleElement>,
    pub live_region: Option<AriaLive>,

    // Legacy fields kept for existing callers
    pub expanded: Option<bool>,
    pub selected: bool,
    pub disabled: bool,
    pub level: Option<u32>,
    pub pos_in_set: Option<u32>,
    pub set_size: Option<u32>,
    pub checked: Option<bool>,
    pub live: Option<AriaLive>,
    pub owns: Vec<String>,
    pub controls: Option<String>,
    pub labelled_by: Option<String>,
    pub described_by: Option<String>,
}

impl AccessibleElement {
    pub fn new(role: AriaRole, label: impl Into<String>) -> Self {
        Self {
            role,
            label: label.into(),
            description: None,
            value: None,
            state: AccessibleState::default(),
            actions: Vec::new(),
            children: Vec::new(),
            live_region: None,
            expanded: None,
            selected: false,
            disabled: false,
            level: None,
            pos_in_set: None,
            set_size: None,
            checked: None,
            live: None,
            owns: Vec::new(),
            controls: None,
            labelled_by: None,
            described_by: None,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn with_expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self.state.expanded = Some(expanded);
        self
    }

    pub fn with_selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self.state.selected = selected;
        self
    }

    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self.state.disabled = disabled;
        self
    }

    pub fn with_level(mut self, level: u32) -> Self {
        self.level = Some(level);
        self.state.level = Some(level);
        self
    }

    pub fn with_position(mut self, pos: u32, size: u32) -> Self {
        self.pos_in_set = Some(pos);
        self.set_size = Some(size);
        self.state.position_in_set = Some(pos);
        self.state.set_size = Some(size);
        self
    }

    pub fn with_children(mut self, children: Vec<AccessibleElement>) -> Self {
        self.children = children;
        self
    }

    pub fn with_actions(mut self, actions: Vec<AccessibleAction>) -> Self {
        self.actions = actions;
        self
    }

    pub fn with_live_region(mut self, live: AriaLive) -> Self {
        self.live_region = Some(live);
        self.live = Some(live);
        self
    }

    /// Counts total nodes in this subtree.
    pub fn node_count(&self) -> usize {
        1 + self.children.iter().map(AccessibleElement::node_count).sum::<usize>()
    }
}

// ── Accessibility service ────────────────────────────────────────────────────

/// Top-level accessibility service tracking system-wide a11y preferences and
/// providing screen reader announcements.
#[derive(Clone, Debug)]
pub struct AccessibilityService {
    pub screen_reader_active: bool,
    pub reduced_motion: bool,
    pub high_contrast: bool,
    pub keyboard_navigation: bool,
    announcement_queue: VecDeque<Announcement>,
}

#[derive(Clone, Debug)]
struct Announcement {
    message: String,
    urgency: AriaLive,
}

impl Default for AccessibilityService {
    fn default() -> Self {
        Self {
            screen_reader_active: false,
            reduced_motion: false,
            high_contrast: false,
            keyboard_navigation: true,
            announcement_queue: VecDeque::new(),
        }
    }
}

impl AccessibilityService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Detect platform accessibility preferences at startup.
    pub fn detect_platform_settings(&mut self) {
        // High-contrast: check environment hint (Windows sets this, macOS has
        // NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification, etc.)
        if std::env::var("HIGH_CONTRAST").is_ok_and(|v| v == "1") {
            self.high_contrast = true;
        }
        if std::env::var("REDUCE_MOTION").is_ok_and(|v| v == "1") {
            self.reduced_motion = true;
        }
    }

    // ── Announcements ────────────────────────────────────────────────────

    /// Speaks a message via the screen reader at the given urgency level.
    pub fn announce(&mut self, message: &str, urgency: AriaLive) {
        self.announcement_queue.push_back(Announcement {
            message: message.to_string(),
            urgency,
        });
    }

    /// Convenience: polite announcement.
    pub fn announce_polite(&mut self, message: &str) {
        self.announce(message, AriaLive::Polite);
    }

    /// Convenience: assertive (interrupt) announcement.
    pub fn announce_assertive(&mut self, message: &str) {
        self.announce(message, AriaLive::Assertive);
    }

    /// Drains the announcement queue.
    pub fn drain_announcements(&mut self) -> Vec<(String, AriaLive)> {
        self.announcement_queue
            .drain(..)
            .map(|a| (a.message, a.urgency))
            .collect()
    }

    pub fn has_pending_announcements(&self) -> bool {
        !self.announcement_queue.is_empty()
    }

    // ── Editor-specific announcements ────────────────────────────────────

    pub fn announce_line_content(&mut self, line_number: usize, text: &str) {
        self.announce(
            &format!("Line {line_number}: {text}"),
            AriaLive::Polite,
        );
    }

    pub fn announce_diagnostic(&mut self, severity: &str, message: &str, line: usize) {
        self.announce(
            &format!("{severity} on line {line}: {message}"),
            AriaLive::Assertive,
        );
    }

    pub fn announce_completion(&mut self, label: &str, kind: &str) {
        self.announce(
            &format!("Completion: {label} ({kind})"),
            AriaLive::Polite,
        );
    }

    pub fn announce_cursor_position(&mut self, line: usize, column: usize) {
        self.announce(
            &format!("Line {line}, Column {column}"),
            AriaLive::Polite,
        );
    }

    // ── Accessibility tree construction ──────────────────────────────────

    /// Builds an accessibility tree from a layout tree. Each layout node becomes
    /// an accessible element with a generic `Group` role; callers should
    /// override roles on the returned tree as appropriate.
    pub fn build_accessibility_tree(root: &LayoutNode) -> AccessibleElement {
        build_tree_recursive(root, 0)
    }
}

fn build_tree_recursive(node: &LayoutNode, index: usize) -> AccessibleElement {
    let label = format!("layout-node-{index}");
    let children: Vec<AccessibleElement> = node
        .children
        .iter()
        .enumerate()
        .map(|(i, child)| build_tree_recursive(child, i))
        .collect();

    AccessibleElement::new(AriaRole::Group, label).with_children(children)
}

// ── AccessibilityState (legacy compat) ───────────────────────────────────────

/// Legacy screen-reader state tracker kept for backwards compatibility.
/// New code should prefer [`AccessibilityService`].
#[derive(Clone, Debug)]
pub struct AccessibilityState {
    pub screen_reader_active: bool,
    pub reduce_motion: bool,
    pub high_contrast: bool,
    announcement_queue: VecDeque<LegacyAnnouncement>,
}

#[derive(Clone, Debug)]
struct LegacyAnnouncement {
    message: String,
    priority: AriaLive,
}

impl Default for AccessibilityState {
    fn default() -> Self {
        Self {
            screen_reader_active: false,
            reduce_motion: false,
            high_contrast: false,
            announcement_queue: VecDeque::new(),
        }
    }
}

impl AccessibilityState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn announce(&mut self, message: &str) {
        self.announcement_queue.push_back(LegacyAnnouncement {
            message: message.to_string(),
            priority: AriaLive::Polite,
        });
    }

    pub fn announce_assertive(&mut self, message: &str) {
        self.announcement_queue.push_back(LegacyAnnouncement {
            message: message.to_string(),
            priority: AriaLive::Assertive,
        });
    }

    pub fn drain_announcements(&mut self) -> Vec<(String, AriaLive)> {
        self.announcement_queue
            .drain(..)
            .map(|a| (a.message, a.priority))
            .collect()
    }

    pub fn has_pending_announcements(&self) -> bool {
        !self.announcement_queue.is_empty()
    }
}

// ── Focus trap ───────────────────────────────────────────────────────────────

/// Manages a focus trap that keeps keyboard focus within a set of elements
/// (e.g., within a dialog or modal).
#[derive(Clone, Debug)]
pub struct FocusTrap {
    elements: Vec<AccessibleElement>,
    focused_index: usize,
    active: bool,
}

impl FocusTrap {
    pub fn new(elements: Vec<AccessibleElement>) -> Self {
        Self {
            elements,
            focused_index: 0,
            active: true,
        }
    }

    pub fn activate(&mut self) {
        self.active = true;
        self.focused_index = 0;
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn focus_next(&mut self) -> Option<&AccessibleElement> {
        if self.elements.is_empty() || !self.active {
            return None;
        }
        self.focused_index = (self.focused_index + 1) % self.elements.len();
        self.skip_disabled_forward();
        self.elements.get(self.focused_index)
    }

    pub fn focus_prev(&mut self) -> Option<&AccessibleElement> {
        if self.elements.is_empty() || !self.active {
            return None;
        }
        if self.focused_index == 0 {
            self.focused_index = self.elements.len() - 1;
        } else {
            self.focused_index -= 1;
        }
        self.skip_disabled_backward();
        self.elements.get(self.focused_index)
    }

    pub fn current(&self) -> Option<&AccessibleElement> {
        if self.active {
            self.elements.get(self.focused_index)
        } else {
            None
        }
    }

    pub fn set_elements(&mut self, elements: Vec<AccessibleElement>) {
        self.elements = elements;
        if self.focused_index >= self.elements.len() {
            self.focused_index = 0;
        }
    }

    fn skip_disabled_forward(&mut self) {
        let start = self.focused_index;
        loop {
            if let Some(el) = self.elements.get(self.focused_index) {
                if !el.disabled {
                    return;
                }
            }
            self.focused_index = (self.focused_index + 1) % self.elements.len();
            if self.focused_index == start {
                return;
            }
        }
    }

    fn skip_disabled_backward(&mut self) {
        let start = self.focused_index;
        loop {
            if let Some(el) = self.elements.get(self.focused_index) {
                if !el.disabled {
                    return;
                }
            }
            if self.focused_index == 0 {
                self.focused_index = self.elements.len() - 1;
            } else {
                self.focused_index -= 1;
            }
            if self.focused_index == start {
                return;
            }
        }
    }
}

// ── Tab order management ─────────────────────────────────────────────────────

/// Manages tab order indices for a set of widgets.
#[derive(Clone, Debug, Default)]
pub struct TabOrder {
    entries: Vec<TabOrderEntry>,
}

#[derive(Clone, Debug)]
struct TabOrderEntry {
    widget_id: u64,
    tab_index: i32,
    focusable: bool,
}

impl TabOrder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, widget_id: u64, tab_index: i32) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.widget_id == widget_id) {
            entry.tab_index = tab_index;
            entry.focusable = tab_index >= 0;
        } else {
            self.entries.push(TabOrderEntry {
                widget_id,
                tab_index,
                focusable: tab_index >= 0,
            });
        }
    }

    pub fn unregister(&mut self, widget_id: u64) {
        self.entries.retain(|e| e.widget_id != widget_id);
    }

    pub fn next_after(&self, widget_id: u64) -> Option<u64> {
        let sorted = self.sorted_focusable();
        let pos = sorted.iter().position(|id| *id == widget_id)?;
        let next = (pos + 1) % sorted.len();
        sorted.get(next).copied()
    }

    pub fn prev_before(&self, widget_id: u64) -> Option<u64> {
        let sorted = self.sorted_focusable();
        let pos = sorted.iter().position(|id| *id == widget_id)?;
        let prev = if pos == 0 { sorted.len() - 1 } else { pos - 1 };
        sorted.get(prev).copied()
    }

    pub fn first(&self) -> Option<u64> {
        self.sorted_focusable().first().copied()
    }

    fn sorted_focusable(&self) -> Vec<u64> {
        let mut focusable: Vec<_> = self.entries.iter().filter(|e| e.focusable).collect();
        focusable.sort_by_key(|e| e.tab_index);
        focusable.iter().map(|e| e.widget_id).collect()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aria_role_strings() {
        assert_eq!(AriaRole::Button.as_str(), "button");
        assert_eq!(AriaRole::TreeItem.as_str(), "treeitem");
        assert_eq!(AriaRole::GridCell.as_str(), "gridcell");
        assert_eq!(AriaRole::MenuItemCheckbox.as_str(), "menuitemcheckbox");
    }

    #[test]
    fn accessible_element_builder() {
        let el = AccessibleElement::new(AriaRole::Button, "Save")
            .with_description("Save the current file")
            .with_disabled(false)
            .with_actions(vec![AccessibleAction::new("press", "file.save")]);

        assert_eq!(el.label, "Save");
        assert_eq!(el.description.as_deref(), Some("Save the current file"));
        assert_eq!(el.actions.len(), 1);
        assert_eq!(el.actions[0].command, "file.save");
    }

    #[test]
    fn accessibility_service_announcements() {
        let mut svc = AccessibilityService::new();
        svc.announce_polite("File saved");
        svc.announce_assertive("Build failed!");
        svc.announce_diagnostic("error", "missing semicolon", 42);

        let announcements = svc.drain_announcements();
        assert_eq!(announcements.len(), 3);
        assert_eq!(announcements[0].0, "File saved");
        assert_eq!(announcements[1].1, AriaLive::Assertive);
        assert!(announcements[2].0.contains("line 42"));
    }

    #[test]
    fn build_tree_from_layout() {
        let root = LayoutNode {
            children: vec![LayoutNode::fixed(100.0), LayoutNode::flex(1.0)],
            ..LayoutNode::default()
        };
        let tree = AccessibilityService::build_accessibility_tree(&root);
        assert_eq!(tree.children.len(), 2);
        assert_eq!(tree.node_count(), 3);
    }

    #[test]
    fn focus_trap_cycle() {
        let elements = vec![
            AccessibleElement::new(AriaRole::Button, "OK"),
            AccessibleElement::new(AriaRole::Button, "Cancel"),
        ];
        let mut trap = FocusTrap::new(elements);

        assert_eq!(trap.current().unwrap().label, "OK");
        trap.focus_next();
        assert_eq!(trap.current().unwrap().label, "Cancel");
        trap.focus_next();
        assert_eq!(trap.current().unwrap().label, "OK");
    }

    #[test]
    fn focus_trap_skips_disabled() {
        let elements = vec![
            AccessibleElement::new(AriaRole::Button, "A"),
            AccessibleElement::new(AriaRole::Button, "B").with_disabled(true),
            AccessibleElement::new(AriaRole::Button, "C"),
        ];
        let mut trap = FocusTrap::new(elements);
        trap.focus_next();
        assert_eq!(trap.current().unwrap().label, "C");
    }

    #[test]
    fn tab_order_navigation() {
        let mut order = TabOrder::new();
        order.register(10, 0);
        order.register(20, 1);
        order.register(30, 2);

        assert_eq!(order.next_after(10), Some(20));
        assert_eq!(order.next_after(30), Some(10));
        assert_eq!(order.prev_before(10), Some(30));
    }

    #[test]
    fn legacy_accessibility_state() {
        let mut state = AccessibilityState::new();
        state.announce("hello");
        state.announce_assertive("urgent");
        assert!(state.has_pending_announcements());

        let msgs = state.drain_announcements();
        assert_eq!(msgs.len(), 2);
        assert!(!state.has_pending_announcements());
    }

    #[test]
    fn node_count_recursive() {
        let child = AccessibleElement::new(AriaRole::ListItem, "item")
            .with_children(vec![
                AccessibleElement::new(AriaRole::Button, "btn"),
            ]);
        let root = AccessibleElement::new(AriaRole::List, "list")
            .with_children(vec![child]);
        assert_eq!(root.node_count(), 3);
    }
}
