//! Hover information engine wrapping LSP `textDocument/hover`.
//!
//! Provides a simplified API over the raw LSP hover response, converting
//! markup content into editor-friendly types. Includes markdown parsing
//! helpers for rendering hover content with syntax-highlighted code blocks.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::client::LspClient;
use crate::conversion::{lsp_to_range, position_to_lsp};

// ── MarkupContent ───────────────────────────────────────────────────────────

/// Markup content for hover display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkupContent {
    /// Plain text content.
    Plaintext(String),
    /// Markdown-formatted content.
    Markdown(String),
}

// ── HoverContent ────────────────────────────────────────────────────────────

/// A structured hover content block for rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoverContent {
    /// Plain text.
    PlainText(String),
    /// Markdown string.
    Markdown(String),
    /// A syntax-highlighted code block.
    Code { language: String, value: String },
}

// ── RenderedHoverBlock ──────────────────────────────────────────────────────

/// A pre-parsed block from hover markdown, ready for rendering.
#[derive(Debug, Clone)]
pub enum RenderedHoverBlock {
    /// A paragraph of plain/styled text.
    Paragraph(String),
    /// A fenced code block with language.
    CodeBlock { language: String, code: String },
    /// A separator (horizontal rule).
    Separator,
}

/// Parses hover markdown into renderable blocks.
pub fn render_hover_markdown(content: &str) -> Vec<RenderedHoverBlock> {
    let mut blocks = Vec::new();
    let mut lines = content.lines().peekable();
    let mut current_para = String::new();

    while let Some(line) = lines.next() {
        if line.starts_with("```") {
            if !current_para.is_empty() {
                blocks.push(RenderedHoverBlock::Paragraph(
                    current_para.trim().to_owned(),
                ));
                current_para.clear();
            }

            let language = line.trim_start_matches('`').trim().to_owned();
            let mut code = String::new();
            for code_line in lines.by_ref() {
                if code_line.starts_with("```") {
                    break;
                }
                if !code.is_empty() {
                    code.push('\n');
                }
                code.push_str(code_line);
            }
            blocks.push(RenderedHoverBlock::CodeBlock { language, code });
        } else if line.trim() == "---" || line.trim() == "***" {
            if !current_para.is_empty() {
                blocks.push(RenderedHoverBlock::Paragraph(
                    current_para.trim().to_owned(),
                ));
                current_para.clear();
            }
            blocks.push(RenderedHoverBlock::Separator);
        } else if line.trim().is_empty() {
            if !current_para.is_empty() {
                blocks.push(RenderedHoverBlock::Paragraph(
                    current_para.trim().to_owned(),
                ));
                current_para.clear();
            }
        } else {
            if !current_para.is_empty() {
                current_para.push(' ');
            }
            current_para.push_str(line);
        }
    }
    if !current_para.is_empty() {
        blocks.push(RenderedHoverBlock::Paragraph(
            current_para.trim().to_owned(),
        ));
    }

    blocks
}

// ── HoverInfo ───────────────────────────────────────────────────────────────

/// Hover information returned to the editor.
#[derive(Debug, Clone)]
pub struct HoverInfo {
    /// One or more content blocks to display.
    pub contents: Vec<MarkupContent>,
    /// Structured content blocks for richer rendering.
    pub structured_contents: Vec<HoverContent>,
    /// Optional range of the symbol that was hovered.
    pub range: Option<sidex_text::Range>,
}

impl HoverInfo {
    /// Renders all content blocks to `RenderedHoverBlock`s.
    pub fn render(&self) -> Vec<RenderedHoverBlock> {
        let mut blocks = Vec::new();
        for content in &self.structured_contents {
            match content {
                HoverContent::PlainText(text) => {
                    blocks.push(RenderedHoverBlock::Paragraph(text.clone()));
                }
                HoverContent::Markdown(md) => {
                    blocks.extend(render_hover_markdown(md));
                }
                HoverContent::Code { language, value } => {
                    blocks.push(RenderedHoverBlock::CodeBlock {
                        language: language.clone(),
                        code: value.clone(),
                    });
                }
            }
        }
        blocks
    }

    /// Returns `true` if this hover has no content.
    pub fn is_empty(&self) -> bool {
        self.contents.is_empty() && self.structured_contents.is_empty()
    }
}

// ── LSP request ─────────────────────────────────────────────────────────────

/// Requests hover information from the language server.
pub async fn request_hover(
    client: &LspClient,
    uri: &str,
    pos: sidex_text::Position,
) -> Result<Option<HoverInfo>> {
    let lsp_pos = position_to_lsp(pos);
    let response = client.hover(uri, lsp_pos).await?;

    let Some(hover) = response else {
        return Ok(None);
    };

    let (contents, structured) = convert_hover_contents(hover.contents);
    let range = hover.range.map(lsp_to_range);

    Ok(Some(HoverInfo {
        contents,
        structured_contents: structured,
        range,
    }))
}

fn convert_hover_contents(
    contents: lsp_types::HoverContents,
) -> (Vec<MarkupContent>, Vec<HoverContent>) {
    let mut markup = Vec::new();
    let mut structured = Vec::new();

    match contents {
        lsp_types::HoverContents::Scalar(value) => {
            let (m, s) = convert_marked_string(value);
            markup.push(m);
            structured.push(s);
        }
        lsp_types::HoverContents::Array(values) => {
            for value in values {
                let (m, s) = convert_marked_string(value);
                markup.push(m);
                structured.push(s);
            }
        }
        lsp_types::HoverContents::Markup(mc) => {
            let (m, s) = convert_markup_content(mc);
            markup.push(m);
            structured.push(s);
        }
    }

    (markup, structured)
}

fn convert_marked_string(ms: lsp_types::MarkedString) -> (MarkupContent, HoverContent) {
    match ms {
        lsp_types::MarkedString::String(s) => (
            MarkupContent::Plaintext(s.clone()),
            HoverContent::PlainText(s),
        ),
        lsp_types::MarkedString::LanguageString(ls) => {
            let md = format!("```{}\n{}\n```", ls.language, ls.value);
            (
                MarkupContent::Markdown(md),
                HoverContent::Code {
                    language: ls.language,
                    value: ls.value,
                },
            )
        }
    }
}

fn convert_markup_content(mc: lsp_types::MarkupContent) -> (MarkupContent, HoverContent) {
    match mc.kind {
        lsp_types::MarkupKind::PlainText => (
            MarkupContent::Plaintext(mc.value.clone()),
            HoverContent::PlainText(mc.value),
        ),
        lsp_types::MarkupKind::Markdown => (
            MarkupContent::Markdown(mc.value.clone()),
            HoverContent::Markdown(mc.value),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_scalar_string() {
        let contents =
            lsp_types::HoverContents::Scalar(lsp_types::MarkedString::String("hello".into()));
        let (markup, structured) = convert_hover_contents(contents);
        assert_eq!(markup.len(), 1);
        assert_eq!(markup[0], MarkupContent::Plaintext("hello".into()));
        assert!(matches!(&structured[0], HoverContent::PlainText(s) if s == "hello"));
    }

    #[test]
    fn convert_scalar_language_string() {
        let contents = lsp_types::HoverContents::Scalar(lsp_types::MarkedString::LanguageString(
            lsp_types::LanguageString {
                language: "rust".into(),
                value: "fn main()".into(),
            },
        ));
        let (markup, structured) = convert_hover_contents(contents);
        assert!(matches!(&markup[0], MarkupContent::Markdown(s) if s.contains("rust")));
        assert!(matches!(&structured[0], HoverContent::Code { language, .. } if language == "rust"));
    }

    #[test]
    fn convert_array() {
        let contents = lsp_types::HoverContents::Array(vec![
            lsp_types::MarkedString::String("first".into()),
            lsp_types::MarkedString::String("second".into()),
        ]);
        let (markup, structured) = convert_hover_contents(contents);
        assert_eq!(markup.len(), 2);
        assert_eq!(structured.len(), 2);
    }

    #[test]
    fn render_hover_markdown_basic() {
        let blocks = render_hover_markdown("Hello **world**");
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], RenderedHoverBlock::Paragraph(s) if s.contains("world")));
    }

    #[test]
    fn render_hover_markdown_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let blocks = render_hover_markdown(md);
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], RenderedHoverBlock::CodeBlock { language, code }
            if language == "rust" && code.contains("fn main()")));
    }

    #[test]
    fn render_hover_markdown_separator() {
        let md = "First\n\n---\n\nSecond";
        let blocks = render_hover_markdown(md);
        assert_eq!(blocks.len(), 3);
        assert!(matches!(&blocks[1], RenderedHoverBlock::Separator));
    }

    #[test]
    fn render_hover_markdown_mixed() {
        let md = "Type: `i32`\n\n```rust\nlet x: i32 = 5;\n```\n\nDocumentation here.";
        let blocks = render_hover_markdown(md);
        assert!(blocks.len() >= 3);
    }

    #[test]
    fn hover_info_is_empty() {
        let info = HoverInfo {
            contents: vec![],
            structured_contents: vec![],
            range: None,
        };
        assert!(info.is_empty());
    }

    #[test]
    fn hover_info_render() {
        let info = HoverInfo {
            contents: vec![],
            structured_contents: vec![
                HoverContent::Code {
                    language: "rust".into(),
                    value: "fn foo()".into(),
                },
                HoverContent::Markdown("Some **docs**.".into()),
            ],
            range: None,
        };
        let blocks = info.render();
        assert!(blocks.len() >= 2);
    }

    #[test]
    fn markup_content_serialize() {
        let mc = MarkupContent::Markdown("# Title".into());
        let json = serde_json::to_string(&mc).unwrap();
        let back: MarkupContent = serde_json::from_str(&json).unwrap();
        assert_eq!(mc, back);
    }
}
