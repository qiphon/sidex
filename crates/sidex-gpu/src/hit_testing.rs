//! Hit testing service for mouse interactions.
//!
//! Maintains a spatial index of interactive regions with z-ordering. Each
//! region maps a screen-space rectangle to a [`HitTarget`] describing what
//! the user would be interacting with, along with a mouse cursor to display.

use crate::editor_compositor::Rect;

// ── Hit targets ─────────────────────────────────────────────────────────────

/// Describes what the mouse is over.
#[derive(Debug, Clone)]
pub enum HitTarget {
    EditorContent { line: u32, column: u32 },
    Gutter { line: u32, zone: GutterZone },
    Minimap { line: u32 },
    ScrollbarThumb { orientation: ScrollbarHitOrientation },
    ScrollbarTrack { orientation: ScrollbarHitOrientation },
    Tab { group: u32, index: u32 },
    TabClose { group: u32, index: u32 },
    Button { id: String },
    TreeItem { view: String, item_id: String },
    Link { url: String },
    ResizeHandle { direction: ResizeDirection },
    StatusBarItem { id: String },
    MenuItem { menu: String, item: usize },
    Nothing,
}

/// Which zone of the gutter was hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GutterZone {
    LineNumbers,
    FoldingIndicator,
    Breakpoint,
    GitDecoration,
}

/// Orientation for scrollbar hit targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarHitOrientation {
    Vertical,
    Horizontal,
}

/// Direction of a resize handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeDirection {
    Horizontal,
    Vertical,
    Both,
}

/// Mouse cursor to display over a hit region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseCursor {
    Default,
    Text,
    Pointer,
    ResizeHorizontal,
    ResizeVertical,
    Grab,
    Grabbing,
    NotAllowed,
    Crosshair,
}

// ── Hit region ──────────────────────────────────────────────────────────────

/// A rectangular interactive region with z-ordering.
#[derive(Debug, Clone)]
pub struct HitRegion {
    pub bounds: Rect,
    pub z_index: i32,
    pub target: HitTarget,
    pub cursor: MouseCursor,
}

// ── Hit test service ────────────────────────────────────────────────────────

/// Maintains a collection of hit regions and performs point-in-rect queries,
/// returning the topmost (highest z-index) region under the cursor.
pub struct HitTestService {
    pub regions: Vec<HitRegion>,
}

impl HitTestService {
    pub fn new() -> Self {
        Self { regions: Vec::new() }
    }

    /// Performs a hit test at screen coordinates `(x, y)`.
    ///
    /// Returns a reference to the [`HitTarget`] of the topmost region
    /// that contains the point. If no region matches, returns
    /// [`HitTarget::Nothing`].
    pub fn hit_test(&self, x: f32, y: f32) -> &HitTarget {
        self.regions
            .iter()
            .filter(|r| r.bounds.contains_point(x, y))
            .max_by_key(|r| r.z_index)
            .map(|r| &r.target)
            .unwrap_or(&HIT_TARGET_NOTHING)
    }

    /// Returns the cursor to display at `(x, y)`.
    pub fn cursor_at(&self, x: f32, y: f32) -> MouseCursor {
        self.regions
            .iter()
            .filter(|r| r.bounds.contains_point(x, y))
            .max_by_key(|r| r.z_index)
            .map(|r| r.cursor)
            .unwrap_or(MouseCursor::Default)
    }

    /// Registers an interactive region.
    pub fn register_region(&mut self, region: HitRegion) {
        self.regions.push(region);
    }

    /// Convenience: register a region with builder-style parameters.
    pub fn register(
        &mut self,
        bounds: Rect,
        z_index: i32,
        target: HitTarget,
        cursor: MouseCursor,
    ) {
        self.regions.push(HitRegion { bounds, z_index, target, cursor });
    }

    /// Removes all registered regions. Call at the start of each frame.
    pub fn clear(&mut self) {
        self.regions.clear();
    }

    /// Returns the number of registered regions.
    pub fn region_count(&self) -> usize {
        self.regions.len()
    }

    /// Returns all regions whose bounds contain the given point, sorted by
    /// z-index descending (topmost first).
    pub fn all_at(&self, x: f32, y: f32) -> Vec<&HitRegion> {
        let mut hits: Vec<&HitRegion> = self
            .regions
            .iter()
            .filter(|r| r.bounds.contains_point(x, y))
            .collect();
        hits.sort_by(|a, b| b.z_index.cmp(&a.z_index));
        hits
    }
}

impl Default for HitTestService {
    fn default() -> Self {
        Self::new()
    }
}

static HIT_TARGET_NOTHING: HitTarget = HitTarget::Nothing;
