//! File tree model — in-memory tree of workspace files.
//!
//! Uses the `ignore` crate to respect `.gitignore` rules and provides
//! lazy loading: only one level deep is scanned initially, with subtrees
//! expanded on demand via [`FileTree::expand`].
//!
//! Extended features:
//! - File decorations (badges, colors, strikethrough for git status)
//! - Drag-and-drop move, multi-select, inline rename, new file/folder
//! - Cut/copy/paste, file nesting, sort modes, compact folders
//! - Open editors section, timeline integration

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use sidex_theme::color::Color;

// ---------------------------------------------------------------------------
// File decorations
// ---------------------------------------------------------------------------

/// Visual decorations applied to file entries (e.g. git status badges).
#[derive(Debug, Clone, Default, Serialize)]
pub struct FileDecorations {
    pub items: HashMap<PathBuf, Vec<FileDecoration>>,
}

impl FileDecorations {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, path: PathBuf, decoration: FileDecoration) {
        self.items.entry(path).or_default().push(decoration);
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn get(&self, path: &Path) -> Option<&Vec<FileDecoration>> {
        self.items.get(path)
    }

    pub fn remove(&mut self, path: &Path) {
        self.items.remove(path);
    }
}

/// A single decoration on a file entry.
#[derive(Debug, Clone, Default, Serialize)]
pub struct FileDecoration {
    pub badge: Option<String>,
    pub badge_color: Option<Color>,
    pub tooltip: Option<String>,
    pub strikethrough: bool,
    pub faded: bool,
    pub color: Option<Color>,
}

// ---------------------------------------------------------------------------
// File nesting
// ---------------------------------------------------------------------------

/// Rule that nests related files under a parent (e.g. `.test.ts` under `.ts`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileNestingRule {
    pub pattern: String,
    pub children: Vec<String>,
}

/// Default nesting rules matching VS Code behavior.
pub fn default_nesting_rules() -> Vec<FileNestingRule> {
    vec![
        FileNestingRule {
            pattern: ".ts".into(),
            children: vec![".test.ts".into(), ".spec.ts".into(), ".d.ts".into()],
        },
        FileNestingRule {
            pattern: ".tsx".into(),
            children: vec![
                ".test.tsx".into(),
                ".spec.tsx".into(),
                ".stories.tsx".into(),
                ".module.css".into(),
            ],
        },
        FileNestingRule {
            pattern: ".js".into(),
            children: vec![
                ".test.js".into(),
                ".spec.js".into(),
                ".min.js".into(),
                ".map".into(),
            ],
        },
        FileNestingRule {
            pattern: ".jsx".into(),
            children: vec![
                ".test.jsx".into(),
                ".spec.jsx".into(),
                ".stories.jsx".into(),
            ],
        },
        FileNestingRule {
            pattern: ".css".into(),
            children: vec![".css.map".into()],
        },
        FileNestingRule {
            pattern: ".rs".into(),
            children: vec![],
        },
        FileNestingRule {
            pattern: ".py".into(),
            children: vec![".pyi".into()],
        },
    ]
}

/// Apply nesting rules: given a flat list of file names in a directory,
/// return a map of parent → nested children.
pub fn compute_nesting(
    names: &[String],
    rules: &[FileNestingRule],
) -> HashMap<String, Vec<String>> {
    let mut nested: HashMap<String, Vec<String>> = HashMap::new();
    let mut claimed: std::collections::HashSet<String> = std::collections::HashSet::new();

    for rule in rules {
        for name in names {
            if !name.ends_with(&rule.pattern) {
                continue;
            }
            let stem = &name[..name.len() - rule.pattern.len()];
            for child_suffix in &rule.children {
                let child_name = format!("{stem}{child_suffix}");
                if names.contains(&child_name) && !claimed.contains(&child_name) {
                    nested
                        .entry(name.clone())
                        .or_default()
                        .push(child_name.clone());
                    claimed.insert(child_name);
                }
            }
        }
    }

    nested
}

// ---------------------------------------------------------------------------
// Sort order
// ---------------------------------------------------------------------------

/// How files are sorted in the explorer tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FileSortOrder {
    #[default]
    Name,
    Type,
    Modified,
    Size,
}

// ---------------------------------------------------------------------------
// Multi-select / clipboard
// ---------------------------------------------------------------------------

/// Tracks multi-selection state in the file explorer.
#[derive(Debug, Clone, Default, Serialize)]
pub struct FileSelection {
    pub selected: Vec<PathBuf>,
    pub anchor: Option<PathBuf>,
}

impl FileSelection {
    pub fn clear(&mut self) {
        self.selected.clear();
        self.anchor = None;
    }

    pub fn select_single(&mut self, path: PathBuf) {
        self.selected.clear();
        self.selected.push(path.clone());
        self.anchor = Some(path);
    }

    pub fn toggle(&mut self, path: PathBuf) {
        if let Some(pos) = self.selected.iter().position(|p| p == &path) {
            self.selected.remove(pos);
        } else {
            self.selected.push(path.clone());
            self.anchor = Some(path);
        }
    }

    pub fn is_selected(&self, path: &Path) -> bool {
        self.selected.iter().any(|p| p == path)
    }

    pub fn count(&self) -> usize {
        self.selected.len()
    }
}

/// Clipboard for cut/copy/paste file operations.
#[derive(Debug, Clone, Default, Serialize)]
pub struct FileClipboard {
    pub paths: Vec<PathBuf>,
    pub is_cut: bool,
}

impl FileClipboard {
    pub fn cut(&mut self, paths: Vec<PathBuf>) {
        self.paths = paths;
        self.is_cut = true;
    }

    pub fn copy(&mut self, paths: Vec<PathBuf>) {
        self.paths = paths;
        self.is_cut = false;
    }

    pub fn clear(&mut self) {
        self.paths.clear();
        self.is_cut = false;
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Drag-and-drop
// ---------------------------------------------------------------------------

/// State for a drag-and-drop file move operation.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DragDropState {
    pub source_paths: Vec<PathBuf>,
    pub target_dir: Option<PathBuf>,
    pub is_dragging: bool,
}

impl DragDropState {
    pub fn begin(&mut self, paths: Vec<PathBuf>) {
        self.source_paths = paths;
        self.is_dragging = true;
        self.target_dir = None;
    }

    pub fn hover(&mut self, dir: PathBuf) {
        self.target_dir = Some(dir);
    }

    pub fn cancel(&mut self) {
        *self = Self::default();
    }

    /// Finalize the drop; returns (sources, target) if valid.
    pub fn finish(&mut self) -> Option<(Vec<PathBuf>, PathBuf)> {
        if !self.is_dragging {
            return None;
        }
        let target = self.target_dir.take()?;
        let sources = std::mem::take(&mut self.source_paths);
        self.is_dragging = false;
        if sources.is_empty() {
            return None;
        }
        Some((sources, target))
    }
}

// ---------------------------------------------------------------------------
// Inline rename / create
// ---------------------------------------------------------------------------

/// State for an inline rename or new-file/new-folder input.
#[derive(Debug, Clone, Serialize)]
pub enum InlineInputKind {
    Rename { original_path: PathBuf },
    NewFile { parent_dir: PathBuf },
    NewFolder { parent_dir: PathBuf },
}

#[derive(Debug, Clone, Serialize)]
pub struct InlineInput {
    pub kind: InlineInputKind,
    pub value: String,
}

// ---------------------------------------------------------------------------
// Open editors section
// ---------------------------------------------------------------------------

/// An entry in the "Open Editors" section at the top of the explorer.
#[derive(Debug, Clone, Serialize)]
pub struct OpenEditorEntry {
    pub path: PathBuf,
    pub is_modified: bool,
    pub is_pinned: bool,
    pub is_preview: bool,
}

/// Tracks open editor tabs for display in the explorer.
#[derive(Debug, Clone, Default, Serialize)]
pub struct OpenEditorsSection {
    pub entries: Vec<OpenEditorEntry>,
    pub visible: bool,
}

impl OpenEditorsSection {
    pub fn update(&mut self, entries: Vec<OpenEditorEntry>) {
        self.entries = entries;
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

// ---------------------------------------------------------------------------
// File node (extended)
// ---------------------------------------------------------------------------

/// A single node in the file tree — either a file or a directory.
#[derive(Debug, Clone, Serialize)]
pub struct FileNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    /// `None` for files; `Some(vec)` for directories.
    /// An empty `Some(vec![])` means the directory has been expanded but is empty.
    /// `None` on a directory means it has not been expanded yet.
    pub children: Option<Vec<FileNode>>,
    /// Nested related files (computed from `FileNestingRule`s).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nested_children: Option<Vec<FileNode>>,
    /// Size in bytes (populated on demand).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// Last modification time (populated on demand).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<SystemTime>,
    /// Whether this node is a compacted folder chain (e.g. "src/utils").
    #[serde(skip_serializing_if = "is_false")]
    pub is_compact: bool,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(b: &bool) -> bool {
    !b
}

impl FileNode {
    fn file(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            is_dir: false,
            children: None,
            nested_children: None,
            size: None,
            modified: None,
            is_compact: false,
        }
    }

    fn file_with_meta(name: String, path: PathBuf, size: u64, modified: SystemTime) -> Self {
        Self {
            name,
            path,
            is_dir: false,
            children: None,
            nested_children: None,
            size: Some(size),
            modified: Some(modified),
            is_compact: false,
        }
    }

    fn dir(name: String, path: PathBuf, children: Vec<FileNode>) -> Self {
        Self {
            name,
            path,
            is_dir: true,
            children: Some(children),
            nested_children: None,
            size: None,
            modified: None,
            is_compact: false,
        }
    }

    fn dir_lazy(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            is_dir: true,
            children: None,
            nested_children: None,
            size: None,
            modified: None,
            is_compact: false,
        }
    }

    /// Extension of the file, if any.
    pub fn extension(&self) -> Option<&str> {
        self.path.extension().and_then(|e| e.to_str())
    }
}

/// In-memory file tree for a workspace.
#[derive(Debug, Clone, Serialize)]
pub struct FileTree {
    pub root: FileNode,
    #[serde(skip)]
    pub sort_order: FileSortOrder,
    #[serde(skip)]
    pub compact_folders: bool,
    #[serde(skip)]
    pub nesting_rules: Vec<FileNestingRule>,
    #[serde(skip)]
    pub selection: FileSelection,
    #[serde(skip)]
    pub clipboard: FileClipboard,
    #[serde(skip)]
    pub drag_drop: DragDropState,
    #[serde(skip)]
    pub inline_input: Option<InlineInput>,
    #[serde(skip)]
    pub open_editors: OpenEditorsSection,
    #[serde(skip)]
    pub decorations: FileDecorations,
}

impl FileTree {
    /// Scan `root` one level deep, respecting `.gitignore`.
    pub fn scan(root: &Path) -> Self {
        let root_name = root
            .file_name()
            .unwrap_or_else(|| OsStr::new(""))
            .to_string_lossy()
            .into_owned();

        let children = scan_one_level(root);
        let root_node = FileNode::dir(root_name, root.to_path_buf(), children);

        Self {
            root: root_node,
            sort_order: FileSortOrder::default(),
            compact_folders: true,
            nesting_rules: default_nesting_rules(),
            selection: FileSelection::default(),
            clipboard: FileClipboard::default(),
            drag_drop: DragDropState::default(),
            inline_input: None,
            open_editors: OpenEditorsSection::default(),
            decorations: FileDecorations::default(),
        }
    }

    /// Expand a directory at `path` — replaces its children with a fresh one-level scan.
    pub fn expand(&mut self, path: &Path) {
        let sort_order = self.sort_order;
        let compact = self.compact_folders;
        if let Some(node) = self.find_mut(path) {
            if node.is_dir {
                let mut children = scan_one_level(path);
                sort_nodes(&mut children, sort_order);
                if compact {
                    compact_single_child_dirs(&mut children);
                }
                node.children = Some(children);
            }
        }
    }

    /// Rescan a subtree rooted at `path`.
    pub fn refresh(&mut self, path: &Path) {
        self.expand(path);
    }

    /// Find a node by its path (immutable).
    pub fn find(&self, path: &Path) -> Option<&FileNode> {
        find_in_node(&self.root, path)
    }

    /// Find a node by its path (mutable).
    fn find_mut(&mut self, path: &Path) -> Option<&mut FileNode> {
        find_in_node_mut(&mut self.root, path)
    }

    /// Change the sort order and re-sort all expanded directories.
    pub fn set_sort_order(&mut self, order: FileSortOrder) {
        self.sort_order = order;
        resort_recursive(&mut self.root, order);
    }

    /// Toggle compact-folders mode and re-expand the root.
    pub fn set_compact_folders(&mut self, enabled: bool) {
        self.compact_folders = enabled;
    }

    /// Begin an inline rename for the given path.
    pub fn begin_rename(&mut self, path: &Path) {
        if let Some(node) = self.find(path) {
            self.inline_input = Some(InlineInput {
                kind: InlineInputKind::Rename {
                    original_path: node.path.clone(),
                },
                value: node.name.clone(),
            });
        }
    }

    /// Begin creating a new file in the given directory.
    pub fn begin_new_file(&mut self, parent_dir: &Path) {
        self.inline_input = Some(InlineInput {
            kind: InlineInputKind::NewFile {
                parent_dir: parent_dir.to_path_buf(),
            },
            value: String::new(),
        });
    }

    /// Begin creating a new folder in the given directory.
    pub fn begin_new_folder(&mut self, parent_dir: &Path) {
        self.inline_input = Some(InlineInput {
            kind: InlineInputKind::NewFolder {
                parent_dir: parent_dir.to_path_buf(),
            },
            value: String::new(),
        });
    }

    /// Cancel any active inline input.
    pub fn cancel_inline_input(&mut self) {
        self.inline_input = None;
    }

    /// Apply file nesting rules to a given directory node.
    pub fn apply_nesting(&mut self, dir_path: &Path) {
        let rules = self.nesting_rules.clone();
        if let Some(node) = self.find_mut(dir_path) {
            if let Some(ref mut children) = node.children {
                let names: Vec<String> = children.iter().map(|c| c.name.clone()).collect();
                let nesting = compute_nesting(&names, &rules);

                let mut nested_names: std::collections::HashSet<String> =
                    std::collections::HashSet::new();
                for children_list in nesting.values() {
                    for name in children_list {
                        nested_names.insert(name.clone());
                    }
                }

                // Pre-collect nested nodes by cloning from the children list.
                let children_snapshot: Vec<FileNode> = children.clone();

                for child in children.iter_mut() {
                    if let Some(nested) = nesting.get(&child.name) {
                        let nested_nodes: Vec<FileNode> = nested
                            .iter()
                            .filter_map(|n| {
                                children_snapshot.iter().find(|c| c.name == *n).cloned()
                            })
                            .collect();
                        child.nested_children = Some(nested_nodes);
                    }
                }

                children.retain(|c| !nested_names.contains(&c.name));
            }
        }
    }
}

/// Scan one level of a directory, respecting `.gitignore`.
fn scan_one_level(dir: &Path) -> Vec<FileNode> {
    let mut entries = Vec::new();

    for result in WalkBuilder::new(dir)
        .max_depth(Some(1))
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build()
    {
        let Ok(entry) = result else { continue };

        // Skip the root directory itself.
        if entry.path() == dir {
            continue;
        }

        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path().to_path_buf();

        let node = if entry.file_type().is_some_and(|ft| ft.is_dir()) {
            FileNode::dir_lazy(name, path)
        } else {
            let meta = entry.metadata().ok();
            match meta {
                Some(m) => {
                    let size = m.len();
                    let modified = m.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                    FileNode::file_with_meta(name, path, size, modified)
                }
                None => FileNode::file(name, path),
            }
        };
        entries.push(node);
    }

    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then_with(|| {
            a.name
                .to_ascii_lowercase()
                .cmp(&b.name.to_ascii_lowercase())
        })
    });

    entries
}

/// Sort nodes in-place according to the given order.
fn sort_nodes(nodes: &mut [FileNode], order: FileSortOrder) {
    match order {
        FileSortOrder::Name => {
            nodes.sort_by(|a, b| {
                b.is_dir.cmp(&a.is_dir).then_with(|| {
                    a.name
                        .to_ascii_lowercase()
                        .cmp(&b.name.to_ascii_lowercase())
                })
            });
        }
        FileSortOrder::Type => {
            nodes.sort_by(|a, b| {
                b.is_dir.cmp(&a.is_dir).then_with(|| {
                    let ext_a = a.extension().unwrap_or("");
                    let ext_b = b.extension().unwrap_or("");
                    ext_a.cmp(ext_b).then_with(|| {
                        a.name
                            .to_ascii_lowercase()
                            .cmp(&b.name.to_ascii_lowercase())
                    })
                })
            });
        }
        FileSortOrder::Modified => {
            nodes.sort_by(|a, b| {
                b.is_dir.cmp(&a.is_dir).then_with(|| {
                    let ma = a.modified.unwrap_or(SystemTime::UNIX_EPOCH);
                    let mb = b.modified.unwrap_or(SystemTime::UNIX_EPOCH);
                    mb.cmp(&ma)
                })
            });
        }
        FileSortOrder::Size => {
            nodes.sort_by(|a, b| {
                b.is_dir.cmp(&a.is_dir).then_with(|| {
                    let sa = a.size.unwrap_or(0);
                    let sb = b.size.unwrap_or(0);
                    sb.cmp(&sa)
                })
            });
        }
    }
}

/// Recursively re-sort all expanded children.
fn resort_recursive(node: &mut FileNode, order: FileSortOrder) {
    if let Some(ref mut children) = node.children {
        sort_nodes(children, order);
        for child in children.iter_mut() {
            resort_recursive(child, order);
        }
    }
}

/// Compact single-child directory chains (e.g. `src/` → `src/utils/`).
fn compact_single_child_dirs(nodes: &mut [FileNode]) {
    for node in nodes.iter_mut() {
        if !node.is_dir {
            continue;
        }
        while let Some(ref children) = node.children {
            if children.len() == 1 && children[0].is_dir {
                let child = children[0].clone();
                node.name = format!("{}/{}", node.name, child.name);
                node.path = child.path;
                node.children = child.children;
                node.is_compact = true;
            } else {
                break;
            }
        }
    }
}

fn find_in_node<'a>(node: &'a FileNode, target: &Path) -> Option<&'a FileNode> {
    if node.path == target {
        return Some(node);
    }
    if let Some(children) = &node.children {
        for child in children {
            if let Some(found) = find_in_node(child, target) {
                return Some(found);
            }
        }
    }
    None
}

fn find_in_node_mut<'a>(node: &'a mut FileNode, target: &Path) -> Option<&'a mut FileNode> {
    if node.path == target {
        return Some(node);
    }
    if let Some(children) = &mut node.children {
        for child in children {
            if let Some(found) = find_in_node_mut(child, target) {
                return Some(found);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_tree() -> TempDir {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        fs::write(tmp.path().join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(tmp.path().join("README.md"), "# hi").unwrap();
        tmp
    }

    #[test]
    fn scan_lists_entries() {
        let tmp = make_tree();
        let tree = FileTree::scan(tmp.path());
        assert!(tree.root.is_dir);
        let children = tree.root.children.as_ref().unwrap();
        assert!(children.iter().any(|n| n.name == "src" && n.is_dir));
        assert!(children.iter().any(|n| n.name == "README.md" && !n.is_dir));
    }

    #[test]
    fn find_returns_node() {
        let tmp = make_tree();
        let tree = FileTree::scan(tmp.path());
        let node = tree.find(&tmp.path().join("src"));
        assert!(node.is_some());
        assert!(node.unwrap().is_dir);
    }

    #[test]
    fn expand_populates_children() {
        let tmp = make_tree();
        let mut tree = FileTree::scan(tmp.path());

        let src = tree.find(&tmp.path().join("src")).unwrap();
        assert!(src.children.is_none(), "lazy: children not loaded yet");

        tree.expand(&tmp.path().join("src"));

        let src = tree.find(&tmp.path().join("src")).unwrap();
        let children = src.children.as_ref().unwrap();
        assert!(children.iter().any(|n| n.name == "main.rs"));
    }
}
