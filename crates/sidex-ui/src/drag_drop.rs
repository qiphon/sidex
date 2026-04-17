//! Drag and drop system — manages drag sessions, drop targets, and drop zone indicators.

use std::path::PathBuf;

use crate::layout::Rect;

// ── Drag data ────────────────────────────────────────────────────────────────

/// Payload carried by a drag session.
#[derive(Clone, Debug)]
pub enum DragData {
    Files(Vec<PathBuf>),
    Text(String),
    EditorTab { group_id: usize, tab_index: usize },
    TreeItem { view_id: String, item_id: String },
    Terminal { instance_id: u32 },
    Uri(Vec<String>),
}

/// Coarse kind discriminant for matching against [`DropTarget::accepts`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DragDataKind {
    Files,
    Text,
    EditorTab,
    TreeItem,
    Terminal,
    Uri,
}

impl DragData {
    pub fn kind(&self) -> DragDataKind {
        match self {
            Self::Files(_) => DragDataKind::Files,
            Self::Text(_) => DragDataKind::Text,
            Self::EditorTab { .. } => DragDataKind::EditorTab,
            Self::TreeItem { .. } => DragDataKind::TreeItem,
            Self::Terminal { .. } => DragDataKind::Terminal,
            Self::Uri(_) => DragDataKind::Uri,
        }
    }
}

// ── Source / effect / preview ────────────────────────────────────────────────

/// Where the drag originated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DragSource {
    Explorer,
    EditorTabs,
    EditorContent,
    Terminal,
    External,
    TreeView(String),
}

/// Visual feedback for the drag operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragEffect {
    None,
    Copy,
    Move,
    Link,
}

/// A label/icon ghost shown under the cursor during drag.
#[derive(Clone, Debug)]
pub struct DragPreview {
    pub label: String,
    pub icon: Option<String>,
    pub badge: Option<String>,
}

impl DragPreview {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            badge: None,
        }
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn with_badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }
}

// ── Drag session ─────────────────────────────────────────────────────────────

/// An in-flight drag operation.
#[derive(Clone, Debug)]
pub struct DragSession {
    pub data: DragData,
    pub source: DragSource,
    pub position: (f32, f32),
    pub preview: DragPreview,
    pub allowed_effects: DragEffect,
}

impl DragSession {
    pub fn update_position(&mut self, x: f32, y: f32) {
        self.position = (x, y);
    }
}

// ── Drop target ──────────────────────────────────────────────────────────────

/// A registered region that can accept drops.
#[derive(Clone, Debug)]
pub struct DropTarget {
    pub id: String,
    pub bounds: Rect,
    pub accepts: Vec<DragDataKind>,
    pub split_zones: bool,
}

impl DropTarget {
    pub fn new(id: impl Into<String>, bounds: Rect, accepts: Vec<DragDataKind>) -> Self {
        Self {
            id: id.into(),
            bounds,
            accepts,
            split_zones: false,
        }
    }

    pub fn with_split_zones(mut self) -> Self {
        self.split_zones = true;
        self
    }

    pub fn accepts_kind(&self, kind: DragDataKind) -> bool {
        self.accepts.contains(&kind)
    }

    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        self.bounds.contains(x, y)
    }
}

// ── Drop zone indicator ──────────────────────────────────────────────────────

/// Where the drop would land relative to the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropZonePosition {
    Before,
    After,
    On,
    Left,
    Right,
    Top,
    Bottom,
}

/// Visual indicator showing where a drop will land.
#[derive(Clone, Debug)]
pub struct DropZoneIndicator {
    pub target_id: String,
    pub position: DropZonePosition,
    pub rect: Rect,
}

// ── Drag-drop manager ────────────────────────────────────────────────────────

/// Manages the global drag-and-drop state: active session, registered targets,
/// and drop zone indicators.
#[derive(Debug)]
pub struct DragDropManager {
    pub active_drag: Option<DragSession>,
    drop_targets: Vec<DropTarget>,
    current_indicator: Option<DropZoneIndicator>,
    hover_target_id: Option<String>,
}

impl Default for DragDropManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DragDropManager {
    pub fn new() -> Self {
        Self {
            active_drag: None,
            drop_targets: Vec::new(),
            current_indicator: None,
            hover_target_id: None,
        }
    }

    // ── Target registration ──────────────────────────────────────────────

    pub fn register_target(&mut self, target: DropTarget) {
        self.drop_targets.retain(|t| t.id != target.id);
        self.drop_targets.push(target);
    }

    pub fn unregister_target(&mut self, id: &str) {
        self.drop_targets.retain(|t| t.id != id);
    }

    pub fn clear_targets(&mut self) {
        self.drop_targets.clear();
    }

    pub fn targets(&self) -> &[DropTarget] {
        &self.drop_targets
    }

    // ── Drag lifecycle ───────────────────────────────────────────────────

    /// Begins a new drag operation.
    pub fn handle_drag_start(&mut self, source: DragSource, data: DragData) -> &DragSession {
        let preview = default_preview(&data);
        let effect = default_effect(&source);
        self.active_drag = Some(DragSession {
            data,
            source,
            position: (0.0, 0.0),
            preview,
            allowed_effects: effect,
        });
        self.active_drag.as_ref().unwrap()
    }

    /// Updates cursor position and computes drop zone feedback.
    /// Returns the effect that would apply if dropped here.
    pub fn handle_drag_over(&mut self, x: f32, y: f32) -> DragEffect {
        if let Some(ref mut session) = self.active_drag {
            session.update_position(x, y);
        }

        let data_kind = match &self.active_drag {
            Some(s) => s.data.kind(),
            None => return DragEffect::None,
        };

        let hit = self
            .drop_targets
            .iter()
            .find(|t| t.contains_point(x, y) && t.accepts_kind(data_kind));

        match hit {
            Some(target) => {
                let zone_pos = if target.split_zones {
                    compute_split_zone(x, y, target.bounds)
                } else {
                    DropZonePosition::On
                };

                let indicator_rect = compute_indicator_rect(target.bounds, zone_pos);

                self.hover_target_id = Some(target.id.clone());
                self.current_indicator = Some(DropZoneIndicator {
                    target_id: target.id.clone(),
                    position: zone_pos,
                    rect: indicator_rect,
                });

                match data_kind {
                    DragDataKind::EditorTab | DragDataKind::Terminal => DragEffect::Move,
                    DragDataKind::Files | DragDataKind::Uri => DragEffect::Copy,
                    DragDataKind::Text => DragEffect::Copy,
                    DragDataKind::TreeItem => DragEffect::Move,
                }
            }
            None => {
                self.hover_target_id = None;
                self.current_indicator = None;
                DragEffect::None
            }
        }
    }

    /// Completes the drop. Returns the session and the target it landed on,
    /// or `None` if no valid target was hit.
    pub fn handle_drop(&mut self) -> Option<(DragSession, String, DropZonePosition)> {
        let session = self.active_drag.take()?;
        let indicator = self.current_indicator.take()?;
        self.hover_target_id = None;

        let target = self.drop_targets.iter().find(|t| t.id == indicator.target_id)?;
        if !target.accepts_kind(session.data.kind()) {
            return None;
        }

        Some((session, indicator.target_id, indicator.position))
    }

    /// Cancels the current drag without dropping.
    pub fn cancel_drag(&mut self) {
        self.active_drag = None;
        self.current_indicator = None;
        self.hover_target_id = None;
    }

    // ── Queries ──────────────────────────────────────────────────────────

    pub fn is_dragging(&self) -> bool {
        self.active_drag.is_some()
    }

    pub fn current_indicator(&self) -> Option<&DropZoneIndicator> {
        self.current_indicator.as_ref()
    }

    pub fn hover_target_id(&self) -> Option<&str> {
        self.hover_target_id.as_deref()
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn default_preview(data: &DragData) -> DragPreview {
    match data {
        DragData::Files(paths) => {
            let label = if paths.len() == 1 {
                paths[0]
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "file".into())
            } else {
                format!("{} files", paths.len())
            };
            DragPreview::new(label).with_icon("file")
        }
        DragData::Text(t) => {
            let label = if t.len() > 40 {
                format!("{}…", &t[..40])
            } else {
                t.clone()
            };
            DragPreview::new(label)
        }
        DragData::EditorTab { .. } => DragPreview::new("Tab").with_icon("file"),
        DragData::TreeItem { item_id, .. } => DragPreview::new(item_id.clone()),
        DragData::Terminal { instance_id } => {
            DragPreview::new(format!("Terminal #{instance_id}")).with_icon("terminal")
        }
        DragData::Uri(uris) => {
            let label = if uris.len() == 1 {
                uris[0].clone()
            } else {
                format!("{} items", uris.len())
            };
            DragPreview::new(label).with_icon("link")
        }
    }
}

fn default_effect(source: &DragSource) -> DragEffect {
    match source {
        DragSource::Explorer => DragEffect::Move,
        DragSource::EditorTabs => DragEffect::Move,
        DragSource::EditorContent => DragEffect::Copy,
        DragSource::Terminal => DragEffect::Move,
        DragSource::External => DragEffect::Copy,
        DragSource::TreeView(_) => DragEffect::Move,
    }
}

const EDGE_FRACTION: f32 = 0.25;
const INDICATOR_THICKNESS: f32 = 2.0;

fn compute_split_zone(x: f32, y: f32, bounds: Rect) -> DropZonePosition {
    let rel_x = (x - bounds.x) / bounds.width;
    let rel_y = (y - bounds.y) / bounds.height;

    if rel_x < EDGE_FRACTION {
        DropZonePosition::Left
    } else if rel_x > 1.0 - EDGE_FRACTION {
        DropZonePosition::Right
    } else if rel_y < EDGE_FRACTION {
        DropZonePosition::Top
    } else if rel_y > 1.0 - EDGE_FRACTION {
        DropZonePosition::Bottom
    } else {
        DropZonePosition::On
    }
}

fn compute_indicator_rect(bounds: Rect, position: DropZonePosition) -> Rect {
    match position {
        DropZonePosition::Before | DropZonePosition::Left => {
            Rect::new(bounds.x, bounds.y, INDICATOR_THICKNESS, bounds.height)
        }
        DropZonePosition::After | DropZonePosition::Right => Rect::new(
            bounds.x + bounds.width - INDICATOR_THICKNESS,
            bounds.y,
            INDICATOR_THICKNESS,
            bounds.height,
        ),
        DropZonePosition::Top => {
            Rect::new(bounds.x, bounds.y, bounds.width, INDICATOR_THICKNESS)
        }
        DropZonePosition::Bottom => Rect::new(
            bounds.x,
            bounds.y + bounds.height - INDICATOR_THICKNESS,
            bounds.width,
            INDICATOR_THICKNESS,
        ),
        DropZonePosition::On => bounds,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_lifecycle() {
        let mut mgr = DragDropManager::new();

        mgr.register_target(DropTarget::new(
            "editor",
            Rect::new(0.0, 0.0, 800.0, 600.0),
            vec![DragDataKind::Files, DragDataKind::EditorTab],
        ));

        let data = DragData::Files(vec![PathBuf::from("main.rs")]);
        mgr.handle_drag_start(DragSource::Explorer, data);
        assert!(mgr.is_dragging());

        let effect = mgr.handle_drag_over(400.0, 300.0);
        assert_ne!(effect, DragEffect::None);
        assert!(mgr.current_indicator().is_some());

        let result = mgr.handle_drop();
        assert!(result.is_some());
        assert!(!mgr.is_dragging());
    }

    #[test]
    fn cancel_clears_state() {
        let mut mgr = DragDropManager::new();
        mgr.handle_drag_start(DragSource::External, DragData::Text("hello".into()));
        assert!(mgr.is_dragging());

        mgr.cancel_drag();
        assert!(!mgr.is_dragging());
        assert!(mgr.current_indicator().is_none());
    }

    #[test]
    fn split_zones() {
        let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
        assert_eq!(compute_split_zone(5.0, 50.0, bounds), DropZonePosition::Left);
        assert_eq!(compute_split_zone(95.0, 50.0, bounds), DropZonePosition::Right);
        assert_eq!(compute_split_zone(50.0, 5.0, bounds), DropZonePosition::Top);
        assert_eq!(compute_split_zone(50.0, 95.0, bounds), DropZonePosition::Bottom);
        assert_eq!(compute_split_zone(50.0, 50.0, bounds), DropZonePosition::On);
    }

    #[test]
    fn rejects_unaccepted_kind() {
        let mut mgr = DragDropManager::new();

        mgr.register_target(DropTarget::new(
            "tab-bar",
            Rect::new(0.0, 0.0, 800.0, 35.0),
            vec![DragDataKind::EditorTab],
        ));

        mgr.handle_drag_start(DragSource::External, DragData::Text("x".into()));
        let effect = mgr.handle_drag_over(400.0, 15.0);
        assert_eq!(effect, DragEffect::None);
    }

    #[test]
    fn data_kind_round_trip() {
        let cases: Vec<(DragData, DragDataKind)> = vec![
            (DragData::Files(vec![]), DragDataKind::Files),
            (DragData::Text(String::new()), DragDataKind::Text),
            (
                DragData::EditorTab { group_id: 0, tab_index: 0 },
                DragDataKind::EditorTab,
            ),
            (
                DragData::TreeItem { view_id: String::new(), item_id: String::new() },
                DragDataKind::TreeItem,
            ),
            (DragData::Terminal { instance_id: 1 }, DragDataKind::Terminal),
            (DragData::Uri(vec![]), DragDataKind::Uri),
        ];

        for (data, expected) in cases {
            assert_eq!(data.kind(), expected);
        }
    }
}
