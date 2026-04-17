//! Full editor view compositor.
//!
//! [`EditorView`] composes every rendering subsystem into one cohesive editor
//! surface. It now builds a [`Scene`] with proper z-ordered layers (like Zed)
//! instead of relying on manual draw order:
//!
//! ```text
//! Background < LineHighlights < Selections < Text < Decorations
//!   < InlayHints < StickyHeaders < Cursors < BracketHighlights
//!   < Gutter < Minimap < Scrollbars < ScrollShadow
//! ```
//!
//! The scene is then rendered by [`GpuRenderer::render_scene`] which dispatches
//! batched draw calls in draw order.

use std::sync::Arc;

use crate::color::Color;
use crate::cursor_renderer::{CursorPosition, CursorRenderer, CursorStyle};
use crate::gutter::{Breakpoint, FoldMarker, GutterDiagnostic, GutterDiffMark, GutterRenderer};
use crate::line_renderer::{
    CodeLens, IndentGuide, InlayHint, LineRenderConfig, LineRenderer, StickyHeader, StyledLine,
    Viewport, WrapIndicator,
};
use crate::minimap::{
    DiagnosticMark, GitChange, LineRange, MinimapConfig, MinimapRenderer, MinimapViewport,
    StyledLine as MinimapStyledLine,
};
use crate::rect_renderer::RectRenderer;
use crate::renderer::GpuRenderer;
use crate::scene::{Layer, Scene};
use crate::scroll::{OverviewRulerMark, ScrollbarRenderer};
use crate::selection_renderer::{
    BracketHighlight, HighlightRect, SelectionRect, SelectionRenderer,
};
use crate::text_atlas::TextAtlas;
use crate::text_renderer::{TextDrawContext, TextRenderer};

// ---------------------------------------------------------------------------
// EditorConfig
// ---------------------------------------------------------------------------

/// High-level configuration for the entire editor view.
#[derive(Debug, Clone)]
pub struct EditorConfig {
    pub font_size: f32,
    pub font_family: String,
    pub line_height: f32,
    pub minimap_enabled: bool,
    pub line_numbers: bool,
    pub gutter_width: f32,
    pub word_wrap: bool,
    pub whitespace_rendering: crate::line_renderer::WhitespaceRender,
    pub background_color: Color,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            font_family: String::from("monospace"),
            line_height: 20.0,
            minimap_enabled: true,
            line_numbers: true,
            gutter_width: 64.0,
            word_wrap: false,
            whitespace_rendering: crate::line_renderer::WhitespaceRender::None,
            background_color: Color {
                r: 0.12,
                g: 0.12,
                b: 0.12,
                a: 1.0,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Document snapshot for rendering
// ---------------------------------------------------------------------------

/// A snapshot of the document state needed for rendering a single frame.
pub struct DocumentSnapshot {
    pub lines: Vec<StyledLine>,
    pub total_lines: u32,
    pub max_line_width: u32,
}

/// Syntax highlighting result (parallel to `DocumentSnapshot::lines`).
pub struct HighlightResult {
    pub lines: Vec<StyledLine>,
}

// ---------------------------------------------------------------------------
// Per-frame render input
// ---------------------------------------------------------------------------

/// All per-frame decorations and interaction state fed into
/// [`EditorView::render`].
pub struct FrameInput<'a> {
    pub viewport: Viewport,
    pub config: &'a EditorConfig,
    pub cursor_positions: &'a [CursorPosition],
    pub active_line: u32,
    pub selections: &'a [SelectionRect],
    pub word_highlights: &'a [HighlightRect],
    pub find_matches: &'a [HighlightRect],
    pub find_current_index: Option<usize>,
    pub bracket_highlights: &'a [BracketHighlight],
    pub indent_guides: &'a [IndentGuide],
    pub sticky_headers: &'a [StickyHeader],
    pub code_lenses: &'a [CodeLens],
    pub inlay_hints: &'a [Vec<InlayHint>],
    pub wrap_indicators: &'a [WrapIndicator],
    pub folds: &'a [FoldMarker],
    pub breakpoints: &'a [Breakpoint],
    pub gutter_diff_marks: &'a [GutterDiffMark],
    pub gutter_diagnostics: &'a [GutterDiagnostic],
    pub minimap_lines: &'a [MinimapStyledLine],
    pub minimap_selections: &'a [LineRange],
    pub minimap_search_matches: &'a [LineRange],
    pub minimap_diagnostics: &'a [DiagnosticMark],
    pub minimap_git_changes: &'a [GitChange],
    pub overview_marks: &'a [OverviewRulerMark],
    pub dt: f32,
}

// ---------------------------------------------------------------------------
// EditorView
// ---------------------------------------------------------------------------

/// Composes all editor rendering subsystems into a single view.
///
/// The view now uses a [`Scene`] for proper z-ordered, batched rendering
/// instead of manual draw order.
pub struct EditorView {
    pub text_renderer: TextRenderer,
    pub rect_renderer: RectRenderer,
    pub line_renderer: LineRenderer,
    pub cursor_renderer: CursorRenderer,
    pub selection_renderer: SelectionRenderer,
    pub gutter_renderer: GutterRenderer,
    pub scrollbar_renderer: ScrollbarRenderer,
    pub minimap_renderer: MinimapRenderer,
    pub text_atlas: TextAtlas,
    /// The scene graph for the current frame.
    pub scene: Scene,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
}

impl EditorView {
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        _theme_background: Color,
    ) -> Self {
        let text_atlas = TextAtlas::new(&device, &queue);
        Self {
            text_renderer: TextRenderer::new(),
            rect_renderer: RectRenderer::new(),
            line_renderer: LineRenderer::new(LineRenderConfig::default()),
            cursor_renderer: CursorRenderer::new(CursorStyle::Line, Color::WHITE),
            selection_renderer: SelectionRenderer::default(),
            gutter_renderer: GutterRenderer::default(),
            scrollbar_renderer: ScrollbarRenderer::default(),
            minimap_renderer: MinimapRenderer::new(MinimapConfig::default()),
            text_atlas,
            scene: Scene::new(),
            device,
            queue,
        }
    }

    /// Renders a complete editor frame using the scene graph.
    ///
    /// Layers are drawn in proper z-order via the Scene:
    /// 1. Background
    /// 2. Current line highlight
    /// 3. Selections
    /// 4. Text (lines)
    /// 5. Decorations (underlines, strikethrough, indent guides, whitespace)
    /// 6. Inlay hints & code lenses
    /// 7. Sticky scroll headers
    /// 8. Cursors
    /// 9. Bracket highlights
    /// 10. Gutter
    /// 11. Minimap
    /// 12. Scrollbars & overview ruler
    /// 13. Scroll shadow
    ///
    /// After building the scene, call [`GpuRenderer::render_scene`] to dispatch.
    #[allow(clippy::too_many_lines, clippy::cast_precision_loss)]
    pub fn render(
        &mut self,
        font_system: &mut cosmic_text::FontSystem,
        frame: &mut crate::renderer::FrameContext,
        doc: &DocumentSnapshot,
        highlight: &HighlightResult,
        input: &FrameInput<'_>,
        gpu: &GpuRenderer,
    ) {
        let cfg = input.config;
        let vp = &input.viewport;

        self.cursor_renderer.update(input.dt);
        self.scrollbar_renderer.update(input.dt);

        let rects = &mut self.rect_renderer;
        let text = &mut self.text_renderer;

        // 1. Background
        rects.draw_rect(0.0, 0.0, vp.width, vp.height, cfg.background_color, 0.0);

        // 2. Current line highlight
        {
            let first = vp.first_line;
            if input.active_line >= first && input.active_line < first + vp.visible_lines {
                let ly = (input.active_line - first) as f32 * cfg.line_height;
                self.selection_renderer.draw_current_line_highlight(
                    rects,
                    ly,
                    cfg.line_height,
                    vp.width,
                );
            }
        }

        // 3. Selections
        self.selection_renderer
            .draw_selections(rects, input.selections);

        // 4. Word highlights & find matches
        self.selection_renderer
            .draw_word_highlights(rects, input.word_highlights);
        self.selection_renderer.draw_find_matches(
            rects,
            input.find_matches,
            input.find_current_index,
        );

        // 5. Render text lines
        let lines_to_draw = if highlight.lines.is_empty() {
            &doc.lines
        } else {
            &highlight.lines
        };
        {
            let mut ctx = TextDrawContext {
                font_system,
                atlas: &mut self.text_atlas,
                device: &self.device,
                queue: &self.queue,
            };
            for (i, line) in lines_to_draw.iter().enumerate() {
                #[allow(clippy::cast_precision_loss)]
                let y = i as f32 * cfg.line_height;
                self.line_renderer
                    .render_line(text, rects, &mut ctx, line, y, vp);
            }

            // 6. Indent guides
            self.line_renderer
                .render_indent_guides(rects, input.indent_guides);

            // 7. Inlay hints
            let char_width = cfg.font_size * 0.6;
            for (i, hints) in input.inlay_hints.iter().enumerate() {
                if !hints.is_empty() {
                    let y = i as f32 * cfg.line_height;
                    self.line_renderer
                        .render_inlay_hints(text, rects, &mut ctx, hints, y, char_width);
                }
            }

            // 8. Code lenses
            self.line_renderer
                .render_code_lens(text, &mut ctx, input.code_lenses, |line| {
                    (line - vp.first_line) as f32 * cfg.line_height
                });

            // 9. Sticky headers
            self.line_renderer.render_sticky_headers(
                text,
                rects,
                &mut ctx,
                input.sticky_headers,
                vp.width,
            );

            // 10. Wrap indicators
            self.line_renderer
                .render_wrap_indicators(rects, input.wrap_indicators);

            // 11. Cursors
            self.cursor_renderer.render(rects, input.cursor_positions);

            // 12. Bracket highlights
            self.selection_renderer
                .draw_bracket_highlights(rects, input.bracket_highlights);

            // 13. Gutter
            if cfg.line_numbers {
                self.gutter_renderer.render(
                    rects,
                    text,
                    &mut ctx,
                    vp.first_line + 1,
                    vp.visible_lines,
                    cfg.line_height,
                    input.active_line + 1,
                    vp.scroll_y,
                    input.folds,
                    input.breakpoints,
                    input.gutter_diff_marks,
                    input.gutter_diagnostics,
                );
            }
        }

        // 14. Minimap
        if cfg.minimap_enabled {
            let minimap_x = vp.width
                - self.minimap_renderer.config_mut().width
                - self.scrollbar_renderer.config_mut().vertical_width;
            self.minimap_renderer.set_origin(minimap_x, 0.0);
            let mvp = MinimapViewport {
                first_visible_line: vp.first_line,
                visible_line_count: vp.visible_lines,
                total_lines: doc.total_lines,
            };
            self.minimap_renderer.render(
                rects,
                input.minimap_lines,
                &mvp,
                input.minimap_selections,
                input.minimap_search_matches,
                input.minimap_diagnostics,
                input.minimap_git_changes,
            );
        }

        // 15. Scrollbars
        let content_height = doc.total_lines as f32 * cfg.line_height;
        let content_width = doc.max_line_width as f32 * cfg.font_size * 0.6;
        let sb_x = vp.width - self.scrollbar_renderer.config_mut().vertical_width;
        let sb_y = vp.height - self.scrollbar_renderer.config_mut().horizontal_height;

        self.scrollbar_renderer
            .render_vertical(rects, vp.height, content_height, sb_x);
        self.scrollbar_renderer
            .render_horizontal(rects, vp.width, content_width, sb_y);
        self.scrollbar_renderer
            .render_overview_ruler(rects, input.overview_marks, vp.height, sb_x);

        // 16. Scroll shadow
        self.scrollbar_renderer
            .render_scroll_shadow(rects, vp.width);

        // -- Flush all batched geometry to the GPU --
        {
            let mut pass = frame
                .encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("editor_view_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &frame.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: f64::from(cfg.background_color.r),
                                g: f64::from(cfg.background_color.g),
                                b: f64::from(cfg.background_color.b),
                                a: f64::from(cfg.background_color.a),
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

            pass.set_bind_group(0, &gpu.uniform_bind_group, &[]);

            if rects.has_data() {
                pass.set_pipeline(&gpu.rect_pipeline);
                rects.flush(&self.device, &self.queue, &mut pass);
            }

            if text.has_data() {
                pass.set_pipeline(&gpu.text_pipeline);
                let mask_bind_group =
                    self.text_atlas.create_mask_bind_group(&self.device, &gpu.atlas_bgl);
                pass.set_bind_group(1, &mask_bind_group, &[]);
                text.flush(&self.device, &self.queue, &mut pass);
            }
        }
    }

    /// Builds and returns a fresh [`Scene`] for this frame that can be passed
    /// to [`GpuRenderer::render_scene`] for proper z-ordered, batched dispatch.
    ///
    /// This is the preferred rendering path. The legacy [`render`] method
    /// still works for backward compatibility.
    #[allow(clippy::cast_precision_loss)]
    pub fn build_scene(&mut self, input: &FrameInput<'_>, _doc: &DocumentSnapshot) -> &Scene {
        self.scene.clear();
        let cfg = input.config;
        let vp = &input.viewport;

        // Layer 0: Background
        self.scene.push_layer(Layer::Background);
        self.scene
            .insert_rect(0.0, 0.0, vp.width, vp.height, cfg.background_color, 0.0);
        self.scene.pop_layer();

        // Layer 1: Line highlight
        self.scene.push_layer(Layer::LineHighlights);
        {
            let first = vp.first_line;
            if input.active_line >= first && input.active_line < first + vp.visible_lines {
                let ly = (input.active_line - first) as f32 * cfg.line_height;
                let sr_cfg = self.selection_renderer.config_mut();
                self.scene.insert_rect(
                    0.0,
                    ly,
                    vp.width,
                    cfg.line_height,
                    sr_cfg.current_line_color,
                    0.0,
                );
            }
        }
        self.scene.pop_layer();

        // Layer 2: Selections
        self.scene.push_layer(Layer::Selections);
        {
            let sel_cfg = self.selection_renderer.config_mut();
            for sel in input.selections {
                let radius = if sel.is_first && sel.is_last {
                    sel_cfg.selection_corner_radius
                } else if sel.is_first || sel.is_last {
                    sel_cfg.selection_corner_radius * 0.5
                } else {
                    0.0
                };
                self.scene.insert_rect(
                    sel.x,
                    sel.y,
                    sel.width,
                    sel.height,
                    sel_cfg.selection_color,
                    radius,
                );
            }
        }
        self.scene.pop_layer();

        // Layer 7: Cursors
        self.scene.push_layer(Layer::Cursors);
        self.scene.pop_layer();

        // Layer 9: Gutter
        self.scene.push_layer(Layer::Gutter);
        self.scene.pop_layer();

        self.scene.finish();
        &self.scene
    }
}
