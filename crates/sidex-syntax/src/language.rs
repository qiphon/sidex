//! Language registry mapping file extensions to tree-sitter grammars.
//!
//! Each [`Language`] bundles a tree-sitter grammar with its highlight queries,
//! file extension associations, comment syntax, and editor behaviours like
//! auto-closing pairs and indentation rules. The [`LanguageRegistry`] provides
//! fast lookup by extension or name.
//!
//! [`LanguageConfig`] provides a purely-data description of a language's editor
//! behaviour (comments, brackets, indentation patterns) that does not require a
//! compiled tree-sitter grammar. Built-in configs for 30+ common languages are
//! available via [`builtin_language_configs`].
//!
//! [`LanguageConfiguration`] is an enriched, serialisable language description
//! that includes comment configuration, bracket pairs, auto-closing pairs,
//! surrounding pairs, folding config, indent rules, and on-enter rules.

use std::collections::HashMap;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::indent::{FoldingRules, IndentRule, OnEnterRule};

/// A language definition that pairs a tree-sitter grammar with metadata.
#[derive(Clone)]
pub struct Language {
    pub name: String,
    pub ts_language: tree_sitter::Language,
    pub highlight_query: Option<String>,
    pub injection_query: Option<String>,
    pub file_extensions: Vec<String>,
    pub line_comment: Option<String>,
    pub block_comment: Option<(String, String)>,
    pub auto_closing_pairs: Vec<(String, String)>,
    pub surrounding_pairs: Vec<(String, String)>,
    pub indent_rules: Vec<IndentRule>,
    pub word_pattern: Option<Regex>,
    pub on_enter_rules: Vec<OnEnterRule>,
    pub folding_rules: Option<FoldingRules>,
}

impl std::fmt::Debug for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Language")
            .field("name", &self.name)
            .field("has_highlight_query", &self.highlight_query.is_some())
            .field("has_injection_query", &self.injection_query.is_some())
            .field("file_extensions", &self.file_extensions)
            .field("line_comment", &self.line_comment)
            .field("block_comment", &self.block_comment)
            .field("auto_closing_pairs", &self.auto_closing_pairs.len())
            .field("surrounding_pairs", &self.surrounding_pairs.len())
            .field("indent_rules", &self.indent_rules.len())
            .field("has_word_pattern", &self.word_pattern.is_some())
            .field("on_enter_rules", &self.on_enter_rules.len())
            .field("has_folding_rules", &self.folding_rules.is_some())
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// LanguageConfig — static language configuration (no tree-sitter needed)
// ---------------------------------------------------------------------------

/// Static language configuration that does not require a tree-sitter grammar.
#[derive(Debug, Clone)]
pub struct LanguageConfig {
    pub name: String,
    pub file_extensions: Vec<String>,
    pub first_line_pattern: Option<Regex>,
    pub line_comment: Option<String>,
    pub block_comment: Option<(String, String)>,
    pub auto_closing_pairs: Vec<(String, String)>,
    pub surrounding_pairs: Vec<(String, String)>,
    pub indent_pattern: Option<Regex>,
    pub outdent_pattern: Option<Regex>,
    pub folding_start: Option<Regex>,
    pub folding_end: Option<Regex>,
    pub word_pattern: Option<Regex>,
}

impl LanguageConfig {
    fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            file_extensions: Vec::new(),
            first_line_pattern: None,
            line_comment: None,
            block_comment: None,
            auto_closing_pairs: Vec::new(),
            surrounding_pairs: Vec::new(),
            indent_pattern: None,
            outdent_pattern: None,
            folding_start: None,
            folding_end: None,
            word_pattern: None,
        }
    }
    fn exts(mut self, e: &[&str]) -> Self {
        self.file_extensions = e.iter().map(|s| (*s).into()).collect();
        self
    }
    fn first_line(mut self, p: &str) -> Self {
        self.first_line_pattern = Regex::new(p).ok();
        self
    }
    fn line_cmt(mut self, s: &str) -> Self {
        self.line_comment = Some(s.into());
        self
    }
    fn block_cmt(mut self, o: &str, c: &str) -> Self {
        self.block_comment = Some((o.into(), c.into()));
        self
    }
    fn auto_close(mut self, p: &[(&str, &str)]) -> Self {
        self.auto_closing_pairs = p.iter().map(|(a, b)| ((*a).into(), (*b).into())).collect();
        self
    }
    fn surround(mut self, p: &[(&str, &str)]) -> Self {
        self.surrounding_pairs = p.iter().map(|(a, b)| ((*a).into(), (*b).into())).collect();
        self
    }
    fn indent(mut self, p: &str) -> Self {
        self.indent_pattern = Regex::new(p).ok();
        self
    }
    fn outdent(mut self, p: &str) -> Self {
        self.outdent_pattern = Regex::new(p).ok();
        self
    }
    fn folding(mut self, s: &str, e: &str) -> Self {
        self.folding_start = Regex::new(s).ok();
        self.folding_end = Regex::new(e).ok();
        self
    }
    fn word_pat(mut self, p: &str) -> Self {
        self.word_pattern = Regex::new(p).ok();
        self
    }
}

// ---------------------------------------------------------------------------
// LanguageConfiguration — enriched, serialisable language description
// ---------------------------------------------------------------------------

/// Comment style configuration for a language.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommentConfig {
    pub line_comment: Option<String>,
    pub block_comment: Option<(String, String)>,
}

/// An auto-closing pair with optional exclusion contexts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoClosingPair {
    pub open: String,
    pub close: String,
    #[serde(default)]
    pub not_in: Vec<String>,
}

impl AutoClosingPair {
    #[must_use]
    pub fn new(open: &str, close: &str) -> Self {
        Self { open: open.into(), close: close.into(), not_in: Vec::new() }
    }
    #[must_use]
    pub fn not_in(mut self, contexts: &[&str]) -> Self {
        self.not_in = contexts.iter().map(|s| (*s).into()).collect();
        self
    }
}

/// Configuration for code folding markers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FoldingConfig {
    pub markers: Option<FoldingMarkers>,
    pub off_side: bool,
}

/// Regex markers for region-based folding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoldingMarkers {
    pub start: String,
    pub end: String,
}

/// Indent/outdent pattern rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndentRules {
    pub increase_indent_pattern: String,
    pub decrease_indent_pattern: String,
    pub indent_next_line_pattern: Option<String>,
    pub unindented_line_pattern: Option<String>,
}

/// Rule evaluated when the user presses Enter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnEnterRuleConfig {
    pub before_text: String,
    pub after_text: Option<String>,
    pub action: EnterAction,
}

/// What the editor should do when Enter is pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnterAction {
    None,
    Indent,
    IndentOutdent,
    Outdent,
}

/// Full language configuration with all editor behaviours, suitable for
/// serialisation to/from JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageConfiguration {
    pub id: String,
    pub name: String,
    pub extensions: Vec<String>,
    #[serde(default)]
    pub filenames: Vec<String>,
    pub first_line_pattern: Option<String>,
    pub comments: CommentConfig,
    pub brackets: Vec<(String, String)>,
    pub auto_closing_pairs: Vec<AutoClosingPair>,
    pub surrounding_pairs: Vec<(String, String)>,
    pub folding: FoldingConfig,
    pub word_pattern: Option<String>,
    pub indent_rules: Option<IndentRules>,
    #[serde(default)]
    pub on_enter_rules: Vec<OnEnterRuleConfig>,
}

impl LanguageConfiguration {
    fn builder(id: &str, name: &str) -> LangConfigBuilder {
        LangConfigBuilder {
            id: id.into(), name: name.into(),
            extensions: Vec::new(), filenames: Vec::new(),
            first_line_pattern: None,
            line_comment: None, block_comment: None,
            brackets: vec![
                ("(".into(), ")".into()), ("[".into(), "]".into()), ("{".into(), "}".into()),
            ],
            auto_closing_pairs: vec![
                AutoClosingPair::new("(", ")"),
                AutoClosingPair::new("[", "]"),
                AutoClosingPair::new("{", "}"),
                AutoClosingPair::new("\"", "\"").not_in(&["string"]),
                AutoClosingPair::new("'", "'").not_in(&["string", "comment"]),
            ],
            surrounding_pairs: vec![
                ("(".into(), ")".into()), ("[".into(), "]".into()), ("{".into(), "}".into()),
                ("\"".into(), "\"".into()), ("'".into(), "'".into()),
            ],
            folding_markers: None, off_side: false,
            word_pattern: None, indent_rules: None,
            on_enter_rules: Vec::new(),
        }
    }
}

struct LangConfigBuilder {
    id: String, name: String,
    extensions: Vec<String>, filenames: Vec<String>,
    first_line_pattern: Option<String>,
    line_comment: Option<String>, block_comment: Option<(String, String)>,
    brackets: Vec<(String, String)>,
    auto_closing_pairs: Vec<AutoClosingPair>,
    surrounding_pairs: Vec<(String, String)>,
    folding_markers: Option<FoldingMarkers>, off_side: bool,
    word_pattern: Option<String>, indent_rules: Option<IndentRules>,
    on_enter_rules: Vec<OnEnterRuleConfig>,
}

impl LangConfigBuilder {
    fn exts(mut self, e: &[&str]) -> Self {
        self.extensions = e.iter().map(|s| (*s).into()).collect(); self
    }
    fn filenames(mut self, f: &[&str]) -> Self {
        self.filenames = f.iter().map(|s| (*s).into()).collect(); self
    }
    fn first_line(mut self, p: &str) -> Self {
        self.first_line_pattern = Some(p.into()); self
    }
    fn line_cmt(mut self, s: &str) -> Self {
        self.line_comment = Some(s.into()); self
    }
    fn block_cmt(mut self, o: &str, c: &str) -> Self {
        self.block_comment = Some((o.into(), c.into())); self
    }
    fn off_side(mut self) -> Self {
        self.off_side = true; self
    }
    fn fold_markers(mut self, s: &str, e: &str) -> Self {
        self.folding_markers = Some(FoldingMarkers { start: s.into(), end: e.into() }); self
    }
    fn indent(mut self, inc: &str, dec: &str) -> Self {
        self.indent_rules = Some(IndentRules {
            increase_indent_pattern: inc.into(),
            decrease_indent_pattern: dec.into(),
            indent_next_line_pattern: None,
            unindented_line_pattern: None,
        }); self
    }
    #[allow(dead_code)]
    fn word_pat(mut self, p: &str) -> Self {
        self.word_pattern = Some(p.into()); self
    }
    fn build(self) -> LanguageConfiguration {
        LanguageConfiguration {
            id: self.id, name: self.name,
            extensions: self.extensions, filenames: self.filenames,
            first_line_pattern: self.first_line_pattern,
            comments: CommentConfig { line_comment: self.line_comment, block_comment: self.block_comment },
            brackets: self.brackets,
            auto_closing_pairs: self.auto_closing_pairs,
            surrounding_pairs: self.surrounding_pairs,
            folding: FoldingConfig { markers: self.folding_markers, off_side: self.off_side },
            word_pattern: self.word_pattern,
            indent_rules: self.indent_rules,
            on_enter_rules: self.on_enter_rules,
        }
    }
}

const C_INC: &str = r"^.*(\{[^}]*|\([^)]*|\[[^\]]*)$";
const C_DEC: &str = r"^\s*[\}\]\)]";
const C_FOLD_S: &str = r"^\s*/\*|^\s*\{";
const C_FOLD_E: &str = r"^\s*\*/|^\s*\}";

/// Returns enriched [`LanguageConfiguration`]s for 30+ common languages.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn builtin_language_configurations() -> Vec<LanguageConfiguration> {
    vec![
        LanguageConfiguration::builder("rust", "Rust")
            .exts(&[".rs"]).line_cmt("//").block_cmt("/*", "*/")
            .indent(C_INC, C_DEC).fold_markers(C_FOLD_S, C_FOLD_E).build(),
        LanguageConfiguration::builder("typescript", "TypeScript")
            .exts(&[".ts", ".tsx"]).line_cmt("//").block_cmt("/*", "*/")
            .indent(C_INC, C_DEC).fold_markers(C_FOLD_S, C_FOLD_E).build(),
        LanguageConfiguration::builder("javascript", "JavaScript")
            .exts(&[".js", ".jsx", ".mjs", ".cjs"]).first_line(r"^#!.*\bnode\b")
            .line_cmt("//").block_cmt("/*", "*/")
            .indent(C_INC, C_DEC).fold_markers(C_FOLD_S, C_FOLD_E).build(),
        LanguageConfiguration::builder("python", "Python")
            .exts(&[".py", ".pyi", ".pyw"]).first_line(r"^#!.*\bpython[23w]?\b")
            .line_cmt("#").off_side()
            .indent(r"^\s*(def|class|for|if|elif|else|while|try|with|finally|except|async)\b.*:\s*$",
                    r"^\s*(pass|break|continue|raise|return)\b").build(),
        LanguageConfiguration::builder("go", "Go")
            .exts(&[".go"]).line_cmt("//").block_cmt("/*", "*/")
            .indent(C_INC, C_DEC).fold_markers(C_FOLD_S, C_FOLD_E).build(),
        LanguageConfiguration::builder("java", "Java")
            .exts(&[".java"]).line_cmt("//").block_cmt("/*", "*/")
            .indent(C_INC, C_DEC).fold_markers(C_FOLD_S, C_FOLD_E).build(),
        LanguageConfiguration::builder("c", "C")
            .exts(&[".c", ".h"]).line_cmt("//").block_cmt("/*", "*/")
            .indent(C_INC, C_DEC)
            .fold_markers(r"^\s*/\*|^\s*\{|^\s*#\s*region", r"^\s*\*/|^\s*\}|^\s*#\s*endregion").build(),
        LanguageConfiguration::builder("cpp", "C++")
            .exts(&[".cpp", ".hpp", ".cc", ".cxx", ".hxx", ".hh"]).line_cmt("//").block_cmt("/*", "*/")
            .indent(C_INC, C_DEC)
            .fold_markers(r"^\s*/\*|^\s*\{|^\s*#\s*region", r"^\s*\*/|^\s*\}|^\s*#\s*endregion").build(),
        LanguageConfiguration::builder("csharp", "C#")
            .exts(&[".cs"]).line_cmt("//").block_cmt("/*", "*/")
            .indent(C_INC, C_DEC)
            .fold_markers(r"^\s*/\*|^\s*\{|^\s*#\s*region", r"^\s*\*/|^\s*\}|^\s*#\s*endregion").build(),
        LanguageConfiguration::builder("swift", "Swift")
            .exts(&[".swift"]).line_cmt("//").block_cmt("/*", "*/")
            .indent(C_INC, C_DEC).fold_markers(C_FOLD_S, C_FOLD_E).build(),
        LanguageConfiguration::builder("kotlin", "Kotlin")
            .exts(&[".kt", ".kts"]).line_cmt("//").block_cmt("/*", "*/")
            .indent(C_INC, C_DEC).fold_markers(C_FOLD_S, C_FOLD_E).build(),
        LanguageConfiguration::builder("scala", "Scala")
            .exts(&[".scala", ".sc"]).line_cmt("//").block_cmt("/*", "*/")
            .indent(C_INC, C_DEC).fold_markers(C_FOLD_S, C_FOLD_E).build(),
        LanguageConfiguration::builder("html", "HTML")
            .exts(&[".html", ".htm", ".xhtml"])
            .indent(r"<(?!(area|base|br|col|embed|hr|img|input|link|meta|param|source|track|wbr)\b)[a-zA-Z][^/]*>",
                    r"^\s*</[a-zA-Z]")
            .fold_markers(r"<[a-zA-Z]", r"</[a-zA-Z]").build(),
        LanguageConfiguration::builder("css", "CSS")
            .exts(&[".css"]).block_cmt("/*", "*/")
            .indent(r"\{[^}]*$", r"^\s*\}").fold_markers(r"\{", r"\}").build(),
        LanguageConfiguration::builder("scss", "SCSS")
            .exts(&[".scss"]).line_cmt("//").block_cmt("/*", "*/")
            .indent(r"\{[^}]*$", r"^\s*\}").fold_markers(r"\{", r"\}").build(),
        LanguageConfiguration::builder("less", "Less")
            .exts(&[".less"]).line_cmt("//").block_cmt("/*", "*/")
            .indent(r"\{[^}]*$", r"^\s*\}").fold_markers(r"\{", r"\}").build(),
        LanguageConfiguration::builder("json", "JSON")
            .exts(&[".json"])
            .indent(r"[\{\[]\s*$", r"^\s*[\}\]]").fold_markers(r"[\{\[]", r"[\}\]]").build(),
        LanguageConfiguration::builder("jsonc", "JSONC")
            .exts(&[".jsonc"]).line_cmt("//").block_cmt("/*", "*/")
            .indent(r"[\{\[]\s*$", r"^\s*[\}\]]").fold_markers(r"[\{\[]|/\*", r"[\}\]]|\*/").build(),
        LanguageConfiguration::builder("yaml", "YAML")
            .exts(&[".yml", ".yaml"]).line_cmt("#").off_side()
            .indent(r"^\s*[^#].*:\s*$", r"^\s*$").build(),
        LanguageConfiguration::builder("toml", "TOML")
            .exts(&[".toml"]).line_cmt("#")
            .fold_markers(r"^\s*\[", r"^\s*$").build(),
        LanguageConfiguration::builder("xml", "XML")
            .exts(&[".xml", ".xsd", ".xsl", ".xslt", ".svg"])
            .block_cmt("<!--", "-->")
            .indent(r"<(?![\?!/])[a-zA-Z][^/]*[^/]>", r"^\s*</[a-zA-Z]")
            .fold_markers(r"<[a-zA-Z]", r"</[a-zA-Z]").build(),
        LanguageConfiguration::builder("markdown", "Markdown")
            .exts(&[".md", ".markdown", ".mdown", ".mkd"])
            .fold_markers(r"^\s*```|^#{1,6}\s", r"^\s*```").build(),
        LanguageConfiguration::builder("sql", "SQL")
            .exts(&[".sql"]).line_cmt("--").block_cmt("/*", "*/")
            .indent(r"(?i)^\s*(begin|case|create|alter|if|loop|while|for)\b",
                    r"(?i)^\s*(end|else)\b")
            .fold_markers(r"(?i)^\s*(begin|case)\b", r"(?i)^\s*end\b").build(),
        LanguageConfiguration::builder("shellscript", "Shell")
            .exts(&[".sh", ".bash", ".zsh", ".ksh"]).first_line(r"^#!.*\b(ba|z|k)?sh\b")
            .line_cmt("#")
            .indent(r"(^\s*(if|elif|else|for|while|until|do|case|then)\b|.*\{\s*$)",
                    r"^\s*(fi|done|esac)\b|^\s*\}")
            .fold_markers(r"^\s*(if|for|while|until|case)\b|.*\{\s*$",
                          r"^\s*(fi|done|esac)\b|^\s*\}").build(),
        LanguageConfiguration::builder("powershell", "PowerShell")
            .exts(&[".ps1", ".psm1", ".psd1"]).line_cmt("#").block_cmt("<#", "#>")
            .indent(C_INC, C_DEC).fold_markers(C_FOLD_S, C_FOLD_E).build(),
        LanguageConfiguration::builder("dockerfile", "Dockerfile")
            .filenames(&["Dockerfile", "Dockerfile.*"]).exts(&[".dockerfile"])
            .line_cmt("#").build(),
        LanguageConfiguration::builder("makefile", "Makefile")
            .filenames(&["Makefile", "makefile", "GNUmakefile"]).exts(&[".mk", ".mak"])
            .line_cmt("#").indent(r"^[^\t].*:\s*$", r"^\S").build(),
        LanguageConfiguration::builder("ruby", "Ruby")
            .exts(&[".rb", ".erb", ".rake", ".gemspec"]).first_line(r"^#!.*\bruby\b")
            .line_cmt("#")
            .indent(r"^\s*(def|class|module|if|elsif|else|unless|case|when|while|until|for|begin|do)\b",
                    r"^\s*(end|else|elsif|when|rescue|ensure)\b")
            .fold_markers(r"^\s*(def|class|module|if|unless|case|while|until|for|begin|do)\b",
                          r"^\s*end\b").build(),
        LanguageConfiguration::builder("php", "PHP")
            .exts(&[".php", ".phtml"]).first_line(r"<\?php")
            .line_cmt("//").block_cmt("/*", "*/")
            .indent(C_INC, C_DEC).fold_markers(C_FOLD_S, C_FOLD_E).build(),
        LanguageConfiguration::builder("lua", "Lua")
            .exts(&[".lua"]).line_cmt("--").block_cmt("--[[", "]]")
            .indent(r"^\s*(function|if|for|while|repeat|else|elseif|do)\b",
                    r"^\s*(end|else|elseif|until)\b")
            .fold_markers(r"^\s*(function|if|for|while|repeat|do)\b", r"^\s*end\b").build(),
        LanguageConfiguration::builder("r", "R")
            .exts(&[".r", ".R", ".rmd"]).line_cmt("#")
            .indent(C_INC, C_DEC).fold_markers(C_FOLD_S, C_FOLD_E).build(),
    ]
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Registry that maps file extensions and names to [`Language`] and
/// [`LanguageConfig`] definitions.
#[derive(Debug, Default)]
pub struct LanguageRegistry {
    by_name: HashMap<String, usize>,
    by_extension: HashMap<String, usize>,
    languages: Vec<Language>,
    by_config_name: HashMap<String, usize>,
    by_config_ext: HashMap<String, usize>,
    configs: Vec<LanguageConfig>,
}

impl LanguageRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, language: Language) {
        let idx = self.languages.len();
        self.by_name.insert(language.name.clone(), idx);
        for ext in &language.file_extensions {
            self.by_extension.insert(ext.clone(), idx);
        }
        self.languages.push(language);
    }

    #[must_use]
    pub fn language_for_extension(&self, ext: &str) -> Option<&Language> {
        self.by_extension.get(ext).map(|&idx| &self.languages[idx])
    }

    #[must_use]
    pub fn language_for_name(&self, name: &str) -> Option<&Language> {
        self.by_name.get(name).map(|&idx| &self.languages[idx])
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.languages.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.languages.is_empty()
    }

    pub fn register_config(&mut self, config: LanguageConfig) {
        let idx = self.configs.len();
        self.by_config_name.insert(config.name.clone(), idx);
        for ext in &config.file_extensions {
            self.by_config_ext.insert(ext.clone(), idx);
        }
        self.configs.push(config);
    }

    #[must_use]
    pub fn config_for_extension(&self, ext: &str) -> Option<&LanguageConfig> {
        self.by_config_ext.get(ext).map(|&i| &self.configs[i])
    }

    #[must_use]
    pub fn config_for_name(&self, name: &str) -> Option<&LanguageConfig> {
        self.by_config_name.get(name).map(|&i| &self.configs[i])
    }

    pub fn register_builtin_configs(&mut self) {
        for config in builtin_language_configs() {
            self.register_config(config);
        }
    }

    #[must_use]
    pub fn configs(&self) -> &[LanguageConfig] {
        &self.configs
    }
}

// ---------------------------------------------------------------------------
// Shared bracket/quote pair constants
// ---------------------------------------------------------------------------

const BQ: &[(&str, &str)] = &[
    ("(", ")"),
    ("[", "]"),
    ("{", "}"),
    ("\"", "\""),
    ("'", "'"),
    ("`", "`"),
];
const SURROUND: &[(&str, &str)] = &[
    ("(", ")"),
    ("[", "]"),
    ("{", "}"),
    ("\"", "\""),
    ("'", "'"),
    ("`", "`"),
];

// C-family shared patterns
const CI: &str = r"^.*(\{[^}]*|\([^)]*|\[[^\]]*)$";
const CO: &str = r"^\s*[\}\]\)]";
const CFS: &str = r"^\s*/\*|^\s*\{";
const CFE: &str = r"^\s*\*/|^\s*\}";
const CW: &str =
    r#"(-?\d*\.\d\w*)|([^\`\~\!\@\#\%\^\&\*\(\)\-\=\+\[\{\]\}\\\|\;\:\'\"\,\.\<\>\/\?\s]+)"#;

/// Returns built-in [`LanguageConfig`]s for the 30+ most common languages.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn builtin_language_configs() -> Vec<LanguageConfig> {
    vec![
        LanguageConfig::new("rust")
            .exts(&[".rs"])
            .line_cmt("//").block_cmt("/*", "*/")
            .auto_close(BQ).surround(SURROUND)
            .indent(CI).outdent(CO).folding(CFS, CFE).word_pat(CW),

        LanguageConfig::new("typescript")
            .exts(&[".ts", ".tsx"])
            .line_cmt("//").block_cmt("/*", "*/")
            .auto_close(BQ).surround(SURROUND)
            .indent(CI).outdent(CO).folding(CFS, CFE).word_pat(CW),

        LanguageConfig::new("javascript")
            .exts(&[".js", ".jsx", ".mjs", ".cjs"])
            .first_line(r"^#!.*\bnode\b")
            .line_cmt("//").block_cmt("/*", "*/")
            .auto_close(BQ).surround(SURROUND)
            .indent(CI).outdent(CO).folding(CFS, CFE).word_pat(CW),

        LanguageConfig::new("python")
            .exts(&[".py", ".pyi", ".pyw"])
            .first_line(r"^#!.*\bpython[23w]?\b")
            .line_cmt("#")
            .auto_close(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'")])
            .surround(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'")])
            .indent(r"^\s*(def|class|for|if|elif|else|while|try|with|finally|except|async)\b.*:\s*$")
            .outdent(r"^\s*(pass|break|continue|raise|return)\b")
            .folding(r"^\s*(def|class|if|elif|else|for|while|try|with)\b", r"^\s*$")
            .word_pat(r"([a-zA-Z_]\w*)"),

        LanguageConfig::new("go")
            .exts(&[".go"])
            .line_cmt("//").block_cmt("/*", "*/")
            .auto_close(BQ).surround(SURROUND)
            .indent(CI).outdent(CO).folding(CFS, CFE).word_pat(CW),

        LanguageConfig::new("c")
            .exts(&[".c", ".h"])
            .line_cmt("//").block_cmt("/*", "*/")
            .auto_close(BQ).surround(SURROUND)
            .indent(CI).outdent(CO)
            .folding(r"^\s*/\*|^\s*\{|^\s*#\s*region", r"^\s*\*/|^\s*\}|^\s*#\s*endregion")
            .word_pat(CW),

        LanguageConfig::new("cpp")
            .exts(&[".cpp", ".hpp", ".cc", ".cxx", ".hxx", ".hh"])
            .line_cmt("//").block_cmt("/*", "*/")
            .auto_close(BQ).surround(SURROUND)
            .indent(CI).outdent(CO)
            .folding(r"^\s*/\*|^\s*\{|^\s*#\s*region", r"^\s*\*/|^\s*\}|^\s*#\s*endregion")
            .word_pat(CW),

        LanguageConfig::new("java")
            .exts(&[".java"])
            .line_cmt("//").block_cmt("/*", "*/")
            .auto_close(BQ).surround(SURROUND)
            .indent(CI).outdent(CO).folding(CFS, CFE).word_pat(CW),

        LanguageConfig::new("csharp")
            .exts(&[".cs"])
            .line_cmt("//").block_cmt("/*", "*/")
            .auto_close(BQ).surround(SURROUND)
            .indent(CI).outdent(CO)
            .folding(r"^\s*/\*|^\s*\{|^\s*#\s*region", r"^\s*\*/|^\s*\}|^\s*#\s*endregion")
            .word_pat(CW),

        LanguageConfig::new("html")
            .exts(&[".html", ".htm", ".xhtml"])
            .auto_close(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'")])
            .surround(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'"), ("<", ">")])
            .indent(r"<(?!(area|base|br|col|embed|hr|img|input|link|meta|param|source|track|wbr)\b)[a-zA-Z][^/]*>")
            .outdent(r"^\s*</[a-zA-Z]")
            .folding(r"<[a-zA-Z]", r"</[a-zA-Z]"),

        LanguageConfig::new("css")
            .exts(&[".css"])
            .block_cmt("/*", "*/")
            .auto_close(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'")])
            .surround(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'")])
            .indent(r"\{[^}]*$").outdent(r"^\s*\}")
            .folding(r"\{", r"\}"),

        LanguageConfig::new("json")
            .exts(&[".json"])
            .auto_close(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\"")])
            .surround(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\"")])
            .indent(r"[\{\[]\s*$").outdent(r"^\s*[\}\]]")
            .folding(r"[\{\[]", r"[\}\]]"),

        LanguageConfig::new("jsonc")
            .exts(&[".jsonc"])
            .line_cmt("//").block_cmt("/*", "*/")
            .auto_close(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\"")])
            .surround(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\"")])
            .indent(r"[\{\[]\s*$").outdent(r"^\s*[\}\]]")
            .folding(r"[\{\[]|/\*", r"[\}\]]|\*/"),

        LanguageConfig::new("markdown")
            .exts(&[".md", ".markdown", ".mdown", ".mkd"])
            .auto_close(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'"), ("`", "`")])
            .surround(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'"), ("`", "`"), ("*", "*"), ("_", "_")])
            .folding(r"^\s*```|^#{1,6}\s", r"^\s*```"),

        LanguageConfig::new("yaml")
            .exts(&[".yml", ".yaml"])
            .line_cmt("#")
            .auto_close(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'")])
            .surround(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'")])
            .indent(r"^\s*[^#].*:\s*$").outdent(r"^\s*$"),

        LanguageConfig::new("toml")
            .exts(&[".toml"])
            .line_cmt("#")
            .auto_close(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'")])
            .surround(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'")])
            .folding(r"^\s*\[", r"^\s*$"),

        LanguageConfig::new("shellscript")
            .exts(&[".sh", ".bash", ".zsh", ".ksh"])
            .first_line(r"^#!.*\b(ba|z|k)?sh\b")
            .line_cmt("#")
            .auto_close(BQ).surround(SURROUND)
            .indent(r"(^\s*(if|elif|else|for|while|until|do|case|then)\b|.*\{\s*$)")
            .outdent(r"^\s*(fi|done|esac)\b|^\s*\}")
            .folding(r"^\s*(if|for|while|until|case)\b|.*\{\s*$", r"^\s*(fi|done|esac)\b|^\s*\}"),

        LanguageConfig::new("sql")
            .exts(&[".sql"])
            .line_cmt("--").block_cmt("/*", "*/")
            .auto_close(&[("(", ")"), ("[", "]"), ("\"", "\""), ("'", "'")])
            .surround(&[("(", ")"), ("[", "]"), ("\"", "\""), ("'", "'")])
            .indent(r"(?i)^\s*(begin|case|create|alter|if|loop|while|for)\b")
            .outdent(r"(?i)^\s*(end|else)\b")
            .folding(r"(?i)^\s*(begin|case)\b", r"(?i)^\s*end\b"),

        LanguageConfig::new("php")
            .exts(&[".php", ".phtml"])
            .first_line(r"<\?php")
            .line_cmt("//").block_cmt("/*", "*/")
            .auto_close(BQ).surround(SURROUND)
            .indent(CI).outdent(CO).folding(CFS, CFE).word_pat(CW),

        LanguageConfig::new("ruby")
            .exts(&[".rb", ".erb", ".rake", ".gemspec"])
            .first_line(r"^#!.*\bruby\b")
            .line_cmt("#")
            .auto_close(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'"), ("|", "|")])
            .surround(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'")])
            .indent(r"^\s*(def|class|module|if|elsif|else|unless|case|when|while|until|for|begin|do)\b")
            .outdent(r"^\s*(end|else|elsif|when|rescue|ensure)\b")
            .folding(r"^\s*(def|class|module|if|unless|case|while|until|for|begin|do)\b", r"^\s*end\b"),

        LanguageConfig::new("swift")
            .exts(&[".swift"])
            .line_cmt("//").block_cmt("/*", "*/")
            .auto_close(BQ).surround(SURROUND)
            .indent(CI).outdent(CO).folding(CFS, CFE).word_pat(CW),

        LanguageConfig::new("kotlin")
            .exts(&[".kt", ".kts"])
            .line_cmt("//").block_cmt("/*", "*/")
            .auto_close(BQ).surround(SURROUND)
            .indent(CI).outdent(CO).folding(CFS, CFE).word_pat(CW),

        LanguageConfig::new("scala")
            .exts(&[".scala", ".sc"])
            .line_cmt("//").block_cmt("/*", "*/")
            .auto_close(BQ).surround(SURROUND)
            .indent(CI).outdent(CO).folding(CFS, CFE).word_pat(CW),

        LanguageConfig::new("scss")
            .exts(&[".scss"])
            .line_cmt("//").block_cmt("/*", "*/")
            .auto_close(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'")])
            .surround(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'")])
            .indent(r"\{[^}]*$").outdent(r"^\s*\}")
            .folding(r"\{", r"\}"),

        LanguageConfig::new("less")
            .exts(&[".less"])
            .line_cmt("//").block_cmt("/*", "*/")
            .auto_close(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'")])
            .surround(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'")])
            .indent(r"\{[^}]*$").outdent(r"^\s*\}")
            .folding(r"\{", r"\}"),

        LanguageConfig::new("xml")
            .exts(&[".xml", ".xsd", ".xsl", ".xslt", ".svg"])
            .block_cmt("<!--", "-->")
            .auto_close(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'"), ("<", ">")])
            .surround(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'"), ("<", ">")])
            .indent(r"<(?![\?!/])[a-zA-Z][^/]*[^/]>").outdent(r"^\s*</[a-zA-Z]")
            .folding(r"<[a-zA-Z]", r"</[a-zA-Z]"),

        LanguageConfig::new("powershell")
            .exts(&[".ps1", ".psm1", ".psd1"])
            .line_cmt("#").block_cmt("<#", "#>")
            .auto_close(BQ).surround(SURROUND)
            .indent(CI).outdent(CO).folding(CFS, CFE),

        LanguageConfig::new("dockerfile")
            .exts(&[".dockerfile"])
            .line_cmt("#")
            .auto_close(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'")])
            .surround(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'")]),

        LanguageConfig::new("makefile")
            .exts(&[".mk", ".mak"])
            .line_cmt("#")
            .auto_close(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'")])
            .surround(&[("(", ")"), ("[", "]"), ("{", "}"), ("\"", "\""), ("'", "'")])
            .indent(r"^[^\t].*:\s*$").outdent(r"^\S"),

        LanguageConfig::new("lua")
            .exts(&[".lua"])
            .line_cmt("--").block_cmt("--[[", "]]")
            .auto_close(BQ).surround(SURROUND)
            .indent(r"^\s*(function|if|for|while|repeat|else|elseif|do)\b")
            .outdent(r"^\s*(end|else|elseif|until)\b")
            .folding(r"^\s*(function|if|for|while|repeat|do)\b", r"^\s*end\b"),

        LanguageConfig::new("r")
            .exts(&[".r", ".R", ".rmd"])
            .line_cmt("#")
            .auto_close(BQ).surround(SURROUND)
            .indent(CI).outdent(CO).folding(CFS, CFE),
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rust_language() -> Language {
        Language {
            name: "rust".into(),
            ts_language: tree_sitter_rust::LANGUAGE.into(),
            highlight_query: None,
            file_extensions: vec![".rs".into()],
            line_comment: Some("//".into()),
            block_comment: Some(("/*".into(), "*/".into())),
            injection_query: None,
            auto_closing_pairs: vec![
                ("(".into(), ")".into()),
                ("{".into(), "}".into()),
                ("[".into(), "]".into()),
                ("\"".into(), "\"".into()),
            ],
            surrounding_pairs: vec![
                ("(".into(), ")".into()),
                ("{".into(), "}".into()),
                ("[".into(), "]".into()),
                ("\"".into(), "\"".into()),
            ],
            indent_rules: crate::indent::default_indent_rules(),
            word_pattern: None,
            on_enter_rules: crate::indent::default_on_enter_rules(),
            folding_rules: None,
        }
    }

    #[test]
    fn register_and_lookup_by_name() {
        let mut registry = LanguageRegistry::new();
        registry.register(make_rust_language());
        let lang = registry.language_for_name("rust").unwrap();
        assert_eq!(lang.name, "rust");
    }

    #[test]
    fn lookup_by_extension() {
        let mut registry = LanguageRegistry::new();
        registry.register(make_rust_language());
        let lang = registry.language_for_extension(".rs").unwrap();
        assert_eq!(lang.name, "rust");
    }

    #[test]
    fn lookup_missing_returns_none() {
        let registry = LanguageRegistry::new();
        assert!(registry.language_for_name("rust").is_none());
        assert!(registry.language_for_extension(".rs").is_none());
    }

    #[test]
    fn len_and_is_empty() {
        let mut registry = LanguageRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        registry.register(make_rust_language());
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn multiple_extensions() {
        let mut registry = LanguageRegistry::new();
        let lang = Language {
            name: "typescript".into(),
            ts_language: tree_sitter_rust::LANGUAGE.into(),
            highlight_query: None,
            injection_query: None,
            file_extensions: vec![".ts".into(), ".tsx".into()],
            line_comment: Some("//".into()),
            block_comment: Some(("/*".into(), "*/".into())),
            auto_closing_pairs: vec![],
            surrounding_pairs: vec![],
            indent_rules: vec![],
            word_pattern: None,
            on_enter_rules: vec![],
            folding_rules: None,
        };
        registry.register(lang);
        assert!(registry.language_for_extension(".ts").is_some());
        assert!(registry.language_for_extension(".tsx").is_some());
        assert_eq!(
            registry.language_for_extension(".ts").unwrap().name,
            "typescript"
        );
    }

    #[test]
    fn debug_impl() {
        let lang = make_rust_language();
        let dbg = format!("{lang:?}");
        assert!(dbg.contains("rust"));
    }

    // -- LanguageConfig tests --

    #[test]
    fn builtin_configs_count() {
        let configs = builtin_language_configs();
        assert!(
            configs.len() >= 30,
            "should have at least 30 built-in configs, got {}",
            configs.len()
        );
    }

    #[test]
    fn builtin_configs_have_names_and_exts() {
        for cfg in builtin_language_configs() {
            assert!(!cfg.name.is_empty());
            assert!(
                !cfg.file_extensions.is_empty(),
                "{} has no extensions",
                cfg.name
            );
        }
    }

    #[test]
    fn builtin_config_rust() {
        let configs = builtin_language_configs();
        let rust = configs.iter().find(|c| c.name == "rust").unwrap();
        assert!(rust.file_extensions.contains(&".rs".to_string()));
        assert_eq!(rust.line_comment.as_deref(), Some("//"));
        assert!(rust.block_comment.is_some());
        assert!(!rust.auto_closing_pairs.is_empty());
        assert!(rust.indent_pattern.is_some());
        assert!(rust.outdent_pattern.is_some());
    }

    #[test]
    fn builtin_config_python() {
        let configs = builtin_language_configs();
        let py = configs.iter().find(|c| c.name == "python").unwrap();
        assert!(py.file_extensions.contains(&".py".to_string()));
        assert_eq!(py.line_comment.as_deref(), Some("#"));
        assert!(py.block_comment.is_none());
        assert!(py.first_line_pattern.is_some());
    }

    #[test]
    fn builtin_config_html_no_line_comment() {
        let configs = builtin_language_configs();
        let html = configs.iter().find(|c| c.name == "html").unwrap();
        assert!(html.line_comment.is_none());
    }

    #[test]
    fn register_and_lookup_config() {
        let mut registry = LanguageRegistry::new();
        registry.register_builtin_configs();
        assert!(registry.config_for_name("rust").is_some());
        assert!(registry.config_for_extension(".py").is_some());
        assert!(registry.config_for_name("nonexistent").is_none());
    }

    #[test]
    fn builtin_unique_names() {
        let configs = builtin_language_configs();
        let mut names: Vec<&str> = configs.iter().map(|c| c.name.as_str()).collect();
        names.sort();
        names.dedup();
        assert_eq!(
            names.len(),
            configs.len(),
            "all config names must be unique"
        );
    }

    #[test]
    fn enriched_configs_count() {
        let configs = builtin_language_configurations();
        assert!(
            configs.len() >= 30,
            "should have at least 30 enriched configs, got {}",
            configs.len()
        );
    }

    #[test]
    fn enriched_configs_have_ids_and_exts() {
        for cfg in builtin_language_configurations() {
            assert!(!cfg.id.is_empty());
            assert!(
                !cfg.extensions.is_empty() || !cfg.filenames.is_empty(),
                "{} has no extensions or filenames", cfg.id,
            );
        }
    }
}
