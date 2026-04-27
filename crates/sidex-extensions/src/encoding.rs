//! Strict UTF-8 decoding for extension manifest text.
//!
//! Extension `package.json`, NLS files, READMEs and similar text inside `.vsix`
//! archives or on disk are required to be UTF-8. Some publishers ship files
//! with a UTF-8 BOM (`EF BB BF`) which `serde_json` does not accept and which
//! breaks downstream display strings. Other times, files arrive with broken
//! bytes and naive `from_utf8_lossy` silently mojibakes `displayName` etc.
//!
//! Decoding via these helpers preserves all valid bytes verbatim, strips a
//! leading BOM if present, and surfaces a real error on invalid UTF-8 instead
//! of silently substituting `U+FFFD`.

use std::path::Path;

use anyhow::{Context, Result};

/// Decode a byte slice as strict UTF-8 and strip a leading UTF-8 BOM.
pub fn decode_manifest_text(bytes: &[u8]) -> Result<String> {
    let s = std::str::from_utf8(bytes).context("extension text is not valid UTF-8")?;
    Ok(s.strip_prefix('\u{FEFF}').unwrap_or(s).to_owned())
}

/// Read a file from disk as strict UTF-8 with BOM stripped.
pub fn read_manifest_file(path: &Path) -> Result<String> {
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    decode_manifest_text(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_utf8_bom() {
        let bytes = b"\xEF\xBB\xBF{\"name\":\"foo\"}";
        let s = decode_manifest_text(bytes).unwrap();
        assert_eq!(s, r#"{"name":"foo"}"#);
    }

    #[test]
    fn preserves_cjk() {
        let original = r#"{"displayName":"中文扩展 🚀"}"#;
        let s = decode_manifest_text(original.as_bytes()).unwrap();
        assert_eq!(s, original);
    }

    #[test]
    fn preserves_cjk_with_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(r#"{"displayName":"日本語拡張"}"#.as_bytes());
        let s = decode_manifest_text(&bytes).unwrap();
        assert_eq!(s, r#"{"displayName":"日本語拡張"}"#);
    }

    #[test]
    fn rejects_invalid_utf8() {
        let bytes = b"\xFF\xFE\x00garbage";
        assert!(decode_manifest_text(bytes).is_err());
    }

    #[test]
    fn passthrough_ascii() {
        let s = decode_manifest_text(b"{\"a\":1}").unwrap();
        assert_eq!(s, r#"{"a":1}"#);
    }
}
