use serde::Serialize;
use sidex_lsp::registry::ServerRegistry;
use std::sync::OnceLock;

static REGISTRY: OnceLock<ServerRegistry> = OnceLock::new();

fn registry() -> &'static ServerRegistry {
    REGISTRY.get_or_init(ServerRegistry::new)
}

#[derive(Debug, Serialize)]
pub struct LspServerInfo {
    pub name: String,
    pub languages: Vec<String>,
    pub command: String,
    pub args: Vec<String>,
}

#[allow(clippy::unnecessary_wraps)]
#[tauri::command]
pub fn lsp_get_server_registry() -> Result<Vec<LspServerInfo>, String> {
    let reg = registry();
    let mut seen = std::collections::HashMap::<String, usize>::new();
    let mut servers: Vec<LspServerInfo> = Vec::new();

    let mut langs: Vec<String> = reg.language_ids().map(String::from).collect();
    langs.sort();

    for lang in langs {
        let Some(cfg) = reg.get(&lang) else {
            continue;
        };
        if let Some(&idx) = seen.get(&cfg.command) {
            servers[idx].languages.push(lang);
        } else {
            seen.insert(cfg.command.clone(), servers.len());
            servers.push(LspServerInfo {
                name: cfg.command.clone(),
                languages: vec![lang],
                command: cfg.command.clone(),
                args: cfg.args.clone(),
            });
        }
    }

    Ok(servers)
}

#[allow(clippy::unnecessary_wraps)]
#[tauri::command]
pub fn lsp_get_supported_languages() -> Result<Vec<String>, String> {
    let mut langs: Vec<String> = registry().language_ids().map(String::from).collect();
    langs.sort();
    Ok(langs)
}
