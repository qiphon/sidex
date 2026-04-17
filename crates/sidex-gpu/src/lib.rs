//! GPU-accelerated rendering for the `SideX` editor.
//!
//! This crate provides the rendering layer built on top of [`wgpu`]. It
//! contains:
//!
//! - [`Scene`] — scene graph with draw call batching and z-ordered layer
//!   dispatch, modeled after Zed's `gpui::scene`.
//! - [`GpuRenderer`] — core wgpu device, queue, surface management, and
//!   scene-based rendering with camera transform.
//! - [`TextLayoutSystem`] — text layout system using `cosmic-text` with
//!   line height, letter spacing, tab stops, word wrap, bidi, and caching.
//! - [`TextAtlas`] — dual-atlas glyph texture management (mask R8 + color
//!   Rgba8) with LRU eviction, subpixel positioning, and atlas compaction.
//! - [`TextRenderer`] — batched text drawing via instanced quads.
//! - [`RectRenderer`] — batched rectangle / shape drawing.
//! - [`Color`] — simple RGBA color type with conversions.
//! - [`CursorRenderer`] — blinking cursor with smooth animation.
//! - [`SelectionRenderer`] — selection backgrounds, highlights, bracket pairs.
//! - [`LineRenderer`] — full line rendering with decorations.
//! - [`GutterRenderer`] — line numbers, folds, breakpoints, git/diagnostic indicators.
//! - [`MinimapRenderer`] — scaled-down document overview.
//! - [`ScrollbarRenderer`] — scrollbars with overview ruler and smooth scrolling.
//! - [`EditorView`] — compositor that assembles all renderers into one frame.
//! - Vertex types and shader pipelines (text, subpixel text, rect, shadow, underline).

pub mod animation;
pub mod color;
pub mod cursor_renderer;
pub mod diagnostic_gutter;
pub mod editor_compositor;
pub mod editor_view;
pub mod font;
pub mod gutter;
pub mod gutter_renderer;
pub mod hit_testing;
pub mod line_renderer;
pub mod minimap;
pub mod pipeline;
pub mod rect_renderer;
pub mod renderer;
pub mod scene;
pub mod scroll;
pub mod selection_renderer;
pub mod squiggly;
pub mod text_atlas;
pub mod text_layout;
pub mod text_renderer;
pub mod texture_cache;
pub mod ui_renderer;
pub mod vertex;

pub use animation::{
    ActiveAnimation, AnimationState, Animator, EasingFunction,
    cursor_blink_animation, cursor_move_animation, fade_in_animation, fade_out_animation,
    smooth_scroll_animation,
};
pub use color::Color;
pub use cursor_renderer::{CursorAnimConfig, CursorPosition, CursorRenderer, CursorStyle};
pub use editor_view::{DocumentSnapshot, EditorConfig, EditorView, FrameInput, HighlightResult};
pub use editor_compositor::{
    BracketPair, ComposedFrame, CompositorConfig, CompositorDiagnostic,
    CompositorDiagnosticSeverity, CompositorDocumentSnapshot, CompositorLayer, CompositorTheme,
    CursorBlinking, DiagnosticRange, DirtyReason, DirtyRegion, EditorCompositor, FindMatch,
    FindState, FrameTiming, GradientDirection, LayerContent, LayerId, LineNumbers, MinimapSide,
    MinimapRenderData, OverlayWidget, PanelType, Rect, RenderWhitespace, ScrollState,
    ScrollbarOrientation, SidebarContent, TabRenderData, Transform2D, WordWrap,
};
pub use font::{
    FontConfig as FontMgrConfig, FontStyle as FontMgrStyle, FontWeight as FontMgrWeight,
};
pub use font::{
    DetailedTextMetrics, FontFallbackChain, FontFamily, FontId, FontManager, FontMetrics,
    TextMetrics, detect_ligature, LIGATURE_SEQUENCES,
};
pub use gutter::{
    Breakpoint, FoldMarker, FoldState, GutterConfig, GutterDiagnostic, GutterDiagnosticSeverity,
    GutterDiffKind, GutterDiffMark, GutterRenderer,
};
pub use gutter_renderer::{
    BreakpointKind, GitLineChange, GutterLineInput, GutterMargins, GutterPrimitive,
    GutterRenderer as GutterLineRenderer, GutterTheme, IconKind, LineFoldState,
    render_gutter_line,
};
pub use line_renderer::{
    CodeLens, IndentGuide, InlayHint, LineRenderConfig, LineRenderer, StickyHeader, StyledLine,
    StyledSpan, TextStyle, Viewport, WhitespaceRender, WrapIndicator,
};
pub use minimap::{
    DiagnosticMark, DiagnosticSeverity, GitChange, GitChangeKind, LineRange, MinimapClickResult,
    MinimapConfig, MinimapRenderer, MinimapViewport,
};
pub use pipeline::{
    create_line_pipeline, create_rect_pipeline, create_shadow_pipeline,
    create_subpixel_text_pipeline, create_text_pipeline, create_underline_pipeline,
    ViewportUniform,
};
pub use rect_renderer::RectRenderer;
pub use renderer::{
    FrameContext, GpuError, GpuRenderer, ImagePipeline, LineInstance, LinePipeline, RectInstance,
    RectPipeline, RenderBatch, RenderCommand, RenderFrame, RenderFrameBuilder, RenderLayer,
    TextInstance, TextPipeline, UnderlineStyle,
};
pub use scene::{
    ContentMask, Layer, MonochromeSprite, PolychromeSprite, Quad, Scene, Shadow, SubpixelSprite,
    Underline,
};
pub use scroll::{
    OverviewMarkKind, OverviewRulerMark, ScrollbarConfig, ScrollbarRenderer, SmoothScrollAxis,
};
pub use selection_renderer::{
    BracketHighlight, HighlightRect, SelectionRect, SelectionRenderConfig, SelectionRenderer,
};
pub use text_atlas::{
    AtlasKind, AtlasPage, ExtendedCacheKey, FontVariant, GlyphInfo, GlyphKey, LigatureEntry,
    LigatureKey, MultiPageAtlas, SubpixelBin, TextAtlas, SUBPIXEL_BINS_X, SUBPIXEL_BINS_Y,
};
pub use text_layout::{
    FontConfig, FontStyle, FontWeight, LineHeightMode, LineLayout, ShapedGlyph, ShapedRun,
    StyledTextRun, TextLayoutConfig, TextLayoutSystem, WrapBoundary, WrapMode,
};
pub use text_renderer::{TextDrawContext, TextRenderer};
pub use vertex::{LineVertex, RectVertex, TextVertex};

pub use diagnostic_gutter::{
    coalesce_gutter_icons, DiagnosticGutterRenderer, GutterDiagnosticIcon, GutterIconSeverity,
};
pub use squiggly::{SquigglyDecoration, SquigglyRenderer, SquigglySeverity};

pub use hit_testing::{
    GutterZone, HitRegion, HitTarget, HitTestService, MouseCursor, ResizeDirection,
    ScrollbarHitOrientation,
};
pub use texture_cache::{CachedTexture, ImageFormat, TextureCache, TextureId};
pub use ui_renderer::{
    ActionButton, ButtonState, ButtonStyle, CheckboxStyle, DropdownStyle, InputStyle,
    NotificationRenderData, NotificationSeverity, NotificationStyle, Orientation,
    PanelHeaderStyle, ProgressStyle, ScrollbarStyle, SeparatorStyle, TabStyle, TooltipStyle,
    TreeStyle, UiRenderer, UiScrollbarOrientation,
};
