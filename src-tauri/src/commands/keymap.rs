use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Serialize;
use serde_json::Value;
use sidex_keymap::{
    default_keybindings, format_keybinding, parse_keybinding_string, ContextKeys, ContextValue,
    KeybindingResolver, KeybindingSource,
};

static RESOLVER: OnceLock<KeybindingResolver> = OnceLock::new();

fn resolver() -> &'static KeybindingResolver {
    RESOLVER.get_or_init(KeybindingResolver::new)
}

#[derive(Debug, Serialize)]
pub struct KeybindingEntry {
    pub key: String,
    pub command: String,
    pub when: Option<String>,
    pub source: String,
}

fn source_label(source: &KeybindingSource) -> String {
    match source {
        KeybindingSource::Default => "default".into(),
        KeybindingSource::User => "user".into(),
        KeybindingSource::Extension(id) => format!("extension:{id}"),
    }
}

fn context_from_map(context: &HashMap<String, Value>) -> ContextKeys {
    let mut ctx = ContextKeys::new();
    for (k, v) in context {
        match v {
            Value::Bool(b) => ctx.set_bool(k, *b),
            Value::String(s) => ctx.set_string(k, s),
            Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    ctx.set(k, ContextValue::Number(f));
                }
            }
            _ => {}
        }
    }
    ctx
}

#[allow(clippy::unnecessary_wraps)]
#[tauri::command]
pub fn keymap_get_defaults() -> Result<Vec<KeybindingEntry>, String> {
    let entries: Vec<KeybindingEntry> = default_keybindings()
        .into_iter()
        .map(|b| KeybindingEntry {
            key: b.key.to_string(),
            command: b.command.clone(),
            when: b.when.clone(),
            source: "default".into(),
        })
        .collect();
    Ok(entries)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn keymap_resolve(
    key: String,
    context: HashMap<String, Value>,
) -> Result<Option<String>, String> {
    let combos = parse_keybinding_string(&key).map_err(|e| format!("invalid key string: {e}"))?;
    let ctx = context_from_map(&context);
    let r = resolver();
    let cmd = match combos.len() {
        1 => r.resolve(&combos[0], &ctx).map(str::to_owned),
        2 => r
            .resolve_chord(&combos[0], &combos[1], &ctx)
            .map(str::to_owned),
        _ => None,
    };
    Ok(cmd)
}

/// Resolve a two-part chord (e.g. `Ctrl+K Ctrl+S`) against the current
/// keybinding registry and `when`-clause context.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn keymap_resolve_chord(
    first: String,
    second: String,
    context: HashMap<String, Value>,
) -> Result<Option<String>, String> {
    let first_combos =
        parse_keybinding_string(&first).map_err(|e| format!("invalid first key: {e}"))?;
    let second_combos =
        parse_keybinding_string(&second).map_err(|e| format!("invalid second key: {e}"))?;

    let Some(a) = first_combos.first() else {
        return Ok(None);
    };
    let Some(b) = second_combos.first() else {
        return Ok(None);
    };

    let ctx = context_from_map(&context);
    Ok(resolver().resolve_chord(a, b, &ctx).map(str::to_owned))
}

#[allow(clippy::unnecessary_wraps)]
#[tauri::command]
pub fn keymap_get_all() -> Result<Vec<KeybindingEntry>, String> {
    let resolved = resolver().resolved_bindings();
    let entries: Vec<KeybindingEntry> = resolved
        .iter()
        .filter(|r| !r.command.starts_with('-'))
        .map(|r| KeybindingEntry {
            key: format_keybinding(&r.keys),
            command: r.command.clone(),
            when: r.when.clone(),
            source: source_label(&r.source),
        })
        .collect();
    Ok(entries)
}
