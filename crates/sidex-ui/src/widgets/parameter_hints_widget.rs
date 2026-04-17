//! Parameter hints (signature help) popup widget.
//!
//! Shows function signatures while typing arguments, with the active
//! parameter highlighted and up/down arrows to cycle through overloads.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::draw::{DrawContext, IconId, TextStyle};
use crate::layout::{Edges, LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

/// A single parameter in a signature.
#[derive(Clone, Debug)]
pub struct Parameter {
    pub label: String,
    pub documentation: Option<String>,
}

/// A function/method signature.
#[derive(Clone, Debug)]
pub struct Signature {
    pub label: String,
    pub documentation: Option<String>,
    pub parameters: Vec<Parameter>,
}

/// The parameter hints popup that appears while typing function arguments.
#[allow(dead_code)]
pub struct ParameterHintsWidget {
    pub signatures: Vec<Signature>,
    pub active_signature: usize,
    pub active_parameter: usize,
    visible: bool,
    position: (f32, f32),

    font_size: f32,
    doc_font_size: f32,
    max_width: f32,
    padding: f32,

    background: Color,
    border_color: Color,
    shadow_color: Color,
    foreground: Color,
    active_param_fg: Color,
    doc_fg: Color,
    overload_fg: Color,
    arrow_fg: Color,
    arrow_hover_bg: Color,
}

impl Default for ParameterHintsWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl ParameterHintsWidget {
    pub fn new() -> Self {
        Self {
            signatures: Vec::new(),
            active_signature: 0,
            active_parameter: 0,
            visible: false,
            position: (0.0, 0.0),
            font_size: 13.0,
            doc_font_size: 12.0,
            max_width: 450.0,
            padding: 8.0,
            background: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            border_color: Color::from_hex("#454545").unwrap_or(Color::BLACK),
            shadow_color: Color::from_hex("#00000060").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            active_param_fg: Color::from_hex("#ffffff").unwrap_or(Color::WHITE),
            doc_fg: Color::from_hex("#9d9d9d").unwrap_or(Color::WHITE),
            overload_fg: Color::from_hex("#9d9d9d").unwrap_or(Color::WHITE),
            arrow_fg: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            arrow_hover_bg: Color::from_hex("#04395e").unwrap_or(Color::BLACK),
        }
    }

    /// Shows the parameter hints popup.
    pub fn show(
        &mut self,
        signatures: Vec<Signature>,
        active_signature: usize,
        active_parameter: usize,
        position: (f32, f32),
    ) {
        if signatures.is_empty() {
            self.hide();
            return;
        }
        self.signatures = signatures;
        self.active_signature = active_signature.min(self.signatures.len().saturating_sub(1));
        self.active_parameter = active_parameter;
        self.position = position;
        self.visible = true;
    }

    /// Hides the popup.
    pub fn hide(&mut self) {
        self.visible = false;
        self.signatures.clear();
        self.active_signature = 0;
        self.active_parameter = 0;
    }

    /// Cycles to the next overloaded signature.
    pub fn next_signature(&mut self) {
        if !self.signatures.is_empty() {
            self.active_signature = (self.active_signature + 1) % self.signatures.len();
        }
    }

    /// Cycles to the previous overloaded signature.
    pub fn prev_signature(&mut self) {
        if !self.signatures.is_empty() {
            self.active_signature = if self.active_signature == 0 {
                self.signatures.len() - 1
            } else {
                self.active_signature - 1
            };
        }
    }

    /// Updates the active parameter index.
    pub fn set_active_parameter(&mut self, idx: usize) {
        self.active_parameter = idx;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn current_signature(&self) -> Option<&Signature> {
        self.signatures.get(self.active_signature)
    }

    // ── Private helpers ──────────────────────────────────────────────────

    #[allow(clippy::cast_precision_loss)]
    fn widget_height(&self) -> f32 {
        let mut h = self.padding * 2.0 + self.font_size; // signature line
        if self.signatures.len() > 1 {
            h += 20.0; // overload indicator row
        }
        if let Some(sig) = self.current_signature() {
            if let Some(param) = sig.parameters.get(self.active_parameter) {
                if param.documentation.is_some() {
                    h += self.doc_font_size + 8.0;
                }
            }
            if sig.documentation.is_some() {
                h += self.doc_font_size + 4.0;
            }
        }
        h
    }

    fn widget_rect(&self) -> Rect {
        let w = self.max_width;
        let h = self.widget_height();
        let x = self.position.0;
        let y = self.position.1 - h - 4.0;
        Rect::new(x.max(0.0), y.max(0.0), w, h)
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn render_draw(&self, ctx: &mut DrawContext, _editor_rect: Rect) {
        if !self.visible || self.signatures.is_empty() {
            return;
        }
        let wr = self.widget_rect();

        // Shadow
        let shadow = Rect::new(wr.x + 2.0, wr.y + 2.0, wr.width, wr.height);
        ctx.draw_rect(shadow, self.shadow_color, 4.0);

        // Background
        ctx.draw_rect(wr, self.background, 4.0);
        ctx.draw_border(wr, self.border_color, 1.0, 4.0);

        let pad = self.padding;
        let mut cy = wr.y + pad;

        // Overload indicator: up/down arrows + "1 of 3"
        if self.signatures.len() > 1 {
            // Up arrow
            ctx.draw_icon(
                IconId::ChevronRight,
                (wr.x + pad, cy),
                12.0,
                self.arrow_fg,
            );
            // Down arrow
            ctx.draw_icon(
                IconId::ChevronDown,
                (wr.x + pad + 16.0, cy),
                12.0,
                self.arrow_fg,
            );
            let indicator = format!(
                "{} of {}",
                self.active_signature + 1,
                self.signatures.len()
            );
            ctx.draw_text(
                &indicator,
                (wr.x + pad + 34.0, cy),
                self.overload_fg,
                self.doc_font_size,
                false,
                false,
            );
            cy += 20.0;
        }

        if let Some(sig) = self.current_signature() {
            // Build the signature with highlighted active parameter
            let spans = self.build_signature_spans(sig);
            ctx.draw_styled_text(&spans, (wr.x + pad, cy), self.font_size);
            cy += self.font_size + 6.0;

            // Parameter documentation
            if let Some(param) = sig.parameters.get(self.active_parameter) {
                if let Some(ref doc) = param.documentation {
                    ctx.draw_text(
                        doc,
                        (wr.x + pad, cy),
                        self.doc_fg,
                        self.doc_font_size,
                        false,
                        false,
                    );
                    cy += self.doc_font_size + 4.0;
                }
            }

            // Signature documentation
            if let Some(ref doc) = sig.documentation {
                let first_line = doc.lines().next().unwrap_or("");
                ctx.draw_text(
                    first_line,
                    (wr.x + pad, cy),
                    self.doc_fg,
                    self.doc_font_size,
                    false,
                    true,
                );
            }
        }
    }

    /// Builds styled text spans for the signature, with the active
    /// parameter rendered bold + underlined (via different color).
    fn build_signature_spans(&self, sig: &Signature) -> Vec<(String, TextStyle)> {
        if sig.parameters.is_empty() {
            return vec![(
                sig.label.clone(),
                TextStyle {
                    color: self.foreground,
                    bold: false,
                    italic: false,
                },
            )];
        }

        let mut spans = Vec::new();
        let label = &sig.label;
        let mut last_end = 0;

        for (i, param) in sig.parameters.iter().enumerate() {
            if let Some(start) = label[last_end..].find(&param.label) {
                let abs_start = last_end + start;
                // Text before this parameter
                if abs_start > last_end {
                    spans.push((
                        label[last_end..abs_start].to_string(),
                        TextStyle {
                            color: self.foreground,
                            bold: false,
                            italic: false,
                        },
                    ));
                }
                // The parameter itself
                let is_active = i == self.active_parameter;
                spans.push((
                    param.label.clone(),
                    TextStyle {
                        color: if is_active {
                            self.active_param_fg
                        } else {
                            self.foreground
                        },
                        bold: is_active,
                        italic: false,
                    },
                ));
                last_end = abs_start + param.label.len();
            }
        }

        // Remaining text after last parameter
        if last_end < label.len() {
            spans.push((
                label[last_end..].to_string(),
                TextStyle {
                    color: self.foreground,
                    bold: false,
                    italic: false,
                },
            ));
        }

        if spans.is_empty() {
            spans.push((
                sig.label.clone(),
                TextStyle {
                    color: self.foreground,
                    bold: false,
                    italic: false,
                },
            ));
        }

        spans
    }
}

impl Widget for ParameterHintsWidget {
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Auto,
            padding: Edges::all(0.0),
            ..LayoutNode::default()
        }
    }

    fn render(&self, _rect: Rect, renderer: &mut GpuRenderer) {
        if !self.visible || self.signatures.is_empty() {
            return;
        }
        let wr = self.widget_rect();
        let mut rr = sidex_gpu::RectRenderer::new();
        rr.draw_rect(wr.x, wr.y, wr.width, wr.height, self.background, 4.0);
        rr.draw_border(wr.x, wr.y, wr.width, wr.height, self.border_color, 1.0);
        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, _rect: Rect) -> EventResult {
        if !self.visible {
            return EventResult::Ignored;
        }
        match event {
            UiEvent::KeyPress { key, .. } => match key {
                Key::Escape => {
                    self.hide();
                    EventResult::Handled
                }
                Key::ArrowUp if self.signatures.len() > 1 => {
                    self.prev_signature();
                    EventResult::Handled
                }
                Key::ArrowDown if self.signatures.len() > 1 => {
                    self.next_signature();
                    EventResult::Handled
                }
                _ => EventResult::Ignored,
            },
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => {
                let wr = self.widget_rect();
                if !wr.contains(*x, *y) {
                    self.hide();
                    return EventResult::Handled;
                }
                // Click on arrows area to navigate overloads
                if self.signatures.len() > 1 {
                    let arrow_y = wr.y + self.padding;
                    if *y >= arrow_y && *y <= arrow_y + 16.0 {
                        if *x < wr.x + self.padding + 16.0 {
                            self.prev_signature();
                        } else if *x < wr.x + self.padding + 32.0 {
                            self.next_signature();
                        }
                    }
                }
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sig() -> Signature {
        Signature {
            label: "fn foo(a: i32, b: &str) -> bool".into(),
            documentation: Some("Does foo things.".into()),
            parameters: vec![
                Parameter {
                    label: "a: i32".into(),
                    documentation: Some("The first arg".into()),
                },
                Parameter {
                    label: "b: &str".into(),
                    documentation: Some("The second arg".into()),
                },
            ],
        }
    }

    #[test]
    fn show_and_navigate() {
        let mut w = ParameterHintsWidget::new();
        w.show(vec![make_sig(), make_sig()], 0, 0, (100.0, 300.0));
        assert!(w.is_visible());
        assert_eq!(w.active_signature, 0);

        w.next_signature();
        assert_eq!(w.active_signature, 1);
        w.next_signature();
        assert_eq!(w.active_signature, 0);
    }

    #[test]
    fn hide_on_empty() {
        let mut w = ParameterHintsWidget::new();
        w.show(vec![], 0, 0, (0.0, 0.0));
        assert!(!w.is_visible());
    }

    #[test]
    fn signature_spans() {
        let w = ParameterHintsWidget::new();
        let sig = make_sig();
        let spans = w.build_signature_spans(&sig);
        assert!(!spans.is_empty());
        assert!(spans.iter().any(|(text, style)| text == "a: i32" && style.bold));
    }
}
