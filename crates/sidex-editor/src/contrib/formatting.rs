//! Document formatting — mirrors VS Code's `DocumentFormattingEditProvider`,
//! `DocumentRangeFormattingEditProvider`, and `OnTypeFormattingEditProvider`.

use sidex_text::{Position, Range};

/// A single text edit produced by a formatter.
#[derive(Debug, Clone)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

/// Options controlling how the formatter behaves.
#[derive(Debug, Clone)]
pub struct FormattingOptions {
    pub tab_size: u32,
    pub insert_spaces: bool,
    pub trim_trailing_whitespace: bool,
    pub insert_final_newline: bool,
    pub trim_final_newlines: bool,
}

impl Default for FormattingOptions {
    fn default() -> Self {
        Self { tab_size: 4, insert_spaces: true, trim_trailing_whitespace: true,
               insert_final_newline: true, trim_final_newlines: true }
    }
}

/// Metadata about a registered formatter.
#[derive(Debug, Clone)]
pub struct FormatterInfo {
    /// Unique identifier (e.g. `"esbenp.prettier-vscode"`).
    pub id: String,
    /// Human-readable name shown in UI.
    pub display_name: String,
    /// Language identifiers this formatter supports.
    pub languages: Vec<String>,
}

/// Characters that trigger format-on-type.
const FORMAT_ON_TYPE_TRIGGERS: &[char] = &['}', ';', '\n'];

/// Editor settings that control automatic formatting behaviour.
#[derive(Debug, Clone)]
pub struct FormattingSettings {
    /// `editor.formatOnSave`
    pub format_on_save: bool,
    /// `editor.formatOnPaste`
    pub format_on_paste: bool,
    /// `editor.formatOnType`
    pub format_on_type: bool,
    /// `editor.defaultFormatter` per language id.
    pub default_formatters: Vec<(String, String)>,
}

impl Default for FormattingSettings {
    fn default() -> Self {
        Self { format_on_save: false, format_on_paste: false,
               format_on_type: false, default_formatters: Vec::new() }
    }
}

/// Orchestrates document formatting requests.
#[derive(Debug, Clone)]
pub struct FormattingService {
    formatters: Vec<FormatterInfo>,
    settings: FormattingSettings,
}

impl FormattingService {
    #[must_use]
    pub fn new(settings: FormattingSettings) -> Self {
        Self { formatters: Vec::new(), settings }
    }

    /// Register a formatter so it can be selected for matching languages.
    pub fn register_formatter(&mut self, info: FormatterInfo) {
        self.formatters.push(info);
    }

    /// Select the best formatter for `language`, preferring the user's
    /// `editor.defaultFormatter` setting when one is configured.
    #[must_use]
    pub fn select_formatter(&self, language: &str) -> Option<&FormatterInfo> {
        let preferred_id = self.settings.default_formatters.iter()
            .find(|(lang, _)| lang == language)
            .map(|(_, id)| id.as_str());
        if let Some(id) = preferred_id {
            if let Some(f) = self.formatters.iter().find(|f| f.id == id) {
                return Some(f);
            }
        }
        self.formatters.iter().find(|f| f.languages.iter().any(|l| l == language))
    }

    /// Format an entire document.
    pub fn format_document(
        &self, _uri: &str, _options: &FormattingOptions,
    ) -> Result<Vec<TextEdit>, FormattingError> {
        self.active_formatter_for_uri(_uri).ok_or(FormattingError::NoFormatter)?;
        Ok(Vec::new())
    }

    /// Format a selection within a document.
    pub fn format_selection(
        &self, _uri: &str, _range: &Range, _options: &FormattingOptions,
    ) -> Result<Vec<TextEdit>, FormattingError> {
        self.active_formatter_for_uri(_uri).ok_or(FormattingError::NoFormatter)?;
        Ok(Vec::new())
    }

    /// Format after typing a trigger character.
    pub fn format_on_type(
        &self, _uri: &str, _position: Position, ch: char, _options: &FormattingOptions,
    ) -> Result<Vec<TextEdit>, FormattingError> {
        if !self.settings.format_on_type { return Err(FormattingError::Disabled); }
        if !FORMAT_ON_TYPE_TRIGGERS.contains(&ch) { return Ok(Vec::new()); }
        self.active_formatter_for_uri(_uri).ok_or(FormattingError::NoFormatter)?;
        Ok(Vec::new())
    }

    /// Returns `true` when format-on-save is enabled and a formatter exists.
    #[must_use]
    pub fn should_format_on_save(&self, uri: &str) -> bool {
        self.settings.format_on_save && self.active_formatter_for_uri(uri).is_some()
    }

    /// Returns `true` when format-on-paste is enabled.
    #[must_use]
    pub fn should_format_on_paste(&self, uri: &str) -> bool {
        self.settings.format_on_paste && self.active_formatter_for_uri(uri).is_some()
    }

    /// Resolve the language id from a URI (simplified: uses the file extension).
    fn language_for_uri(uri: &str) -> Option<&str> {
        Some(match uri.rsplit('.').next()? {
            "rs" => "rust",
            "ts" | "tsx" => "typescript",
            "js" | "jsx" => "javascript",
            "py" => "python",
            "go" => "go",
            "json" => "json",
            "html" | "htm" => "html",
            "css" => "css",
            "md" => "markdown",
            other => other,
        })
    }

    fn active_formatter_for_uri(&self, uri: &str) -> Option<&FormatterInfo> {
        let lang = Self::language_for_uri(uri)?;
        self.select_formatter(lang)
    }
}

/// Errors that can occur during formatting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormattingError {
    NoFormatter,
    Disabled,
    ProviderFailed(String),
}

impl std::fmt::Display for FormattingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoFormatter => write!(f, "no formatter registered for this language"),
            Self::Disabled => write!(f, "formatting is disabled"),
            Self::ProviderFailed(msg) => write!(f, "formatter failed: {msg}"),
        }
    }
}

impl std::error::Error for FormattingError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn service_with_prettier() -> FormattingService {
        let mut svc = FormattingService::new(FormattingSettings {
            format_on_save: true, format_on_paste: true, format_on_type: true,
            default_formatters: vec![("typescript".into(), "esbenp.prettier-vscode".into())],
        });
        svc.register_formatter(FormatterInfo {
            id: "esbenp.prettier-vscode".into(),
            display_name: "Prettier".into(),
            languages: vec!["typescript".into(), "javascript".into(), "json".into()],
        });
        svc
    }

    #[test]
    fn selects_preferred_formatter() {
        let svc = service_with_prettier();
        let f = svc.select_formatter("typescript").unwrap();
        assert_eq!(f.id, "esbenp.prettier-vscode");
    }

    #[test]
    fn falls_back_to_language_match() {
        let svc = service_with_prettier();
        let f = svc.select_formatter("json").unwrap();
        assert_eq!(f.display_name, "Prettier");
    }

    #[test]
    fn returns_none_for_unknown_language() {
        let svc = service_with_prettier();
        assert!(svc.select_formatter("haskell").is_none());
    }

    #[test]
    fn format_on_type_rejects_non_trigger() {
        let svc = service_with_prettier();
        let edits = svc.format_on_type("file.ts", Position::new(0, 0), 'a',
                                        &FormattingOptions::default()).unwrap();
        assert!(edits.is_empty());
    }

    #[test]
    fn format_on_type_disabled() {
        let mut svc = service_with_prettier();
        svc.settings.format_on_type = false;
        let err = svc.format_on_type("file.ts", Position::new(0, 0), '}',
                                      &FormattingOptions::default()).unwrap_err();
        assert_eq!(err, FormattingError::Disabled);
    }

    #[test]
    fn should_format_on_save() {
        let svc = service_with_prettier();
        assert!(svc.should_format_on_save("main.ts"));
        assert!(!svc.should_format_on_save("main.hs"));
    }
}
