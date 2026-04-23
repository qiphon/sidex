//! Extension tree views — data-provider-based tree UIs contributed by
//! extensions.
//!
//! Extensions register tree data providers that supply `TreeItem` nodes.
//! Nodes can be lazily loaded (children fetched on expand), refreshed on
//! demand, and support drag-and-drop, inline actions, and selection events.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A tree view registered by an extension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionTreeView {
    pub id: String,
    pub title: String,
    pub view_container: ViewContainer,
    pub extension_id: String,
    /// Root-level items (populated lazily by the data provider).
    pub root_items: Vec<TreeItem>,
    pub is_loading: bool,
    /// Whether drag and drop is enabled.
    pub drag_and_drop_enabled: bool,
    /// Context value used for `when` clause resolution.
    pub can_select_many: bool,
}

/// Where a tree view appears in the workbench.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewContainer {
    Explorer,
    SourceControl,
    Debug,
    Extensions,
    Custom(String),
}

/// A single node in an extension tree view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeItem {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tooltip: Option<String>,
    #[serde(default)]
    pub icon: Option<TreeItemIcon>,
    pub collapsible_state: CollapsibleState,
    #[serde(default)]
    pub command: Option<TreeItemCommand>,
    #[serde(default)]
    pub context_value: Option<String>,
    #[serde(default)]
    pub children: Option<Vec<TreeItem>>,
    /// Resource URI associated with this item (e.g. a file path).
    #[serde(default)]
    pub resource_uri: Option<String>,
    /// Accessibility label override.
    #[serde(default)]
    pub accessibility_information: Option<AccessibilityInfo>,
}

/// Collapsible state of a tree item.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollapsibleState {
    #[default]
    None,
    Collapsed,
    Expanded,
}

/// Theme-aware icon for a tree item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeItemIcon {
    pub light: String,
    pub dark: String,
}

/// Command executed when a tree item is clicked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeItemCommand {
    pub command: String,
    pub title: String,
    #[serde(default)]
    pub arguments: Vec<serde_json::Value>,
}

/// Inline action on a tree item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeItemInlineAction {
    pub command: String,
    pub title: String,
    pub icon: Option<TreeItemIcon>,
    pub when: Option<String>,
}

/// Accessibility information for a tree item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibilityInfo {
    pub label: String,
    #[serde(default)]
    pub role: Option<String>,
}

/// Drag-and-drop transfer data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeDragData {
    pub source_view_id: String,
    pub source_item_ids: Vec<String>,
    pub mime_type: String,
    pub data: serde_json::Value,
}

/// Event emitted on tree view changes.
#[derive(Debug, Clone)]
pub enum TreeViewEvent {
    SelectionChanged {
        view_id: String,
        selected_ids: Vec<String>,
    },
    ExpandChanged {
        view_id: String,
        item_id: String,
        expanded: bool,
    },
    VisibilityChanged {
        view_id: String,
        visible: bool,
    },
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Manages all tree views registered by extensions.
pub struct TreeViewRegistry {
    views: HashMap<String, ExtensionTreeView>,
    /// Pending refresh requests (view ids waiting for data provider callback).
    refresh_pending: Vec<String>,
    /// Per-view selection state.
    selections: HashMap<String, Vec<String>>,
    /// Inline actions registered per view.
    inline_actions: HashMap<String, Vec<TreeItemInlineAction>>,
}

impl TreeViewRegistry {
    pub fn new() -> Self {
        Self {
            views: HashMap::new(),
            refresh_pending: Vec::new(),
            selections: HashMap::new(),
            inline_actions: HashMap::new(),
        }
    }

    // -- Registration -----------------------------------------------------

    /// Registers a new tree view.
    pub fn register_tree_view(
        &mut self,
        id: impl Into<String>,
        title: impl Into<String>,
        container: ViewContainer,
        extension_id: impl Into<String>,
    ) {
        let view_id = id.into();
        let view = ExtensionTreeView {
            id: view_id.clone(),
            title: title.into(),
            view_container: container,
            extension_id: extension_id.into(),
            root_items: Vec::new(),
            is_loading: false,
            drag_and_drop_enabled: false,
            can_select_many: false,
        };
        self.views.insert(view_id, view);
    }

    /// Unregisters a tree view.
    pub fn unregister(&mut self, id: &str) -> bool {
        self.selections.remove(id);
        self.inline_actions.remove(id);
        self.views.remove(id).is_some()
    }

    /// Unregisters all tree views for an extension.
    pub fn unregister_extension(&mut self, extension_id: &str) -> usize {
        let ids: Vec<String> = self
            .views
            .iter()
            .filter(|(_, v)| v.extension_id == extension_id)
            .map(|(id, _)| id.clone())
            .collect();
        let count = ids.len();
        for id in &ids {
            self.unregister(id);
        }
        count
    }

    // -- Data supply ------------------------------------------------------

    /// Sets the root items for a tree view (called after data provider resolves).
    pub fn set_root_items(&mut self, view_id: &str, items: Vec<TreeItem>) {
        if let Some(view) = self.views.get_mut(view_id) {
            view.root_items = items;
            view.is_loading = false;
        }
    }

    /// Sets the children for a specific tree item (lazy load on expand).
    pub fn set_children(&mut self, view_id: &str, parent_id: &str, children: Vec<TreeItem>) {
        if let Some(view) = self.views.get_mut(view_id) {
            set_children_recursive(&mut view.root_items, parent_id, children);
        }
    }

    // -- Refresh ----------------------------------------------------------

    /// Marks a tree view as needing refresh.
    pub fn refresh_tree(&mut self, view_id: &str) {
        if let Some(view) = self.views.get_mut(view_id) {
            view.is_loading = true;
            view.root_items.clear();
        }
        if !self.refresh_pending.contains(&view_id.to_owned()) {
            self.refresh_pending.push(view_id.to_owned());
        }
    }

    /// Takes all pending refresh requests.
    pub fn take_refresh_pending(&mut self) -> Vec<String> {
        std::mem::take(&mut self.refresh_pending)
    }

    // -- Reveal -----------------------------------------------------------

    /// Expands the tree path to reveal a specific item.
    pub fn reveal_tree_item(&mut self, view_id: &str, item_id: &str) -> bool {
        let Some(view) = self.views.get_mut(view_id) else {
            return false;
        };
        expand_path_to(&mut view.root_items, item_id)
    }

    // -- Selection --------------------------------------------------------

    /// Sets the selected items for a tree view.
    pub fn set_selection(&mut self, view_id: &str, item_ids: Vec<String>) {
        self.selections.insert(view_id.to_owned(), item_ids);
    }

    /// Gets the current selection for a tree view.
    pub fn selection(&self, view_id: &str) -> &[String] {
        self.selections.get(view_id).map_or(&[], Vec::as_slice)
    }

    // -- Inline actions ---------------------------------------------------

    /// Registers inline actions for a tree view.
    pub fn set_inline_actions(&mut self, view_id: &str, actions: Vec<TreeItemInlineAction>) {
        self.inline_actions.insert(view_id.to_owned(), actions);
    }

    /// Gets inline actions for a tree view.
    pub fn inline_actions(&self, view_id: &str) -> &[TreeItemInlineAction] {
        self.inline_actions.get(view_id).map_or(&[], Vec::as_slice)
    }

    // -- Queries ----------------------------------------------------------

    pub fn get(&self, id: &str) -> Option<&ExtensionTreeView> {
        self.views.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut ExtensionTreeView> {
        self.views.get_mut(id)
    }

    pub fn all_views(&self) -> impl Iterator<Item = &ExtensionTreeView> {
        self.views.values()
    }

    pub fn views_in_container(&self, container: &ViewContainer) -> Vec<&ExtensionTreeView> {
        self.views
            .values()
            .filter(|v| &v.view_container == container)
            .collect()
    }

    pub fn views_for_extension(&self, extension_id: &str) -> Vec<&ExtensionTreeView> {
        self.views
            .values()
            .filter(|v| v.extension_id == extension_id)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.views.len()
    }

    pub fn is_empty(&self) -> bool {
        self.views.is_empty()
    }
}

impl Default for TreeViewRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn set_children_recursive(items: &mut [TreeItem], parent_id: &str, children: Vec<TreeItem>) {
    for item in items.iter_mut() {
        if item.id == parent_id {
            item.children = Some(children);
            item.collapsible_state = CollapsibleState::Expanded;
            return;
        }
        if let Some(ref mut kids) = item.children {
            set_children_recursive(kids, parent_id, children.clone());
        }
    }
}

fn expand_path_to(items: &mut [TreeItem], target_id: &str) -> bool {
    for item in items.iter_mut() {
        if item.id == target_id {
            return true;
        }
        if let Some(ref mut children) = item.children {
            if expand_path_to(children, target_id) {
                item.collapsible_state = CollapsibleState::Expanded;
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tree_items() -> Vec<TreeItem> {
        vec![TreeItem {
            id: "root".into(),
            label: "Root".into(),
            collapsible_state: CollapsibleState::Collapsed,
            children: Some(vec![TreeItem {
                id: "child1".into(),
                label: "Child 1".into(),
                collapsible_state: CollapsibleState::None,
                children: None,
                description: None,
                tooltip: None,
                icon: None,
                command: None,
                context_value: None,
                resource_uri: None,
                accessibility_information: None,
            }]),
            description: None,
            tooltip: None,
            icon: None,
            command: None,
            context_value: None,
            resource_uri: None,
            accessibility_information: None,
        }]
    }

    #[test]
    fn register_and_query() {
        let mut reg = TreeViewRegistry::new();
        reg.register_tree_view("myView", "My Tree", ViewContainer::Explorer, "ext.my");
        assert_eq!(reg.len(), 1);
        assert!(reg.get("myView").is_some());
        assert_eq!(reg.views_for_extension("ext.my").len(), 1);
    }

    #[test]
    fn unregister() {
        let mut reg = TreeViewRegistry::new();
        reg.register_tree_view("myView", "My Tree", ViewContainer::Explorer, "ext.my");
        assert!(reg.unregister("myView"));
        assert!(reg.is_empty());
    }

    #[test]
    fn unregister_extension() {
        let mut reg = TreeViewRegistry::new();
        reg.register_tree_view("v1", "V1", ViewContainer::Explorer, "ext.a");
        reg.register_tree_view("v2", "V2", ViewContainer::Debug, "ext.a");
        reg.register_tree_view("v3", "V3", ViewContainer::Explorer, "ext.b");
        assert_eq!(reg.unregister_extension("ext.a"), 2);
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn set_root_items() {
        let mut reg = TreeViewRegistry::new();
        reg.register_tree_view("v", "V", ViewContainer::Explorer, "ext");
        reg.set_root_items("v", sample_tree_items());
        assert_eq!(reg.get("v").unwrap().root_items.len(), 1);
        assert!(!reg.get("v").unwrap().is_loading);
    }

    #[test]
    fn lazy_children() {
        let mut reg = TreeViewRegistry::new();
        reg.register_tree_view("v", "V", ViewContainer::Explorer, "ext");
        reg.set_root_items("v", sample_tree_items());

        let new_children = vec![TreeItem {
            id: "grandchild".into(),
            label: "Grandchild".into(),
            collapsible_state: CollapsibleState::None,
            children: None,
            description: None,
            tooltip: None,
            icon: None,
            command: None,
            context_value: None,
            resource_uri: None,
            accessibility_information: None,
        }];
        reg.set_children("v", "child1", new_children);

        let root = &reg.get("v").unwrap().root_items[0];
        let child = &root.children.as_ref().unwrap()[0];
        assert_eq!(child.children.as_ref().unwrap().len(), 1);
        assert_eq!(child.children.as_ref().unwrap()[0].id, "grandchild");
    }

    #[test]
    fn refresh() {
        let mut reg = TreeViewRegistry::new();
        reg.register_tree_view("v", "V", ViewContainer::Explorer, "ext");
        reg.set_root_items("v", sample_tree_items());

        reg.refresh_tree("v");
        assert!(reg.get("v").unwrap().is_loading);
        assert!(reg.get("v").unwrap().root_items.is_empty());

        let pending = reg.take_refresh_pending();
        assert_eq!(pending, vec!["v"]);
        assert!(reg.take_refresh_pending().is_empty());
    }

    #[test]
    fn reveal() {
        let mut reg = TreeViewRegistry::new();
        reg.register_tree_view("v", "V", ViewContainer::Explorer, "ext");
        reg.set_root_items("v", sample_tree_items());

        assert!(reg.reveal_tree_item("v", "child1"));
        let root = &reg.get("v").unwrap().root_items[0];
        assert_eq!(root.collapsible_state, CollapsibleState::Expanded);
    }

    #[test]
    fn selection() {
        let mut reg = TreeViewRegistry::new();
        reg.register_tree_view("v", "V", ViewContainer::Explorer, "ext");
        reg.set_selection("v", vec!["item1".into(), "item2".into()]);
        assert_eq!(reg.selection("v").len(), 2);
        assert!(reg.selection("other").is_empty());
    }

    #[test]
    fn views_in_container() {
        let mut reg = TreeViewRegistry::new();
        reg.register_tree_view("v1", "V1", ViewContainer::Explorer, "ext");
        reg.register_tree_view("v2", "V2", ViewContainer::Debug, "ext");
        reg.register_tree_view("v3", "V3", ViewContainer::Explorer, "ext");
        assert_eq!(reg.views_in_container(&ViewContainer::Explorer).len(), 2);
        assert_eq!(reg.views_in_container(&ViewContainer::Debug).len(), 1);
    }

    #[test]
    fn inline_actions() {
        let mut reg = TreeViewRegistry::new();
        reg.register_tree_view("v", "V", ViewContainer::Explorer, "ext");
        reg.set_inline_actions(
            "v",
            vec![TreeItemInlineAction {
                command: "myExt.delete".into(),
                title: "Delete".into(),
                icon: None,
                when: None,
            }],
        );
        assert_eq!(reg.inline_actions("v").len(), 1);
        assert!(reg.inline_actions("other").is_empty());
    }

    #[test]
    fn tree_item_serialize() {
        let item = TreeItem {
            id: "test".into(),
            label: "Test Item".into(),
            description: Some("desc".into()),
            tooltip: Some("tip".into()),
            icon: Some(TreeItemIcon {
                light: "light.svg".into(),
                dark: "dark.svg".into(),
            }),
            collapsible_state: CollapsibleState::Collapsed,
            command: Some(TreeItemCommand {
                command: "myExt.open".into(),
                title: "Open".into(),
                arguments: vec![serde_json::json!("arg1")],
            }),
            context_value: Some("myContext".into()),
            children: None,
            resource_uri: Some("file:///test.rs".into()),
            accessibility_information: None,
        };
        let json = serde_json::to_string(&item).unwrap();
        let back: TreeItem = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "test");
        assert_eq!(back.icon.as_ref().unwrap().dark, "dark.svg");
        assert_eq!(back.command.as_ref().unwrap().arguments.len(), 1);
    }
}
