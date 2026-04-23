//! Native `TextMate` tokenizer commands.
//!
//! Thin Tauri wrappers around the `sidex-textmate` crate. Grammars and
//! themes are loaded once into a shared [`TextMateStore`]; the webview
//! then drives line-by-line tokenization over IPC, receiving the
//! packed 32-bit metadata stream Monaco consumes without translation.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sidex_textmate::{
    EmbeddedLanguagesMap, Grammar, RawGrammar, RawSettings, RawThemeSetting, Registry, ScopeField,
    StateStackImpl, Theme, Token, TokenizeLineBinaryResult, TokenizeLineResult,
};

use std::sync::RwLock;

/// Shared textmate store. One registry holds every loaded grammar plus
/// the active theme; a parallel map caches compiled [`Grammar`]s keyed
/// by their scope name. Cloning the store is cheap (everything lives
/// behind `Arc`).
pub struct TextMateStore {
    registry: Arc<Registry>,
    grammars: RwLock<HashMap<String, Arc<Grammar>>>,
    stacks: RwLock<HashMap<u64, Arc<StateStackImpl>>>,
    next_stack: RwLock<u64>,
}

impl Default for TextMateStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TextMateStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            registry: Arc::new(Registry::new(Theme::create_from_raw(&[], None))),
            grammars: RwLock::new(HashMap::new()),
            stacks: RwLock::new(HashMap::new()),
            next_stack: RwLock::new(1),
        }
    }

    fn fresh_stack_handle(&self) -> u64 {
        let mut guard = self
            .next_stack
            .write()
            .expect("textmate next_stack poisoned");
        let id = *guard;
        *guard = guard.saturating_add(1).max(1);
        id
    }
}

/// Payload describing a raw theme setting as it arrives over the wire.
#[derive(Debug, Default, Deserialize)]
pub struct ThemeSettingPayload {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub scope: Option<serde_json::Value>,
    #[serde(default)]
    pub settings: ThemeInnerPayload,
}

#[derive(Debug, Default, Deserialize)]
pub struct ThemeInnerPayload {
    #[serde(default, rename = "fontStyle")]
    pub font_style: Option<String>,
    #[serde(default)]
    pub foreground: Option<String>,
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default, rename = "fontFamily")]
    pub font_family: Option<String>,
    #[serde(default, rename = "fontSize")]
    pub font_size: Option<f64>,
    #[serde(default, rename = "lineHeight")]
    pub line_height: Option<f64>,
}

fn to_raw_setting(payload: ThemeSettingPayload) -> RawThemeSetting {
    #[allow(clippy::match_same_arms)]
    let scope = match payload.scope {
        None => ScopeField::Missing,
        Some(serde_json::Value::Null) => ScopeField::Missing,
        Some(serde_json::Value::String(s)) => ScopeField::String(s),
        Some(serde_json::Value::Array(arr)) => ScopeField::Array(
            arr.into_iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
        ),
        _ => ScopeField::Missing,
    };
    RawThemeSetting {
        name: payload.name,
        scope,
        settings: RawSettings {
            font_style: payload.settings.font_style,
            foreground: payload.settings.foreground,
            background: payload.settings.background,
            font_family: payload.settings.font_family,
            font_size: payload.settings.font_size,
            line_height: payload.settings.line_height,
        },
    }
}

/// Loads or replaces a grammar by its scope name. The grammar source
/// must be a `.tmLanguage.json` document (plist grammars are converted
/// upstream of this command).
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn textmate_load_grammar(
    store: tauri::State<'_, Arc<TextMateStore>>,
    scope_name: String,
    grammar_json: String,
    initial_language_id: u32,
    embedded_languages: Option<HashMap<String, u32>>,
    injection_scope_names: Option<Vec<String>>,
) -> Result<(), String> {
    let raw: RawGrammar =
        serde_json::from_str(&grammar_json).map_err(|e| format!("parse grammar JSON: {e}"))?;

    store
        .registry
        .add_grammar(raw.clone(), injection_scope_names);

    let embedded = embedded_languages.unwrap_or_default();
    let grammar = Grammar::new(
        scope_name.clone(),
        &raw,
        initial_language_id,
        &EmbeddedLanguagesMap::from_iter(embedded),
        None,
        None,
        Arc::clone(&store.registry),
    );
    store
        .grammars
        .write()
        .expect("textmate grammars poisoned")
        .insert(scope_name, Arc::new(grammar));
    Ok(())
}

/// Replaces the theme. Call again whenever the user changes color
/// scheme; compiled grammars cache scanners independently of the
/// theme, so the swap is cheap.
#[tauri::command(rename_all = "camelCase")]
#[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
pub fn textmate_update_theme(
    store: tauri::State<'_, Arc<TextMateStore>>,
    settings: Vec<ThemeSettingPayload>,
    color_map: Option<Vec<String>>,
) -> Result<Vec<String>, String> {
    let raw_settings: Vec<RawThemeSetting> = settings.into_iter().map(to_raw_setting).collect();
    let theme = Theme::create_from_raw(&raw_settings, color_map);
    store.registry.set_theme(theme);
    Ok(store.registry.color_map())
}

/// Plain-mode tokenization — returns `(scopes, tokens)` per line. The
/// opaque `rule_stack` handle is passed back on the next call so the
/// tokenizer resumes at the correct rule-stack state.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenizeLineResponse {
    pub tokens: Vec<WireToken>,
    pub rule_stack: u64,
    pub stopped_early: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WireToken {
    pub start_index: u32,
    pub end_index: u32,
    pub scopes: Vec<String>,
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn textmate_tokenize_line(
    store: tauri::State<'_, Arc<TextMateStore>>,
    scope_name: String,
    line_text: String,
    prev_stack: Option<u64>,
    time_limit_ms: Option<u64>,
) -> Result<TokenizeLineResponse, String> {
    let grammar = store
        .grammars
        .read()
        .expect("textmate grammars poisoned")
        .get(&scope_name)
        .cloned()
        .ok_or_else(|| format!("grammar not loaded: {scope_name}"))?;

    let prev = prev_stack.and_then(|id| {
        store
            .stacks
            .read()
            .expect("textmate stacks poisoned")
            .get(&id)
            .cloned()
    });
    let TokenizeLineResult {
        tokens,
        rule_stack,
        stopped_early,
        ..
    } = grammar.tokenize_line(&line_text, prev, time_limit_ms);

    let handle = store.fresh_stack_handle();
    store
        .stacks
        .write()
        .expect("textmate stacks poisoned")
        .insert(handle, Arc::clone(&rule_stack));

    Ok(TokenizeLineResponse {
        tokens: tokens.into_iter().map(wire_token).collect(),
        rule_stack: handle,
        stopped_early,
    })
}

fn wire_token(token: Token) -> WireToken {
    WireToken {
        start_index: u32::try_from(token.start_index).unwrap_or(u32::MAX),
        end_index: u32::try_from(token.end_index).unwrap_or(u32::MAX),
        scopes: token.scopes,
    }
}

/// Binary mode — returns the packed `[startIndex, metadata]` stream
/// Monaco consumes as a `Uint32Array`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenizeLineBinaryResponse {
    pub tokens: Vec<u32>,
    pub rule_stack: u64,
    pub stopped_early: bool,
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn textmate_tokenize_line_binary(
    store: tauri::State<'_, Arc<TextMateStore>>,
    scope_name: String,
    line_text: String,
    prev_stack: Option<u64>,
    time_limit_ms: Option<u64>,
) -> Result<TokenizeLineBinaryResponse, String> {
    let grammar = store
        .grammars
        .read()
        .expect("textmate grammars poisoned")
        .get(&scope_name)
        .cloned()
        .ok_or_else(|| format!("grammar not loaded: {scope_name}"))?;

    let prev = prev_stack.and_then(|id| {
        store
            .stacks
            .read()
            .expect("textmate stacks poisoned")
            .get(&id)
            .cloned()
    });
    let TokenizeLineBinaryResult {
        tokens,
        rule_stack,
        stopped_early,
        ..
    } = grammar.tokenize_line_binary(&line_text, prev, time_limit_ms);

    let handle = store.fresh_stack_handle();
    store
        .stacks
        .write()
        .expect("textmate stacks poisoned")
        .insert(handle, Arc::clone(&rule_stack));

    Ok(TokenizeLineBinaryResponse {
        tokens,
        rule_stack: handle,
        stopped_early,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenizeDocumentResponse {
    pub lines: Vec<TokenizeLineBinaryResponse>,
    pub final_stack: u64,
}

#[tauri::command(rename_all = "camelCase")]
#[allow(clippy::needless_pass_by_value)]
pub fn textmate_tokenize_document(
    store: tauri::State<'_, Arc<TextMateStore>>,
    scope_name: String,
    lines: Vec<String>,
    start_stack: Option<u64>,
    time_limit_ms: Option<u64>,
) -> Result<TokenizeDocumentResponse, String> {
    let grammar = store
        .grammars
        .read()
        .expect("textmate grammars poisoned")
        .get(&scope_name)
        .cloned()
        .ok_or_else(|| format!("grammar not loaded: {scope_name}"))?;

    let mut current_stack = start_stack.and_then(|id| {
        store
            .stacks
            .read()
            .expect("textmate stacks poisoned")
            .get(&id)
            .cloned()
    });

    let mut results = Vec::with_capacity(lines.len());

    for line_text in &lines {
        let TokenizeLineBinaryResult {
            tokens,
            rule_stack,
            stopped_early,
            ..
        } = grammar.tokenize_line_binary(line_text, current_stack.clone(), time_limit_ms);

        let handle = store.fresh_stack_handle();
        store
            .stacks
            .write()
            .expect("textmate stacks poisoned")
            .insert(handle, Arc::clone(&rule_stack));

        current_stack = Some(rule_stack);
        results.push(TokenizeLineBinaryResponse {
            tokens,
            rule_stack: handle,
            stopped_early,
        });
    }

    let final_stack = results.last().map_or(0, |r| r.rule_stack);
    Ok(TokenizeDocumentResponse {
        lines: results,
        final_stack,
    })
}

/// Releases a rule-stack handle. Callers should invoke this when the
/// document backing a tokenization session closes so the store's
/// handle map doesn't grow unbounded.
#[tauri::command]
#[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
pub fn textmate_release_stack(
    store: tauri::State<'_, Arc<TextMateStore>>,
    stack_id: u64,
) -> Result<(), String> {
    store
        .stacks
        .write()
        .expect("textmate stacks poisoned")
        .remove(&stack_id);
    Ok(())
}
