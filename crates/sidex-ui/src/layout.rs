//! Flexbox-style layout engine.
//!
//! Provides a simple tree of [`LayoutNode`]s with row/column direction, flex
//! sizing, and min/max constraints.  [`compute_layout`] resolves the tree into
//! a flat list of [`Rect`]s in the same pre-order as the node tree.

use serde::{Deserialize, Serialize};

// ── Primitives ───────────────────────────────────────────────────────────────

/// Layout direction for a flex container.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// Children are laid out left-to-right.
    #[default]
    Row,
    /// Children are laid out top-to-bottom.
    Column,
}

/// How a node should be sized along the main axis.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Size {
    /// An absolute pixel size.
    Fixed(f32),
    /// A flex weight — share of remaining space proportional to total flex.
    Flex(f32),
    /// Sized to fit its content (falls back to 0 when no children).
    Auto,
}

impl Default for Size {
    fn default() -> Self {
        Self::Auto
    }
}

/// Edge insets (padding or margin) in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edges {
    /// Uniform insets on all four sides.
    pub const fn all(v: f32) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }

    /// Symmetric horizontal / vertical insets.
    pub const fn symmetric(horizontal: f32, vertical: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    /// Total horizontal inset (left + right).
    pub fn horizontal(self) -> f32 {
        self.left + self.right
    }

    /// Total vertical inset (top + bottom).
    pub fn vertical(self) -> f32 {
        self.top + self.bottom
    }
}

/// An axis-aligned rectangle in pixel coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };

    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Returns `true` if the point `(px, py)` lies within this rectangle.
    pub fn contains(self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }

    /// Shrinks the rectangle inward by the given edges.
    pub fn inset(self, edges: Edges) -> Self {
        Self {
            x: self.x + edges.left,
            y: self.y + edges.top,
            width: (self.width - edges.horizontal()).max(0.0),
            height: (self.height - edges.vertical()).max(0.0),
        }
    }

    /// The right edge `x + width`.
    pub fn right(self) -> f32 {
        self.x + self.width
    }

    /// The bottom edge `y + height`.
    pub fn bottom(self) -> f32 {
        self.y + self.height
    }
}

// ── Layout node ──────────────────────────────────────────────────────────────

/// A node in the layout tree.
#[derive(Clone, Debug, Default)]
pub struct LayoutNode {
    /// Flex direction for this container's children.
    pub direction: Direction,
    /// How this node is sized along its parent's main axis.
    pub size: Size,
    /// Optional minimum size along the parent's main axis.
    pub min_size: Option<f32>,
    /// Optional maximum size along the parent's main axis.
    pub max_size: Option<f32>,
    /// Inner padding.
    pub padding: Edges,
    /// Outer margin.
    pub margin: Edges,
    /// Child layout nodes.
    pub children: Vec<LayoutNode>,
}

impl LayoutNode {
    /// Creates a leaf node with `Fixed` size and no children.
    pub fn fixed(size: f32) -> Self {
        Self {
            size: Size::Fixed(size),
            ..Self::default()
        }
    }

    /// Creates a flex node with the given weight.
    pub fn flex(weight: f32) -> Self {
        Self {
            size: Size::Flex(weight),
            ..Self::default()
        }
    }

    /// Creates a column container wrapping the given children.
    pub fn column(children: Vec<LayoutNode>) -> Self {
        Self {
            direction: Direction::Column,
            size: Size::Flex(1.0),
            children,
            ..Self::default()
        }
    }

    /// Creates a row container wrapping the given children.
    pub fn row(children: Vec<LayoutNode>) -> Self {
        Self {
            direction: Direction::Row,
            size: Size::Flex(1.0),
            children,
            ..Self::default()
        }
    }
}

// ── Layout computation ───────────────────────────────────────────────────────

/// Computes the pixel [`Rect`] for every node in the tree (pre-order).
///
/// The returned `Vec` is parallel to a pre-order traversal of `root`: index 0
/// is `root` itself, then its children recursively.
pub fn compute_layout(root: &LayoutNode, available: Rect) -> Vec<Rect> {
    let mut rects = Vec::new();
    layout_node(root, available, &mut rects);
    rects
}

fn layout_node(node: &LayoutNode, available: Rect, out: &mut Vec<Rect>) {
    let outer = apply_margin(available, node.margin);
    let content = outer.inset(node.padding);

    out.push(outer);

    if node.children.is_empty() {
        return;
    }

    let is_row = node.direction == Direction::Row;

    let total_main = if is_row {
        content.width
    } else {
        content.height
    };

    // First pass: measure fixed / auto children, accumulate flex weights.
    let mut fixed_total: f32 = 0.0;
    let mut flex_total: f32 = 0.0;

    for child in &node.children {
        let child_margin_main = if is_row {
            child.margin.horizontal()
        } else {
            child.margin.vertical()
        };
        match child.size {
            Size::Fixed(v) => {
                fixed_total += clamp_size(v, child.min_size, child.max_size) + child_margin_main;
            }
            Size::Flex(w) => {
                flex_total += w;
                fixed_total += child_margin_main;
            }
            Size::Auto => {
                fixed_total += child_margin_main;
            }
        }
    }

    let remaining = (total_main - fixed_total).max(0.0);

    // Second pass: assign rects.
    let mut cursor = if is_row { content.x } else { content.y };

    for child in &node.children {
        let child_margin_main = if is_row {
            child.margin.horizontal()
        } else {
            child.margin.vertical()
        };

        let main_size = match child.size {
            Size::Fixed(v) => clamp_size(v, child.min_size, child.max_size),
            Size::Flex(w) => {
                let share = if flex_total > 0.0 {
                    remaining * (w / flex_total)
                } else {
                    0.0
                };
                clamp_size(share, child.min_size, child.max_size)
            }
            Size::Auto => 0.0,
        };

        let child_rect = if is_row {
            Rect::new(
                cursor,
                content.y,
                main_size + child_margin_main,
                content.height,
            )
        } else {
            Rect::new(
                content.x,
                cursor,
                content.width,
                main_size + child_margin_main,
            )
        };

        layout_node(child, child_rect, out);
        cursor += if is_row {
            child_rect.width
        } else {
            child_rect.height
        };
    }
}

fn apply_margin(available: Rect, margin: Edges) -> Rect {
    Rect {
        x: available.x + margin.left,
        y: available.y + margin.top,
        width: (available.width - margin.horizontal()).max(0.0),
        height: (available.height - margin.vertical()).max(0.0),
    }
}

fn clamp_size(value: f32, min: Option<f32>, max: Option<f32>) -> f32 {
    let v = if let Some(lo) = min {
        value.max(lo)
    } else {
        value
    };
    if let Some(hi) = max {
        v.min(hi)
    } else {
        v
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn single_node_fills_available() {
        let node = LayoutNode::flex(1.0);
        let rects = compute_layout(&node, Rect::new(0.0, 0.0, 800.0, 600.0));
        assert_eq!(rects.len(), 1);
        assert!(approx_eq(rects[0].width, 800.0));
        assert!(approx_eq(rects[0].height, 600.0));
    }

    #[test]
    fn two_fixed_row_children() {
        let root = LayoutNode {
            direction: Direction::Row,
            size: Size::Flex(1.0),
            children: vec![LayoutNode::fixed(200.0), LayoutNode::fixed(300.0)],
            ..LayoutNode::default()
        };
        let rects = compute_layout(&root, Rect::new(0.0, 0.0, 800.0, 600.0));
        assert_eq!(rects.len(), 3);
        assert!(approx_eq(rects[1].width, 200.0));
        assert!(approx_eq(rects[2].x, 200.0));
        assert!(approx_eq(rects[2].width, 300.0));
    }

    #[test]
    fn flex_children_share_space() {
        let root = LayoutNode {
            direction: Direction::Row,
            size: Size::Flex(1.0),
            children: vec![LayoutNode::flex(1.0), LayoutNode::flex(2.0)],
            ..LayoutNode::default()
        };
        let rects = compute_layout(&root, Rect::new(0.0, 0.0, 900.0, 600.0));
        assert_eq!(rects.len(), 3);
        assert!(approx_eq(rects[1].width, 300.0));
        assert!(approx_eq(rects[2].width, 600.0));
    }

    #[test]
    fn column_direction() {
        let root = LayoutNode {
            direction: Direction::Column,
            size: Size::Flex(1.0),
            children: vec![LayoutNode::fixed(100.0), LayoutNode::flex(1.0)],
            ..LayoutNode::default()
        };
        let rects = compute_layout(&root, Rect::new(0.0, 0.0, 800.0, 600.0));
        assert_eq!(rects.len(), 3);
        assert!(approx_eq(rects[1].height, 100.0));
        assert!(approx_eq(rects[2].y, 100.0));
        assert!(approx_eq(rects[2].height, 500.0));
    }

    #[test]
    fn padding_shrinks_content() {
        let root = LayoutNode {
            direction: Direction::Row,
            size: Size::Flex(1.0),
            padding: Edges::all(10.0),
            children: vec![LayoutNode::flex(1.0)],
            ..LayoutNode::default()
        };
        let rects = compute_layout(&root, Rect::new(0.0, 0.0, 200.0, 100.0));
        assert!(approx_eq(rects[1].x, 10.0));
        assert!(approx_eq(rects[1].y, 10.0));
        assert!(approx_eq(rects[1].width, 180.0));
        assert!(approx_eq(rects[1].height, 80.0));
    }

    #[test]
    fn min_max_clamp() {
        let root = LayoutNode {
            direction: Direction::Row,
            size: Size::Flex(1.0),
            children: vec![
                LayoutNode {
                    size: Size::Flex(1.0),
                    min_size: Some(400.0),
                    ..LayoutNode::default()
                },
                LayoutNode::flex(1.0),
            ],
            ..LayoutNode::default()
        };
        let rects = compute_layout(&root, Rect::new(0.0, 0.0, 600.0, 400.0));
        assert!(rects[1].width >= 400.0);
    }

    #[test]
    fn nested_layout() {
        let root = LayoutNode {
            direction: Direction::Column,
            size: Size::Flex(1.0),
            children: vec![
                LayoutNode::fixed(50.0),
                LayoutNode {
                    direction: Direction::Row,
                    size: Size::Flex(1.0),
                    children: vec![LayoutNode::fixed(200.0), LayoutNode::flex(1.0)],
                    ..LayoutNode::default()
                },
                LayoutNode::fixed(30.0),
            ],
            ..LayoutNode::default()
        };
        let rects = compute_layout(&root, Rect::new(0.0, 0.0, 1000.0, 800.0));
        // root + 3 direct children + 2 grandchildren = 6
        assert_eq!(rects.len(), 6);
        // Title bar (50px)
        assert!(approx_eq(rects[1].height, 50.0));
        // Middle area takes remaining
        assert!(approx_eq(rects[2].y, 50.0));
        assert!(approx_eq(rects[2].height, 720.0));
        // Status bar at bottom
        assert!(approx_eq(rects[5].height, 30.0));
    }

    #[test]
    fn rect_contains() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert!(r.contains(10.0, 20.0));
        assert!(r.contains(50.0, 40.0));
        assert!(!r.contains(9.0, 20.0));
        assert!(!r.contains(110.0, 20.0));
    }

    #[test]
    fn rect_inset() {
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let inset = r.inset(Edges::all(10.0));
        assert!(approx_eq(inset.x, 10.0));
        assert!(approx_eq(inset.y, 10.0));
        assert!(approx_eq(inset.width, 80.0));
        assert!(approx_eq(inset.height, 80.0));
    }

    #[test]
    fn edges_helpers() {
        let e = Edges::symmetric(5.0, 10.0);
        assert!(approx_eq(e.horizontal(), 10.0));
        assert!(approx_eq(e.vertical(), 20.0));
    }
}
