//! Smart indentation engine — mirrors VS Code's `AutoIndent` contribution.
//!
//! Provides language-specific rules for increasing/decreasing indent on Enter,
//! auto-adjusting pasted text, and handling electric characters.

use std::collections::HashMap;

use regex::Regex;

const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input",
    "link", "meta", "param", "source", "track", "wbr",
];

/// Top-level auto-indent engine with per-language rules.
#[derive(Debug, Clone)]
pub struct AutoIndentEngine {
    pub rules: HashMap<String, IndentRules>,
    pub use_tabs: bool,
    pub tab_size: u32,
}

impl Default for AutoIndentEngine {
    fn default() -> Self {
        let mut rules = HashMap::new();
        rules.insert("rust".into(), IndentRules::rust());
        rules.insert("javascript".into(), IndentRules::c_like());
        rules.insert("typescript".into(), IndentRules::c_like());
        rules.insert("c".into(), IndentRules::c_like());
        rules.insert("cpp".into(), IndentRules::c_like());
        rules.insert("java".into(), IndentRules::c_like());
        rules.insert("go".into(), IndentRules::c_like());
        rules.insert("python".into(), IndentRules::python());
        rules.insert("html".into(), IndentRules::html());
        rules.insert("css".into(), IndentRules::css());
        Self {
            rules,
            use_tabs: false,
            tab_size: 4,
        }
    }
}

/// Indent rules for a given language, using regex patterns.
#[derive(Debug, Clone)]
pub struct IndentRules {
    pub increase_indent_pattern: Option<String>,
    pub decrease_indent_pattern: Option<String>,
    pub indent_next_line_pattern: Option<String>,
    pub unindented_line_pattern: Option<String>,
}

impl Default for IndentRules {
    fn default() -> Self {
        Self::c_like()
    }
}

impl IndentRules {
    /// Rules for C-like languages (Rust, JS, TS, C, C++, Go, Java).
    #[must_use]
    pub fn c_like() -> Self {
        Self {
            increase_indent_pattern: Some(r"[{(\[]\s*$".into()),
            decrease_indent_pattern: Some(r"^\s*[})\]]".into()),
            indent_next_line_pattern: None,
            unindented_line_pattern: Some(r"^\s*#".into()),
        }
    }

    /// Rules specific to Rust.
    #[must_use]
    pub fn rust() -> Self {
        Self {
            increase_indent_pattern: Some(r"(\{[^}]*$|\([^)]*$|\[[^\]]*$|=>\s*$)".into()),
            decrease_indent_pattern: Some(r"^\s*[})\]]".into()),
            indent_next_line_pattern: None,
            unindented_line_pattern: Some(r"^\s*#".into()),
        }
    }

    /// Rules for Python.
    #[must_use]
    pub fn python() -> Self {
        Self {
            increase_indent_pattern: Some(r":\s*(#.*)?$".into()),
            decrease_indent_pattern: Some(r"^\s*(return|break|continue|pass|raise)\b".into()),
            indent_next_line_pattern: None,
            unindented_line_pattern: None,
        }
    }

    /// Rules for HTML.
    #[must_use]
    pub fn html() -> Self {
        Self {
            increase_indent_pattern: Some(r"<\w+[^/>]*>\s*$".into()),
            decrease_indent_pattern: Some(r"^\s*</".into()),
            indent_next_line_pattern: None,
            unindented_line_pattern: None,
        }
    }

    /// Rules for CSS / SCSS / Less.
    #[must_use]
    pub fn css() -> Self {
        Self {
            increase_indent_pattern: Some(r"\{\s*$".into()),
            decrease_indent_pattern: Some(r"^\s*\}".into()),
            indent_next_line_pattern: None,
            unindented_line_pattern: None,
        }
    }

    fn matches_increase(&self, line: &str) -> bool {
        self.increase_indent_pattern.as_ref().is_some_and(|p| {
            Regex::new(p).map_or(false, |re| {
                if let Some(m) = re.find(line) {
                    let tag_text = m.as_str();
                    if tag_text.starts_with('<') && !tag_text.starts_with("</") {
                        let tag_name: String = tag_text
                            .trim_start_matches('<')
                            .chars()
                            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                            .collect();
                        let lower = tag_name.to_ascii_lowercase();
                        return !VOID_ELEMENTS.contains(&lower.as_str());
                    }
                    true
                } else {
                    false
                }
            })
        })
    }

    fn matches_decrease(&self, line: &str) -> bool {
        self.decrease_indent_pattern.as_ref().is_some_and(|p| {
            Regex::new(p).map_or(false, |re| re.is_match(line))
        })
    }

    fn matches_indent_next(&self, line: &str) -> bool {
        self.indent_next_line_pattern.as_ref().is_some_and(|p| {
            Regex::new(p).map_or(false, |re| re.is_match(line))
        })
    }
}

/// The result of computing indentation for a new line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndentAction {
    /// Keep the same indentation as the previous line.
    Keep,
    /// Increase indent by one level.
    Increase,
    /// Decrease indent by one level.
    Decrease,
    /// Increase and then decrease (e.g. `{|}` → `{\n  \n}`).
    IndentOutdent,
}

impl AutoIndentEngine {
    fn indent_str(&self) -> String {
        if self.use_tabs {
            "\t".to_string()
        } else {
            " ".repeat(self.tab_size as usize)
        }
    }

    fn rules_for(&self, language: &str) -> IndentRules {
        self.rules.get(language).cloned().unwrap_or_default()
    }

    /// Determines what indent action to take when pressing Enter
    /// after `prev_line` and before `current_line`.
    #[must_use]
    pub fn indent_action(
        &self,
        prev_line: &str,
        current_line: &str,
        language: &str,
    ) -> IndentAction {
        let rules = self.rules_for(language);
        let increases = rules.matches_increase(prev_line);
        let decreases = rules.matches_decrease(current_line);

        if increases && decreases {
            IndentAction::IndentOutdent
        } else if increases || rules.matches_indent_next(prev_line) {
            IndentAction::Increase
        } else if decreases {
            IndentAction::Decrease
        } else {
            IndentAction::Keep
        }
    }

    /// Computes the indentation string for a new line given the
    /// previous line and (optionally) the current line content.
    #[must_use]
    pub fn compute_indent(
        &self,
        prev_line: &str,
        current_line: &str,
        language: &str,
    ) -> String {
        let base_indent = extract_indent(prev_line);
        let indent_unit = self.indent_str();

        match self.indent_action(prev_line, current_line, language) {
            IndentAction::Keep => base_indent,
            IndentAction::Increase => format!("{base_indent}{indent_unit}"),
            IndentAction::Decrease => deindent(&base_indent, &indent_unit),
            IndentAction::IndentOutdent => format!("{base_indent}{indent_unit}"),
        }
    }

    /// Adjusts the indentation of pasted text to match the target context.
    ///
    /// `pasted_text` is the multi-line text being pasted.
    /// `target_indent` is the indentation at the paste destination.
    /// `source_indent` is the minimum indentation found in the pasted text.
    #[must_use]
    pub fn adjust_pasted_indent(
        &self,
        pasted_text: &str,
        target_indent: &str,
        source_indent: &str,
    ) -> String {
        adjust_pasted_indent(pasted_text, target_indent, source_indent)
    }

    /// Computes whether typing a character should trigger re-indentation.
    #[must_use]
    pub fn should_reindent_on_type(
        &self,
        typed_char: char,
        line_text: &str,
        language: &str,
    ) -> bool {
        let rules = self.rules_for(language);
        let closing = matches!(typed_char, '}' | ')' | ']');
        if !closing {
            return false;
        }
        let before_cursor = line_text.trim_start();
        if before_cursor.len() > 1 {
            return false;
        }
        rules.matches_decrease(line_text)
    }
}

/// Extracts the leading whitespace from a line.
#[must_use]
pub fn extract_indent(line: &str) -> String {
    line.chars().take_while(|c| c.is_whitespace()).collect()
}

fn deindent(indent: &str, unit: &str) -> String {
    if let Some(stripped) = indent.strip_suffix(unit) {
        stripped.to_string()
    } else if indent.ends_with('\t') {
        indent[..indent.len() - 1].to_string()
    } else {
        let mut s = indent.to_string();
        while s.ends_with(' ') && s.len() > indent.len().saturating_sub(unit.len()) {
            s.pop();
        }
        s
    }
}

/// Computes the indentation for a new line given the previous line
/// and (optionally) the current line. Convenience free function.
#[must_use]
pub fn compute_indent(prev_line: &str, current_line: &str, rules: &IndentRules) -> String {
    let base = extract_indent(prev_line);
    let indent_unit = "    ";

    let increases = rules.matches_increase(prev_line);
    let decreases = rules.matches_decrease(current_line);

    if increases && decreases {
        format!("{base}{indent_unit}")
    } else if increases {
        format!("{base}{indent_unit}")
    } else if decreases {
        deindent(&base, indent_unit)
    } else {
        base
    }
}

/// Adjusts pasted text indentation. Free function version.
#[must_use]
pub fn adjust_pasted_indent(
    pasted_text: &str,
    target_indent: &str,
    source_indent: &str,
) -> String {
    let lines: Vec<&str> = pasted_text.split('\n').collect();
    if lines.len() <= 1 {
        return pasted_text.to_string();
    }

    let mut result = lines[0].to_string();
    for line in &lines[1..] {
        result.push('\n');
        if line.trim().is_empty() {
            result.push_str(line);
        } else {
            let stripped = line.strip_prefix(source_indent).unwrap_or(line.trim_start());
            result.push_str(target_indent);
            result.push_str(stripped);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_indent_spaces() {
        assert_eq!(extract_indent("    hello"), "    ");
        assert_eq!(extract_indent("hello"), "");
        assert_eq!(extract_indent("\t\thello"), "\t\t");
    }

    #[test]
    fn c_like_increase_after_brace() {
        let engine = AutoIndentEngine::default();
        let indent = engine.compute_indent("fn main() {", "", "rust");
        assert_eq!(indent, "    ");
    }

    #[test]
    fn c_like_keep_indent() {
        let engine = AutoIndentEngine::default();
        let indent = engine.compute_indent("    let x = 1;", "", "rust");
        assert_eq!(indent, "    ");
    }

    #[test]
    fn c_like_decrease_on_close() {
        let engine = AutoIndentEngine::default();
        let indent = engine.compute_indent("    let x = 1;", "    }", "rust");
        assert_eq!(indent, "");
    }

    #[test]
    fn python_increase_after_colon() {
        let engine = AutoIndentEngine::default();
        let indent = engine.compute_indent("def foo():", "", "python");
        assert_eq!(indent, "    ");
    }

    #[test]
    fn python_keep() {
        let engine = AutoIndentEngine::default();
        let indent = engine.compute_indent("    x = 1", "", "python");
        assert_eq!(indent, "    ");
    }

    #[test]
    fn indent_outdent_action() {
        let engine = AutoIndentEngine::default();
        let action = engine.indent_action("fn main() {", "}", "rust");
        assert_eq!(action, IndentAction::IndentOutdent);
    }

    #[test]
    fn adjust_paste_indentation() {
        let pasted = "line1\n    line2\n    line3";
        let result = adjust_pasted_indent(pasted, "        ", "    ");
        assert!(result.contains("        line2"));
        assert!(result.contains("        line3"));
    }

    #[test]
    fn adjust_paste_single_line() {
        assert_eq!(adjust_pasted_indent("hello", "    ", ""), "hello");
    }

    #[test]
    fn should_reindent_closing_brace() {
        let engine = AutoIndentEngine::default();
        assert!(engine.should_reindent_on_type('}', "    }", "rust"));
        assert!(!engine.should_reindent_on_type('}', "    x}", "rust"));
        assert!(!engine.should_reindent_on_type('a', "    a", "rust"));
    }

    #[test]
    fn compute_indent_free_fn() {
        let rules = IndentRules::c_like();
        let result = compute_indent("if (true) {", "", &rules);
        assert_eq!(result, "    ");
    }

    #[test]
    fn tabs_mode() {
        let mut engine = AutoIndentEngine::default();
        engine.use_tabs = true;
        let indent = engine.compute_indent("fn main() {", "", "rust");
        assert_eq!(indent, "\t");
    }

    #[test]
    fn html_indent_rules() {
        let engine = AutoIndentEngine::default();
        let indent = engine.compute_indent("<div>", "", "html");
        assert_eq!(indent, "    ");
    }

    #[test]
    fn html_decrease_closing() {
        let engine = AutoIndentEngine::default();
        let indent = engine.compute_indent("    <p>hello</p>", "    </div>", "html");
        assert_eq!(indent, "");
    }
}
