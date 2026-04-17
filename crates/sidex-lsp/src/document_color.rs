//! Document color support wrapping LSP `textDocument/documentColor` and
//! `textDocument/colorPresentation`.
//!
//! Provides inline color swatches and a color picker for color literals in code.

use std::str::FromStr;

use anyhow::{Context, Result};
use lsp_types::{
    ColorPresentationParams, DocumentColorParams, PartialResultParams, TextDocumentIdentifier, Uri,
    WorkDoneProgressParams,
};
use serde::{Deserialize, Serialize};

use crate::client::LspClient;
use crate::conversion::{lsp_to_range, range_to_lsp};

/// An RGBA color value from the language server.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LspColor {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

impl LspColor {
    pub fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    /// Convert to a CSS hex string (e.g. `#ff0000ff`).
    pub fn to_hex(&self) -> String {
        let r = (self.red * 255.0) as u8;
        let g = (self.green * 255.0) as u8;
        let b = (self.blue * 255.0) as u8;
        let a = (self.alpha * 255.0) as u8;
        if a == 255 {
            format!("#{r:02x}{g:02x}{b:02x}")
        } else {
            format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
        }
    }
}

/// A color found at a specific location in the document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorInformation {
    pub range: sidex_text::Range,
    pub color: LspColor,
}

/// A text edit that applies a color presentation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEdit {
    pub range: sidex_text::Range,
    pub new_text: String,
}

/// A presentation of a color value (e.g. `rgb(255, 0, 0)`, `#ff0000`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorPresentation {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_edit: Option<TextEdit>,
    pub additional_edits: Vec<TextEdit>,
}

/// Service for document colors and color presentations.
pub struct DocumentColorService;

impl DocumentColorService {
    /// Request color information for all color literals in a document.
    pub async fn provide_document_colors(
        client: &LspClient,
        uri: &str,
    ) -> Result<Vec<ColorInformation>> {
        provide_document_colors(client, uri).await
    }

    /// Request alternative color presentations for a given color and range.
    pub async fn provide_color_presentations(
        client: &LspClient,
        uri: &str,
        color: &LspColor,
        range: sidex_text::Range,
    ) -> Result<Vec<ColorPresentation>> {
        provide_color_presentations(client, uri, color, range).await
    }
}

/// Requests all color literals in a document.
pub async fn provide_document_colors(
    client: &LspClient,
    uri: &str,
) -> Result<Vec<ColorInformation>> {
    let params = DocumentColorParams {
        text_document: TextDocumentIdentifier::new(Uri::from_str(uri).context("invalid URI")?),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };
    let val = serde_json::to_value(params)?;
    let result = client
        .raw_request("textDocument/documentColor", Some(val))
        .await?;

    if result.is_null() {
        return Ok(vec![]);
    }
    let infos: Vec<lsp_types::ColorInformation> =
        serde_json::from_value(result).context("failed to parse document colors")?;
    Ok(infos.into_iter().map(convert_color_info).collect())
}

/// Requests alternative color presentations for a color at a range.
pub async fn provide_color_presentations(
    client: &LspClient,
    uri: &str,
    color: &LspColor,
    range: sidex_text::Range,
) -> Result<Vec<ColorPresentation>> {
    let lsp_color = lsp_types::Color {
        red: color.red,
        green: color.green,
        blue: color.blue,
        alpha: color.alpha,
    };
    let params = ColorPresentationParams {
        text_document: TextDocumentIdentifier::new(Uri::from_str(uri).context("invalid URI")?),
        color: lsp_color,
        range: range_to_lsp(range),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };
    let val = serde_json::to_value(params)?;
    let result = client
        .raw_request("textDocument/colorPresentation", Some(val))
        .await?;

    if result.is_null() {
        return Ok(vec![]);
    }
    let presentations: Vec<lsp_types::ColorPresentation> =
        serde_json::from_value(result).context("failed to parse color presentations")?;
    Ok(presentations
        .into_iter()
        .map(convert_color_presentation)
        .collect())
}

fn convert_color_info(info: lsp_types::ColorInformation) -> ColorInformation {
    ColorInformation {
        range: lsp_to_range(info.range),
        color: LspColor {
            red: info.color.red,
            green: info.color.green,
            blue: info.color.blue,
            alpha: info.color.alpha,
        },
    }
}

fn convert_color_presentation(pres: lsp_types::ColorPresentation) -> ColorPresentation {
    ColorPresentation {
        label: pres.label,
        text_edit: pres.text_edit.map(|e| TextEdit {
            range: lsp_to_range(e.range),
            new_text: e.new_text,
        }),
        additional_edits: pres
            .additional_text_edits
            .unwrap_or_default()
            .into_iter()
            .map(|e| TextEdit {
                range: lsp_to_range(e.range),
                new_text: e.new_text,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsp_color_to_hex_opaque() {
        let c = LspColor::new(1.0, 0.0, 0.0, 1.0);
        assert_eq!(c.to_hex(), "#ff0000");
    }

    #[test]
    fn lsp_color_to_hex_transparent() {
        let c = LspColor::new(0.0, 1.0, 0.0, 0.5);
        let hex = c.to_hex();
        assert!(hex.starts_with('#'));
        assert_eq!(hex.len(), 9);
    }

    #[test]
    fn lsp_color_serde() {
        let c = LspColor::new(0.5, 0.5, 0.5, 1.0);
        let json = serde_json::to_string(&c).unwrap();
        let back: LspColor = serde_json::from_str(&json).unwrap();
        assert!((back.red - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn color_information_serde() {
        let info = ColorInformation {
            range: sidex_text::Range::new(
                sidex_text::Position::new(1, 10),
                sidex_text::Position::new(1, 17),
            ),
            color: LspColor::new(1.0, 0.0, 0.0, 1.0),
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: ColorInformation = serde_json::from_str(&json).unwrap();
        assert_eq!(back.range.start.line, 1);
    }

    #[test]
    fn color_presentation_serde() {
        let pres = ColorPresentation {
            label: "#ff0000".into(),
            text_edit: Some(TextEdit {
                range: sidex_text::Range::new(
                    sidex_text::Position::new(0, 0),
                    sidex_text::Position::new(0, 7),
                ),
                new_text: "#ff0000".into(),
            }),
            additional_edits: vec![],
        };
        let json = serde_json::to_string(&pres).unwrap();
        let back: ColorPresentation = serde_json::from_str(&json).unwrap();
        assert_eq!(back.label, "#ff0000");
        assert!(back.text_edit.is_some());
    }

    #[test]
    fn convert_lsp_color_info() {
        let lsp_info = lsp_types::ColorInformation {
            range: lsp_types::Range::new(
                lsp_types::Position::new(5, 10),
                lsp_types::Position::new(5, 17),
            ),
            color: lsp_types::Color {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            },
        };
        let info = convert_color_info(lsp_info);
        assert_eq!(info.range.start.line, 5);
        assert!((info.color.red - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn convert_lsp_color_presentation() {
        let lsp_pres = lsp_types::ColorPresentation {
            label: "rgb(255, 0, 0)".into(),
            text_edit: None,
            additional_text_edits: None,
        };
        let pres = convert_color_presentation(lsp_pres);
        assert_eq!(pres.label, "rgb(255, 0, 0)");
        assert!(pres.text_edit.is_none());
        assert!(pres.additional_edits.is_empty());
    }
}
