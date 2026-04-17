//! Snippet library — manages snippet collections per language.
//!
//! Loads snippet definitions from VS Code-compatible JSON, provides prefix
//! matching and built-in snippets for common languages.

use serde::{Deserialize, Serialize};

/// A snippet definition matching VS Code's snippet JSON format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetDefinition {
    /// Trigger prefixes (e.g. `["for", "fori"]`).
    pub prefix: Vec<String>,
    /// Template body lines.
    pub body: Vec<String>,
    /// Human-readable description.
    #[serde(default)]
    pub description: String,
    /// Optional language scope restriction (e.g. `"rust,python"`).
    #[serde(default)]
    pub scope: Option<String>,
}

/// Manages snippet collections keyed by language identifier.
#[derive(Debug, Default)]
pub struct SnippetLibrary {
    snippets: Vec<(String, SnippetDefinition)>,
}

impl SnippetLibrary {
    /// Create an empty library.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a library pre-populated with built-in snippets.
    #[must_use]
    pub fn with_builtins() -> Self {
        let mut lib = Self::new();
        lib.load_builtins();
        lib
    }

    /// Load snippets from VS Code-compatible JSON for a given language.
    ///
    /// The JSON format is `{ "Snippet Name": { "prefix": ..., "body": ..., "description": ... } }`.
    pub fn load_from_json(&mut self, json: &str, language: &str) -> Result<usize, String> {
        let defs = load_snippets_from_json(json, language)?;
        let count = defs.len();
        for def in defs {
            self.snippets.push((language.to_string(), def));
        }
        Ok(count)
    }

    /// Add a single snippet definition for a language.
    pub fn add(&mut self, language: &str, def: SnippetDefinition) {
        self.snippets.push((language.to_string(), def));
    }

    /// Get all snippets for a given language.
    #[must_use]
    pub fn get_snippets(&self, language: &str) -> Vec<&SnippetDefinition> {
        self.snippets
            .iter()
            .filter(|(lang, _)| lang == language)
            .map(|(_, def)| def)
            .collect()
    }

    /// Find snippets whose prefix starts with the given text.
    #[must_use]
    pub fn match_prefix(&self, language: &str, prefix: &str) -> Vec<&SnippetDefinition> {
        self.snippets
            .iter()
            .filter(|(lang, def)| {
                lang == language
                    && def
                        .prefix
                        .iter()
                        .any(|p| p.starts_with(prefix) || prefix.starts_with(p.as_str()))
            })
            .map(|(_, def)| def)
            .collect()
    }

    /// Return total snippet count across all languages.
    #[must_use]
    pub fn len(&self) -> usize {
        self.snippets.len()
    }

    /// Whether the library has no snippets.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.snippets.is_empty()
    }

    fn load_builtins(&mut self) {
        self.load_language_builtins("rust", &RUST_SNIPPETS);
        self.load_language_builtins("python", &PYTHON_SNIPPETS);
        self.load_language_builtins("javascript", &JS_SNIPPETS);
        self.load_language_builtins("typescript", &TS_SNIPPETS);
        self.load_language_builtins("go", &GO_SNIPPETS);
        self.load_language_builtins("c", &C_SNIPPETS);
        self.load_language_builtins("cpp", &CPP_SNIPPETS);
    }

    fn load_language_builtins(&mut self, language: &str, snippets: &[(&str, &[&str], &str)]) {
        for &(prefix, body, description) in snippets {
            self.snippets.push((
                language.to_string(),
                SnippetDefinition {
                    prefix: vec![prefix.to_string()],
                    body: body.iter().map(|s| (*s).to_string()).collect(),
                    description: description.to_string(),
                    scope: None,
                },
            ));
        }
    }
}

/// Parse VS Code-style snippet JSON and return definitions tagged with a language.
pub fn load_snippets_from_json(json: &str, language: &str) -> Result<Vec<SnippetDefinition>, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;

    let obj = value
        .as_object()
        .ok_or_else(|| "expected top-level object".to_string())?;

    let mut defs = Vec::new();

    for (_name, entry) in obj {
        let prefix = match entry.get("prefix") {
            Some(serde_json::Value::String(s)) => vec![s.clone()],
            Some(serde_json::Value::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            _ => continue,
        };

        let body = match entry.get("body") {
            Some(serde_json::Value::String(s)) => vec![s.clone()],
            Some(serde_json::Value::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            _ => continue,
        };

        let description = entry
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let scope = entry
            .get("scope")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| Some(language.to_string()));

        defs.push(SnippetDefinition {
            prefix,
            body,
            description,
            scope,
        });
    }

    Ok(defs)
}

// ── Built-in snippets ────────────────────────────────────────────────────────
// (prefix, body_lines, description)

const RUST_SNIPPETS: &[(&str, &[&str], &str)] = &[
    ("fn", &["fn ${1:name}(${2:params}) ${3:-> ${4:ReturnType} }{", "\t$0", "}"], "Function definition"),
    ("for", &["for ${1:item} in ${2:iter} {", "\t$0", "}"], "For loop"),
    ("if", &["if ${1:condition} {", "\t$0", "}"], "If statement"),
    ("ifl", &["if let ${1:Some(val)} = ${2:option} {", "\t$0", "}"], "If let"),
    ("while", &["while ${1:condition} {", "\t$0", "}"], "While loop"),
    ("match", &["match ${1:expr} {", "\t${2:pattern} => ${3:value},", "\t$0", "}"], "Match expression"),
    ("impl", &["impl ${1:Type} {", "\t$0", "}"], "Impl block"),
    ("struct", &["struct ${1:Name} {", "\t${2:field}: ${3:Type},", "}"], "Struct definition"),
    ("enum", &["enum ${1:Name} {", "\t${2:Variant},", "}"], "Enum definition"),
    ("test", &["#[test]", "fn ${1:test_name}() {", "\t$0", "}"], "Test function"),
];

const PYTHON_SNIPPETS: &[(&str, &[&str], &str)] = &[
    ("def", &["def ${1:name}(${2:params}):", "\t${0:pass}"], "Function definition"),
    ("for", &["for ${1:item} in ${2:iterable}:", "\t${0:pass}"], "For loop"),
    ("if", &["if ${1:condition}:", "\t${0:pass}"], "If statement"),
    ("while", &["while ${1:condition}:", "\t${0:pass}"], "While loop"),
    ("class", &["class ${1:Name}:", "\tdef __init__(self${2:, params}):", "\t\t${0:pass}"], "Class definition"),
    ("with", &["with ${1:expr} as ${2:var}:", "\t${0:pass}"], "With statement"),
    ("try", &["try:", "\t${1:pass}", "except ${2:Exception} as ${3:e}:", "\t${0:pass}"], "Try/except"),
    ("main", &["if __name__ == \"__main__\":", "\t${0:main()}"], "Main guard"),
];

const JS_SNIPPETS: &[(&str, &[&str], &str)] = &[
    ("fn", &["function ${1:name}(${2:params}) {", "\t$0", "}"], "Function declaration"),
    ("af", &["const ${1:name} = (${2:params}) => {", "\t$0", "};"], "Arrow function"),
    ("for", &["for (let ${1:i} = 0; ${1:i} < ${2:length}; ${1:i}++) {", "\t$0", "}"], "For loop"),
    ("forof", &["for (const ${1:item} of ${2:iterable}) {", "\t$0", "}"], "For...of loop"),
    ("if", &["if (${1:condition}) {", "\t$0", "}"], "If statement"),
    ("while", &["while (${1:condition}) {", "\t$0", "}"], "While loop"),
    ("try", &["try {", "\t$0", "} catch (${1:error}) {", "\t", "}"], "Try/catch"),
    ("cl", &["console.log($0);"], "Console log"),
];

const TS_SNIPPETS: &[(&str, &[&str], &str)] = &[
    ("fn", &["function ${1:name}(${2:params}): ${3:void} {", "\t$0", "}"], "Function declaration"),
    ("af", &["const ${1:name} = (${2:params}): ${3:void} => {", "\t$0", "};"], "Arrow function"),
    ("for", &["for (let ${1:i} = 0; ${1:i} < ${2:length}; ${1:i}++) {", "\t$0", "}"], "For loop"),
    ("if", &["if (${1:condition}) {", "\t$0", "}"], "If statement"),
    ("while", &["while (${1:condition}) {", "\t$0", "}"], "While loop"),
    ("interface", &["interface ${1:Name} {", "\t${2:property}: ${3:type};", "}"], "Interface"),
    ("type", &["type ${1:Name} = ${0:type};"], "Type alias"),
    ("class", &["class ${1:Name} {", "\tconstructor(${2:params}) {", "\t\t$0", "\t}", "}"], "Class"),
];

const GO_SNIPPETS: &[(&str, &[&str], &str)] = &[
    ("fn", &["func ${1:name}(${2:params}) ${3:returnType} {", "\t$0", "}"], "Function"),
    ("for", &["for ${1:i} := 0; ${1:i} < ${2:n}; ${1:i}++ {", "\t$0", "}"], "For loop"),
    ("forr", &["for ${1:key}, ${2:value} := range ${3:collection} {", "\t$0", "}"], "For range"),
    ("if", &["if ${1:condition} {", "\t$0", "}"], "If statement"),
    ("ife", &["if err != nil {", "\t${0:return err}", "}"], "If error"),
    ("struct", &["type ${1:Name} struct {", "\t${2:Field} ${3:Type}", "}"], "Struct"),
    ("interface", &["type ${1:Name} interface {", "\t${2:Method}(${3:params}) ${4:returnType}", "}"], "Interface"),
];

const C_SNIPPETS: &[(&str, &[&str], &str)] = &[
    ("fn", &["${1:void} ${2:name}(${3:params}) {", "\t$0", "}"], "Function"),
    ("for", &["for (int ${1:i} = 0; ${1:i} < ${2:n}; ${1:i}++) {", "\t$0", "}"], "For loop"),
    ("if", &["if (${1:condition}) {", "\t$0", "}"], "If statement"),
    ("while", &["while (${1:condition}) {", "\t$0", "}"], "While loop"),
    ("main", &["int main(int argc, char *argv[]) {", "\t$0", "\treturn 0;", "}"], "Main function"),
    ("struct", &["typedef struct {", "\t${1:member};", "} ${2:Name};"], "Struct typedef"),
];

const CPP_SNIPPETS: &[(&str, &[&str], &str)] = &[
    ("fn", &["${1:void} ${2:name}(${3:params}) {", "\t$0", "}"], "Function"),
    ("for", &["for (int ${1:i} = 0; ${1:i} < ${2:n}; ++${1:i}) {", "\t$0", "}"], "For loop"),
    ("forr", &["for (const auto& ${1:item} : ${2:container}) {", "\t$0", "}"], "Range-based for"),
    ("if", &["if (${1:condition}) {", "\t$0", "}"], "If statement"),
    ("while", &["while (${1:condition}) {", "\t$0", "}"], "While loop"),
    ("class", &["class ${1:Name} {", "public:", "\t${1:Name}(${2:params});", "\t~${1:Name}();", "private:", "\t$0", "};"], "Class"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_json_snippets() {
        let json = r#"{
            "For Loop": {
                "prefix": ["for", "fori"],
                "body": ["for i in 0..${1:n} {", "\t$0", "}"],
                "description": "For loop"
            }
        }"#;
        let defs = load_snippets_from_json(json, "rust").unwrap();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].prefix, vec!["for", "fori"]);
        assert_eq!(defs[0].body.len(), 3);
        assert_eq!(defs[0].description, "For loop");
    }

    #[test]
    fn load_json_string_prefix() {
        let json = r#"{
            "Log": {
                "prefix": "log",
                "body": "console.log($0);",
                "description": "Log"
            }
        }"#;
        let defs = load_snippets_from_json(json, "javascript").unwrap();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].prefix, vec!["log"]);
    }

    #[test]
    fn builtin_snippets() {
        let lib = SnippetLibrary::with_builtins();
        assert!(!lib.is_empty());

        let rust_snippets = lib.get_snippets("rust");
        assert!(!rust_snippets.is_empty());

        let py_snippets = lib.get_snippets("python");
        assert!(!py_snippets.is_empty());
    }

    #[test]
    fn match_prefix_works() {
        let lib = SnippetLibrary::with_builtins();
        let matches = lib.match_prefix("rust", "fo");
        assert!(matches.iter().any(|s| s.prefix.contains(&"for".to_string())));
    }

    #[test]
    fn add_and_retrieve() {
        let mut lib = SnippetLibrary::new();
        lib.add(
            "ruby",
            SnippetDefinition {
                prefix: vec!["def".into()],
                body: vec!["def ${1:name}".into(), "\t$0".into(), "end".into()],
                description: "Method".into(),
                scope: None,
            },
        );
        assert_eq!(lib.get_snippets("ruby").len(), 1);
        assert_eq!(lib.get_snippets("python").len(), 0);
    }

    #[test]
    fn load_from_json_method() {
        let mut lib = SnippetLibrary::new();
        let json = r#"{ "Test": { "prefix": "tt", "body": "test", "description": "t" } }"#;
        let count = lib.load_from_json(json, "lua").unwrap();
        assert_eq!(count, 1);
        assert_eq!(lib.get_snippets("lua").len(), 1);
    }
}
