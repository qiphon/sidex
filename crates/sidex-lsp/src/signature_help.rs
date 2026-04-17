//! Signature help / parameter hints engine wrapping LSP
//! `textDocument/signatureHelp`.
//!
//! Provides rich multi-signature support with active parameter tracking,
//! parameter-level documentation, label offset rendering, manual and
//! auto-trigger handling, and overload navigation.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::client::LspClient;
use crate::conversion::position_to_lsp;

// ── ParameterLabel ──────────────────────────────────────────────────────────

/// How the parameter label is specified.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParameterLabel {
    /// A simple string label (e.g. `"x: i32"`).
    Simple(String),
    /// Byte offsets into the signature label string.
    Offsets(u32, u32),
}

// ── ParameterInfo ───────────────────────────────────────────────────────────

/// Information about a single parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterInfo {
    /// The parameter label.
    pub label: String,
    /// The label kind (for offset-based highlighting).
    pub label_kind: ParameterLabel,
    /// Optional documentation for the parameter.
    pub documentation: Option<String>,
}

// ── SignatureInfo ────────────────────────────────────────────────────────────

/// Information about a single function/method signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureInfo {
    /// The full signature label (e.g. `"fn foo(x: i32, y: &str) -> bool"`).
    pub label: String,
    /// Optional documentation for the signature.
    pub documentation: Option<String>,
    /// Parameter information.
    pub parameters: Vec<ParameterInfo>,
    /// Index of the currently active parameter.
    pub active_parameter: usize,
}

impl SignatureInfo {
    /// Returns the active parameter, if one exists.
    pub fn active_param(&self) -> Option<&ParameterInfo> {
        self.parameters.get(self.active_parameter)
    }

    /// Returns the byte offsets of the active parameter within the label,
    /// suitable for bold rendering.
    pub fn active_param_offsets(&self) -> Option<(usize, usize)> {
        let param = self.active_param()?;
        match &param.label_kind {
            ParameterLabel::Offsets(start, end) => Some((*start as usize, *end as usize)),
            ParameterLabel::Simple(text) => {
                let start = self.label.find(text.as_str())?;
                Some((start, start + text.len()))
            }
        }
    }
}

// ── SignatureHelpState ──────────────────────────────────────────────────────

/// State for an active signature help session in the editor.
#[derive(Debug, Clone)]
pub struct SignatureHelpState {
    /// All available signatures (overloads).
    pub signatures: Vec<SignatureInfo>,
    /// Which signature is currently displayed.
    pub active_signature: usize,
    /// The trigger character that started this session.
    pub trigger_char: Option<char>,
    /// Whether this was manually triggered.
    pub manually_triggered: bool,
}

impl SignatureHelpState {
    /// Returns the currently active signature.
    pub fn current_signature(&self) -> Option<&SignatureInfo> {
        self.signatures.get(self.active_signature)
    }

    /// Cycle to the next overload.
    pub fn next_signature(&mut self) {
        if !self.signatures.is_empty() {
            self.active_signature = (self.active_signature + 1) % self.signatures.len();
        }
    }

    /// Cycle to the previous overload.
    pub fn prev_signature(&mut self) {
        if !self.signatures.is_empty() {
            self.active_signature = self
                .active_signature
                .checked_sub(1)
                .unwrap_or(self.signatures.len() - 1);
        }
    }

    /// Returns the number of available overloads.
    pub fn overload_count(&self) -> usize {
        self.signatures.len()
    }

    /// Returns the overload indicator text (e.g. `"1/3"`).
    pub fn overload_label(&self) -> String {
        if self.signatures.len() <= 1 {
            String::new()
        } else {
            format!("{}/{}", self.active_signature + 1, self.signatures.len())
        }
    }
}

// ── Trigger characters ──────────────────────────────────────────────────────

/// Default trigger characters for signature help.
pub const DEFAULT_TRIGGER_CHARS: &[char] = &['(', ','];

/// Default re-trigger characters.
pub const DEFAULT_RETRIGGER_CHARS: &[char] = &[','];

/// Characters that dismiss signature help.
pub const DISMISS_CHARS: &[char] = &[')'];

/// Returns `true` if `ch` should trigger signature help.
pub fn is_trigger_char(ch: char) -> bool {
    DEFAULT_TRIGGER_CHARS.contains(&ch)
}

/// Returns `true` if `ch` should dismiss signature help.
pub fn is_dismiss_char(ch: char) -> bool {
    DISMISS_CHARS.contains(&ch)
}

// ── LSP request ─────────────────────────────────────────────────────────────

/// Requests signature help from the language server.
pub async fn request_signature(
    client: &LspClient,
    uri: &str,
    pos: sidex_text::Position,
) -> Result<Option<Vec<SignatureInfo>>> {
    let lsp_pos = position_to_lsp(pos);
    let response = client.signature_help(uri, lsp_pos).await?;

    let Some(sig_help) = response else {
        return Ok(None);
    };

    if sig_help.signatures.is_empty() {
        return Ok(None);
    }

    let active_sig = sig_help.active_signature.unwrap_or(0) as usize;
    let active_param = sig_help.active_parameter.unwrap_or(0) as usize;

    let signatures: Vec<SignatureInfo> = sig_help
        .signatures
        .into_iter()
        .enumerate()
        .map(|(i, sig)| {
            let documentation = sig.documentation.map(|doc| match doc {
                lsp_types::Documentation::String(s) => s,
                lsp_types::Documentation::MarkupContent(mc) => mc.value,
            });

            let parameters: Vec<ParameterInfo> = sig
                .parameters
                .unwrap_or_default()
                .into_iter()
                .map(|p| {
                    let (label, label_kind) = match p.label {
                        lsp_types::ParameterLabel::Simple(s) => {
                            let kind = ParameterLabel::Simple(s.clone());
                            (s, kind)
                        }
                        lsp_types::ParameterLabel::LabelOffsets([start, end]) => {
                            let text = if (start as usize) < sig.label.len()
                                && (end as usize) <= sig.label.len()
                            {
                                sig.label[start as usize..end as usize].to_owned()
                            } else {
                                format!("[{start}..{end}]")
                            };
                            let kind = ParameterLabel::Offsets(start, end);
                            (text, kind)
                        }
                    };
                    let documentation = p.documentation.map(|doc| match doc {
                        lsp_types::Documentation::String(s) => s,
                        lsp_types::Documentation::MarkupContent(mc) => mc.value,
                    });
                    ParameterInfo {
                        label,
                        label_kind,
                        documentation,
                    }
                })
                .collect();

            let effective_active = if i == active_sig { active_param } else { 0 };

            SignatureInfo {
                label: sig.label,
                documentation,
                parameters,
                active_parameter: effective_active,
            }
        })
        .collect();

    Ok(Some(signatures))
}

/// Requests signature help and returns a full `SignatureHelpState`.
pub async fn request_signature_state(
    client: &LspClient,
    uri: &str,
    pos: sidex_text::Position,
    trigger_char: Option<char>,
    manually_triggered: bool,
) -> Result<Option<SignatureHelpState>> {
    let sigs = request_signature(client, uri, pos).await?;
    let Some(signatures) = sigs else {
        return Ok(None);
    };
    if signatures.is_empty() {
        return Ok(None);
    }
    Ok(Some(SignatureHelpState {
        signatures,
        active_signature: 0,
        trigger_char,
        manually_triggered,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sig(label: &str, params: Vec<(&str, Option<&str>)>) -> SignatureInfo {
        SignatureInfo {
            label: label.to_owned(),
            documentation: None,
            parameters: params
                .into_iter()
                .map(|(l, doc)| ParameterInfo {
                    label: l.to_owned(),
                    label_kind: ParameterLabel::Simple(l.to_owned()),
                    documentation: doc.map(|d| d.to_owned()),
                })
                .collect(),
            active_parameter: 0,
        }
    }

    #[test]
    fn signature_info_active_param() {
        let sig = make_sig("fn foo(x: i32, y: &str)", vec![("x: i32", None), ("y: &str", None)]);
        assert_eq!(sig.active_param().unwrap().label, "x: i32");
    }

    #[test]
    fn signature_info_active_param_offsets() {
        let sig = make_sig("fn foo(x: i32, y: &str)", vec![("x: i32", None), ("y: &str", None)]);
        let (start, end) = sig.active_param_offsets().unwrap();
        assert!(start < end);
    }

    #[test]
    fn signature_help_state_overloads() {
        let mut state = SignatureHelpState {
            signatures: vec![
                make_sig("fn a()", vec![]),
                make_sig("fn b(x: i32)", vec![("x: i32", None)]),
                make_sig("fn c(x: i32, y: i32)", vec![]),
            ],
            active_signature: 0,
            trigger_char: Some('('),
            manually_triggered: false,
        };
        assert_eq!(state.overload_count(), 3);
        assert_eq!(state.overload_label(), "1/3");

        state.next_signature();
        assert_eq!(state.active_signature, 1);
        assert_eq!(state.overload_label(), "2/3");

        state.next_signature();
        state.next_signature();
        assert_eq!(state.active_signature, 0);

        state.prev_signature();
        assert_eq!(state.active_signature, 2);
    }

    #[test]
    fn trigger_chars() {
        assert!(is_trigger_char('('));
        assert!(is_trigger_char(','));
        assert!(!is_trigger_char('.'));
    }

    #[test]
    fn dismiss_chars() {
        assert!(is_dismiss_char(')'));
        assert!(!is_dismiss_char('('));
    }

    #[test]
    fn parameter_label_offsets() {
        let sig = SignatureInfo {
            label: "fn foo(x: i32, y: &str)".to_owned(),
            documentation: None,
            parameters: vec![ParameterInfo {
                label: "x: i32".to_owned(),
                label_kind: ParameterLabel::Offsets(7, 13),
                documentation: None,
            }],
            active_parameter: 0,
        };
        let (start, end) = sig.active_param_offsets().unwrap();
        assert_eq!(start, 7);
        assert_eq!(end, 13);
    }

    #[test]
    fn parameter_info_serialize() {
        let param = ParameterInfo {
            label: "x: i32".into(),
            label_kind: ParameterLabel::Simple("x: i32".into()),
            documentation: Some("the x param".into()),
        };
        let json = serde_json::to_string(&param).unwrap();
        let back: ParameterInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.label, "x: i32");
    }
}
