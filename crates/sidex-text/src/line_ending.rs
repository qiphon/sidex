use serde::{Deserialize, Serialize};

/// Represents the type of line ending used in a text document.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineEnding {
    /// Unix-style line feed (`\n`).
    #[default]
    Lf,
    /// Windows-style carriage return + line feed (`\r\n`).
    CrLf,
    /// Classic Mac-style carriage return (`\r`).
    Cr,
    /// File has mixed line endings.
    Mixed,
}

impl LineEnding {
    /// Returns the string representation of this line ending.
    ///
    /// `Mixed` falls back to the platform default.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Lf | Self::Mixed => "\n",
            Self::CrLf => "\r\n",
            Self::Cr => "\r",
        }
    }

    /// Returns the default line ending for the current OS.
    #[must_use]
    pub fn os_default() -> Self {
        if cfg!(windows) {
            Self::CrLf
        } else {
            Self::Lf
        }
    }
}

impl std::fmt::Display for LineEnding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(line_ending_label(*self))
    }
}

/// Returns a short human-readable label for the line ending style (for
/// status bar display).
#[must_use]
pub fn line_ending_label(ending: LineEnding) -> &'static str {
    match ending {
        LineEnding::Lf => "LF",
        LineEnding::CrLf => "CRLF",
        LineEnding::Cr => "CR",
        LineEnding::Mixed => "Mixed",
    }
}

/// Detects the predominant line ending in a string.
///
/// Scans the first 10,000 characters and counts occurrences of each
/// line ending type. Returns `Mixed` if more than one style has significant
/// presence (the minority style is at least 20% of the total). Otherwise
/// returns the most common one, defaulting to `Lf`.
#[must_use]
pub fn detect_line_ending(text: &str) -> LineEnding {
    let (lf, crlf, cr) = count_line_endings(text);

    let total = lf + crlf + cr;
    if total == 0 {
        return LineEnding::Lf;
    }

    let dominant = lf.max(crlf).max(cr);
    let minority = total - dominant;

    if minority > 0 && minority * 5 >= total {
        return LineEnding::Mixed;
    }

    if crlf >= lf && crlf >= cr && crlf > 0 {
        LineEnding::CrLf
    } else if cr >= lf && cr > 0 {
        LineEnding::Cr
    } else {
        LineEnding::Lf
    }
}

/// Counts line endings in a text, returning `(lf_count, crlf_count, cr_count)`.
///
/// Scans the first 10,000 bytes for performance.
#[must_use]
pub fn count_line_endings(text: &str) -> (u32, u32, u32) {
    let sample = if text.len() > 10_000 {
        &text[..10_000]
    } else {
        text
    };

    let mut lf_count = 0u32;
    let mut crlf_count = 0u32;
    let mut cr_count = 0u32;

    let bytes = sample.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\r' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                crlf_count += 1;
                i += 2;
            } else {
                cr_count += 1;
                i += 1;
            }
        } else if bytes[i] == b'\n' {
            lf_count += 1;
            i += 1;
        } else {
            i += 1;
        }
    }

    (lf_count, crlf_count, cr_count)
}

/// Normalizes all line endings in the given text to the target type.
///
/// If `target` is `Mixed`, normalizes to the platform default.
#[must_use]
pub fn normalize_line_endings(text: &str, target: LineEnding) -> String {
    let effective = if target == LineEnding::Mixed {
        LineEnding::os_default()
    } else {
        target
    };
    let target_str = effective.as_str();
    let mut result = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'\r' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                i += 2;
            } else {
                i += 1;
            }
            result.push_str(target_str);
        } else if bytes[i] == b'\n' {
            result.push_str(target_str);
            i += 1;
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_lf() {
        assert_eq!(detect_line_ending("hello\nworld\n"), LineEnding::Lf);
    }

    #[test]
    fn detect_crlf() {
        assert_eq!(detect_line_ending("hello\r\nworld\r\n"), LineEnding::CrLf);
    }

    #[test]
    fn detect_cr() {
        assert_eq!(detect_line_ending("hello\rworld\r"), LineEnding::Cr);
    }

    #[test]
    fn detect_empty_defaults_to_lf() {
        assert_eq!(detect_line_ending(""), LineEnding::Lf);
    }

    #[test]
    fn detect_mixed() {
        // 3 LF and 3 CRLF = evenly split → mixed
        let text = "a\nb\nc\nd\r\ne\r\nf\r\n";
        assert_eq!(detect_line_ending(text), LineEnding::Mixed);
    }

    #[test]
    fn normalize_crlf_to_lf() {
        let result = normalize_line_endings("hello\r\nworld\r\n", LineEnding::Lf);
        assert_eq!(result, "hello\nworld\n");
    }

    #[test]
    fn normalize_lf_to_crlf() {
        let result = normalize_line_endings("hello\nworld\n", LineEnding::CrLf);
        assert_eq!(result, "hello\r\nworld\r\n");
    }

    #[test]
    fn normalize_mixed() {
        let result = normalize_line_endings("a\nb\r\nc\rd\n", LineEnding::Lf);
        assert_eq!(result, "a\nb\nc\nd\n");
    }

    #[test]
    fn normalize_mixed_target_uses_os_default() {
        let result = normalize_line_endings("a\nb\r\n", LineEnding::Mixed);
        let expected_eol = LineEnding::os_default().as_str();
        assert!(result.contains(expected_eol));
    }

    #[test]
    fn line_ending_as_str() {
        assert_eq!(LineEnding::Lf.as_str(), "\n");
        assert_eq!(LineEnding::CrLf.as_str(), "\r\n");
        assert_eq!(LineEnding::Cr.as_str(), "\r");
    }

    #[test]
    fn count_line_endings_basic() {
        let (lf, crlf, cr) = count_line_endings("a\nb\r\nc\rd\n");
        assert_eq!(lf, 2);
        assert_eq!(crlf, 1);
        assert_eq!(cr, 1);
    }

    #[test]
    fn count_line_endings_empty() {
        assert_eq!(count_line_endings(""), (0, 0, 0));
    }

    #[test]
    fn count_line_endings_no_newlines() {
        assert_eq!(count_line_endings("hello"), (0, 0, 0));
    }

    #[test]
    fn line_ending_label_values() {
        assert_eq!(line_ending_label(LineEnding::Lf), "LF");
        assert_eq!(line_ending_label(LineEnding::CrLf), "CRLF");
        assert_eq!(line_ending_label(LineEnding::Cr), "CR");
        assert_eq!(line_ending_label(LineEnding::Mixed), "Mixed");
    }

    #[test]
    fn display_matches_label() {
        assert_eq!(format!("{}", LineEnding::Lf), "LF");
        assert_eq!(format!("{}", LineEnding::Mixed), "Mixed");
    }

    #[test]
    fn os_default_is_lf_or_crlf() {
        let def = LineEnding::os_default();
        assert!(def == LineEnding::Lf || def == LineEnding::CrLf);
    }
}
