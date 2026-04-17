use serde::Serialize;
use sidex_syntax::language::{builtin_language_configurations, LanguageConfiguration};
use std::path::Path;
use std::sync::OnceLock;

static LANGUAGE_CONFIGS: OnceLock<Vec<LanguageConfiguration>> = OnceLock::new();

fn configs() -> &'static [LanguageConfiguration] {
    LANGUAGE_CONFIGS.get_or_init(builtin_language_configurations)
}

#[derive(Debug, Serialize)]
pub struct LanguageInfo {
    pub id: String,
    pub name: String,
    pub extensions: Vec<String>,
    pub filenames: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct AutoClosePair {
    pub open: String,
    pub close: String,
    pub not_in: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct LanguageConfigResponse {
    pub line_comment: Option<String>,
    pub block_comment: Option<(String, String)>,
    pub brackets: Vec<(String, String)>,
    pub auto_closing_pairs: Vec<AutoClosePair>,
}

#[allow(clippy::unnecessary_wraps)]
#[tauri::command]
pub fn syntax_get_languages() -> Result<Vec<LanguageInfo>, String> {
    let langs = configs()
        .iter()
        .map(|cfg| LanguageInfo {
            id: cfg.id.clone(),
            name: cfg.name.clone(),
            extensions: cfg.extensions.clone(),
            filenames: cfg.filenames.clone(),
        })
        .collect();
    Ok(langs)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn syntax_detect_language(filename: String) -> Result<String, String> {
    let ext = Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"));

    let fname = Path::new(&filename)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");

    for cfg in configs() {
        if let Some(ref ext) = ext {
            if cfg.extensions.iter().any(|e| e == ext) {
                return Ok(cfg.id.clone());
            }
        }
        if cfg.filenames.iter().any(|f| f == fname) {
            return Ok(cfg.id.clone());
        }
    }

    Err(format!("No language detected for '{filename}'"))
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn syntax_get_language_config(language_id: String) -> Result<LanguageConfigResponse, String> {
    let cfg = configs()
        .iter()
        .find(|c| c.id == language_id)
        .ok_or_else(|| format!("Unknown language ID '{language_id}'"))?;

    Ok(LanguageConfigResponse {
        line_comment: cfg.comments.line_comment.clone(),
        block_comment: cfg.comments.block_comment.clone(),
        brackets: cfg.brackets.clone(),
        auto_closing_pairs: cfg
            .auto_closing_pairs
            .iter()
            .map(|p| AutoClosePair {
                open: p.open.clone(),
                close: p.close.clone(),
                not_in: p.not_in.clone(),
            })
            .collect(),
    })
}
