//! JSON editing features — schema validation, path resolution, formatting,
//! and built-in schema associations for common config files.

use std::fmt;

/// Diagnostic severity for JSON validation issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

impl fmt::Display for DiagnosticSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
            Self::Info => write!(f, "info"),
            Self::Hint => write!(f, "hint"),
        }
    }
}

/// A single diagnostic produced by JSON validation.
#[derive(Debug, Clone)]
pub struct JsonDiagnostic {
    pub line: u32,
    pub column: u32,
    pub message: String,
    pub severity: DiagnosticSeverity,
}

/// Maps a filename glob pattern to a JSON schema URL.
#[derive(Debug, Clone)]
pub struct SchemaAssociation {
    pub file_pattern: String,
    pub schema_url: String,
}

/// Top-level JSON editing feature state.
#[derive(Debug, Clone)]
pub struct JsonFeatures {
    pub schema_validation: bool,
    associations: Vec<SchemaAssociation>,
}

impl Default for JsonFeatures {
    fn default() -> Self {
        Self {
            schema_validation: true,
            associations: built_in_associations(),
        }
    }
}

const SCHEMA_STORE: &str = "https://json.schemastore.org";

fn built_in_associations() -> Vec<SchemaAssociation> {
    [
        ("package.json", "package.json"),
        ("tsconfig.json", "tsconfig.json"),
        ("tsconfig.*.json", "tsconfig.json"),
        ("jsconfig.json", "jsconfig.json"),
        (".eslintrc.json", "eslintrc.json"),
        (".prettierrc", "prettierrc.json"),
        (".prettierrc.json", "prettierrc.json"),
        ("launch.json", "launch.json"),
        ("tasks.json", "tasks.json"),
        ("settings.json", "vscode-settings.json"),
        (".babelrc", "babelrc.json"),
        ("nest-cli.json", "nest-cli.json"),
        ("deno.json", "deno.json"),
        ("composer.json", "composer.json"),
        ("turbo.json", "turborepo.json"),
    ]
    .into_iter()
    .map(|(pat, schema)| SchemaAssociation {
        file_pattern: pat.to_owned(),
        schema_url: format!("{SCHEMA_STORE}/{schema}"),
    })
    .collect()
}

/// Returns the schema URL for a given filename, matching against built-in
/// associations. Glob patterns use a simple trailing-wildcard check.
#[must_use]
pub fn get_schema_for_file(filename: &str) -> Option<&'static str> {
    static PAIRS: &[(&str, &str)] = &[
        ("package.json", "https://json.schemastore.org/package.json"),
        ("tsconfig.json", "https://json.schemastore.org/tsconfig.json"),
        ("jsconfig.json", "https://json.schemastore.org/jsconfig.json"),
        (".eslintrc.json", "https://json.schemastore.org/eslintrc.json"),
        (".prettierrc", "https://json.schemastore.org/prettierrc.json"),
        (".prettierrc.json", "https://json.schemastore.org/prettierrc.json"),
        ("launch.json", "https://json.schemastore.org/launch.json"),
        ("tasks.json", "https://json.schemastore.org/tasks.json"),
        ("settings.json", "https://json.schemastore.org/vscode-settings.json"),
        (".babelrc", "https://json.schemastore.org/babelrc.json"),
        ("nest-cli.json", "https://json.schemastore.org/nest-cli.json"),
        ("deno.json", "https://json.schemastore.org/deno.json"),
        ("composer.json", "https://json.schemastore.org/composer.json"),
        ("turbo.json", "https://json.schemastore.org/turborepo.json"),
    ];
    let base = filename.rsplit('/').next().unwrap_or(filename);
    PAIRS
        .iter()
        .find(|(pat, _)| pat == &base)
        .map(|(_, url)| *url)
        .or_else(|| {
            // tsconfig.*.json
            if base.starts_with("tsconfig.") && base.ends_with(".json") {
                Some("https://json.schemastore.org/tsconfig.json")
            } else {
                None
            }
        })
}

impl JsonFeatures {
    pub fn new(schema_validation: bool) -> Self {
        Self {
            schema_validation,
            ..Default::default()
        }
    }

    /// Adds a custom schema association.
    pub fn add_association(&mut self, pattern: String, schema_url: String) {
        self.associations.push(SchemaAssociation {
            file_pattern: pattern,
            schema_url,
        });
    }

    /// Finds a schema URL in the instance association list.
    #[must_use]
    pub fn schema_for(&self, filename: &str) -> Option<&str> {
        let base = filename.rsplit('/').next().unwrap_or(filename);
        self.associations
            .iter()
            .find(|a| {
                if let Some(prefix) = a.file_pattern.strip_suffix("*.json") {
                    base.starts_with(prefix) && base.ends_with(".json")
                } else {
                    a.file_pattern == base
                }
            })
            .map(|a| a.schema_url.as_str())
    }
}

/// Basic JSON schema validation. Parses both `content` and `schema` as JSON
/// and checks top-level required/type constraints. This is intentionally
/// lightweight; full JSON Schema Draft-07 support would come from a dedicated
/// crate.
#[must_use]
pub fn validate_json_schema(content: &str, schema: &str) -> Vec<JsonDiagnostic> {
    let mut diagnostics = Vec::new();

    let doc: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(e) => {
            diagnostics.push(JsonDiagnostic {
                line: e.line() as u32,
                column: e.column() as u32,
                message: format!("Parse error: {e}"),
                severity: DiagnosticSeverity::Error,
            });
            return diagnostics;
        }
    };

    let schema_val: serde_json::Value = match serde_json::from_str(schema) {
        Ok(v) => v,
        Err(_) => return diagnostics,
    };

    if let Some(expected_type) = schema_val.get("type").and_then(|t| t.as_str()) {
        let actual_ok = match expected_type {
            "object" => doc.is_object(),
            "array" => doc.is_array(),
            "string" => doc.is_string(),
            "number" | "integer" => doc.is_number(),
            "boolean" => doc.is_boolean(),
            "null" => doc.is_null(),
            _ => true,
        };
        if !actual_ok {
            diagnostics.push(JsonDiagnostic {
                line: 1,
                column: 1,
                message: format!("Expected top-level type \"{expected_type}\""),
                severity: DiagnosticSeverity::Error,
            });
        }
    }

    if let Some(required) = schema_val.get("required").and_then(|r| r.as_array()) {
        if let Some(obj) = doc.as_object() {
            for req in required.iter().filter_map(|v| v.as_str()) {
                if !obj.contains_key(req) {
                    diagnostics.push(JsonDiagnostic {
                        line: 1,
                        column: 1,
                        message: format!("Missing required property \"{req}\""),
                        severity: DiagnosticSeverity::Error,
                    });
                }
            }
        }
    }

    diagnostics
}

/// Returns the JSON-pointer-style path at the given byte `offset` in
/// `content`. For example, `dependencies.serde.version` for an offset
/// inside that nested value.
#[must_use]
pub fn get_json_path(content: &str, offset: usize) -> String {
    let prefix = &content[..offset.min(content.len())];
    let mut segments: Vec<String> = Vec::new();
    let mut depth_stack: Vec<(bool, Option<String>)> = Vec::new(); // (is_array, pending_key)

    let mut chars = prefix.chars().peekable();
    let mut current_key: Option<String> = None;

    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                if let Some(key) = current_key.take() {
                    segments.push(key);
                }
                depth_stack.push((false, None));
            }
            '[' => {
                if let Some(key) = current_key.take() {
                    segments.push(key);
                }
                depth_stack.push((true, None));
                segments.push("0".to_owned());
            }
            '}' => {
                segments.pop();
                depth_stack.pop();
            }
            ']' => {
                segments.pop(); // array index
                segments.pop(); // array key (if any)
                depth_stack.pop();
            }
            '"' => {
                let mut s = String::new();
                loop {
                    match chars.next() {
                        Some('\\') => {
                            if let Some(esc) = chars.next() {
                                s.push(esc);
                            }
                        }
                        Some('"') | None => break,
                        Some(c) => s.push(c),
                    }
                }
                current_key = Some(s);
            }
            ':' => {
                if let Some(key) = current_key.take() {
                    if let Some(last) = depth_stack.last_mut() {
                        last.1 = Some(key.clone());
                    }
                    segments.push(key);
                }
            }
            ',' => {
                if let Some((is_array, _)) = depth_stack.last() {
                    if *is_array {
                        if let Some(idx) = segments.last_mut() {
                            if let Ok(n) = idx.parse::<usize>() {
                                *idx = (n + 1).to_string();
                            }
                        }
                    } else {
                        segments.pop();
                    }
                }
                current_key = None;
            }
            _ => {}
        }
    }
    segments.join(".")
}

/// Pretty-prints JSON with the given indent width.
pub fn format_json(content: &str, indent: u32) -> Result<String, serde_json::Error> {
    let val: serde_json::Value = serde_json::from_str(content)?;
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(
        &b"                "[..indent as usize],
    );
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    serde::Serialize::serialize(&val, &mut ser)?;
    Ok(String::from_utf8(buf).expect("serde_json always produces valid UTF-8"))
}

/// Minifies JSON by removing all unnecessary whitespace.
pub fn minify_json(content: &str) -> Result<String, serde_json::Error> {
    let val: serde_json::Value = serde_json::from_str(content)?;
    serde_json::to_string(&val)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_and_minify() {
        let src = r#"{"a":1,"b":[2,3]}"#;
        let pretty = format_json(src, 2).unwrap();
        assert!(pretty.contains('\n'));
        let mini = minify_json(&pretty).unwrap();
        assert_eq!(mini, src);
    }

    #[test]
    fn schema_lookup() {
        assert!(get_schema_for_file("package.json").is_some());
        assert!(get_schema_for_file("tsconfig.build.json").is_some());
        assert!(get_schema_for_file("random.txt").is_none());
    }

    #[test]
    fn validate_missing_required() {
        let schema = r#"{"type":"object","required":["name"]}"#;
        let doc = r#"{"version":"1.0"}"#;
        let diags = validate_json_schema(doc, schema);
        assert!(diags.iter().any(|d| d.message.contains("name")));
    }

    #[test]
    fn validate_parse_error() {
        let diags = validate_json_schema("{bad", "{}");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, DiagnosticSeverity::Error);
    }

    #[test]
    fn json_path_simple() {
        let src = r#"{"dependencies":{"serde":{"version":"1.0"}}}"#;
        let offset = src.find("1.0").unwrap();
        let path = get_json_path(src, offset);
        assert_eq!(path, "dependencies.serde.version");
    }

    #[test]
    fn instance_schema_for() {
        let feats = JsonFeatures::default();
        assert!(feats.schema_for("package.json").is_some());
        assert!(feats.schema_for("tsconfig.app.json").is_some());
        assert!(feats.schema_for("foo.txt").is_none());
    }
}
