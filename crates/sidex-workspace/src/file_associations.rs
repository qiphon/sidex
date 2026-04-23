//! File associations — maps extensions, filenames, and first-line patterns to languages and icons.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// A language association for a file type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageAssociation {
    pub language_id: String,
    pub extensions: Vec<String>,
    pub filenames: Vec<String>,
    pub first_line_patterns: Vec<String>,
    pub mime_type: String,
}

/// Maps file extensions/names to languages and icons.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAssociations {
    pub language_map: HashMap<String, LanguageAssociation>,
    pub icon_map: HashMap<String, String>,
}

impl Default for FileAssociations {
    fn default() -> Self {
        Self::builtin()
    }
}

impl FileAssociations {
    /// Build the default set of built-in associations.
    pub fn builtin() -> Self {
        let mut fa = Self {
            language_map: HashMap::new(),
            icon_map: HashMap::new(),
        };
        fa.register_builtins();
        fa
    }

    /// Detect the language for a given file path.
    pub fn detect_language(&self, path: &Path) -> Option<&LanguageAssociation> {
        let filename = path.file_name()?.to_str()?;
        let ext = path.extension().and_then(|e| e.to_str());

        // Exact filename match first.
        for assoc in self.language_map.values() {
            if assoc.filenames.iter().any(|f| f == filename) {
                return Some(assoc);
            }
        }

        // Extension match.
        if let Some(ext) = ext {
            let dot_ext = format!(".{ext}");
            for assoc in self.language_map.values() {
                if assoc.extensions.iter().any(|e| e == &dot_ext) {
                    return Some(assoc);
                }
            }
        }

        None
    }

    /// Detect language from the first line of file content (shebang, etc.).
    pub fn detect_from_first_line(&self, first_line: &str) -> Option<&LanguageAssociation> {
        for assoc in self.language_map.values() {
            for pattern in &assoc.first_line_patterns {
                if let Ok(re) = regex::Regex::new(pattern) {
                    if re.is_match(first_line) {
                        return Some(assoc);
                    }
                }
            }
        }
        None
    }

    /// Get the icon name for a file path.
    pub fn get_icon(&self, path: &Path) -> Option<&String> {
        let filename = path.file_name()?.to_str()?;
        if let Some(icon) = self.icon_map.get(filename) {
            return Some(icon);
        }
        let ext = path.extension()?.to_str()?;
        self.icon_map.get(ext)
    }

    /// Register a custom language association.
    pub fn register(&mut self, assoc: LanguageAssociation) {
        self.language_map.insert(assoc.language_id.clone(), assoc);
    }

    fn register_builtins(&mut self) {
        let entries = builtin_associations();
        for assoc in entries {
            self.language_map.insert(assoc.language_id.clone(), assoc);
        }

        let icons = builtin_icon_map();
        for (key, icon) in icons {
            self.icon_map.insert(key, icon);
        }
    }
}

fn lang(
    id: &str,
    extensions: &[&str],
    filenames: &[&str],
    first_line: &[&str],
    mime: &str,
) -> LanguageAssociation {
    LanguageAssociation {
        language_id: id.to_string(),
        extensions: extensions.iter().map(|s| (*s).to_string()).collect(),
        filenames: filenames.iter().map(|s| (*s).to_string()).collect(),
        first_line_patterns: first_line.iter().map(|s| (*s).to_string()).collect(),
        mime_type: mime.to_string(),
    }
}

#[allow(clippy::too_many_lines)]
fn builtin_associations() -> Vec<LanguageAssociation> {
    vec![
        lang("rust", &[".rs"], &[], &[], "text/x-rust"),
        lang(
            "typescript",
            &[".ts", ".mts", ".cts"],
            &[],
            &[],
            "text/typescript",
        ),
        lang("typescriptreact", &[".tsx"], &[], &[], "text/tsx"),
        lang(
            "javascript",
            &[".js", ".mjs", ".cjs"],
            &[],
            &["^#!.*\\bnode\\b"],
            "text/javascript",
        ),
        lang("javascriptreact", &[".jsx"], &[], &[], "text/jsx"),
        lang(
            "python",
            &[".py", ".pyw", ".pyi"],
            &[],
            &["^#!.*\\bpython[23]?\\b"],
            "text/x-python",
        ),
        lang("go", &[".go"], &[], &[], "text/x-go"),
        lang("java", &[".java"], &[], &[], "text/x-java"),
        lang("c", &[".c"], &[], &[], "text/x-c"),
        lang(
            "cpp",
            &[".cpp", ".cc", ".cxx", ".c++"],
            &[],
            &[],
            "text/x-c++",
        ),
        lang(
            "cheader",
            &[".h", ".hh", ".hpp", ".hxx"],
            &[],
            &[],
            "text/x-c-header",
        ),
        lang("csharp", &[".cs", ".csx"], &[], &[], "text/x-csharp"),
        lang(
            "ruby",
            &[".rb", ".gemspec"],
            &["Gemfile", "Rakefile"],
            &["^#!.*\\bruby\\b"],
            "text/x-ruby",
        ),
        lang("php", &[".php", ".phtml"], &[], &["^<\\?php"], "text/x-php"),
        lang("swift", &[".swift"], &[], &[], "text/x-swift"),
        lang("kotlin", &[".kt", ".kts"], &[], &[], "text/x-kotlin"),
        lang("scala", &[".scala", ".sc"], &[], &[], "text/x-scala"),
        lang("html", &[".html", ".htm", ".xhtml"], &[], &[], "text/html"),
        lang("css", &[".css"], &[], &[], "text/css"),
        lang("scss", &[".scss"], &[], &[], "text/x-scss"),
        lang("less", &[".less"], &[], &[], "text/x-less"),
        lang(
            "json",
            &[".json", ".jsonl"],
            &[".babelrc", ".eslintrc", "tsconfig.json"],
            &[],
            "application/json",
        ),
        lang(
            "jsonc",
            &[".jsonc"],
            &["settings.json", "launch.json", "tasks.json"],
            &[],
            "application/json",
        ),
        lang("yaml", &[".yaml", ".yml"], &[], &[], "text/x-yaml"),
        lang(
            "toml",
            &[".toml"],
            &["Cargo.toml", "pyproject.toml"],
            &[],
            "text/x-toml",
        ),
        lang(
            "markdown",
            &[".md", ".markdown", ".mdx"],
            &[],
            &[],
            "text/markdown",
        ),
        lang(
            "xml",
            &[".xml", ".xsd", ".xsl", ".svg"],
            &[],
            &["^<\\?xml"],
            "text/xml",
        ),
        lang("sql", &[".sql"], &[], &[], "text/x-sql"),
        lang(
            "shellscript",
            &[".sh", ".bash", ".zsh"],
            &[".bashrc", ".zshrc", ".profile", ".bash_profile"],
            &["^#!.*\\b(bash|sh|zsh)\\b"],
            "text/x-shellscript",
        ),
        lang(
            "powershell",
            &[".ps1", ".psm1", ".psd1"],
            &[],
            &[],
            "text/x-powershell",
        ),
        lang("bat", &[".bat", ".cmd"], &[], &[], "text/x-bat"),
        lang(
            "dockerfile",
            &[".dockerfile"],
            &["Dockerfile", "Containerfile"],
            &[],
            "text/x-dockerfile",
        ),
        lang(
            "gitignore",
            &[".gitignore"],
            &[".gitignore"],
            &[],
            "text/plain",
        ),
        lang(
            "gitattributes",
            &[".gitattributes"],
            &[".gitattributes"],
            &[],
            "text/plain",
        ),
        lang("lua", &[".lua"], &[], &["^#!.*\\blua\\b"], "text/x-lua"),
        lang("r", &[".r", ".R"], &[], &[], "text/x-r"),
        lang("dart", &[".dart"], &[], &[], "text/x-dart"),
        lang("elixir", &[".ex", ".exs"], &[], &[], "text/x-elixir"),
        lang("erlang", &[".erl", ".hrl"], &[], &[], "text/x-erlang"),
        lang("haskell", &[".hs", ".lhs"], &[], &[], "text/x-haskell"),
        lang(
            "clojure",
            &[".clj", ".cljs", ".cljc", ".edn"],
            &[],
            &[],
            "text/x-clojure",
        ),
        lang(
            "fsharp",
            &[".fs", ".fsi", ".fsx"],
            &[],
            &[],
            "text/x-fsharp",
        ),
        lang(
            "perl",
            &[".pl", ".pm"],
            &[],
            &["^#!.*\\bperl\\b"],
            "text/x-perl",
        ),
        lang(
            "makefile",
            &[],
            &["Makefile", "makefile", "GNUmakefile"],
            &[],
            "text/x-makefile",
        ),
        lang(
            "cmake",
            &[".cmake"],
            &["CMakeLists.txt"],
            &[],
            "text/x-cmake",
        ),
        lang("protobuf", &[".proto"], &[], &[], "text/x-protobuf"),
        lang("graphql", &[".graphql", ".gql"], &[], &[], "text/x-graphql"),
        lang("vue", &[".vue"], &[], &[], "text/x-vue"),
        lang("svelte", &[".svelte"], &[], &[], "text/x-svelte"),
        lang(
            "handlebars",
            &[".hbs", ".handlebars"],
            &[],
            &[],
            "text/x-handlebars",
        ),
        lang("ini", &[".ini", ".cfg", ".conf"], &[], &[], "text/x-ini"),
        lang(
            "env",
            &[".env"],
            &[".env", ".env.local", ".env.development", ".env.production"],
            &[],
            "text/plain",
        ),
        lang(
            "terraform",
            &[".tf", ".tfvars"],
            &[],
            &[],
            "text/x-terraform",
        ),
        lang("zig", &[".zig"], &[], &[], "text/x-zig"),
        lang("nim", &[".nim", ".nims"], &[], &[], "text/x-nim"),
        lang("ocaml", &[".ml", ".mli"], &[], &[], "text/x-ocaml"),
        lang("wasm", &[".wat", ".wast"], &[], &[], "text/x-wasm"),
        lang(
            "plaintext",
            &[".txt", ".text", ".log"],
            &[],
            &[],
            "text/plain",
        ),
    ]
}

fn builtin_icon_map() -> Vec<(String, String)> {
    vec![
        ("rs".into(), "rust".into()),
        ("ts".into(), "typescript".into()),
        ("tsx".into(), "react_ts".into()),
        ("js".into(), "javascript".into()),
        ("jsx".into(), "react".into()),
        ("py".into(), "python".into()),
        ("go".into(), "go".into()),
        ("java".into(), "java".into()),
        ("c".into(), "c".into()),
        ("cpp".into(), "cpp".into()),
        ("h".into(), "c_header".into()),
        ("cs".into(), "csharp".into()),
        ("rb".into(), "ruby".into()),
        ("php".into(), "php".into()),
        ("swift".into(), "swift".into()),
        ("kt".into(), "kotlin".into()),
        ("scala".into(), "scala".into()),
        ("html".into(), "html".into()),
        ("css".into(), "css".into()),
        ("scss".into(), "sass".into()),
        ("less".into(), "less".into()),
        ("json".into(), "json".into()),
        ("yaml".into(), "yaml".into()),
        ("yml".into(), "yaml".into()),
        ("toml".into(), "toml".into()),
        ("md".into(), "markdown".into()),
        ("xml".into(), "xml".into()),
        ("sql".into(), "database".into()),
        ("sh".into(), "shell".into()),
        ("ps1".into(), "powershell".into()),
        ("bat".into(), "windows".into()),
        ("lua".into(), "lua".into()),
        ("dart".into(), "dart".into()),
        ("vue".into(), "vue".into()),
        ("svelte".into(), "svelte".into()),
        ("graphql".into(), "graphql".into()),
        ("proto".into(), "protobuf".into()),
        ("tf".into(), "terraform".into()),
        ("zig".into(), "zig".into()),
        ("Dockerfile".into(), "docker".into()),
        ("Makefile".into(), "makefile".into()),
        ("Cargo.toml".into(), "rust".into()),
        ("package.json".into(), "npm".into()),
        ("tsconfig.json".into(), "tsconfig".into()),
        (".gitignore".into(), "git".into()),
        (".env".into(), "env".into()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_rust_by_extension() {
        let fa = FileAssociations::builtin();
        let assoc = fa.detect_language(Path::new("src/main.rs")).unwrap();
        assert_eq!(assoc.language_id, "rust");
    }

    #[test]
    fn detect_dockerfile_by_name() {
        let fa = FileAssociations::builtin();
        let assoc = fa.detect_language(Path::new("Dockerfile")).unwrap();
        assert_eq!(assoc.language_id, "dockerfile");
    }

    #[test]
    fn detect_python_by_shebang() {
        let fa = FileAssociations::builtin();
        let assoc = fa.detect_from_first_line("#!/usr/bin/env python3").unwrap();
        assert_eq!(assoc.language_id, "python");
    }

    #[test]
    fn detect_shell_by_shebang() {
        let fa = FileAssociations::builtin();
        let assoc = fa.detect_from_first_line("#!/bin/bash").unwrap();
        assert_eq!(assoc.language_id, "shellscript");
    }

    #[test]
    fn unknown_extension_returns_none() {
        let fa = FileAssociations::builtin();
        assert!(fa.detect_language(Path::new("foo.qwerty123")).is_none());
    }

    #[test]
    fn icon_lookup() {
        let fa = FileAssociations::builtin();
        let icon = fa.get_icon(Path::new("main.rs")).unwrap();
        assert_eq!(icon, "rust");
    }

    #[test]
    fn builtin_has_at_least_50_languages() {
        let fa = FileAssociations::builtin();
        assert!(
            fa.language_map.len() >= 50,
            "expected >=50 languages, got {}",
            fa.language_map.len()
        );
    }

    #[test]
    fn custom_registration() {
        let mut fa = FileAssociations::builtin();
        fa.register(LanguageAssociation {
            language_id: "mylang".into(),
            extensions: vec![".my".into()],
            filenames: vec![],
            first_line_patterns: vec![],
            mime_type: "text/x-mylang".into(),
        });
        let assoc = fa.detect_language(Path::new("code.my")).unwrap();
        assert_eq!(assoc.language_id, "mylang");
    }
}
